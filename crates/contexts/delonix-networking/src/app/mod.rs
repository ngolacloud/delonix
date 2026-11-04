//! The networking context's use cases (ADR-0040 A4). Today: the plan digest
//! (`plan`) — the `If-Match` guard over a network document's computed plan
//! (ADR-0059 D4), the top-level operation consumers call. It is pure (it
//! fingerprints `super::domain` values); R4 brings the reconcile orchestration
//! here as it leaves the CLI.

pub mod plan;
