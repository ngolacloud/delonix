//! `delonix-host` — the host adapter (ADR-0040 R2, the `delonix-node` split).
//!
//! The process-and-filesystem mechanism the engine's servers and adapters share:
//! atomic file writes, advisory file locking, the dispatch contract between
//! `delonix` and its sibling servers, and the local-socket peer credential. It is
//! an adapter the other adapters and providers may depend on directly (ADR-0040
//! A1); the host FACTS and the event trail stay in the `delonix-node` context
//! (contexts reach host facts through a port, wired in R4), so this crate carries
//! only the pieces no context touches.

mod atomic;
pub mod dispatch;
mod flock;
pub mod peer_cred;

pub use atomic::{unique_tmp_suffix, write_atomic_mode};
pub use flock::{flock_exclusive, flock_exclusive_until, FLOCK_DEADLINE};
