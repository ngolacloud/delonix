//! The compute context's use cases and the mechanism they drive (ADR-0040 A4):
//! the container run use case (`run`), the VM lifecycle (`vm`), the read ports
//! the use cases resolve their inputs through (`ports`), the by-name backend
//! registry (`vm_registry`), the record store, the cgroup leaf, the host
//! capability probe, the per-container network wiring and the owner bookkeeping.
//! These read the host, the state root and `/sys`, or compose over `super::domain`.

pub mod capability_host;
pub mod cgroup_leaf;
pub mod network;
pub mod owners;
pub mod ports;
pub mod record;
pub mod run;
pub mod vm;
pub mod vm_registry;
