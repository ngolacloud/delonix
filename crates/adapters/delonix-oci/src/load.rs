//! Loading images from a `docker save` archive (OCI/legacy format).

use crate::cas::{sha256_hex, StreamingBlob};
use crate::image::{healthcheck_test, now_unix, Image, ImageConfig, ImageStore};
use crate::{Error, Result};
use oci_spec::image::ImageConfiguration;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// An entry of the legacy `manifest.json` from `docker save`. It is NOT an OCI type —
/// the `manifest.json` of `docker save` is a legacy Docker format (array of
/// `{Config, RepoTags, Layers}` with `blobs/sha256/...` paths), which
/// `oci-spec` does not model; hence it is hand-rolled on purpose. The config
/// blob it points to is already an OCI `ImageConfiguration` (see below).
#[derive(Deserialize)]
struct DockerManifest {
    #[serde(rename = "Config")]
    config: String,
    #[serde(rename = "RepoTags")]
    repo_tags: Option<Vec<String>>,
    #[serde(rename = "Layers")]
    layers: Vec<String>,
}

/// Converts a `blobs/sha256/<hex>` path into the `sha256:<hex>` digest.
fn path_to_digest(blob_path: &str) -> String {
    let hex = blob_path.rsplit('/').next().unwrap_or(blob_path);
    format!("sha256:{hex}")
}

/// `true` for the blob files a CLASSIC `docker save` (the pre-OCI daemon
/// format) lays at the top level: `<layer-id>/layer.tar` and the config
/// `<hex>.json`. The OCI layout keeps everything under `blobs/sha256/`; the
/// classic format references these by PATH from `manifest.json` and expects
/// the reader to hash their content. `oci-layout`, `index.json`,
/// `repositories`, `<id>/json` and `<id>/VERSION` are not image blobs and must
/// not be hashed into the CAS.
fn is_classic_blob(name: &str) -> bool {
    name.ends_with("/layer.tar")
        || (!name.contains('/')
            && name.ends_with(".json")
            && name != "manifest.json"
            && name != "index.json")
}

/// Streams one archive entry into the CAS, block by block, and returns its
/// `sha256:<hex>` digest. A layer is multi-GB, so it is never held whole in
/// memory ([`StreamingBlob`] hands fixed blocks to a writer thread); the old
/// path `read_to_end`-ed every layer into a `Vec` first (O7).
fn cas_write_entry<R: Read>(store: &ImageStore, entry: &mut R) -> Result<String> {
    let cas = store.cas();
    let tmp = cas.tmp_path();
    let mut blob = StreamingBlob::create(&tmp)?;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = entry.read(&mut buf)?;
        if n == 0 {
            break;
        }
        blob.append(&buf[..n])?;
    }
    let hex = blob.finish()?;
    cas.adopt(&tmp, &hex)?;
    Ok(format!("sha256:{hex}"))
}

/// Resolves a `manifest.json` blob reference to its CAS digest. The OCI layout
/// addresses blobs by a `blobs/sha256/<hex>` path that already IS the digest;
/// the classic format references them by an arbitrary archive path whose
/// content we hashed into `extracted` as we read it.
fn resolve_blob(reference: &str, extracted: &HashMap<String, String>) -> Result<String> {
    if let Some(digest) = extracted.get(reference) {
        Ok(digest.clone())
    } else if reference.starts_with("blobs/sha256/") {
        // An OCI-layout reference the archive never actually carried — the
        // digest is still known from the path, and O6's presence check below
        // turns the missing blob into a clear refusal.
        Ok(path_to_digest(reference))
    } else {
        Err(Error::Archive(format!(
            "manifest.json references {reference}, which the archive does not contain"
        )))
    }
}

