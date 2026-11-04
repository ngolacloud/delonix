//! The stack context's use cases (ADR-0040 A4): the reconciler that turns a
//! manifest into a plan over the Kinds (`reconcile`, pure over `super::domain`),
//! and the revision store that records what was applied (`revision`, which reads
//! and writes the revision files).

pub mod reconcile;
pub mod revision;
