//! `delonix-scanner` — image vulnerability scanner (cloud-native).
//!
//! Two operations, without root and without running the image:
//! 1. **SBOM** — extracts the list of installed packages (Alpine `apk`, Debian/Ubuntu
//!    `dpkg`) by reading the *layers* directly from the CAS (without mounting anything);
//! 2. **match** — compares the SBOM against a database of *advisories* (CVE) and reports
//!    the vulnerable packages, with severity.
//!
//! Designed to run **before the build** (analyzing the `FROM` base) and on **already
//! existing images** — exactly like `trivy`/`grype`, but embedded in the engine.

use delonix_oci::{Image, ImageStore};
use serde::{Deserialize, Serialize};
use std::io::Read;

mod error;
pub mod pytree;

pub use error::{Error, Result};

/// The package manager that registered a package (determines the advisory source).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ecosystem {
    /// Alpine (`/lib/apk/db/installed`).
    Apk,
    /// Debian/Ubuntu (`/var/lib/dpkg/status`).
    Dpkg,
    /// Python/PyPI (`requirements.txt` of modules — e.g. Odoo modules).
    PyPi,
    /// Go modules read from a binary's embedded build info (`go version -m`).
    Go,
    /// Rust crates read from a binary's embedded `cargo-auditable` section.
    Cargo,
}

/// A package installed in the image.
#[derive(Serialize, Clone, Debug)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub ecosystem: Ecosystem,
}

/// Severity of a vulnerability (orderable for CI *gates*).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Parses `"low"|"medium"|"high"|"critical"` (for `--fail-on`).
    pub fn parse(s: &str) -> Option<Severity> {
        match s.to_ascii_lowercase().as_str() {
            "low" => Some(Severity::Low),
            "medium" | "med" => Some(Severity::Medium),
            "high" => Some(Severity::High),
            "critical" | "crit" => Some(Severity::Critical),
            _ => None,
        }
    }
}

/// An *advisory* (CVE): "package X in ecosystem Y is vulnerable below
/// version `fixed`".
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Advisory {
    pub id: String,
    pub package: String,
    pub ecosystem: Ecosystem,
    /// First FIXED version: the image is vulnerable if `version < fixed`.
    pub fixed: String,
    pub severity: Severity,
    pub summary: String,
}

