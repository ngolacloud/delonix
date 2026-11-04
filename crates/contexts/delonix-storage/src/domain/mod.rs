//! The storage context's standalone pure modules (ADR-0040 A4): the error types,
//! the command-exec contract (`Run`/`Output`) and the helper remedy string — no
//! I/O. The pool model and the allowlist/ownership value types still sit with
//! their I/O use cases in `super::app` for now (the `pool` model depends on the
//! `allowlist`/`ownership` modules, which read files and the password database);
//! R4 lifts those value types here as the use cases that own them are extracted.

pub mod error;
pub mod exec;
pub mod helper;
