//! `kind: Node` — the node's declarative scheduling config (ADR-0101, A6).
//!
//! Placement labels live in `metadata.labels` (k8s-style); taints, schedulability
//! (the cordon flag) and an operator overcommit ratio live in `spec`. The apply writes
//! the SAME `delonix-model::node_meta` record, at the SAME path, that the `node/v1`
//! `SetNode*` RPCs write — so the declarative and the imperative surface share one state
//! (the reason the record type lives in the foundation crate, not here).
//!
//! The apply is a **partial merge**: a dimension the manifest declares replaces what is
//! stored; one it omits is left untouched. So a `kind: Node` that only sets labels does
//! not uncordon a node cordoned over `node/v1`, and vice versa — declarative apply is the
//! source of truth for what it declares and silent about what it omits.

use std::collections::BTreeMap;
use std::path::PathBuf;

use delonix_model::node_meta::{self, NodeRecord, Overcommit, Taint};
use delonix_model::{Error, Result};
use serde::{Deserialize, Serialize};

use super::kinds as k;
use super::manifest::{self, ManifestDoc};

/// `spec` of `kind: Node`. Every field is optional: an omitted field leaves the stored
/// value untouched (partial merge), so declaring one dimension never resets another.
#[derive(Debug, Clone, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeSpec {
    /// The complete taint set, when present — a full replace. Omit the key to leave the
    /// node's taints as they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taints: Option<Vec<TaintSpec>>,
    /// Whether the node accepts new workloads. `false` cordons it (the node then refuses
    /// a new workload create — ADR-0101 §3). Omit to leave schedulability unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedulable: Option<bool>,
    /// A note recorded with a cordon; ignored when `schedulable` is not `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unschedulable_reason: Option<String>,
    /// Operator-declared CPU/memory overcommit ratios. Omit to leave them unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overcommit: Option<OvercommitSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TaintSpec {
    pub key: String,
    #[serde(default)]
    pub value: String,
    /// `NoSchedule`, `PreferNoSchedule` or `NoExecute`.
    pub effect: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OvercommitSpec {
    pub cpu: f64,
    pub memory: f64,
}

/// The spec fields `kind: Node` understands. Shared by the schema and the compared-field
/// table (`stack.rs`), so the two cannot drift. `labels` is metadata, not spec.
pub const NODE_SPEC_FIELDS: &[&str] =
    &["taints", "schedulable", "unschedulableReason", "overcommit"];
/// Every field of a Node converges in place (the record is rewritten, nothing recreated).
pub const RECONCILED_NODE_FIELDS: &[&str] = NODE_SPEC_FIELDS;

fn dir() -> PathBuf {
    super::util::state_root().join("node")
}

fn path() -> PathBuf {
    node_meta::record_path(&super::util::state_root())
}

