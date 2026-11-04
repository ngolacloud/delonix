//! The security context's pure decisions (ADR-0040 A4): what the node refuses,
//! why, how bad it is. Every item is a function of its arguments — no I/O, no
//! clock, no delonix_node. The one impurity of this crate (appending the
//! finding to the event log) lives in `super::app`.

pub mod admission;
pub mod policy;
pub mod redact;
pub mod score;
pub mod severity;