/// Imports a `docker save` archive into the store, returning the image. Reads
/// BOTH formats: the OCI image layout (`blobs/sha256/...`) and the classic
/// daemon `docker save` (`<id>/layer.tar` + `<cfg>.json` at the top level).
pub fn load_docker_archive(store: &ImageStore, tar_path: &Path) -> Result<Image> {
    let file = std::fs::File::open(tar_path)?;
    let mut archive = tar::Archive::new(file);
    let mut manifest_bytes: Option<Vec<u8>> = None;
    // Archive path (as `manifest.json` names it) → the content digest we hashed
    // it to. Populated for every blob of EITHER format, so the classic
    // references (by path) and the OCI ones (by digest-in-path) resolve alike.
    let mut extracted: HashMap<String, String> = HashMap::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let name = entry.path()?.to_string_lossy().into_owned();

        if name == "manifest.json" {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            manifest_bytes = Some(buf);
            continue;
        }

        let is_oci_blob = name.starts_with("blobs/sha256/") && name.len() > "blobs/sha256/".len();
        if is_oci_blob || is_classic_blob(&name) {
            let digest = cas_write_entry(store, &mut entry)?;
            if is_oci_blob {
                // The OCI layout path IS the digest: the content must hash to it.
                let expected = path_to_digest(&name);
                if digest != expected {
                    return Err(Error::Archive(format!(
                        "corrupted blob: {name} has sha256 {digest}"
                    )));
                }
            }
            extracted.insert(name, digest);
        }
    }

    let manifest_bytes = manifest_bytes
        .ok_or_else(|| Error::Archive("manifest.json missing from the archive".into()))?;
    let manifests: Vec<DockerManifest> = serde_json::from_slice(&manifest_bytes)?;
    let manifest = manifests
        .into_iter()
        .next()
        .ok_or_else(|| Error::Archive("empty manifest.json".into()))?;

    let config_digest = resolve_blob(&manifest.config, &extracted)?;
    let layers: Vec<String> = manifest
        .layers
        .iter()
        .map(|l| resolve_blob(l, &extracted))
        .collect::<Result<_>>()?;

    // O6: every blob the manifest names must have actually reached the CAS. A
    // truncated archive (a layer entry missing, or a `blobs/` reference the
    // archive never carried) otherwise produced an Image whose layer digest
    // pointed at nothing — accepted in silence, and the failure surfaced far
    // later at unpack time with no hint of the real cause.
    for digest in std::iter::once(&config_digest).chain(layers.iter()) {
        if !store.cas().has(digest) {
            return Err(Error::Archive(format!(
                "manifest.json references blob {digest}, missing from the archive"
            )));
        }
    }

    let config_bytes = store.cas().read(&config_digest)?;
    if sha256_hex(&config_bytes) != crate::cas::strip(&config_digest) {
        return Err(Error::Archive("config digest mismatch".into()));
    }
    // Read the runtime config from the OCI config blob (`ImageConfiguration`).
    let oci_config: ImageConfiguration = serde_json::from_slice(&config_bytes)?;
    let inner = oci_config.config().clone().unwrap_or_default();
    // `Arch`'s own (de)serialization isn't a plain string round-trip worth
    // fighting here — read the top-level `"architecture"` string straight off
    // the raw JSON instead (already have the bytes; falls back to the host's
    // own arch, same as `ImageConfig::architecture`'s serde default, if the
    // archive's config is missing or malformed on this one field).
    let architecture = serde_json::from_slice::<serde_json::Value>(&config_bytes)
        .ok()
        .and_then(|v| {
            v.get("architecture")
                .and_then(|a| a.as_str().map(str::to_string))
        })
        .unwrap_or_else(crate::image::host_arch_default);

    // `merged_tags_all` and not the archive's list verbatim: the same id can
    // carry several names, and loading one archive must not drop the others.
    // See the doc comment there for the measurement.
    let repo_tags = store.merged_tags_all(
        &config_digest,
        &manifest.repo_tags.clone().unwrap_or_default(),
    );
    let image = Image {
        id: config_digest,
        repo_tags,
        layers,
        config: ImageConfig {
            cmd: inner.cmd().clone().unwrap_or_default(),
            entrypoint: inner.entrypoint().clone().unwrap_or_default(),
            env: inner.env().clone().unwrap_or_default(),
            user: inner.user().clone().unwrap_or_default(),
            working_dir: inner.working_dir().clone().unwrap_or_default(),
            cpus: None,
            memory: None,
            security: Vec::new(),
            // `Healthcheck` is a Docker extension the OCI `Config` type does not
            // model, so it is dug out of the raw config JSON (like `architecture`
            // above) rather than via `oci_spec`. Was hard-coded `None`, which
            // DROPPED the probe of every loaded image — a pulled distroless image
            // with an exec `HEALTHCHECK` had no probe at all (review 2026-11-04).
            // Kept as the OCI `Test` array so the exec-vs-shell form survives.
            healthcheck: healthcheck_test(&config_bytes),
            exposed_ports: inner.exposed_ports().clone().unwrap_or_default(),
            architecture,
        },
        created_unix: now_unix(),
    };
    store.enforce_tag_uniqueness(&image)?;
    store.save(&image)?;
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Appends one regular file to the tar, with the same minimal header the
    /// loader expects (a regular file it can read whole).
    fn append(tar: &mut tar::Builder<std::fs::File>, name: &str, data: &[u8]) {
        let mut h = tar::Header::new_gnu();
        h.set_size(data.len() as u64);
        h.set_mode(0o644);
        h.set_mtime(0);
        h.set_cksum();
        tar.append_data(&mut h, name, data).unwrap();
    }

    /// O5: a CLASSIC `docker save` archive (the pre-OCI daemon format) loads.
    /// Its layers live at `<id>/layer.tar` and its config at `<hex>.json` at the
    /// top level — NOT under `blobs/sha256/` — and `manifest.json` references
    /// them by path, so the digests are the sha256 of the content, which the
    /// loader must compute. The legacy `<id>/json`, `VERSION` and `repositories`
    /// files are ignored.
    #[test]
    fn loads_a_classic_docker_save_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ImageStore::open(tmp.path().join("store")).unwrap();

        let config = br#"{"architecture":"amd64","os":"linux","config":{"Cmd":["/bin/sh"]},"rootfs":{"type":"layers","diff_ids":[]}}"#;
        let cfg_hex = sha256_hex(config);
        let cfg_name = format!("{cfg_hex}.json");
        let layer = b"classic-layer-bytes-not-content-addressed-in-the-path";
        let layer_hex = sha256_hex(layer);
        let layer_name = "abc123def456/layer.tar";

        let manifest = serde_json::json!([{
            "Config": cfg_name,
            "RepoTags": ["classic:1"],
            "Layers": [layer_name],
        }]);

        let tar_path = tmp.path().join("classic.tar");
        {
            let f = std::fs::File::create(&tar_path).unwrap();
            let mut tar = tar::Builder::new(f);
            append(&mut tar, &cfg_name, config);
            append(&mut tar, layer_name, layer);
            // Legacy per-layer and repository metadata the loader must skip.
            append(&mut tar, "abc123def456/VERSION", b"1.0");
            append(&mut tar, "abc123def456/json", b"{}");
            append(&mut tar, "repositories", b"{}");
            append(
                &mut tar,
                "manifest.json",
                &serde_json::to_vec(&manifest).unwrap(),
            );
            tar.finish().unwrap();
        }

        let image = load_docker_archive(&store, &tar_path).unwrap();
        assert_eq!(image.id, format!("sha256:{cfg_hex}"));
        assert_eq!(image.layers, vec![format!("sha256:{layer_hex}")]);
        assert_eq!(image.config.cmd, vec!["/bin/sh".to_string()]);
        assert_eq!(image.config.architecture, "amd64");
        // The content really landed in the CAS under its COMPUTED digest.
        let layer_digest = format!("sha256:{layer_hex}");
        assert!(store.cas().has(&layer_digest));
        assert_eq!(store.cas().read(&layer_digest).unwrap(), layer);
    }

    /// O6: an archive that references a layer it does not actually contain is
    /// REFUSED, not accepted with a dangling digest that blows up at unpack
    /// time. Here an OCI-layout archive carries the config but omits the layer
    /// blob `manifest.json` names.
    #[test]
    fn rejects_an_archive_missing_a_referenced_layer() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ImageStore::open(tmp.path().join("store")).unwrap();

        let config =
            br#"{"architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]}}"#;
        let cfg_hex = sha256_hex(config);
        let missing_layer_hex = sha256_hex(b"the-layer-that-was-left-out-of-the-archive");
        let manifest = serde_json::json!([{
            "Config": format!("blobs/sha256/{cfg_hex}"),
            "RepoTags": ["broken:1"],
            "Layers": [format!("blobs/sha256/{missing_layer_hex}")],
        }]);

        let tar_path = tmp.path().join("broken.tar");
        {
            let f = std::fs::File::create(&tar_path).unwrap();
            let mut tar = tar::Builder::new(f);
            append(&mut tar, &format!("blobs/sha256/{cfg_hex}"), config);
            // The layer blob is deliberately NOT written to the archive.
            append(
                &mut tar,
                "manifest.json",
                &serde_json::to_vec(&manifest).unwrap(),
            );
            tar.finish().unwrap();
        }

        let err = load_docker_archive(&store, &tar_path).unwrap_err();
        assert!(
            err.to_string().contains(&missing_layer_hex),
            "the refusal must name the missing blob: {err}"
        );
    }
}