/// A vulnerability found in the image.
#[derive(Serialize, Clone, Debug)]
pub struct Finding {
    pub id: String,
    pub package: String,
    pub version: String,
    pub fixed: String,
    pub severity: Severity,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// SBOM — extract packages from the layers (reading the CAS, without mounting or running)
// ---------------------------------------------------------------------------

/// Reads a *layer* (tar, optionally gzip) and returns the content of the first
/// file whose path ends in `suffix`, if any.
fn read_member(blob: &[u8], suffix: &str) -> Option<Vec<u8>> {
    let is_gzip = blob.len() >= 2 && blob[0] == 0x1f && blob[1] == 0x8b;
    let reader: Box<dyn Read> = if is_gzip {
        Box::new(flate2::read::GzDecoder::new(blob))
    } else {
        Box::new(blob)
    };
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries().ok()? {
        let mut entry = entry.ok()?;
        let path = entry.path().ok()?.to_string_lossy().into_owned();
        if path.trim_start_matches("./").ends_with(suffix) {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok()?;
            return Some(buf);
        }
    }
    None
}

/// Reads the content of the tar member at EXACTLY `rel_path` (after an optional
/// `./`), if present. Unlike [`read_member`]'s `ends_with` match, this is an
/// exact-path lookup — used to resolve a single-binary image's `Entrypoint`
/// (e.g. `/app`) to its member without accidentally matching `myapp` or
/// `usr/bin/app`.
fn read_member_exact(blob: &[u8], rel_path: &str) -> Option<Vec<u8>> {
    let is_gzip = blob.len() >= 2 && blob[0] == 0x1f && blob[1] == 0x8b;
    let reader: Box<dyn Read> = if is_gzip {
        Box::new(flate2::read::GzDecoder::new(blob))
    } else {
        Box::new(blob)
    };
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries().ok()? {
        let mut entry = entry.ok()?;
        let path = entry.path().ok()?.to_string_lossy().into_owned();
        if path.trim_start_matches("./") == rel_path {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok()?;
            return Some(buf);
        }
    }
    None
}

/// The tar member path of the image's main binary — `argv[0]` of `Entrypoint`
/// (or `Cmd` when there is no entrypoint), but ONLY when it is an absolute path
/// (`/app`, `/usr/local/bin/server`), which a single-binary scratch/distroless
/// image always uses. Returns `None` for a relative or absent command:
/// resolving those would need the image's `PATH`/`WorkingDir` and is
/// deliberately left to a follow-up rather than guessed at here.
fn entrypoint_member_path(config: &delonix_oci::ImageConfig) -> Option<String> {
    let argv0 = config.entrypoint.first().or_else(|| config.cmd.first())?;
    let rel = argv0.strip_prefix('/')?;
    if rel.is_empty() {
        None
    } else {
        Some(rel.to_string())
    }
}

/// Enumerates EVERY tar member whose path (after an optional `./`) begins with
/// `prefix`, returning `(relative_path, bytes)` for each regular file found.
/// Distroless images keep one dpkg stanza per file under
/// `var/lib/dpkg/status.d/` instead of the monolithic `var/lib/dpkg/status`, so
/// a single `read_member` (first match only) can never see them all — this
/// reads the whole directory. The directory marker itself and any nested
/// directory entry are skipped; unreadable entries are ignored rather than
/// failing the whole layer.
fn read_members_with_prefix(blob: &[u8], prefix: &str) -> Vec<(String, Vec<u8>)> {
    let is_gzip = blob.len() >= 2 && blob[0] == 0x1f && blob[1] == 0x8b;
    let reader: Box<dyn Read> = if is_gzip {
        Box::new(flate2::read::GzDecoder::new(blob))
    } else {
        Box::new(blob)
    };
    let mut out = Vec::new();
    let mut archive = tar::Archive::new(reader);
    let Ok(entries) = archive.entries() else {
        return out;
    };
    for entry in entries {
        let Ok(mut entry) = entry else { continue };
        let Ok(path) = entry.path() else { continue };
        let path = path.to_string_lossy().into_owned();
        let rel = path.trim_start_matches("./");
        if let Some(name) = rel.strip_prefix(prefix) {
            if name.is_empty() || name.ends_with('/') {
                continue;
            }
            let mut buf = Vec::new();
            if entry.read_to_end(&mut buf).is_ok() {
                out.push((rel.to_string(), buf));
            }
        }
    }
    out
}

/// Packages from an `apk` database (`P:` name, `V:` version, records separated
/// by a blank line).
fn parse_apk(db: &str) -> Vec<Package> {
    let mut out = Vec::new();
    let mut name = None;
    for line in db.lines() {
        if let Some(n) = line.strip_prefix("P:") {
            name = Some(n.trim().to_string());
        } else if let Some(v) = line.strip_prefix("V:") {
            if let Some(n) = name.take() {
                out.push(Package {
                    name: n,
                    version: v.trim().to_string(),
                    ecosystem: Ecosystem::Apk,
                });
            }
        }
    }
    out
}

/// Packages from a `dpkg/status` (stanzas with `Package:` and `Version:`).
fn parse_dpkg(db: &str) -> Vec<Package> {
    let mut out = Vec::new();
    let mut name = None;
    for line in db.lines() {
        if let Some(n) = line.strip_prefix("Package:") {
            name = Some(n.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Version:") {
            if let Some(n) = name.clone() {
                out.push(Package {
                    name: n,
                    version: v.trim().to_string(),
                    ecosystem: Ecosystem::Dpkg,
                });
            }
        } else if line.is_empty() {
            name = None;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Inventory embedded IN BINARIES — for distroless/scratch images that ship a
// single static binary and no package manager database at all.
// ---------------------------------------------------------------------------

/// Reads an unsigned LEB128 varint at `data[*off..]`, advancing `off`. Returns
/// `None` on truncation or an over-long encoding. Used by the Go build-info
/// decoder, whose inline strings are length-prefixed this way.
fn uvarint(data: &[u8], off: &mut usize) -> Option<u64> {
    let mut x: u64 = 0;
    let mut shift = 0u32;
    loop {
        let b = *data.get(*off)?;
        *off += 1;
        if b < 0x80 {
            if shift >= 64 || (shift == 63 && b > 1) {
                return None;
            }
            return Some(x | ((b as u64) << shift));
        }
        x |= ((b & 0x7f) as u64) << shift;
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
}

/// Reads one uvarint-length-prefixed byte string at `data[*off..]`, advancing
/// `off` past it. The encoding Go's linker uses for the inline build-info
/// version and module strings.
fn decode_string<'a>(data: &'a [u8], off: &mut usize) -> Option<&'a [u8]> {
    let len = uvarint(data, off)? as usize;
    let s = data.get(*off..*off + len)?;
    *off += len;
    Some(s)
}

/// Extracts the Go module list embedded by the linker in a Go binary — the same
/// data `go version -m` prints. Scans the raw bytes for the 14-byte build-info
/// magic (`\xff Go buildinf:`), then reads the modern *inline* format
/// (`flags & 0x2`): a pointer-size byte, a flags byte, 16 bytes of (zeroed, in
/// inline mode) pointer slots, then two uvarint-length strings — the Go version
/// and the module info. The module info is bracketed by two 16-byte sentinels
/// and holds tab-separated `path`/`mod`/`dep`/`build` lines; `mod` and `dep`
/// lines carry `name`, `version`. Pure: takes bytes, returns packages, no ELF
/// crate and no network. The older pointer-indirection format (pre-Go-1.18) is
/// not decoded — resolving those pointers needs the section's virtual address,
/// which is not worth an ELF parser here; such a binary yields no Go packages
/// rather than a wrong answer.
pub fn go_buildinfo_packages(binary: &[u8]) -> Vec<Package> {
    const MAGIC: &[u8] = b"\xff Go buildinf:";
    let mut out = Vec::new();
    let Some(base) = find_subslice(binary, MAGIC) else {
        return out;
    };
    // Header: magic(14) + ptrSize(1) + flags(1) = 16 bytes, then in inline mode
    // 16 zeroed pointer-slot bytes, so the strings begin at offset 32.
    let flags = match binary.get(base + 15) {
        Some(f) => *f,
        None => return out,
    };
    if flags & 0x2 == 0 {
        return out; // old pointer format — not decoded
    }
    let mut off = base + 32;
    let Some(_version) = decode_string(binary, &mut off) else {
        return out;
    };
    let Some(modinfo) = decode_string(binary, &mut off) else {
        return out;
    };
    // Strip the 16-byte sentinels the runtime brackets the module string with,
    // if present (`3077af0c…` … `f9324331…`).
    let body = {
        let mut b = modinfo;
        if b.len() >= 32 {
            b = &b[16..b.len() - 16];
        }
        b
    };
    for line in body.split(|&c| c == b'\n') {
        let mut fields = line.split(|&c| c == b'\t');
        let kind = fields.next().unwrap_or(b"");
        if kind != b"mod" && kind != b"dep" {
            continue;
        }
        let (Some(name), Some(version)) = (fields.next(), fields.next()) else {
            continue;
        };
        let name = String::from_utf8_lossy(name).into_owned();
        let version = String::from_utf8_lossy(version).into_owned();
        // The main module of a `go build` with no version carries `(devel)`;
        // it is not a resolvable module version, so skip it.
        if name.is_empty() || version.is_empty() || version == "(devel)" {
            continue;
        }
        out.push(Package {
            name,
            version,
            ecosystem: Ecosystem::Go,
        });
    }
    out
}

/// Extracts the Rust crate list embedded by `cargo auditable` in the ELF
/// section `.dep-v0` — zlib-compressed JSON of the shape
/// `{"packages":[{"name":..,"version":..}, …]}`. Locates the section by name
/// through the ELF section-header table (32- or 64-bit, either endianness),
/// inflates it with the `flate2` the crate already carries, and reads
/// name+version off each package. Pure, no new dependency. A non-ELF input, a
/// binary without the section, or malformed JSON yields no packages rather than
/// an error.
pub fn cargo_auditable_packages(binary: &[u8]) -> Vec<Package> {
    let Some(section) = elf_section(binary, b".dep-v0") else {
        return Vec::new();
    };
    let mut json = Vec::new();
    if flate2::read::ZlibDecoder::new(section)
        .read_to_end(&mut json)
        .is_err()
    {
        return Vec::new();
    }
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&json) else {
        return Vec::new();
    };
    let Some(packages) = v.get("packages").and_then(|p| p.as_array()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for p in packages {
        let name = p.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let version = p.get("version").and_then(|x| x.as_str()).unwrap_or("");
        if name.is_empty() || version.is_empty() {
            continue;
        }
        out.push(Package {
            name: name.to_string(),
            version: version.to_string(),
            ecosystem: Ecosystem::Cargo,
        });
    }
    out
}

/// First index of `needle` in `haystack`, naive scan — fine for the one-shot
/// magic search over a binary we already hold in memory.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// Returns the bytes of the ELF section named `want`, resolving it through the
/// section-header string table. Supports 32- and 64-bit, little- and
/// big-endian — the four shapes a Linux binary can take — and nothing else
/// (program headers, relocations and symbol tables are irrelevant here).
/// Returns `None` for a non-ELF input or a missing section.
fn elf_section<'a>(bin: &'a [u8], want: &[u8]) -> Option<&'a [u8]> {
    if bin.get(0..4)? != b"\x7fELF" {
        return None;
    }
    let is64 = *bin.get(4)? == 2;
    let le = *bin.get(5)? == 1;
    let rd16 = |o: usize| -> Option<u64> {
        let b = bin.get(o..o + 2)?;
        Some(if le {
            u16::from_le_bytes([b[0], b[1]]) as u64
        } else {
            u16::from_be_bytes([b[0], b[1]]) as u64
        })
    };
    let rd32 = |o: usize| -> Option<u64> {
        let b = bin.get(o..o + 4)?;
        let a = [b[0], b[1], b[2], b[3]];
        Some(if le {
            u32::from_le_bytes(a) as u64
        } else {
            u32::from_be_bytes(a) as u64
        })
    };
    let rd64 = |o: usize| -> Option<u64> {
        let b = bin.get(o..o + 8)?;
        let a = [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
        Some(if le {
            u64::from_le_bytes(a)
        } else {
            u64::from_be_bytes(a)
        })
    };
    // Offsets of e_shoff/e_shentsize/e_shnum/e_shstrndx differ by word size.
    let (shoff, shentsize, shnum, shstrndx) = if is64 {
        (
            rd64(0x28)? as usize,
            rd16(0x3a)? as usize,
            rd16(0x3c)? as usize,
            rd16(0x3e)? as usize,
        )
    } else {
        (
            rd32(0x20)? as usize,
            rd16(0x2e)? as usize,
            rd16(0x30)? as usize,
            rd16(0x32)? as usize,
        )
    };
    if shoff == 0 || shentsize == 0 || shnum == 0 {
        return None;
    }
    // A section header: sh_name(u32) at 0, sh_offset and sh_size at word-size
    // dependent offsets.
    let sh_fields = |idx: usize| -> Option<(usize, usize, usize)> {
        let hdr = shoff + idx * shentsize;
        let name = rd32(hdr)? as usize;
        let (offset, size) = if is64 {
            (rd64(hdr + 0x18)? as usize, rd64(hdr + 0x20)? as usize)
        } else {
            (rd32(hdr + 0x10)? as usize, rd32(hdr + 0x14)? as usize)
        };
        Some((name, offset, size))
    };
    // The section-header string table gives every section its name.
    let (_, strtab_off, strtab_size) = sh_fields(shstrndx)?;
    let strtab = bin.get(strtab_off..strtab_off + strtab_size)?;
    for i in 0..shnum {
        let (name_off, offset, size) = sh_fields(i)?;
        let name_end = strtab[name_off..]
            .iter()
            .position(|&c| c == 0)
            .map(|p| name_off + p)?;
        if &strtab[name_off..name_end] == want {
            return bin.get(offset..offset + size);
        }
    }
    None
}

/// Extracts the SBOM of an image, reading the *layers* from the top down to the
/// base (the most recent package database wins).
pub fn extract_sbom(images: &ImageStore, image: &Image) -> Result<Vec<Package>> {
    let mut apk_db: Option<String> = None;
    let mut dpkg_db: Option<String> = None;
    // Distroless images register each package as its own dpkg stanza file under
    // `var/lib/dpkg/status.d/<package>` instead of the monolithic
    // `var/lib/dpkg/status`. Collect every such member, topmost layer winning
    // per path (the same precedence the monolithic status already follows),
    // keyed by the member path so a package re-shipped by an upper layer is not
    // read twice.
    let mut statusd: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    // A scratch/distroless image carrying a single static binary has no package
    // database at all — its only inventory is embedded IN that binary. Resolve
    // the Entrypoint/Cmd to its layer member (topmost wins) and read it once; a
    // relative or absent command leaves this `None` and the behaviour unchanged.
    let entry_path = entrypoint_member_path(&image.config);
    let mut entry_bin: Option<Vec<u8>> = None;
    for digest in image.layers.iter().rev() {
        let blob = images.cas().read(digest)?;
        if apk_db.is_none() {
            if let Some(b) = read_member(&blob, "lib/apk/db/installed") {
                apk_db = Some(String::from_utf8_lossy(&b).into_owned());
            }
        }
        if dpkg_db.is_none() {
            if let Some(b) = read_member(&blob, "var/lib/dpkg/status") {
                dpkg_db = Some(String::from_utf8_lossy(&b).into_owned());
            }
        }
        for (path, bytes) in read_members_with_prefix(&blob, "var/lib/dpkg/status.d/") {
            statusd
                .entry(path)
                .or_insert_with(|| String::from_utf8_lossy(&bytes).into_owned());
        }
        if entry_bin.is_none() {
            if let Some(ref p) = entry_path {
                entry_bin = read_member_exact(&blob, p);
            }
        }
    }
    let mut pkgs = Vec::new();
    if let Some(db) = apk_db {
        pkgs.extend(parse_apk(&db));
    }
    if let Some(db) = dpkg_db {
        pkgs.extend(parse_dpkg(&db));
    }
    // Each status.d/ file is a single dpkg stanza — reuse the same parser. A
    // `.md5sums` sibling (which carries no `Package:`/`Version:` lines) yields
    // nothing, so it is harmlessly ignored.
    for stanza in statusd.values() {
        pkgs.extend(parse_dpkg(stanza));
    }
    // The Entrypoint binary's embedded inventory (Go build info / Rust
    // cargo-auditable). Each reader returns nothing for the other ecosystem (or
    // for a non-matching binary), so feeding both is safe.
    if let Some(bin) = entry_bin {
        pkgs.extend(go_buildinfo_packages(&bin));
        pkgs.extend(cargo_auditable_packages(&bin));
    }
    // An image may carry BOTH the monolithic status and status.d/ stanzas (or an
    // upper layer may re-list a package); de-duplicate by (ecosystem, name),
    // keeping the first seen — apk, then monolithic dpkg, then status.d.
    let mut seen = std::collections::HashSet::new();
    pkgs.retain(|p| seen.insert((p.ecosystem, p.name.clone())));
    if pkgs.is_empty() {
        return Err(Error::EmptySbom);
    }
    pkgs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(pkgs)
}

// ---------------------------------------------------------------------------
// SBOM as a document (SPDX 2.3) — M03 of the 13-improvements programme
// ---------------------------------------------------------------------------

/// SPDX requires unique ids over a restricted alphabet: letters, digits, `.`,
/// `-`. A package name with `+`/`:` (there are some, in `dpkg`'s epoch-bearing
/// versions) would otherwise produce an invalid id, and a validator rejects
/// the whole document over it. Same rule `scripts/sbom.py` already applies to
/// the engine's own SBOM — kept identical so the two documents this project
/// publishes read the same way.
fn spdx_safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// The `purl` (package URL) type for an ecosystem — the field a consumer
/// actually queries a vulnerability database with.
fn purl(pkg: &Package) -> String {
    let ecosystem = match pkg.ecosystem {
        Ecosystem::Apk => "apk/alpine",
        Ecosystem::Dpkg => "deb",
        Ecosystem::PyPi => "pypi",
        Ecosystem::Go => "golang",
        Ecosystem::Cargo => "cargo",
    };
    format!("pkg:{ecosystem}/{}@{}", pkg.name, pkg.version)
}

/// Builds an SPDX 2.3 document for an image's SBOM — the same shape
/// `scripts/sbom.py` already gives the engine's OWN dependency tree, so the
/// two SBOMs this project produces (the binary's, the image's) read
/// identically to whatever already consumes the first one.
///
/// **Promises** the packages this store can already see: `apk`/`dpkg`
/// entries read straight from the layers, name+version+ecosystem, with a
/// `purl` a vulnerability database can query. **Does not promise** licence
/// data (`NOASSERTION` — nothing here reads a licence file) nor a signed
/// attestation of HOW the image was built; that is a SEPARATE, harder
/// problem (SLSA-style provenance needs a signing key, the way `image sign`
/// already has one for the image itself) and is not something a package
/// listing can honestly claim on its own.
///
/// Deterministic on purpose, like the engine's own SBOM: no generation
/// timestamp (`created` is fixed), so building the SAME image twice gives
/// the SAME SBOM byte for byte — the document namespace is derived from the
/// image id instead, which already changes when the content does.
pub fn sbom_spdx(image: &Image, packages: &[Package]) -> serde_json::Value {
    use serde_json::json;

    let name = image
        .repo_tags
        .first()
        .cloned()
        .unwrap_or_else(|| image.short_id());
    let root_id = format!("SPDXRef-Package-{}", spdx_safe(&image.short_id()));

    let mut spdx_packages = vec![json!({
        "SPDXID": root_id,
        "name": name,
        "versionInfo": image.id,
        "downloadLocation": "NOASSERTION",
        "licenseConcluded": "NOASSERTION",
        "licenseDeclared": "NOASSERTION",
        "filesAnalyzed": false,
    })];
    let mut relationships = vec![json!({
        "spdxElementId": "SPDXRef-DOCUMENT",
        "relationshipType": "DESCRIBES",
        "relatedSpdxElement": root_id,
    })];

    for p in packages {
        let pid = format!(
            "SPDXRef-Package-{}-{}",
            spdx_safe(&p.name),
            spdx_safe(&p.version)
        );
        spdx_packages.push(json!({
            "SPDXID": pid,
            "name": p.name,
            "versionInfo": p.version,
            "downloadLocation": "NOASSERTION",
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
            "filesAnalyzed": false,
            "externalRefs": [{
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": purl(p),
            }],
        }));
        relationships.push(json!({
            "spdxElementId": root_id,
            "relationshipType": "CONTAINS",
            "relatedSpdxElement": pid,
        }));
    }

    json!({
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": format!("{name}-sbom"),
        "documentNamespace": format!(
            "https://github.com/ngolacloud/delonix/image-sbom/{}",
            image.short_id()
        ),
        "creationInfo": {
            "creators": ["Tool: delonix-scanner", "Organization: angolardevops"],
            "created": "1970-01-01T00:00:00Z",
            "comment": "Generated by reading the image's own layers (apk/dpkg package \
                databases), without mounting or running it. Covers installed OS packages \
                only — does not cover application-level dependencies (pip/npm/…) unless \
                they were captured by a package manager this scanner reads, and claims no \
                signed build provenance.",
        },
        "packages": spdx_packages,
        "relationships": relationships,
    })
}

// ---------------------------------------------------------------------------
// in-toto attestation wrapper (GAP-0009) — wraps a predicate (the SPDX SBOM
// above, or a SLSA provenance predicate in GAP-0008) in an in-toto Statement so
// it can be ATTACHED to an image as an OCI attestation, discoverable by a
// standard consumer (`cosign verify-attestation`, `buildx imagetools inspect`).
// RAW Statement, NOT a DSSE envelope — the exact form BuildKit stores in the
// image (`exporter/containerimage/writer.go` marshals the `intoto.Statement`
// directly as the blob; the DSSE envelope is a separate, signed path).
// ---------------------------------------------------------------------------

/// SPDX predicate type (BuildKit `intoto.PredicateSPDX`). The predicate-level
/// identity of an SBOM; the OCI transport constants (layer mediaType, the
/// `in-toto.io/predicate-type` annotation) live in `delonix-oci`'s `registry`,
/// since this crate depends on `delonix-oci` and not the other way around.
pub const PREDICATE_TYPE_SPDX: &str = "https://spdx.dev/Document";
/// in-toto Statement `_type` (v1).
const INTOTO_STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";

/// Wraps `predicate` in an in-toto Statement v1 bound to one subject
/// (`subject_name` at digest `sha256:<subject_sha256_hex>`). Generic over the
/// predicate type on purpose: GAP-0008 (SLSA provenance) reuses this with the
/// same attach path, only a different `predicate_type`/`predicate`. Returns the
/// RAW Statement BuildKit stores as the attestation blob — no DSSE envelope.
pub fn intoto_statement(
    predicate_type: &str,
    subject_name: &str,
    subject_sha256_hex: &str,
    predicate: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "_type": INTOTO_STATEMENT_TYPE,
        "predicateType": predicate_type,
        "subject": [{
            "name": subject_name,
            "digest": { "sha256": subject_sha256_hex },
        }],
        "predicate": predicate,
    })
}

/// Convenience over [`intoto_statement`]: build the SPDX SBOM for
/// `image`/`packages` (via [`sbom_spdx`]) and wrap it in an in-toto Statement
/// whose subject is the image at `subject_sha256_hex` — the image MANIFEST
/// digest the caller resolves at push time (NOT `image.id`, the config digest).
/// The subject name is the image's first repo tag, or its short id.
pub fn sbom_intoto_statement(
    image: &Image,
    packages: &[Package],
    subject_sha256_hex: &str,
) -> serde_json::Value {
    let name = image
        .repo_tags
        .first()
        .cloned()
        .unwrap_or_else(|| image.short_id());
    intoto_statement(
        PREDICATE_TYPE_SPDX,
        &name,
        subject_sha256_hex,
        sbom_spdx(image, packages),
    )
}

// ---------------------------------------------------------------------------
// SLSA provenance (GAP-0008) — a build provenance predicate, wrapped by the
// in-toto helper above and attached by the SAME `push_attestation` path the
// SBOM uses. Pure assembly: the caller gathers the facts (it owns the build),
// this turns them into the SLSA v1 predicate shape BuildKit emits.
// ---------------------------------------------------------------------------

/// SLSA provenance predicate type, v1 (BuildKit's default; `slsa1.PredicateSLSAProvenance`).
pub const PREDICATE_TYPE_SLSA: &str = "https://slsa.dev/provenance/v1";

/// How much the provenance reveals — BuildKit's `mode=min|max`. `Min` is "safe
/// on all builds": timestamps, builder, and the resolved dependencies, but NOT
/// the values of build args (which can carry secrets). `Max` adds the build-arg
/// values. Default is `Max`, as in BuildKit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvenanceMode {
    Min,
    Max,
}

