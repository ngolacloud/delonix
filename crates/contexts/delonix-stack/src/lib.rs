//! The Stack context (`core.delonix.io`): the table of Kinds, the reconciler that
//! plans a manifest against what exists, and the revision history of an apply
//! (ADR-0040, contexts layer). Planning is pure; nothing here opens a store of a
//! concrete resource or runs a command.

pub mod app;
pub mod domain;

// The R2 domain/app split keeps the public API and the internal `crate::<module>`
// paths: every module is re-exported at the crate root. `domain` is the Kinds
// and the condition vocabulary; `app` is the reconciler and the revision store.
pub use app::{reconcile, revision};
pub use domain::{condition, kinds};
