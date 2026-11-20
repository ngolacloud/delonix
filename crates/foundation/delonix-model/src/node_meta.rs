//! The node record (ADR-0101, A6): the node's operator-declared scheduling state —
//! placement labels, taints, the cordon flag and an optional overcommit ratio — in the
//! one crate BOTH the CLI reconciler (`delonix-stack`, through the `kind: Node` apply)
//! and the node API (`delonix-node-api`, through the `SetNode*` RPCs) depend on. The two
//! writers therefore share one type, one on-disk path ([`record_path`]) and one version
//! rule (reusing [`crate::versioning`], the same reason that module lives here).
//!
//! Pure — no I/O. Each writer reads and writes the JSON itself over [`NodeRecord`]; this
//! module owns the *shape*, the *path* and the *stamp*, so neither writer can drift from
//! the other. Node metadata is **single-phase**: the write both sets and realizes the
//! desired spec (there is no reconcile loop behind it), so the stamp sets
//! `observedGeneration == generation` — the CLI happy path of ADR-0090, not the
//! two-phase node-API split of ADR-0099.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::versioning;

/// The `apiVersion` the node record is stored under — the `kind: Node` group (ADR-0086
/// D7). It stays `v1beta1`; A6 promotes `Node` from Reserved to live but not to `v1`.
pub const API_VERSION: &str = "core.delonix.io/v1beta1";

/// The closed set of taint effects a control plane and the node agree on.
pub const TAINT_EFFECTS: [&str; 3] = ["NoSchedule", "PreferNoSchedule", "NoExecute"];

/// A taint repels workloads that do not tolerate it. Advertised by the node and honoured
/// by the control-plane scheduler — the node does NOT enforce tolerations (ADR-0101 §3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Taint {
    pub key: String,
    #[serde(default)]
    pub value: String,
    pub effect: String,
}

/// Operator-declared CPU/memory overcommit ratios (ADR-0101 §5). A ratio of `2.0` means
/// the control plane may place twice the measured allocatable. The engine has no
/// node-level overcommit of its own, so this is purely the operator's declaration —
/// absent (`None`) leaves it `unmeasured` in `Capacity`, never a silent `1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Overcommit {
    pub cpu: f64,
    pub memory: f64,
}

/// The node record's `spec`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeSpec {
    pub taints: Vec<Taint>,
    /// The cordon flag. `true` = schedulable (the default for a fresh node); `false` =
    /// cordoned, and the node refuses a new workload create (ADR-0101 §3).
    pub schedulable: bool,
    pub unschedulable_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overcommit: Option<Overcommit>,
}

impl Default for NodeSpec {
    /// A fresh node is **schedulable**, unlabelled and untainted — not a cordoned one.
    fn default() -> Self {
        Self {
            taints: Vec::new(),
            schedulable: true,
            unschedulable_reason: String::new(),
            overcommit: None,
        }
    }
}

/// The record metadata: placement labels (k8s-aligned — node labels are metadata, not
/// spec) and the `delonix.io/*` version annotations [`crate::versioning`] reads and
/// advances.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeMetadata {
    /// The record's name — the node's identity for the reconcile pairing of a
    /// `kind: Node` apply. Empty on a record first written by a `node/v1` RPC; the
    /// declarative apply sets it. Cosmetic for the node API (which reports the live
    /// hostname), it only keys the CLI reconciler.
    pub name: String,
    /// Placement labels. The reconciler's own bookkeeping labels (`delonix.io/*`) may
    /// also live here; they are filtered from the user-facing surface and from the
    /// version counter (both below).
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
}

/// The reconciler's own label/annotation prefix. Keys under it are bookkeeping
/// (`delonix.io/stack`, `.../managed-by`, `.../generation`…), never user data.
pub const RESERVED_PREFIX: &str = "delonix.io/";

/// The user-facing placement labels: the record's labels minus the reconciler's own
/// `delonix.io/*` bookkeeping. What `GetNodeInfo` returns and what the version counter
/// hashes — so stamping ownership never looks like a relabel.
pub fn user_labels(labels: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    labels
        .iter()
        .filter(|(k, _)| !k.starts_with(RESERVED_PREFIX))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// The persisted node record. Absent on disk ⇒ [`NodeRecord::default`]: schedulable,
/// unlabelled, unversioned — a fresh node, not an error.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NodeRecord {
    pub metadata: NodeMetadata,
    pub spec: NodeSpec,
}

/// The node record's path under a delonix state root — the one path BOTH writers use.
/// A pure join (no I/O): `<root>/node/node.json`.
pub fn record_path(root: &Path) -> PathBuf {
    root.join("node").join("node.json")
}

