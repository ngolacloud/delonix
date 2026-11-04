//! The storage context's use cases and the mechanism they drive (ADR-0040 A4):
//! the pool model and its driver port, the administrator's allowlist (read from
//! the root-owned file), the volume ownership stamp, pool administration, mounts
//! and the by-name driver registry.

pub mod admin;
pub mod allowlist;
pub mod mounts;
pub mod ownership;
pub mod pool;
pub mod registry;
