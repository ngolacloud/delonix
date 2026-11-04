//! The storage context (`storage.delonix.io`, ADR-0040 D2.2; ADR-0067): the
//! port a storage-pool driver implements, the registry of drivers by name,
//! the administrator's allowlist and the owner stamp a pool volume carries.
//!
//! In the shape `delonix-networking` has had since ADR-0059 D7: the drivers
//! depend on this context, never on each other, and the composition root
//! registers them. The `dir` driver lives in `delonix-volume`; the block
//! drivers (btrfs, ZFS, LVM-thin) arrive as provider crates, one per backend.
//!
//! What the port carries today is what has a caller (ADR-0067 P0): probe,
//! allocate, release and usage. Resize, snapshot, rollback and clone enter
//! with the first driver that serves them, not before.

pub mod app;
pub mod domain;

// The R2 domain/app split keeps the public API and the internal `crate::<module>`
// paths: every module is re-exported at the crate root. `domain` holds the
// standalone pure modules (error, exec, helper); `app` holds the pool model and
// the I/O use cases.
pub use app::{admin, allowlist, mounts, ownership, pool, registry};
pub use domain::{error, exec, helper};

pub use error::{Error, Result};
