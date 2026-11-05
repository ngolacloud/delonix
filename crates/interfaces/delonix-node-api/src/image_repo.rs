//! `ImageRepo` — the node's **OCI image store** as this interface needs it
//! (R3d-g, ADR-0040 A5). The interface holds this port instead of naming the
//! `delonix-oci` adapter directly: a bin composes an implementor and hands the
//! server one, so the forbidden interface→adapter edge stays in the bin — the
//! same shape as [`crate::vms::VmPort`], [`crate::store::StorePort`],
//! [`crate::run_resolver::RunResolver`] and [`crate::net_ops::NetworkOps`].
//!
//! This Engine has no image *service* yet (the slice is PRE-R3h), so the
//! implementor is backed by `delonix-oci` DIRECTLY — the same way the bin backs
//! the record store with `delonix-state` and the SDN with `delonix-sdn` — not by
//! an engine image port. When an image service lands, this port is where it is
//! swapped in.
//!
//! The adapter's `delonix_oci::Image` is crossed as the [`StoredImage`] summary
//! — the fields the `ListImages`/`GetImage` read half renders, with the on-disk
//! size already summed by the implementor so the CAS type never leaks past the
//! port. The shared [`delonix_model::Error`] both the engine and this crate
//! speak is the error on the fallible methods; the pure `sha256:` digest rule
//! crosses as itself.

use std::path::Path;

use delonix_model::Error;

/// A stored image as the read half renders it: the fields of `delonix-oci`'s
/// `Image` the node API actually reports, crossed as a summary so the adapter
/// type (and its CAS) does not leak past the port. The on-disk size is summed by
/// the implementor at list time (reading the layer blobs' `stat`, never their
/// bytes), so this interface never touches the content store.
#[derive(Clone, Debug)]
pub struct StoredImage {
    /// Image id = digest of the config blob (`sha256:...`).
    pub id: String,
    /// The tags (`name:tag`) that point to this image; empty for a dangling one.
    pub repo_tags: Vec<String>,
    /// Import/build instant (Unix seconds) — what the list sorts newest-first by.
    pub created_unix: u64,
    /// On-disk size: the sum of the image's layer blobs in the CAS, `0` when no
    /// layer blob is present (unmeasurable), mirroring the CLI's `-`.
    pub size_bytes: i64,
}

/// The OCI image store use cases the node API reads and writes through (R3d-g).
/// Object-safe (`dyn ImageRepo`): every method names only shared types —
/// [`StoredImage`], the shared [`Error`] and `str` — never a `delonix-oci` type,
/// so the implementor lives in the composition and the interface stays clean of
/// the adapter.
pub trait ImageRepo: Send + Sync {
    /// Every stored image under `root`, in no particular order (the caller sorts
    /// newest-first), each with its on-disk size already summed.
    fn list(&self, root: &Path) -> Result<Vec<StoredImage>, Error>;
    /// Remove a reference from the store under `root`: an untag when other tags
    /// remain, the image (blobs) when its last tag goes.
    fn remove(&self, root: &Path, reference: &str) -> Result<(), Error>;
    /// Strip a digest's `sha256:` algorithm prefix, so a reference given with or
    /// without it matches the stored id either way. A pure rule the production
    /// port delegates to the adapter's own `cas::strip`.
    fn strip_digest<'a>(&self, digest: &'a str) -> &'a str;
}

/// An [`ImageRepo`] with no composed store — the empty backing [`crate::router`]
/// hands the server when no engine is injected. Reads are empty, the remove is
/// accepted and discarded; the digest rule answers the safe stripping. The
/// production server never uses it — a bin hands in the `delonix-oci`-backed
/// port — exactly as [`crate::store::NoStore`] is to [`crate::store::StorePort`].
/// The image reads/removes are not exercised through the default router (its
/// tests drive node/provider reads and refusals).
pub(crate) struct NoImages;

impl ImageRepo for NoImages {
    fn list(&self, _root: &Path) -> Result<Vec<StoredImage>, Error> {
        Ok(Vec::new())
    }
    fn remove(&self, _root: &Path, _reference: &str) -> Result<(), Error> {
        Ok(())
    }
    fn strip_digest<'a>(&self, digest: &'a str) -> &'a str {
        // The one `delonix_oci::cas::strip` rule, stated here because this
        // backing names no adapter. The production `delonix-oci`-backed port
        // returns the adapter's own function.
        digest.strip_prefix("sha256:").unwrap_or(digest)
    }
}

