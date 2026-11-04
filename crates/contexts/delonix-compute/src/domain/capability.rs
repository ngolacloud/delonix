//! The capability model moved to the foundation (`delonix_model::provider::capability`,
//! ADR-0040 R2 / D3) so any context can name it without depending on compute.
//! Re-exported here unchanged: `delonix_compute::capability::*` still resolves,
//! so compute's own use cases and every existing caller keep their paths.
pub use delonix_model::provider::capability::*;