/// One resolved build dependency — a base image, by reference and (when known)
/// its manifest/config digest. The `resolvedDependencies` of SLSA v1: what
/// actually went into the build.
#[derive(Clone, Debug)]
pub struct Material {
    pub uri: String,
    pub sha256: Option<String>,
}

/// The facts a build knows about itself, pre-formatted by the caller (which
/// owns the build and the clock) so this stays a pure, testable function.
#[derive(Clone, Debug)]
pub struct ProvenanceFacts {
    /// `runDetails.builder.id` — e.g. `https://github.com/ngolacloud/delonix@0.1.0`.
    pub builder_id: String,
    /// `buildDefinition.buildType` — the recipe URI this engine's builds follow.
    pub build_type: String,
    /// The image reference/tag this build produced.
    pub target: String,
    /// The Dockerfile path (external parameter).
    pub dockerfile: String,
    /// The target platform (`amd64`/`arm64`/…).
    pub platform: String,
    /// Build args as `(key, value)` — values included only in `Max` mode.
    pub build_args: Vec<(String, String)>,
    /// Resolved base images (SLSA `resolvedDependencies`).
    pub materials: Vec<Material>,
    /// `runDetails.metadata.invocationId` — the built image id is a good stable one.
    pub invocation_id: String,
    /// `runDetails.metadata.finishedOn`, RFC3339, when known.
    pub finished_on: Option<String>,
}

/// Builds the SLSA v1 provenance predicate from `facts`. Shape mirrors
/// BuildKit's `ProvenancePredicateSLSA1`: `buildDefinition{buildType,
/// externalParameters, internalParameters, resolvedDependencies}` +
/// `runDetails{builder, metadata}`. `byproducts` is left out — BuildKit does not
/// populate it either. In `Min`, build-arg VALUES are omitted (keys kept, so the
/// shape is stable); in `Max`, values are included.
pub fn slsa_provenance_predicate(
    facts: &ProvenanceFacts,
    mode: ProvenanceMode,
) -> serde_json::Value {
    use serde_json::json;

    let build_args = facts
        .build_args
        .iter()
        .map(|(k, v)| {
            let shown = if mode == ProvenanceMode::Max {
                v.clone()
            } else {
                String::new() // Min: key present, value withheld (may be a secret)
            };
            (k.clone(), json!(shown))
        })
        .collect::<serde_json::Map<String, serde_json::Value>>();

    let resolved_dependencies = facts
        .materials
        .iter()
        .map(|m| {
            let mut d = json!({ "uri": m.uri });
            if let Some(hex) = &m.sha256 {
                d["digest"] = json!({ "sha256": hex });
            }
            d
        })
        .collect::<Vec<_>>();

    let mut metadata = json!({ "invocationId": facts.invocation_id });
    if let Some(ts) = &facts.finished_on {
        metadata["finishedOn"] = json!(ts);
    }

    json!({
        "buildDefinition": {
            "buildType": facts.build_type,
            "externalParameters": {
                "target": facts.target,
                "dockerfile": facts.dockerfile,
                "platform": facts.platform,
                "buildArgs": build_args,
            },
            "internalParameters": {},
            "resolvedDependencies": resolved_dependencies,
        },
        "runDetails": {
            "builder": { "id": facts.builder_id },
            "metadata": metadata,
        },
    })
}

