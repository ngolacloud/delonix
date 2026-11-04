//! The node context's use cases and the mechanism they drive (ADR-0040 A4):
//! host facts, the event trail, virtualisation detection and allocator tuning.
//! Not pure — these read `/proc`, the clock and the event log; the pure value
//! types they produce (`HostCapacity`, `VirtInfo`) live in `super::domain`.

pub mod alloc_tuning;
pub mod events;
pub mod host;
pub mod virt;
