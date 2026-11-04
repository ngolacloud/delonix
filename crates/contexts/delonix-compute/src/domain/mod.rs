//! The compute context's pure model and rules (ADR-0040 A4): Container/Pod/VM
//! value types and invariants, the VM provider ports (`vm_provider`,
//! `vm_backend` — the contracts the backends implement), the capability
//! re-export, the firewall and error rules, the run specification and the launch
//! plan. No I/O — the use cases that execute these and the host/registry
//! mechanism live in `super::app`.

pub mod capability;
pub mod credentials;
pub mod launch;
pub mod notice;
pub mod pod;
pub mod preflight;
pub mod run_opts;
pub mod system_container;
pub mod vm_backend;
pub mod vm_error;
pub mod vm_firewall;
pub mod vm_provider;
pub mod workload_net;