impl NodeRecord {
    /// The field map that feeds the version counter (ADR-0090 `last-applied`): every
    /// dimension a `generation` bump is about. Labels are metadata, but a control plane
    /// treats a relabel as a desired change, so they count too. Stable (`BTreeMap`), so
    /// the same state always encodes the same string.
    pub fn version_fields(&self) -> BTreeMap<String, String> {
        let mut f = BTreeMap::new();
        f.insert("schedulable".to_string(), self.spec.schedulable.to_string());
        if !self.spec.unschedulable_reason.is_empty() {
            f.insert(
                "unschedulableReason".to_string(),
                self.spec.unschedulable_reason.clone(),
            );
        }
        if !self.spec.taints.is_empty() {
            f.insert(
                "taints".to_string(),
                serde_json::to_string(&self.spec.taints).unwrap_or_default(),
            );
        }
        if let Some(oc) = &self.spec.overcommit {
            f.insert(
                "overcommit".to_string(),
                format!("{}:{}", oc.cpu, oc.memory),
            );
        }
        let labels = user_labels(&self.metadata.labels);
        if !labels.is_empty() {
            f.insert(
                "labels".to_string(),
                serde_json::to_string(&labels).unwrap_or_default(),
            );
        }
        f
    }

    /// Stamp the record's version annotations for the state it now holds (ADR-0090,
    /// single-phase: `observedGeneration == generation`). Call AFTER mutating the record
    /// and BEFORE writing it: the counter bumps only when the applied state changed vs
    /// the one already stamped (the [`versioning::next_generation`] rule), so an idempotent
    /// write keeps the generation.
    pub fn stamp(&mut self) {
        let fields = self.version_fields();
        let encoded = versioning::encode_last_applied(&fields);
        let ann =
            versioning::version_annotations(&self.metadata.annotations, &encoded, API_VERSION);
        for (k, v) in ann {
            self.metadata.annotations.insert(k, v);
        }
        self.metadata
            .annotations
            .insert(versioning::LAST_APPLIED.to_string(), encoded);
    }

    /// The desired-spec counter off the record's annotations. `0` when never stamped.
    pub fn generation(&self) -> i64 {
        self.annotation_int(versioning::GENERATION)
    }

    /// The generation the node last realized. Single-phase, so it equals
    /// [`NodeRecord::generation`] after any stamp; `0` when never stamped.
    pub fn observed_generation(&self) -> i64 {
        self.annotation_int(versioning::OBSERVED_GENERATION)
    }

    fn annotation_int(&self, key: &str) -> i64 {
        self.metadata
            .annotations
            .get(key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_node_is_schedulable_and_unversioned() {
        let r = NodeRecord::default();
        assert!(r.spec.schedulable, "a fresh node must be schedulable");
        assert_eq!(r.generation(), 0);
        assert_eq!(r.observed_generation(), 0);
        assert!(r.metadata.labels.is_empty());
    }

    #[test]
    fn record_path_is_the_shared_join() {
        let p = record_path(Path::new("/var/lib/delonix"));
        assert!(p.ends_with("node/node.json"));
    }

    #[test]
    fn stamp_bumps_on_change_keeps_on_idempotent_write_single_phase() {
        let mut r = NodeRecord::default();
        r.stamp();
        assert_eq!(r.generation(), 1);
        // Single-phase: observed equals generation right away.
        assert_eq!(r.observed_generation(), 1);

        // An idempotent re-stamp of the same state keeps the generation.
        r.stamp();
        assert_eq!(r.generation(), 1);

        // A cordon is a desired change: generation rises, observed stays equal.
        r.spec.schedulable = false;
        r.spec.unschedulable_reason = "maintenance".to_string();
        r.stamp();
        assert_eq!(r.generation(), 2);
        assert_eq!(r.observed_generation(), 2);

        // Relabel bumps too.
        r.metadata
            .labels
            .insert("zone".to_string(), "lis1".to_string());
        r.stamp();
        assert_eq!(r.generation(), 3);
    }

    #[test]
    fn record_round_trips_through_json() {
        let mut r = NodeRecord::default();
        r.metadata
            .labels
            .insert("gpu".to_string(), "true".to_string());
        r.spec.taints.push(Taint {
            key: "dedicated".to_string(),
            value: "ml".to_string(),
            effect: "NoSchedule".to_string(),
        });
        r.spec.overcommit = Some(Overcommit {
            cpu: 2.0,
            memory: 1.5,
        });
        r.stamp();
        let raw = serde_json::to_string(&r).unwrap();
        let back: NodeRecord = serde_json::from_str(&raw).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn absent_fields_deserialize_to_schedulable_default() {
        // A record written before a field existed, or a minimal hand-made one.
        let back: NodeRecord = serde_json::from_str("{}").unwrap();
        assert!(back.spec.schedulable);
        assert!(back.spec.taints.is_empty());
        assert!(back.spec.overcommit.is_none());
    }
}
