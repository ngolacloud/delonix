//! The config MODEL: what a `kind: Config` is. The ConfigMap-shaped sibling of
//! [`crate::secret::Secret`] — the SAME key→value shape, but non-secret data:
//! it is stored in CLEARTEXT, never encrypted (that is the whole point against a
//! Secret). Pure — the plaintext store that holds configs is `delonix-state`.
//!
//! A config name and a config key follow the same rules a secret's do, so the
//! validators are shared from [`crate::secret`] rather than copied (a second
//! copy that could drift is the exact defect `kinds.rs` removed for the FACTS).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A named config: a set of `KEY=value` pairs, consumed by a workload as
/// environment variables or files — like a Secret, but **cleartext** on disk.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Config {
    /// Config name (reference used by `run --config <name>`).
    pub name: String,
    /// key→value pairs. The keys are environment variable names.
    #[serde(default)]
    pub data: BTreeMap<String, String>,
    /// Creation/update instant (Unix seconds).
    #[serde(default)]
    pub updated_unix: u64,
    /// Revision counter: 1 when first stored, +1 each time the VALUES change.
    /// Assigned by the store on save, never by the caller; `0` is a record
    /// written before versions existed (read as version 1). Same discipline as
    /// [`crate::secret::Secret::version`].
    #[serde(default)]
    pub version: u32,
}

impl Config {
    /// The pairs in `KEY=value` format for injection as env.
    pub fn env_pairs(&self) -> Vec<String> {
        self.data.iter().map(|(k, v)| format!("{k}={v}")).collect()
    }
}
