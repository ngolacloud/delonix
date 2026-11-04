//! The security context's use cases (ADR-0040 A4): recording a finding on the
//! event log the engine already has. Not pure — it reads the clock and appends
//! through `delonix_node::events`; the decision it records is `super::domain`.

pub mod event;