// ---------------------------------------------------------------------------
// Scan policy + compare (GAP-0013) — the maturity layer over the OSV match
// above: a declarative policy (threshold + waivers) a CI gate reads, and a
// delta between two images' findings. Pure: `cmd_scan` already owns the I/O and
// the exit code; these turn findings into a verdict and a diff.
// ---------------------------------------------------------------------------

/// A declarative scan policy, loaded from JSON — the thing a CI pipeline commits
/// next to its code. Beyond the single `--fail-on` severity: it also carries
/// **waivers** (advisory ids accepted with an audit trail in the file) and can
/// ignore findings with no fix available yet.
#[derive(Deserialize, Default, Clone, Debug)]
pub struct Policy {
    /// Fail when the worst NON-WAIVED finding is at least this severe.
    #[serde(default)]
    pub fail_on: Option<Severity>,
    /// Advisory/CVE ids to waive — treated as not present. The file is the audit
    /// trail (why each is accepted), which a bare `--fail-on` cannot record.
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Drop findings with no fixed version available (`fixed` empty): nothing the
    /// operator can act on today, so some policies prefer not to gate on them.
    #[serde(default)]
    pub ignore_unfixed: bool,
}

impl Policy {
    /// Loads a policy from JSON. An empty/absent `fail_on` means the policy
    /// gates on nothing by severity (only waivers/ignore apply) — explicit, not
    /// a silent "allow everything".
    pub fn load(json: &str) -> Result<Policy> {
        serde_json::from_str(json).map_err(Error::AdvisoryDb)
    }
}

/// The verdict of applying a [`Policy`] to a set of findings.
#[derive(Debug)]
pub struct PolicyOutcome {
    /// Findings that survived the waivers / `ignore_unfixed`.
    pub kept: Vec<Finding>,
    /// Findings removed by the `ignore` waiver list.
    pub waived: Vec<Finding>,
    /// The worst severity among `kept` (what the gate judges).
    pub worst: Option<Severity>,
    /// `true` when `worst >= fail_on` — the CI gate should fail.
    pub failed: bool,
}

/// Applies `policy` to `findings`: removes waived ids and (optionally) unfixed
/// findings, then judges the remainder against `fail_on`. Pure — the caller
/// prints and sets the exit code.
pub fn evaluate_policy(findings: &[Finding], policy: &Policy) -> PolicyOutcome {
    let ignored: std::collections::HashSet<&str> =
        policy.ignore.iter().map(String::as_str).collect();
    let mut kept = Vec::new();
    let mut waived = Vec::new();
    for f in findings {
        if ignored.contains(f.id.as_str()) {
            waived.push(f.clone());
        } else if policy.ignore_unfixed && f.fixed.is_empty() {
            // Not waived (no audit trail), just not gated on: dropped silently
            // from the verdict per the policy's own `ignore_unfixed`.
        } else {
            kept.push(f.clone());
        }
    }
    let worst = kept.iter().map(|f| f.severity).max();
    let failed = match (policy.fail_on, worst) {
        (Some(th), Some(w)) => w >= th,
        _ => false,
    };
    PolicyOutcome {
        kept,
        waived,
        worst,
        failed,
    }
}

/// The difference between two images' findings — what a `scan compare` reports.
/// Keyed by `(advisory id, package)`: the same CVE on two packages is two
/// entries, which is what an operator reasoning about a bump wants.
#[derive(Debug)]
pub struct ScanDiff {
    /// Present in `target`, absent in `base` — regressions a bump introduced.
    pub added: Vec<Finding>,
    /// Present in `base`, absent in `target` — what the bump fixed.
    pub removed: Vec<Finding>,
}

/// Computes `target` − `base` and `base` − `target` by `(id, package)`.
pub fn compare_findings(base: &[Finding], target: &[Finding]) -> ScanDiff {
    let key = |f: &Finding| (f.id.clone(), f.package.clone());
    let base_keys: std::collections::HashSet<(String, String)> = base.iter().map(key).collect();
    let target_keys: std::collections::HashSet<(String, String)> = target.iter().map(key).collect();
    let added = target
        .iter()
        .filter(|f| !base_keys.contains(&key(f)))
        .cloned()
        .collect();
    let removed = base
        .iter()
        .filter(|f| !target_keys.contains(&key(f)))
        .cloned()
        .collect();
    ScanDiff { added, removed }
}

// ---------------------------------------------------------------------------
// Version comparison (apk/dpkg style, simplified)
// ---------------------------------------------------------------------------

/// Splits a version into alternating sequences of digits / non-digits.
fn tokens(v: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut digit = false;
    for c in v.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_digit() != digit && !cur.is_empty() {
                out.push((digit, std::mem::take(&mut cur)));
            }
            digit = c.is_ascii_digit();
            cur.push(c);
        } else if !cur.is_empty() {
            out.push((digit, std::mem::take(&mut cur)));
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push((digit, cur));
    }
    out
}

