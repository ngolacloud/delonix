//! The stack context's pure model (ADR-0040 A4): the manifest Kinds and their
//! schema (`kinds`) and the condition vocabulary (`condition`). Types and rules,
//! no I/O — the reconcile use case and the revision store that drive them are in
//! `super::app`.

pub mod condition;
pub mod kinds;
