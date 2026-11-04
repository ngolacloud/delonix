//! The Compute context (`compute.delonix.io`, ADR-0040): Container, Pod,
//! VirtualMachine and Workload. Today it holds the one run specification that the
//! entry points translate into; the use cases that execute it follow in later P2
//! slices.

pub mod app;
pub mod domain;

// The R2 domain/app split keeps the public API and the internal `crate::<module>`
// paths: every module is re-exported at the crate root. `domain` is the pure
// Container/Pod/VM model, the provider port contracts and the rules; `app` is
// the run/vm use cases, the read ports and the host/registry/record mechanism.
pub use app::{capability_host, cgroup_leaf, network, owners, ports, record, run, vm, vm_registry};
pub use domain::{
    capability, credentials, launch, notice, pod, preflight, run_opts, system_container,
    vm_backend, vm_error, vm_firewall, vm_provider, workload_net,
};

pub use notice::Notice;
pub use record::*;
pub use run_opts::RunOpts;
