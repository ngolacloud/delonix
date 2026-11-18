//! The compute context's `WorkloadImages` port, backed by this crate's store.
//!
//! What the terminal owns stays with the caller as hooks: the line printed before
//! a pull, the progress shown while an image is unpacked, and the wording of a
//! `--user` the image does not have.

use crate::rootfs_user::{resolve_user, UserLookupError};
use crate::{Image, ImageStore};
use delonix_compute::ports::ImageConfig;
use delonix_model::{Error, Result};
use std::path::{Path, PathBuf};

/// Unpacks a rootfs, showing whatever progress the caller wants around it.
pub type UnpackFn<'a> = dyn Fn(&mut dyn FnMut() -> Result<PathBuf>) -> Result<PathBuf> + 'a;

pub struct HostImages<'a> {
    pub store: &'a ImageStore,
    /// Called with the reference right before a pull starts.
    pub announce_pull: &'a dyn Fn(&str),
    /// Per-layer pull progress (`registry::PullEvent`), when the caller has a
    /// terminal to draw it on — the CLI's `docker pull`-style block. `None`
    /// for headless callers (node API, tests): the pull itself is identical.
    pub pull_events: Option<crate::registry::PullEventCb<'a>>,
    /// Wraps the unpacking of a new rootfs (e.g. in a progress line).
    pub unpacking: &'a UnpackFn<'a>,
    /// Words a `--user` the image cannot satisfy as an error.
    pub user_error: &'a dyn Fn(UserLookupError) -> Error,
}

impl delonix_compute::ports::WorkloadImages for HostImages<'_> {
    type Image = Image;

    fn resolve(&self, reference: &str) -> Result<Image> {
        // `registry::resolve_or_pull`'s local-first logic, with the per-layer
        // events the terminal hook wants threaded through on the miss path.
        if let Ok(img) = self.store.resolve(reference) {
            return Ok(img);
        }
        (self.announce_pull)(reference);
        Ok(crate::registry::pull_from_registry_with_events(
            self.store,
            reference,
            None,
            None,
            self.pull_events,
        )?)
    }

    fn config(&self, img: &Image) -> ImageConfig {
        ImageConfig {
            entrypoint: img.config.entrypoint.clone(),
            cmd: img.config.cmd.clone(),
            env: img.config.env.clone(),
            working_dir: img.config.working_dir.clone(),
            user: img.config.user.clone(),
            exposed_ports: img.config.exposed_ports.clone(),
        }
    }

    fn prepare_rootfs(
        &self,
        img: &Image,
        id: &str,
        second_pass: bool,
        ephemeral: bool,
    ) -> Result<String> {
        // The re-exec's second pass reuses the rootfs the first pass prepared: a
        // full extraction again over a populated tree costs full price (measured).
        let prepare = || Ok(self.store.prepare_container_rootfs(img, id, ephemeral)?);
        let path = if second_pass && delonix_node::is_rootless() {
            match self.store.existing_rootfs_path(id) {
                Some(p) => p,
                None => prepare()?,
            }
        } else {
            (self.unpacking)(&mut || prepare())?
        };
        Ok(path.to_string_lossy().into_owned())
    }

    fn resolve_user(&self, rootfs: &str, spec: &str) -> Result<(u32, Option<u32>)> {
        resolve_user(Path::new(rootfs), spec).map_err(self.user_error)
    }
}