/// `true` if `a < b` (version comparison). Numeric tokens compare as
/// numbers; the rest, lexicographically; whoever ends first is "smaller".
pub fn version_lt(a: &str, b: &str) -> bool {
    let (ta, tb) = (tokens(a), tokens(b));
    for i in 0..ta.len().max(tb.len()) {
        match (ta.get(i), tb.get(i)) {
            (None, Some(_)) => return true,  // a ended -> a < b
            (Some(_), None) => return false, // b ended -> a > b
            (Some((da, sa)), Some((db, sb))) => {
                let ord = if *da && *db {
                    sa.parse::<u64>()
                        .unwrap_or(0)
                        .cmp(&sb.parse::<u64>().unwrap_or(0))
                } else {
                    sa.cmp(sb)
                };
                if ord != std::cmp::Ordering::Equal {
                    return ord == std::cmp::Ordering::Less;
                }
            }
            (None, None) => break,
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Advisory database + matching
// ---------------------------------------------------------------------------

/// The *advisory* database (loaded from JSON; in production it would sync from
/// OSV / NVD / Alpine secdb).
pub struct AdvisoryDb {
    advisories: Vec<Advisory>,
}

impl AdvisoryDb {
    /// Loads the database from JSON (`[ {id, package, ecosystem, fixed, ...} ]`).
    pub fn load(json: &str) -> Result<Self> {
        let advisories: Vec<Advisory> = serde_json::from_str(json).map_err(Error::AdvisoryDb)?;
        Ok(Self { advisories })
    }

    /// Number of loaded advisories.
    pub fn len(&self) -> usize {
        self.advisories.len()
    }
    pub fn is_empty(&self) -> bool {
        self.advisories.is_empty()
    }

    /// Cross-references the SBOM with the advisories: a package is vulnerable if
    /// there is an advisory of the same ecosystem/name with `version < fixed`.
    pub fn scan(&self, sbom: &[Package]) -> Vec<Finding> {
        let mut out = Vec::new();
        for adv in &self.advisories {
            for pkg in sbom {
                if pkg.ecosystem == adv.ecosystem
                    && pkg.name == adv.package
                    && version_lt(&pkg.version, &adv.fixed)
                {
                    out.push(Finding {
                        id: adv.id.clone(),
                        package: pkg.name.clone(),
                        version: pkg.version.clone(),
                        fixed: adv.fixed.clone(),
                        severity: adv.severity,
                        summary: adv.summary.clone(),
                    });
                }
            }
        }
        // most severe first
        out.sort_by_key(|f| std::cmp::Reverse(f.severity));
        out
    }
}

// ---------------------------------------------------------------------------
// Ingestion of real feeds (OSV) + database staleness
// ---------------------------------------------------------------------------

/// Maps the OSV `ecosystem` (e.g. `Alpine:v3.18`, `Debian:11`, `Ubuntu:22.04`)
/// to the package ecosystem we know how to cross-reference. Returns `None` for
/// the ones we don't support (npm, Go, …) — they are ignored, not turned into junk.
fn map_osv_ecosystem(s: &str) -> Option<Ecosystem> {
    if s.starts_with("Alpine") {
        Some(Ecosystem::Apk)
    } else if s.starts_with("Debian") || s.starts_with("Ubuntu") {
        Some(Ecosystem::Dpkg)
    } else if s == "PyPI" {
        Some(Ecosystem::PyPi)
    } else if s == "Go" {
        Some(Ecosystem::Go)
    } else if s == "crates.io" {
        Some(Ecosystem::Cargo)
    } else {
        None
    }
}

/// Extracts a severity from an OSV object (vuln or affected) from the
/// `database_specific.severity` / `ecosystem_specific.severity` (string). The CVSS
/// vector is NOT interpreted (it would require computing the score) — anything
/// without a label falls back to the caller's default.
fn osv_severity_label(obj: &serde_json::Value) -> Option<Severity> {
    for key in ["database_specific", "ecosystem_specific"] {
        if let Some(s) = obj
            .get(key)
            .and_then(|d| d.get("severity"))
            .and_then(|v| v.as_str())
        {
            if let Some(sev) = Severity::parse(s) {
                return Some(sev);
            }
        }
    }
    None
}

/// First FIXED version (`{"fixed": …}`) in the `ranges[].events[]` of an
/// OSV `affected` object.
fn osv_first_fixed(affected: &serde_json::Value) -> Option<String> {
    for range in affected.get("ranges")?.as_array()? {
        let Some(events) = range.get("events").and_then(|e| e.as_array()) else {
            continue;
        };
        for ev in events {
            if let Some(f) = ev.get("fixed").and_then(|v| v.as_str()) {
                return Some(f.to_string());
            }
        }
    }
    None
}

/// Converts ONE OSV vuln object into advisories — one per affected package
/// with a first-fixed version, for the ecosystems we cross-reference (the rest
/// ignored) — appending to `out`. Shared by the array/`{vulns}` feed path and
/// the per-file zip path, so the two feed shapes map a record identically.
fn push_osv_vuln(vuln: &serde_json::Value, out: &mut Vec<Advisory>) {
    let id = vuln.get("id").and_then(|x| x.as_str()).unwrap_or("");
    if id.is_empty() {
        return;
    }
    let summary = vuln
        .get("summary")
        .and_then(|x| x.as_str())
        .or_else(|| vuln.get("details").and_then(|x| x.as_str()))
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    let vuln_sev = osv_severity_label(vuln);
    let Some(affected) = vuln.get("affected").and_then(|x| x.as_array()) else {
        return;
    };
    for aff in affected {
        let Some(pkg) = aff.get("package") else {
            continue;
        };
        let name = pkg.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let eco = pkg.get("ecosystem").and_then(|x| x.as_str()).unwrap_or("");
        let (Some(ecosystem), false) = (map_osv_ecosystem(eco), name.is_empty()) else {
            continue;
        };
        let Some(fixed) = osv_first_fixed(aff) else {
            continue;
        };
        let severity = osv_severity_label(aff)
            .or(vuln_sev)
            .unwrap_or(Severity::Medium);
        out.push(Advisory {
            id: id.to_string(),
            package: name.to_string(),
            ecosystem,
            fixed,
            severity,
            summary: summary.clone(),
        });
    }
}

/// Converts an **OSV** feed (an array of vulns, or `{"vulns":[…]}`) into the
/// internal advisory database. Best-effort: each vuln generates one advisory per
/// (package, ecosystem, first-fixed-version) for the supported ecosystems
/// (Alpine→apk, Debian/Ubuntu→dpkg); the rest are ignored. Without a severity
/// label, it assumes `Medium`. Pure function — testable without the network.
pub fn advisories_from_osv(json: &str) -> Result<Vec<Advisory>> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(Error::OsvNotJson)?;
    let vulns = if let Some(arr) = v.as_array() {
        arr.clone()
    } else if let Some(arr) = v.get("vulns").and_then(|x| x.as_array()) {
        arr.clone()
    } else {
        return Err(Error::OsvShape);
    };
    let mut out = Vec::new();
    for vuln in &vulns {
        push_osv_vuln(vuln, &mut out);
    }
    Ok(out)
}

/// Minimal reader for the ZIP container OSV distributes its per-ecosystem
/// export as (`<ecosystem>/all.zip`: one JSON record per vuln). Only the two
/// things that archive actually uses are handled — STORED (0) and DEFLATE (8)
/// — read through the CENTRAL DIRECTORY, whose sizes and offsets are always
/// correct (a local header may defer them to a trailing data descriptor).
/// No zip64: OSV's exports are a few thousand small files, far under the
/// u16/u32 limits the base format carries; a zip that needs zip64 is refused
/// rather than mis-read. Returns the decompressed bytes of every `.json`
/// member. Zero new dependencies — the deflate decoder is the `flate2` the
/// crate already uses for layers. Pure (no network, no fs), so the whole
/// OSV→advisory path stays testable without either.
fn zip_json_members(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    fn u16le(b: &[u8], o: usize) -> Option<usize> {
        Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]) as usize)
    }
    fn u32le(b: &[u8], o: usize) -> Option<usize> {
        Some(
            u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?])
                as usize,
        )
    }
    let bad = |m: &str| Error::OsvZip(m.to_string());

    // End of Central Directory record (signature PK\x05\x06), 22 bytes plus an
    // optional trailing comment — scan backwards for it over the last 64 KiB.
    const EOCD: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    if data.len() < 22 {
        return Err(bad("too small to be a zip"));
    }
    let scan_from = data.len().saturating_sub(22 + 0xffff);
    let eocd = (scan_from..=data.len() - 22)
        .rev()
        .find(|&i| data[i..i + 4] == EOCD)
        .ok_or_else(|| bad("no end-of-central-directory record (not a zip?)"))?;
    let count = u16le(data, eocd + 10).ok_or_else(|| bad("truncated EOCD"))?;
    let cd_off = u32le(data, eocd + 16).ok_or_else(|| bad("truncated EOCD"))?;
    if count == 0xffff || cd_off == 0xffff_ffff {
        return Err(bad("zip64 is not supported"));
    }

    let mut out = Vec::with_capacity(count);
    let mut p = cd_off;
    for _ in 0..count {
        // Central-directory file header (signature PK\x01\x02), 46 bytes fixed.
        if data.get(p..p + 4) != Some(&[0x50, 0x4b, 0x01, 0x02]) {
            return Err(bad("bad central-directory header"));
        }
        let method = u16le(data, p + 10).ok_or_else(|| bad("truncated CD header"))?;
        let comp_size = u32le(data, p + 20).ok_or_else(|| bad("truncated CD header"))?;
        let name_len = u16le(data, p + 28).ok_or_else(|| bad("truncated CD header"))?;
        let extra_len = u16le(data, p + 30).ok_or_else(|| bad("truncated CD header"))?;
        let comment_len = u16le(data, p + 32).ok_or_else(|| bad("truncated CD header"))?;
        let lho = u32le(data, p + 42).ok_or_else(|| bad("truncated CD header"))?;
        let name = data
            .get(p + 46..p + 46 + name_len)
            .ok_or_else(|| bad("truncated file name"))?;
        let is_json = name.ends_with(b".json");
        p += 46 + name_len + extra_len + comment_len;
        if !is_json {
            continue;
        }

        // Local file header (signature PK\x03\x04), 30 bytes fixed, then its
        // OWN name+extra (the extra length can differ from the CD's), then the
        // file data of `comp_size` bytes.
        if data.get(lho..lho + 4) != Some(&[0x50, 0x4b, 0x03, 0x04]) {
            return Err(bad("bad local file header"));
        }
        let lname = u16le(data, lho + 26).ok_or_else(|| bad("truncated local header"))?;
        let lextra = u16le(data, lho + 28).ok_or_else(|| bad("truncated local header"))?;
        let start = lho + 30 + lname + lextra;
        let comp = data
            .get(start..start + comp_size)
            .ok_or_else(|| bad("truncated file data"))?;
        let bytes = match method {
            0 => comp.to_vec(),
            8 => {
                let mut buf = Vec::new();
                flate2::read::DeflateDecoder::new(comp)
                    .read_to_end(&mut buf)
                    .map_err(|e| bad(&format!("deflate failed: {e}")))?;
                buf
            }
            m => return Err(bad(&format!("unsupported compression method {m}"))),
        };
        out.push(bytes);
    }
    Ok(out)
}

/// Converts an **OSV ZIP export** (`<ecosystem>/all.zip`, one JSON record per
/// `.json` member) into advisories. A member that is not valid JSON, or not an
/// OSV record, is skipped — one bad file in a 1700-file export must not sink
/// the whole sync; a wholly unreadable zip (not the files, the container) is
/// the hard error [`Error::OsvZip`]. Pure function.
pub fn advisories_from_osv_zip(zip: &[u8]) -> Result<Vec<Advisory>> {
    let members = zip_json_members(zip)?;
    let mut out = Vec::new();
    for raw in &members {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(raw) {
            push_osv_vuln(&v, &mut out);
        }
    }
    Ok(out)
}

