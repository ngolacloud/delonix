//! ADR-0099: stamp the `generation`/`observedGeneration` record metadata on the
//! node-API mutation path, and read it back onto the contract's `ResourceMeta`.
//!
//! The counter rule and annotation keys come from [`delonix_model::versioning`]
//! — the exact ones the CLI reconciler stamps with — so the two writers cannot
//! drift (ADR-0099 §1). This module is only the node-API glue: a two-phase
//! stamp (desired at accept, observed on a successful reconcile) and the typed
//! read that keeps the engine's own `delonix.io/*` annotations off the wire.

use std::collections::BTreeMap;
use std::path::Path;

use delonix_model::versioning as v;

/// The stored apiVersion of the `Container` Kind (`kinds::facts(CONTAINER)` in
/// the CLI, which this interface crate deliberately does not depend on — it
/// carries the literal instead, ADR-0099 §1).
pub const CONTAINER_API_VERSION: &str = "compute.delonix.io/v1beta1";

/// Event action marking that a new desired spec was accepted — the desired stamp
/// just bumped `generation` (ADR-0100). At this instant the record reads
/// `generation > observedGeneration`: asked for, not yet realized.
pub const ACTION_APPLIED: &str = "applied";
/// Event action marking that the node reconciled to the desired spec — the
/// observed advance just set `observedGeneration == generation` (ADR-0100).
pub const ACTION_CONVERGED: &str = "converged";

/// Emit the ADR-0100 `applied` version event after a desired stamp, so a desired
/// change the control plane asked for is VISIBLE on `WatchEvents` (the mutation
/// path emits nothing else). Best-effort, like every `events::emit`.
pub fn emit_applied(root: &Path, kind: &str, id: &str, name: &str) {
    delonix_node::events::emit(root, kind, ACTION_APPLIED, id, name, None);
}

/// Emit the ADR-0100 `converged` version event after a successful observed
/// advance, so the control plane sees on the stream that the node caught up.
pub fn emit_converged(root: &Path, kind: &str, id: &str, name: &str) {
    delonix_node::events::emit(root, kind, ACTION_CONVERGED, id, name, None);
}

/// Stamp the DESIRED generation onto a record's annotations BEFORE it is
/// persisted (ADR-0099): bump `generation` for `fields` by the ADR-0090 rule,
/// write `storedVersion` and `last-applied`, and LEAVE `observedGeneration` at
/// its prior value — so the record reads `generation > observedGeneration` until
/// a successful reconcile advances it. `api_version` is the Kind's stored version.
pub fn stamp_desired(
    annotations: &mut BTreeMap<String, String>,
    fields: &BTreeMap<String, String>,
    api_version: &str,
) {
    for (key, val) in v::desired_annotations(annotations, fields, api_version) {
        annotations.insert(key, val);
    }
}

/// Advance `observedGeneration` to the record's current `generation` after a
/// successful reconcile (ADR-0099). A no-op on a record the desired stamp never
/// touched (`generation` absent ⇒ `observedGeneration` stays 0).
pub fn stamp_observed(annotations: &mut BTreeMap<String, String>) {
    let (key, val) = v::observed_generation_entry(annotations);
    annotations.insert(key, val);
}

/// The ADR-0090/0099 record metadata read off a record's annotations for the
/// contract's typed `ResourceMeta` fields, plus the user-facing annotations with
/// the engine's own `delonix.io/*` keys filtered out — so `generation` rides as
/// a typed int a consumer branches on, and a big `last-applied` blob never rides
/// every read. Absent-safe: an unversioned record reads `0`/`""`.
pub struct ReadVersion {
    pub generation: i64,
    pub observed_generation: i64,
    pub stored_version: String,
    pub annotations: BTreeMap<String, String>,
}

/// Build a [`ReadVersion`] off a record's annotations (ADR-0099). The engine's
/// own `delonix.io/*` keys are the counters and the last-applied fingerprint,
/// kept on the record but off the contract's annotations map.
pub fn read_version(annotations: &BTreeMap<String, String>) -> ReadVersion {
    let as_i64 = |k: &str| {
        annotations
            .get(k)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };
    ReadVersion {
        generation: as_i64(v::GENERATION),
        observed_generation: as_i64(v::OBSERVED_GENERATION),
        stored_version: annotations
            .get(v::STORED_VERSION)
            .cloned()
            .unwrap_or_default(),
        annotations: annotations
            .iter()
            .filter(|(k, _)| !k.starts_with("delonix.io/"))
            .map(|(k, val)| (k.clone(), val.clone()))
            .collect(),
    }
}
