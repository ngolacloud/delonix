//! The node record on disk (ADR-0101, A6) — the node API's read and write of the one
//! shared [`NodeRecord`] type (`delonix-model::node_meta`), at the one shared path. The
//! declarative `kind: Node` apply writes the SAME file over the SAME type from the CLI;
//! this module is only the node API's I/O half, so the two cannot drift on shape or path.

use std::path::Path;

use delonix_model::node_meta::{NodeRecord, Overcommit, Taint};
use delonix_model::Error;

use crate::node::state_root;

/// Reads the node record. Absent ⇒ the default (schedulable, unlabelled, unversioned):
/// a fresh node, not an error. A file that EXISTS but does not parse IS an error — a
/// damaged record read as the default would silently report a cordoned node as
/// schedulable, exactly the fail-open the `unmeasured` doctrine refuses next door.
pub fn load(root: &Path) -> Result<NodeRecord, Error> {
    let path = delonix_model::node_meta::record_path(root);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
            Error::Invalid(format!(
                "node record {} is damaged ({e}) — restore it or remove it by hand",
                path.display()
            ))
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(NodeRecord::default()),
        Err(e) => Err(Error::Invalid(format!("node record: {e}"))),
    }
}

/// Writes the node record atomically, creating `<root>/node/` on demand.
pub fn store(root: &Path, record: &NodeRecord) -> Result<(), Error> {
    let path = delonix_model::node_meta::record_path(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::Invalid(format!("node record: {e}")))?;
    }
    let json = serde_json::to_vec_pretty(record).map_err(|e| Error::Invalid(e.to_string()))?;
    delonix_host::write_atomic_mode(&path, &json, None)
        .map_err(|e| Error::Invalid(format!("node record: {e}")))
}

/// Loads the record from the node's own state root (the no-argument read callers use).
pub fn current() -> Result<NodeRecord, Error> {
    load(&state_root())
}

/// Refuse a new workload create when the node is cordoned (ADR-0101 §3): `DX-5003`,
/// Conflict class → HTTP 409 / gRPC `FAILED_PRECONDITION`. The workload create paths
/// (`CreateContainer`/`CreatePod`/`CreateVirtualMachine`) call this before claiming a
/// name or beginning an operation. An absent record is schedulable — a fresh node
/// accepts work; a damaged record propagates its read error (fail-closed, not fail-open).
pub fn ensure_schedulable(root: &Path) -> Result<(), Error> {
    let record = load(root)?;
    if !record.spec.schedulable {
        let detail = if record.spec.unschedulable_reason.is_empty() {
            String::new()
        } else {
            format!(": {}", record.spec.unschedulable_reason)
        };
        return Err(Error::coded(
            5003,
            Error::Conflict(format!("the node is cordoned (unschedulable){detail}")),
        ));
    }
    Ok(())
}

/// proto `Taint` → model `Taint`.
pub fn taint_from_proto(t: &crate::proto::v1::Taint) -> Taint {
    Taint {
        key: t.key.clone(),
        value: t.value.clone(),
        effect: t.effect.clone(),
    }
}

/// model `Taint` → proto `Taint`.
pub fn taint_to_proto(t: &Taint) -> crate::proto::v1::Taint {
    crate::proto::v1::Taint {
        key: t.key.clone(),
        value: t.value.clone(),
        effect: t.effect.clone(),
    }
}

/// model `Overcommit` → proto `OvercommitRatios`.
pub fn overcommit_to_proto(o: &Overcommit) -> crate::proto::v1::OvercommitRatios {
    crate::proto::v1::OvercommitRatios {
        cpu: o.cpu,
        memory: o.memory,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_record_loads_as_schedulable_default() {
        let dir =
            std::env::temp_dir().join(format!("delonix-node-meta-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let r = load(&dir).unwrap();
        assert!(r.spec.schedulable);
        assert_eq!(r.generation(), 0);
    }

    #[test]
    fn store_then_load_round_trips_and_bumps_generation() {
        let dir = std::env::temp_dir().join(format!("delonix-node-meta-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut r = NodeRecord::default();
        r.spec.schedulable = false;
        r.spec.unschedulable_reason = "drain".to_string();
        r.metadata
            .labels
            .insert("zone".to_string(), "lis1".to_string());
        r.stamp();
        store(&dir, &r).unwrap();
        let back = load(&dir).unwrap();
        assert_eq!(back, r);
        assert!(!back.spec.schedulable);
        assert_eq!(back.generation(), 1);
        assert_eq!(back.observed_generation(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_schedulable_passes_when_schedulable_refuses_when_cordoned() {
        let dir =
            std::env::temp_dir().join(format!("delonix-node-meta-cordon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Absent record ⇒ fresh node ⇒ schedulable ⇒ create is allowed.
        assert!(ensure_schedulable(&dir).is_ok());

        // Cordon it, and a new create is refused with DX-5003.
        let mut r = NodeRecord::default();
        r.spec.schedulable = false;
        r.spec.unschedulable_reason = "maintenance".to_string();
        store(&dir, &r).unwrap();
        let err = ensure_schedulable(&dir).unwrap_err();
        assert_eq!(err.number(), 5003, "a cordoned node refuses with DX-5003");
        assert!(
            err.to_string().contains("maintenance"),
            "the reason is surfaced"
        );

        // Uncordon, and create is allowed again.
        let mut r = NodeRecord::default();
        r.spec.schedulable = true;
        store(&dir, &r).unwrap();
        assert!(ensure_schedulable(&dir).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_record_is_an_error_not_a_silent_default() {
        let dir =
            std::env::temp_dir().join(format!("delonix-node-meta-damaged-{}", std::process::id()));
        let path = delonix_model::node_meta::record_path(&dir);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{ not json").unwrap();
        assert!(
            load(&dir).is_err(),
            "a damaged record must not read as schedulable"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