/// `true` if the advisory database is STALE (synced more than `max_age_days`
/// ago, or never synced). Used to warn that a scan with no findings may not
/// be trustworthy. Pure function.
pub fn db_is_stale(synced_unix: Option<u64>, now_unix: u64, max_age_days: u64) -> bool {
    match synced_unix {
        None => true,
        Some(t) => now_unix.saturating_sub(t) > max_age_days.saturating_mul(86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(id: &str, tags: &[&str]) -> Image {
        Image {
            id: id.to_string(),
            repo_tags: tags.iter().map(|t| t.to_string()).collect(),
            layers: vec![],
            config: delonix_oci::ImageConfig::default(),
            created_unix: 0,
        }
    }

    #[test]
    fn sbom_spdx_names_every_package_with_a_purl() {
        let img = image("sha256:abc123", &["myapp:latest"]);
        let pkgs = vec![
            Package {
                name: "busybox".into(),
                version: "1.36.1-r20".into(),
                ecosystem: Ecosystem::Apk,
            },
            Package {
                name: "libc6".into(),
                version: "2.36-9".into(),
                ecosystem: Ecosystem::Dpkg,
            },
        ];
        let doc = sbom_spdx(&img, &pkgs);
        assert_eq!(doc["spdxVersion"], "SPDX-2.3");
        let packages = doc["packages"].as_array().unwrap();
        // The image itself, plus the two packages.
        assert_eq!(packages.len(), 3);
        let purls: Vec<&str> = packages
            .iter()
            .filter_map(|p| p["externalRefs"][0]["referenceLocator"].as_str())
            .collect();
        assert_eq!(
            purls,
            vec!["pkg:apk/alpine/busybox@1.36.1-r20", "pkg:deb/libc6@2.36-9"]
        );
    }

    #[test]
    fn sbom_spdx_is_deterministic_for_the_same_image() {
        let img = image("sha256:same", &["x:1"]);
        let pkgs = vec![Package {
            name: "a".into(),
            version: "1".into(),
            ecosystem: Ecosystem::Apk,
        }];
        assert_eq!(sbom_spdx(&img, &pkgs), sbom_spdx(&img, &pkgs));
    }

    #[test]
    fn sbom_spdx_falls_back_to_the_short_id_when_untagged() {
        let img = image(
            "sha256:deadbeefcafebabe0000000000000000000000000000000000000000000000",
            &[],
        );
        let doc = sbom_spdx(&img, &[]);
        assert_eq!(doc["packages"][0]["name"], "deadbeefcafe");
    }

    #[test]
    fn intoto_statement_binds_predicate_to_subject() {
        let st = intoto_statement(
            PREDICATE_TYPE_SPDX,
            "myapp:latest",
            "abc123",
            serde_json::json!({"spdxVersion": "SPDX-2.3"}),
        );
        assert_eq!(st["_type"], "https://in-toto.io/Statement/v1");
        assert_eq!(st["predicateType"], "https://spdx.dev/Document");
        // one subject, named, with the sha256 digest the attach binds to.
        assert_eq!(st["subject"].as_array().unwrap().len(), 1);
        assert_eq!(st["subject"][0]["name"], "myapp:latest");
        assert_eq!(st["subject"][0]["digest"]["sha256"], "abc123");
        // the predicate is carried through verbatim.
        assert_eq!(st["predicate"]["spdxVersion"], "SPDX-2.3");
    }

    fn facts() -> ProvenanceFacts {
        ProvenanceFacts {
            builder_id: "https://github.com/ngolacloud/delonix@0.1.0".into(),
            build_type: "https://github.com/ngolacloud/delonix/build@v1".into(),
            target: "myapp:v1".into(),
            dockerfile: "Dockerfile".into(),
            platform: "amd64".into(),
            build_args: vec![("TOKEN".into(), "s3cr3t".into())],
            materials: vec![Material {
                uri: "alpine:3.20".into(),
                sha256: Some("bf8527eb".into()),
            }],
            invocation_id: "sha256:abc".into(),
            finished_on: Some("2026-11-12T00:00:00Z".into()),
        }
    }

    fn finding(id: &str, pkg: &str, sev: Severity, fixed: &str) -> Finding {
        Finding {
            id: id.into(),
            package: pkg.into(),
            version: "1.0".into(),
            fixed: fixed.into(),
            severity: sev,
            summary: String::new(),
        }
    }

    #[test]
    fn policy_waives_by_id_and_judges_the_rest_against_fail_on() {
        let findings = vec![
            finding("CVE-1", "openssl", Severity::Critical, "3.1"),
            finding("CVE-2", "zlib", Severity::High, "1.3"),
        ];
        // Waive the critical; gate on high.
        let policy = Policy {
            fail_on: Some(Severity::High),
            ignore: vec!["CVE-1".into()],
            ignore_unfixed: false,
        };
        let out = evaluate_policy(&findings, &policy);
        assert_eq!(out.waived.len(), 1);
        assert_eq!(out.kept.len(), 1);
        assert_eq!(out.worst, Some(Severity::High));
        assert!(out.failed, "a kept High meets the High threshold");

        // Waiving the only remaining one too → nothing left → pass.
        let policy2 = Policy {
            fail_on: Some(Severity::High),
            ignore: vec!["CVE-1".into(), "CVE-2".into()],
            ignore_unfixed: false,
        };
        let out2 = evaluate_policy(&findings, &policy2);
        assert_eq!(out2.worst, None);
        assert!(!out2.failed);
    }

    #[test]
    fn policy_ignore_unfixed_drops_findings_with_no_fix() {
        let findings = vec![finding("CVE-9", "glibc", Severity::Critical, "")];
        let gated = evaluate_policy(
            &findings,
            &Policy {
                fail_on: Some(Severity::Low),
                ignore: vec![],
                ignore_unfixed: false,
            },
        );
        assert!(gated.failed, "unfixed critical fails when not ignored");
        let lenient = evaluate_policy(
            &findings,
            &Policy {
                fail_on: Some(Severity::Low),
                ignore: vec![],
                ignore_unfixed: true,
            },
        );
        assert!(!lenient.failed, "ignore_unfixed drops it from the gate");
        assert_eq!(lenient.worst, None);
    }

    #[test]
    fn compare_reports_added_and_removed_by_id_and_package() {
        let base = vec![
            finding("CVE-1", "openssl", Severity::High, "3.1"),
            finding("CVE-2", "zlib", Severity::Low, "1.3"),
        ];
        let target = vec![
            finding("CVE-2", "zlib", Severity::Low, "1.3"), // unchanged
            finding("CVE-3", "curl", Severity::Critical, "8.5"), // new regression
        ];
        let diff = compare_findings(&base, &target);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].id, "CVE-3");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].id, "CVE-1");
    }

    #[test]
    fn slsa_max_includes_build_arg_values_and_materials() {
        let p = slsa_provenance_predicate(&facts(), ProvenanceMode::Max);
        assert_eq!(
            p["buildDefinition"]["buildType"],
            "https://github.com/ngolacloud/delonix/build@v1"
        );
        // Max reveals the build-arg value.
        assert_eq!(
            p["buildDefinition"]["externalParameters"]["buildArgs"]["TOKEN"],
            "s3cr3t"
        );
        // The base image is a resolved dependency, by uri + digest.
        let dep = &p["buildDefinition"]["resolvedDependencies"][0];
        assert_eq!(dep["uri"], "alpine:3.20");
        assert_eq!(dep["digest"]["sha256"], "bf8527eb");
        assert_eq!(
            p["runDetails"]["builder"]["id"],
            "https://github.com/ngolacloud/delonix@0.1.0"
        );
        assert_eq!(
            p["runDetails"]["metadata"]["finishedOn"],
            "2026-11-12T00:00:00Z"
        );
    }

    #[test]
    fn slsa_min_withholds_build_arg_values_but_keeps_keys() {
        let p = slsa_provenance_predicate(&facts(), ProvenanceMode::Min);
        // Min keeps the key (stable shape) but withholds the value (may be a secret).
        let args = &p["buildDefinition"]["externalParameters"]["buildArgs"];
        assert_eq!(args["TOKEN"], "");
        assert!(args.get("TOKEN").is_some(), "key stays present in min");
    }

    #[test]
    fn sbom_intoto_statement_wraps_the_same_spdx_the_direct_call_produces() {
        let img = image("sha256:abc123", &["myapp:latest"]);
        let pkgs = vec![Package {
            name: "busybox".into(),
            version: "1.36.1-r20".into(),
            ecosystem: Ecosystem::Apk,
        }];
        let st = sbom_intoto_statement(&img, &pkgs, "manifestdigesthex");
        // subject is the MANIFEST digest the caller resolved, not image.id.
        assert_eq!(st["subject"][0]["name"], "myapp:latest");
        assert_eq!(st["subject"][0]["digest"]["sha256"], "manifestdigesthex");
        // predicate is byte-identical to the standalone SPDX document.
        assert_eq!(st["predicate"], sbom_spdx(&img, &pkgs));
        assert_eq!(st["predicateType"], PREDICATE_TYPE_SPDX);
    }

    #[test]
    fn spdx_safe_replaces_characters_outside_the_allowed_alphabet() {
        assert_eq!(spdx_safe("libssl1.1:amd64"), "libssl1.1-amd64");
        assert_eq!(spdx_safe("pkg+epoch:1.0"), "pkg-epoch-1.0");
    }

    #[test]
    fn version_comparison() {
        assert!(version_lt("1.36.1-r20", "1.37.0")); // busybox vulnerable
        assert!(!version_lt("1.3.1-r0", "1.3")); // zlib already fixed
        assert!(version_lt("3.1.7", "3.1.8"));
        assert!(!version_lt("3.1.8-r1", "3.1.8"));
        assert!(version_lt("2.36-9", "2.37"));
    }

    #[test]
    fn parses_apk_db() {
        let db = "C:Q1x\nP:busybox\nV:1.36.1-r20\nA:x86_64\n\nP:zlib\nV:1.3.1-r0\n";
        let pkgs = parse_apk(db);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "busybox");
        assert_eq!(pkgs[0].version, "1.36.1-r20");
    }

    #[test]
    fn parses_dpkg_status() {
        let db = "Package: libc6\nStatus: install ok installed\nVersion: 2.36-9\n\nPackage: bash\nVersion: 5.2-15\n";
        let pkgs = parse_dpkg(db);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[1].name, "bash");
    }

    /// Builds a gzipped tar layer from `(path, bytes)` members and writes it to
    /// the store's CAS, returning a single-layer `Image` over it — the real path
    /// `extract_sbom` reads (no mounting, no running).
    fn layer_image(store: &ImageStore, files: &[(&str, &[u8])]) -> Image {
        use std::io::Write;
        let mut tar_buf = Vec::new();
        {
            let mut ar = tar::Builder::new(&mut tar_buf);
            for (path, data) in files {
                let mut header = tar::Header::new_gnu();
                header.set_path(path).unwrap();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                ar.append(&header, *data).unwrap();
            }
            ar.finish().unwrap();
        }
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&tar_buf).unwrap();
        let layer = gz.finish().unwrap();
        let digest = store.cas().write(&layer).unwrap();
        Image {
            id: "sha256:distroless".into(),
            repo_tags: vec!["cc-debian13:test".into()],
            layers: vec![digest],
            config: delonix_oci::ImageConfig::default(),
            created_unix: 0,
        }
    }

    #[test]
    fn parses_dpkg_status_d() {
        // Distroless (gcr.io/distroless/cc-debian13) keeps ONE stanza per file
        // under var/lib/dpkg/status.d/ — the scanner must read the whole
        // directory, not just a single `status` file (which does not exist).
        let dir = tempfile::tempdir().unwrap();
        let store = ImageStore::open(dir.path()).unwrap();
        let img = layer_image(
            &store,
            &[
                (
                    "var/lib/dpkg/status.d/base-files",
                    b"Package: base-files\nStatus: install ok installed\nVersion: 13.1\n",
                ),
                (
                    "var/lib/dpkg/status.d/libc6",
                    b"Package: libc6\nStatus: install ok installed\nVersion: 2.37-15\n",
                ),
                // A .md5sums sibling (no Package:/Version:) must be ignored, not
                // turned into a junk package.
                (
                    "var/lib/dpkg/status.d/libc6.md5sums",
                    b"abc123  usr/lib/x86_64-linux-gnu/libc.so.6\n",
                ),
            ],
        );
        let sbom = extract_sbom(&store, &img).unwrap();
        assert_eq!(sbom.len(), 2, "two real packages, the .md5sums ignored");
        assert_eq!(sbom[0].name, "base-files");
        assert_eq!(sbom[0].version, "13.1");
        assert_eq!(sbom[0].ecosystem, Ecosystem::Dpkg);
        assert_eq!(sbom[1].name, "libc6");
        assert_eq!(sbom[1].version, "2.37-15");
    }

    #[test]
    fn extract_sbom_merges_status_and_status_d_without_duplicates() {
        // An image carrying BOTH the monolithic status and a status.d/ stanza:
        // the shared package (libc6) must appear once, the extras from each side
        // must all be present.
        let dir = tempfile::tempdir().unwrap();
        let store = ImageStore::open(dir.path()).unwrap();
        let img = layer_image(
            &store,
            &[
                (
                    "var/lib/dpkg/status",
                    b"Package: libc6\nVersion: 2.37-15\n\nPackage: bash\nVersion: 5.2-15\n",
                ),
                (
                    "var/lib/dpkg/status.d/libc6",
                    b"Package: libc6\nVersion: 2.37-15\n",
                ),
                (
                    "var/lib/dpkg/status.d/tzdata",
                    b"Package: tzdata\nVersion: 2025a-1\n",
                ),
            ],
        );
        let sbom = extract_sbom(&store, &img).unwrap();
        let names: Vec<&str> = sbom.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["bash", "libc6", "tzdata"]);
        assert_eq!(
            sbom.iter().filter(|p| p.name == "libc6").count(),
            1,
            "libc6 listed by both sources must not be duplicated"
        );
    }

    /// A hand-built Go binary carrying a build-info blob in the modern inline
    /// format (the one `go version -m` reads), embedded between junk bytes so
    /// the magic scan is exercised. The parser does not check the 16-byte
    /// sentinel VALUES, only strips them, so zero padding stands in for the real
    /// sentinels. Lists `rsc.io/quote@v1.5.2` and `golang.org/x/text@v0.3.0`;
    /// the `(devel)` main module and `path`/`build` lines must be skipped.
    fn go_buildinfo_fixture() -> Vec<u8> {
        fn uvarint_enc(mut n: u64, out: &mut Vec<u8>) {
            loop {
                let mut b = (n & 0x7f) as u8;
                n >>= 7;
                if n != 0 {
                    b |= 0x80;
                }
                out.push(b);
                if n == 0 {
                    break;
                }
            }
        }
        let body = b"path\texample.com/app\nmod\texample.com/app\t(devel)\t\ndep\trsc.io/quote\tv1.5.2\th1:aaa\ndep\tgolang.org/x/text\tv0.3.0\th1:bbb\n";
        let mut modinfo = vec![0u8; 16];
        modinfo.extend_from_slice(body);
        modinfo.extend_from_slice(&[0u8; 16]);

        let mut blob = Vec::new();
        blob.extend_from_slice(b"\xff Go buildinf:"); // magic (14)
        blob.push(8); // ptrSize
        blob.push(2); // flags: inline
        blob.extend_from_slice(&[0u8; 16]); // zeroed pointer slots
        let version = b"go1.21";
        uvarint_enc(version.len() as u64, &mut blob);
        blob.extend_from_slice(version);
        uvarint_enc(modinfo.len() as u64, &mut blob);
        blob.extend_from_slice(&modinfo);

        let mut binary = vec![0xABu8; 100];
        binary.extend_from_slice(&blob);
        binary.extend_from_slice(&[0xCDu8; 50]);
        binary
    }

    #[test]
    fn go_buildinfo_extracts_modules_and_ignores_devel() {
        let binary = go_buildinfo_fixture();
        let pkgs = go_buildinfo_packages(&binary);
        let got: Vec<(&str, &str)> = pkgs
            .iter()
            .map(|p| (p.name.as_str(), p.version.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![("rsc.io/quote", "v1.5.2"), ("golang.org/x/text", "v0.3.0")],
            "the (devel) main module and path/build lines must be skipped"
        );
        assert!(pkgs.iter().all(|p| p.ecosystem == Ecosystem::Go));
        // A binary with no build-info magic yields nothing (not a panic).
        assert!(go_buildinfo_packages(b"not a go binary").is_empty());
    }

    #[test]
    fn cargo_auditable_extracts_crates_from_the_dep_v0_section() {
        use std::io::Write;
        // The cargo-auditable payload: zlib-compressed JSON in the ELF section
        // `.dep-v0`.
        let json = br#"{"packages":[{"name":"serde","version":"1.0.197","source":"registry"},{"name":"libc","version":"0.2.153","source":"registry"}]}"#;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(json).unwrap();
        let dep = enc.finish().unwrap();

        // Minimal 64-bit little-endian ELF: sections [0]=null, [1]=.dep-v0,
        // [2]=.shstrtab. Enough for `elf_section` to resolve a section by name;
        // no program headers, symbols or relocations.
        let shstrtab: &[u8] = b"\0.dep-v0\0.shstrtab\0"; // names at offsets 1 and 9
        let ehsize = 64usize;
        let dep_off = ehsize;
        let strtab_off = dep_off + dep.len();
        let shoff = strtab_off + shstrtab.len();
        let shentsize = 64usize;

        let mut elf = vec![0u8; shoff + 3 * shentsize];
        elf[0..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2; // 64-bit
        elf[5] = 1; // little-endian
        elf[6] = 1; // ELF version
        elf[0x28..0x30].copy_from_slice(&(shoff as u64).to_le_bytes()); // e_shoff
        elf[0x3a..0x3c].copy_from_slice(&(shentsize as u16).to_le_bytes()); // e_shentsize
        elf[0x3c..0x3e].copy_from_slice(&3u16.to_le_bytes()); // e_shnum
        elf[0x3e..0x40].copy_from_slice(&2u16.to_le_bytes()); // e_shstrndx
        elf[dep_off..dep_off + dep.len()].copy_from_slice(&dep);
        elf[strtab_off..strtab_off + shstrtab.len()].copy_from_slice(shstrtab);

        let write_sh = |elf: &mut [u8], idx: usize, name: u32, off: u64, size: u64| {
            let h = shoff + idx * shentsize;
            elf[h..h + 4].copy_from_slice(&name.to_le_bytes());
            elf[h + 0x18..h + 0x20].copy_from_slice(&off.to_le_bytes());
            elf[h + 0x20..h + 0x28].copy_from_slice(&size.to_le_bytes());
        };
        write_sh(&mut elf, 1, 1, dep_off as u64, dep.len() as u64);
        write_sh(&mut elf, 2, 9, strtab_off as u64, shstrtab.len() as u64);

        let pkgs = cargo_auditable_packages(&elf);
        let mut names: Vec<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["libc", "serde"]);
        assert!(pkgs.iter().all(|p| p.ecosystem == Ecosystem::Cargo));
        assert_eq!(
            pkgs.iter().find(|p| p.name == "serde").unwrap().version,
            "1.0.197"
        );
        // A non-ELF input and an ELF without the section both yield nothing.
        assert!(cargo_auditable_packages(b"not an elf").is_empty());
    }

    #[test]
    fn extract_sbom_reads_the_entrypoint_binary_in_a_scratch_image() {
        // A single-binary Go `scratch` image: NO dpkg/apk database anywhere,
        // just a binary at /app. extract_sbom must resolve the Entrypoint to
        // that layer member and read the embedded Go module list — instead of
        // failing EmptySbom, which is what leaves `require-sbom` rejecting it.
        let dir = tempfile::tempdir().unwrap();
        let store = ImageStore::open(dir.path()).unwrap();
        let mut img = layer_image(&store, &[("app", &go_buildinfo_fixture())]);
        img.config.entrypoint = vec!["/app".into()];

        let sbom = extract_sbom(&store, &img).unwrap();
        let go: Vec<&str> = sbom
            .iter()
            .filter(|p| p.ecosystem == Ecosystem::Go)
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(go, vec!["golang.org/x/text", "rsc.io/quote"]); // sorted by name
        assert!(
            sbom.iter()
                .any(|p| p.name == "rsc.io/quote" && p.version == "v1.5.2"),
            "the Go module version must survive the end-to-end read"
        );
    }

    #[test]
    fn extract_sbom_ignores_a_relative_or_absent_entrypoint() {
        // A relative command (or none) must NOT be resolved — the behaviour
        // stays exactly as before (EmptySbom when there is nothing else).
        let dir = tempfile::tempdir().unwrap();
        let store = ImageStore::open(dir.path()).unwrap();
        let mut img = layer_image(&store, &[("app", &go_buildinfo_fixture())]);
        img.config.entrypoint = vec!["app".into()]; // relative → not resolved
        assert!(matches!(extract_sbom(&store, &img), Err(Error::EmptySbom)));
    }

    #[test]
    fn matches_only_vulnerable() {
        let sbom = vec![
            Package {
                name: "busybox".into(),
                version: "1.36.1-r20".into(),
                ecosystem: Ecosystem::Apk,
            },
            Package {
                name: "zlib".into(),
                version: "1.3.1-r0".into(),
                ecosystem: Ecosystem::Apk,
            },
        ];
        let db = AdvisoryDb::load(
            r#"[
              {"id":"CVE-A","package":"busybox","ecosystem":"Apk","fixed":"1.37.0","severity":"high","summary":"x"},
              {"id":"CVE-B","package":"zlib","ecosystem":"Apk","fixed":"1.3","severity":"medium","summary":"y"}
            ]"#,
        )
        .unwrap();
        let f = db.scan(&sbom);
        assert_eq!(f.len(), 1); // only busybox
        assert_eq!(f[0].id, "CVE-A");
    }

    #[test]
    fn osv_converte_alpine_e_ignora_ecossistemas_nao_suportados() {
        // An OSV feed: one Alpine vuln (convertible) + one npm (ignored).
        let feed = r#"{"vulns":[
          {
            "id":"CVE-2023-4567","summary":"busybox oops",
            "database_specific":{"severity":"HIGH"},
            "affected":[{
              "package":{"ecosystem":"Alpine:v3.18","name":"busybox"},
              "ranges":[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"1.36.1-r5"}]}]
            }]
          },
          {
            "id":"GHSA-xxxx","summary":"left-pad",
            "affected":[{"package":{"ecosystem":"npm","name":"left-pad"},
              "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.3.0"}]}]}]
          }
        ]}"#;
        let advs = advisories_from_osv(feed).unwrap();
        assert_eq!(advs.len(), 1); // only the Alpine one
        assert_eq!(advs[0].id, "CVE-2023-4567");
        assert_eq!(advs[0].package, "busybox");
        assert_eq!(advs[0].ecosystem, Ecosystem::Apk);
        assert_eq!(advs[0].fixed, "1.36.1-r5");
        assert_eq!(advs[0].severity, Severity::High);
    }

    #[test]
    fn osv_aceita_array_e_default_medium_sem_rotulo() {
        let feed = r#"[{
          "id":"CVE-1","details":"debian glibc",
          "affected":[{"package":{"ecosystem":"Debian:12","name":"libc6"},
            "ranges":[{"type":"ECOSYSTEM","events":[{"fixed":"2.36-9+deb12u1"}]}]}]
        }]"#;
        let advs = advisories_from_osv(feed).unwrap();
        assert_eq!(advs.len(), 1);
        assert_eq!(advs[0].ecosystem, Ecosystem::Dpkg);
        assert_eq!(advs[0].severity, Severity::Medium); // no label → default
                                                        // the converted advisories cross-reference against a real SBOM.
        let db = AdvisoryDb::load(&serde_json::to_string(&advs_json(&advs)).unwrap()).unwrap();
        let sbom = vec![Package {
            name: "libc6".into(),
            version: "2.36-9".into(),
            ecosystem: Ecosystem::Dpkg,
        }];
        assert_eq!(db.scan(&sbom).len(), 1);
    }

    // helper: Advisory is not Serialize; rebuilds the JSON for the round-trip.
    fn advs_json(advs: &[Advisory]) -> Vec<serde_json::Value> {
        advs.iter().map(|a| serde_json::json!({
            "id": a.id, "package": a.package,
            "ecosystem": match a.ecosystem { Ecosystem::Apk => "Apk", Ecosystem::Dpkg => "Dpkg", Ecosystem::PyPi => "PyPi", Ecosystem::Go => "Go", Ecosystem::Cargo => "Cargo" },
            "fixed": a.fixed,
            "severity": match a.severity { Severity::Low=>"low", Severity::Medium=>"medium", Severity::High=>"high", Severity::Critical=>"critical" },
            "summary": a.summary,
        })).collect()
    }

    /// Builds a minimal ZIP of `(name, bytes)` members — STORED (`deflate =
    /// false`) or DEFLATE (`true`, the method OSV's exports actually use) — so
    /// the zip path is tested without the network and without a `zip`
    /// dependency. CRCs are left zero: the reader does not check them.
    fn build_zip(files: &[(&str, &[u8])], deflate: bool) -> Vec<u8> {
        use std::io::Write;
        let mut out = Vec::new();
        let mut central = Vec::new();
        let mut local_offsets = Vec::new();
        let (method, flag): (u16, u16) = if deflate { (8, 1 << 3) } else { (0, 0) };
        let _ = flag; // flags left 0 below; data-descriptor flag not used
        for (name, data) in files {
            local_offsets.push(out.len() as u32);
            let stored: Vec<u8> = if deflate {
                let mut e =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(data).unwrap();
                e.finish().unwrap()
            } else {
                data.to_vec()
            };
            // local file header
            out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
            out.extend_from_slice(&[20, 0, 0, 0]); // version needed + flags
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0, 0, 0, 0]); // time + date
            out.extend_from_slice(&[0, 0, 0, 0]); // crc32 (unchecked)
            out.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&[0, 0]); // extra len
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&stored);
            // central-directory header, for the same member
            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0]); // made-by + needed + flags
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0]); // time + date
            central.extend_from_slice(&[0, 0, 0, 0]); // crc32
            central.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // extra + comment + disk
            central.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // internal + external attrs
            central.extend_from_slice(local_offsets.last().unwrap().to_le_bytes().as_ref());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);
        // end of central directory
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        out.extend_from_slice(&[0, 0, 0, 0]); // this disk + cd-start disk
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0, 0]); // comment len
        out
    }

    const OSV_ALPINE: &str = r#"{"id":"CVE-2023-4567","summary":"busybox oops",
      "database_specific":{"severity":"HIGH"},
      "affected":[{"package":{"ecosystem":"Alpine:v3.18","name":"busybox"},
        "ranges":[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"1.36.1-r5"}]}]}]}"#;
    const OSV_NPM: &str = r#"{"id":"GHSA-xxxx","summary":"left-pad",
      "affected":[{"package":{"ecosystem":"npm","name":"left-pad"},
        "ranges":[{"type":"SEMVER","events":[{"fixed":"1.3.0"}]}]}]}"#;

    #[test]
    fn osv_zip_stored_converte_membros_json_e_ignora_nao_json() {
        let zip = build_zip(
            &[
                ("CVE-2023-4567.json", OSV_ALPINE.as_bytes()),
                ("GHSA-xxxx.json", OSV_NPM.as_bytes()), // npm → ignored by the mapper
                ("README.txt", b"not json, not a .json member"),
            ],
            false,
        );
        let advs = advisories_from_osv_zip(&zip).unwrap();
        assert_eq!(advs.len(), 1); // only the Alpine one
        assert_eq!(advs[0].id, "CVE-2023-4567");
        assert_eq!(advs[0].package, "busybox");
        assert_eq!(advs[0].ecosystem, Ecosystem::Apk);
        assert_eq!(advs[0].fixed, "1.36.1-r5");
        assert_eq!(advs[0].severity, Severity::High);
    }

    #[test]
    fn osv_zip_deflate_le_o_metodo_que_o_osv_usa() {
        // DEFLATE (method 8) is what the real `all.zip` uses; the stored test
        // above would pass even if the inflate path were broken.
        let zip = build_zip(&[("CVE-2023-4567.json", OSV_ALPINE.as_bytes())], true);
        let advs = advisories_from_osv_zip(&zip).unwrap();
        assert_eq!(advs.len(), 1);
        assert_eq!(advs[0].fixed, "1.36.1-r5");
    }

    #[test]
    fn osv_zip_recusa_o_que_nao_e_zip() {
        // A JSON feed, an HTML error page, an empty body: none is a zip, and
        // the reader says so (it does not silently return zero advisories).
        for bad in [&b"not a zip at all"[..], b"<html>502</html>", b"", b"PK"] {
            assert!(
                matches!(advisories_from_osv_zip(bad), Err(Error::OsvZip(_))),
                "{bad:?} should be OsvZip"
            );
        }
    }

    /// The CI build-failing gate (G1c): the scanner, exercised END TO END on a
    /// REAL image layer (a gzipped tar written to the CAS, read back by
    /// `extract_sbom` — not a hand-built `Package` list), must catch a known
    /// CVE in a known-vulnerable base. If a refactor ever breaks layer reading,
    /// apk parsing, or matching, `cargo test --workspace` fails here and the
    /// build stops — which is exactly "an image scan in CI that fails the build
    /// on a known CVE". The `--fail-on high` decision that turns this into a
    /// non-zero exit is covered by the CLI's own `admission_rejects` test.
    #[test]
    fn image_scan_catches_a_planted_cve_in_a_real_layer() {
        use std::io::Write;

        // A real layer: an apk database with the busybox alpine:3.12 ships
        // (1.31.1-r22), known-vulnerable to CVE-2022-48174 (fixed 1.37.0).
        let apk_db = "C:Q1x\nP:busybox\nV:1.31.1-r22\nA:x86_64\n\nP:zlib\nV:1.2.12-r0\n";
        let mut tar_buf = Vec::new();
        {
            let mut ar = tar::Builder::new(&mut tar_buf);
            let mut header = tar::Header::new_gnu();
            header.set_path("lib/apk/db/installed").unwrap();
            header.set_size(apk_db.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            ar.append(&header, apk_db.as_bytes()).unwrap();
            ar.finish().unwrap();
        }
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&tar_buf).unwrap();
        let layer = gz.finish().unwrap();

        let dir = tempfile::tempdir().unwrap();
        let store = ImageStore::open(dir.path()).unwrap();
        let digest = store.cas().write(&layer).unwrap();
        let img = Image {
            id: "sha256:planted".into(),
            repo_tags: vec!["vuln:test".into()],
            layers: vec![digest],
            config: delonix_oci::ImageConfig::default(),
            created_unix: 0,
        };

        // SBOM read from the layer in the CAS — the path `delonix image scan` runs.
        let sbom = extract_sbom(&store, &img).unwrap();
        assert!(
            sbom.iter()
                .any(|p| p.name == "busybox" && p.version == "1.31.1-r22"),
            "extract_sbom must read busybox out of the real layer"
        );

        // The planted CVE is a real advisory (the shape a synced OSV feed yields).
        let db = AdvisoryDb::load(
            r#"[{"id":"CVE-2022-48174","package":"busybox","ecosystem":"Apk","fixed":"1.37.0","severity":"high","summary":"busybox ash stack overflow"}]"#,
        )
        .unwrap();
        let findings = db.scan(&sbom);
        assert_eq!(findings.len(), 1, "the scan must catch the planted CVE");
        assert_eq!(findings[0].id, "CVE-2022-48174");
        assert_eq!(findings[0].severity, Severity::High);

        // The build-failing decision: the worst severity trips a `--fail-on high` gate.
        let worst = findings.iter().map(|f| f.severity).max().unwrap();
        assert!(worst >= Severity::High);
    }

    #[test]
    fn staleness_deteta_nunca_e_antigo() {
        let dia = 86_400u64;
        assert!(db_is_stale(None, 100 * dia, 30)); // never synced
        assert!(db_is_stale(Some(0), 40 * dia, 30)); // 40 days > 30
        assert!(!db_is_stale(Some(20 * dia), 40 * dia, 30)); // 20 days <= 30
    }
}