// The real `delonix-oci`-backed implementor lives in the composition
// (`delonix-node-api-bin`'s `OciImages`) and, for the integration tests, in the
// harness's `tests/common::AdapterImages` over the `delonix-oci` dev-dependency.
// The node-api crate names no OCI adapter — not even here.

#[cfg(test)]
mod tests {
    use super::*;

    /// A canned [`ImageRepo`] over a fixed set of stored images — no OCI store,
    /// so the read half's ordering, per-tag expansion and digest matching are
    /// covered in-crate without naming `delonix-oci`.
    struct FakeImages(Vec<StoredImage>);
    impl ImageRepo for FakeImages {
        fn list(&self, _root: &Path) -> Result<Vec<StoredImage>, Error> {
            Ok(self.0.clone())
        }
        fn remove(&self, _root: &Path, _reference: &str) -> Result<(), Error> {
            Ok(())
        }
        fn strip_digest<'a>(&self, digest: &'a str) -> &'a str {
            digest.strip_prefix("sha256:").unwrap_or(digest)
        }
    }

    fn stored(id: &str, tags: &[&str], created: u64, size: i64) -> StoredImage {
        StoredImage {
            id: id.to_string(),
            repo_tags: tags.iter().map(|t| t.to_string()).collect(),
            created_unix: created,
            size_bytes: size,
        }
    }

    use crate::proto::v1::{GetImageRequest, ListImagesRequest};
    use std::path::Path as P;

    fn fake() -> FakeImages {
        FakeImages(vec![
            stored("sha256:aaa", &["app:1", "app:latest"], 100, 10),
            stored("sha256:bbb", &[], 300, 0),
            stored("sha256:ccc", &["web:1"], 200, 20),
        ])
    }

    #[test]
    fn list_is_newest_first_with_one_entry_per_tag() {
        let req = ListImagesRequest::default();
        let resp = crate::images::list_in(P::new("/"), &fake(), &req).unwrap();
        // bbb (300, dangling) → one empty reference; ccc (200) → web:1;
        // aaa (100) → both of its tags.
        let refs: Vec<_> = resp.images.iter().map(|i| i.reference.as_str()).collect();
        assert_eq!(refs, vec!["", "web:1", "app:1", "app:latest"]);
        // The dangling image reports its id as the digest, size carried through.
        assert_eq!(resp.images[0].digest, "sha256:bbb");
        assert_eq!(resp.images[1].size_bytes, 20);
    }

    #[test]
    fn list_reference_prefix_filters() {
        let req = ListImagesRequest {
            reference_prefix: "app".into(),
            ..Default::default()
        };
        let resp = crate::images::list_in(P::new("/"), &fake(), &req).unwrap();
        let refs: Vec<_> = resp.images.iter().map(|i| i.reference.as_str()).collect();
        assert_eq!(refs, vec!["app:1", "app:latest"]);
    }

    #[test]
    fn get_resolves_by_tag_id_and_stripped_digest() {
        let get = |r: &str| {
            crate::images::get_in(
                P::new("/"),
                &fake(),
                &GetImageRequest {
                    reference: r.into(),
                },
            )
        };
        // By tag: the asked-for tag comes back as the reference.
        assert_eq!(get("app:latest").unwrap().reference, "app:latest");
        // By full id: the first tag is reported (not the id).
        assert_eq!(get("sha256:aaa").unwrap().reference, "app:1");
        // By id with the algorithm prefix stripped: same image.
        assert_eq!(get("ccc").unwrap().reference, "web:1");
        // A dangling image matched by id reports an empty reference.
        assert_eq!(get("sha256:bbb").unwrap().reference, "");
        // Unknown and empty references are refused.
        assert_eq!(get("nope").unwrap_err().code(), tonic::Code::NotFound);
        assert_eq!(get("").unwrap_err().code(), tonic::Code::InvalidArgument);
    }
}