/// Reads the shared node record. Absent ⇒ default (schedulable); damaged ⇒ error.
fn load() -> Result<NodeRecord> {
    match std::fs::read(path()) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| Error::Invalid(format!("node record is damaged ({e})"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(NodeRecord::default()),
        Err(e) => Err(Error::Invalid(format!("node record: {e}"))),
    }
}

fn write(record: &NodeRecord) -> Result<()> {
    std::fs::create_dir_all(dir()).map_err(|e| Error::Invalid(format!("node: {e}")))?;
    let json = serde_json::to_vec_pretty(record).map_err(|e| Error::Invalid(e.to_string()))?;
    delonix_state::write_atomic(&path(), &json).map_err(|e| Error::Invalid(format!("node: {e}")))
}

fn validate(spec: &NodeSpec) -> Result<()> {
    if let Some(taints) = &spec.taints {
        for t in taints {
            if t.key.trim().is_empty() {
                return Err(Error::Invalid("Node: a taint's `key` is required".into()));
            }
            if !node_meta::TAINT_EFFECTS.contains(&t.effect.as_str()) {
                return Err(Error::Invalid(format!(
                    "Node: taint '{}': effect '{}' is not one of NoSchedule, PreferNoSchedule, \
                     NoExecute",
                    t.key, t.effect
                )));
            }
        }
    }
    Ok(())
}

/// Merge the manifest onto the stored record: declared dimensions replace, omitted ones
/// stay. Placement labels (`metadata.labels`) are replaced wholesale, preserving the
/// reconciler's own `delonix.io/*` bookkeeping labels.
fn merged(doc: &ManifestDoc, spec: &NodeSpec, mut record: NodeRecord) -> NodeRecord {
    record.metadata.name = doc.metadata.name.clone();

    // User placement labels = the manifest's metadata.labels (minus any reserved key a
    // caller wrote by mistake); keep the record's own reconciler bookkeeping.
    let reserved: BTreeMap<String, String> = record
        .metadata
        .labels
        .iter()
        .filter(|(key, _)| key.starts_with(node_meta::RESERVED_PREFIX))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut labels = node_meta::user_labels(&doc.metadata.labels);
    labels.extend(reserved);
    record.metadata.labels = labels;

    if let Some(taints) = &spec.taints {
        record.spec.taints = taints
            .iter()
            .map(|t| Taint {
                key: t.key.clone(),
                value: t.value.clone(),
                effect: t.effect.clone(),
            })
            .collect();
    }
    if let Some(schedulable) = spec.schedulable {
        record.spec.schedulable = schedulable;
        record.spec.unschedulable_reason = if schedulable {
            String::new()
        } else {
            spec.unschedulable_reason
                .clone()
                .unwrap_or(record.spec.unschedulable_reason)
        };
    } else if let Some(reason) = &spec.unschedulable_reason {
        record.spec.unschedulable_reason = reason.clone();
    }
    if let Some(oc) = spec.overcommit {
        record.spec.overcommit = Some(Overcommit {
            cpu: oc.cpu,
            memory: oc.memory,
        });
    }
    record
}

pub(crate) fn desired(doc: &ManifestDoc) -> Result<super::reconcile::Desired> {
    let spec: NodeSpec = manifest::spec_of(doc)?;
    validate(&spec)?;
    let record = merged(doc, &spec, load()?);
    Ok(super::reconcile::Desired {
        kind: k::NODE.into(),
        name: doc.metadata.name.clone(),
        fields: record.version_fields(),
        converges: true,
        // A node is a singleton, not a stack-owned resource (like an Image): never
        // adopted, never a prune candidate. Its own config, not something a stack owns.
        ownable: false,
    })
}

pub(crate) fn actual() -> Result<Vec<super::reconcile::Actual>> {
    let record = load()?;
    // Never written (fresh node) ⇒ nothing to reconcile against; the first apply creates.
    if record == NodeRecord::default() {
        return Ok(vec![]);
    }
    Ok(vec![super::reconcile::Actual {
        kind: k::NODE.into(),
        name: record.metadata.name.clone(),
        fields: record.version_fields(),
        owner: record
            .metadata
            .labels
            .get(super::reconcile::STACK_LABEL)
            .cloned(),
        last_applied: record
            .metadata
            .annotations
            .get(super::reconcile::LAST_APPLIED)
            .and_then(|raw| super::reconcile::decode_last_applied(raw)),
        protect: record
            .metadata
            .labels
            .contains_key(super::reconcile::PROTECT),
    }])
}

fn apply_one(doc: &ManifestDoc) -> Result<()> {
    let spec: NodeSpec = manifest::spec_of(doc)?;
    validate(&spec)?;
    let mut record = merged(doc, &spec, load()?);
    record.stamp();
    write(&record)?;
    println!(
        "{}",
        super::po::tf(
            "node/{name}: scheduling config applied (schedulable={schedulable})",
            &[
                ("name", &doc.metadata.name),
                ("schedulable", &record.spec.schedulable.to_string()),
            ],
        )
    );
    Ok(())
}

pub fn apply(docs: &[ManifestDoc]) -> Result<()> {
    for doc in manifest::of_kind(docs, k::NODE) {
        apply_one(doc)?;
    }
    Ok(())
}

pub(crate) fn converge_doc(doc: &ManifestDoc) -> Result<()> {
    apply_one(doc)
}

/// Whether the node record exists, for `stack ls`. A node is a singleton that always
/// exists physically; this reports whether its declarative config has been written.
pub(crate) fn presence_of(_doc: &ManifestDoc) -> (String, String) {
    match load() {
        Ok(record) if record != NodeRecord::default() => (
            "yes".into(),
            super::po::t("scheduling config present").into(),
        ),
        _ => ("no".into(), "-".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::manifest::Metadata;
    use super::*;

    fn doc(name: &str, labels: &[(&str, &str)]) -> ManifestDoc {
        ManifestDoc {
            api_version: node_meta::API_VERSION.into(),
            kind: k::NODE.into(),
            metadata: Metadata {
                name: name.into(),
                namespace: None,
                labels: labels
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                annotations: Default::default(),
            },
            spec: serde_yaml::Value::Null,
        }
    }

    #[test]
    fn partial_merge_preserves_omitted_dimensions() {
        // A node cordoned (e.g. over node/v1).
        let mut base = NodeRecord::default();
        base.spec.schedulable = false;
        base.spec.unschedulable_reason = "maintenance".into();

        // A manifest that declares only labels must NOT uncordon it.
        let only_labels = NodeSpec::default();
        let out = merged(&doc("n", &[("zone", "lis1")]), &only_labels, base.clone());
        assert!(
            !out.spec.schedulable,
            "declaring only labels must not uncordon"
        );
        assert_eq!(out.spec.unschedulable_reason, "maintenance");
        assert_eq!(
            out.metadata.labels.get("zone").map(String::as_str),
            Some("lis1")
        );

        // A manifest that declares schedulable:true uncordons and clears the reason.
        let uncordon = NodeSpec {
            schedulable: Some(true),
            ..Default::default()
        };
        let out = merged(&doc("n", &[]), &uncordon, base);
        assert!(out.spec.schedulable);
        assert_eq!(out.spec.unschedulable_reason, "");
    }

    #[test]
    fn merge_replaces_user_labels_but_keeps_reconciler_bookkeeping() {
        let mut base = NodeRecord::default();
        base.metadata
            .labels
            .insert("delonix.io/stack".into(), "s".into());
        base.metadata.labels.insert("old".into(), "x".into());
        let out = merged(&doc("n", &[("zone", "lis1")]), &NodeSpec::default(), base);
        assert_eq!(
            out.metadata.labels.get("zone").map(String::as_str),
            Some("lis1")
        );
        assert!(
            !out.metadata.labels.contains_key("old"),
            "user labels are replaced"
        );
        assert_eq!(
            out.metadata
                .labels
                .get("delonix.io/stack")
                .map(String::as_str),
            Some("s"),
            "reconciler bookkeeping is preserved"
        );
    }

    #[test]
    fn validate_rejects_an_unknown_taint_effect() {
        let spec = NodeSpec {
            taints: Some(vec![TaintSpec {
                key: "k".into(),
                value: String::new(),
                effect: "Nope".into(),
            }]),
            ..Default::default()
        };
        assert!(validate(&spec).is_err());
    }
}
