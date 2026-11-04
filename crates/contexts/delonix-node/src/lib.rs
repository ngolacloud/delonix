//! `delonix-node` — the node context (ADR-0040 D2.2): the node's event trail, its
//! virtualisation and host checks, and the host/process questions everything else
//! asks. The process-and-filesystem mechanism (atomic writes, flock, the dispatch
//! contract, the peer credential) moved to the `delonix-host` adapter in the R2
//! split; contexts reach the host facts kept here directly for now, and through a
//! port once the compute/security use cases land in R4.

pub mod alloc_tuning;
pub mod events;
pub mod host;
pub mod virt;

pub use host::{
    fmt_local_ts, generate_id, in_initial_userns, initial_uid_map, is_alive, is_rootless,
    mem_available_mib, now_unix, proc_starttime, safe_to_signal, self_bin,
};
