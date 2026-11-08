//! `kind: Snapshot` — a declarative, convergent point-in-time snapshot of a
//! target `VirtualMachine` or `Volume` (ADR-0086 D3/D5d, ADR-0090).
//!
//! A `Snapshot` ENSURES a named snapshot exists on its target: applying it
//! creates the snapshot if it is missing and does nothing if it is already there
//! (idempotent), and deleting/pruning it removes the snapshot. The target is
//! named by `targetKind` (`VirtualMachine` | `Volume`) + `targetName`; the
//! snapshot's own name defaults to `metadata.name`.
//!
//! **Why a sidecar record.** The snapshot artifact itself — a VM checkpoint in
//! the disk, or a `<vol>/_snapshots/<snap>.tar.gz` — has no room for ownership,
//! the last-applied spec or the protect mark the reconciler stamps. So, exactly
//! as `IPAddress` keeps a small record beside the IPAM lease, a `Snapshot` keeps
//! one of its own (`<root>/snapshot/<name>.json`) for that metadata, while the
//! snapshot itself lives on its target. The record is written only AFTER the
//! snapshot is ensured, so it never names an artifact that was not taken.
//!
//! The two backends it dispatches to are the ones the imperative commands
//! already use: `delonix_vm::{snapshot,snapshots,delete_snapshot}` for a VM
//! (what `vm snapshot` drives, and what `delonix_engine::Engine::snapshot`
//! wraps), and `VolumeStore::{create_snapshot,list_snapshots,remove_snapshot}`
//! for a Volume (what `volume snapshot` drives). The imperative verbs stay, the
//! way `network ipam` stayed for `IPAddress`.
//!
//! **Deferred (ADR-0090 later phase):** retention policy and `Bucket`-destination
//! offload are NOT built here.

use std::collections::BTreeMap;

use super::kinds as k;
use super::manifest::{self, ManifestDoc};
use super::output::OutputFormat;
use delonix_model::{Error, Result};
use delonix_volume::VolumeStore;
use serde::{Deserialize, Serialize};

/// `spec` of `kind: Snapshot`.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SnapshotSpec {
    /// The kind of the resource to snapshot: `VirtualMachine` or `Volume`.
    #[serde(rename = "targetKind")]
    pub target_kind: String,
    /// The name of the target resource. It must already exist (a VM or a Volume,
    /// applied before this), the way an `IPAddress`'s `pool` must.
    #[serde(rename = "targetName")]
    pub target_name: String,
    /// The snapshot's own name. Defaults to `metadata.name`.
    #[serde(rename = "snapshotName", default)]
    pub snapshot_name: Option<String>,
}

pub const SNAPSHOT_SPEC_FIELDS: &[&str] = &["targetKind", "targetName", "snapshotName"];
pub const RECONCILED_SNAPSHOT_FIELDS: &[&str] = &["targetKind", "targetName", "snapshotName"];

/// One snapshot as persisted: the metadata beside the artifact (see the module
/// note on why the artifact alone is not enough). `target_kind` is stored
/// CANONICAL (`VirtualMachine`/`Volume`), so a re-point is detected whatever
/// casing or alias the spec used.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub name: String,
    pub target_kind: String,
    pub target_name: String,
    pub snapshot_name: String,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
}

fn dir() -> std::path::PathBuf {
    super::util::state_root().join("snapshot")
}

fn path_in(d: &std::path::Path, name: &str) -> std::path::PathBuf {
    d.join(format!("{name}.json"))
}

fn get_in(d: &std::path::Path, name: &str) -> Option<SnapshotFile> {
    serde_json::from_slice(&std::fs::read(path_in(d, name)).ok()?).ok()
}

/// Like [`get_in`], but a file that exists and does not parse is an ERROR, not
/// «no such snapshot»: the record names a live artifact, and reading a damaged
/// file as absent lets the next write forget which snapshot this name holds.
fn read_in(d: &std::path::Path, name: &str) -> Result<Option<SnapshotFile>> {
    let path = path_in(d, name);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::Invalid(format!("snapshot {name}: {e}"))),
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|e| {
        Error::Invalid(format!(
            "snapshot {name}: {} is damaged ({e}) — restore it or remove it by hand",
            path.display()
        ))
    })
}

fn list_in(d: &std::path::Path) -> Vec<SnapshotFile> {
    let mut v: Vec<SnapshotFile> = std::fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

fn write_in(d: &std::path::Path, f: &SnapshotFile) -> Result<()> {
    std::fs::create_dir_all(d).map_err(|e| Error::Invalid(format!("snapshot: {e}")))?;
    let json = serde_json::to_vec_pretty(f).map_err(|e| Error::Invalid(e.to_string()))?;
    delonix_state::write_atomic(&path_in(d, &f.name), &json)
        .map_err(|e| Error::Invalid(format!("snapshot: {e}")))
}

pub fn snapshot_get(name: &str) -> Option<SnapshotFile> {
    get_in(&dir(), name)
}

pub fn snapshot_list() -> Vec<SnapshotFile> {
    list_in(&dir())
}

// ---- target resolution ----------------------------------------------------------

/// Which backend a `Snapshot` drives.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Vm,
    Volume,
}

/// Resolves `targetKind` (any casing or alias) to the backend, returning the
/// CANONICAL kind spelling alongside it. A kind that is neither a VM nor a Volume
/// is refused — those are the only two that carry snapshots.
fn resolve_target(target_kind: &str) -> Result<(Target, &'static str)> {
    match manifest::canonical_kind(target_kind) {
        k::VM => Ok((Target::Vm, k::VM)),
        k::VOLUME => Ok((Target::Volume, k::VOLUME)),
        other => Err(Error::Invalid(format!(
            "Snapshot: targetKind '{other}' is not snapshottable (use VirtualMachine or Volume)"
        ))),
    }
}

/// Is the named snapshot already on the target?
fn backend_has(target: Target, target_name: &str, snap: &str) -> Result<bool> {
    match target {
        Target::Vm => {
            let base = super::util::state_root();
            Ok(delonix_vm::snapshots(&base, target_name)?
                .iter()
                .any(|s| s == snap))
        }
        Target::Volume => {
            let store = VolumeStore::open(super::util::state_root())?;
            Ok(store
                .list_snapshots(target_name)?
                .iter()
                .any(|(n, _, _)| n == snap))
        }
    }
}

/// Takes the snapshot on the target. The Volume path reuses the ONE pack
/// implementation on `VolumeStore`, injecting the CLI's mapped-userns re-exec.
fn backend_create(target: Target, target_name: &str, snap: &str) -> Result<()> {
    match target {
        Target::Vm => {
            delonix_vm::snapshot(&super::util::state_root(), target_name, snap)?;
            Ok(())
        }
        Target::Volume => {
            let store = VolumeStore::open(super::util::state_root())?;
            store.create_snapshot(target_name, snap, &|data, tb| {
                super::volume::volsnap_run("create", data, tb)
            })?;
            Ok(())
        }
    }
}

/// Removes a snapshot's artifact. Best-effort: a snapshot already gone (deleted
/// by hand, or a target that no longer exists) is not an error on the remove
/// path — the goal is that it is absent afterwards.
fn backend_remove(prev: &SnapshotFile) -> Result<()> {
    let (target, _) = match resolve_target(&prev.target_kind) {
        Ok(t) => t,
        // A record with an unknown kind: nothing we know how to remove, and
        // failing here would wedge the delete of the record. Drop it quietly.
        Err(_) => return Ok(()),
    };
    match target {
        Target::Vm => {
            let _ = delonix_vm::delete_snapshot(
                &super::util::state_root(),
                &prev.target_name,
                &prev.snapshot_name,
            );
        }
        Target::Volume => {
            if let Ok(store) = VolumeStore::open(super::util::state_root()) {
                let _ = store.remove_snapshot(&prev.target_name, &prev.snapshot_name);
            }
        }
    }
    Ok(())
}

// ---- the Kind -------------------------------------------------------------------

/// The snapshot name a doc resolves to: `spec.snapshotName`, else `metadata.name`.
fn snap_name_of(doc: &ManifestDoc, spec: &SnapshotSpec) -> String {
    spec.snapshot_name
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| doc.metadata.name.clone())
}

fn spec_fields(canonical_kind: &str, target_name: &str, snap: &str) -> BTreeMap<String, String> {
    let mut f = BTreeMap::new();
    f.insert("targetKind".into(), canonical_kind.into());
    f.insert("targetName".into(), target_name.into());
    f.insert("snapshotName".into(), snap.into());
    f
}

pub(crate) fn desired(doc: &ManifestDoc) -> Result<super::reconcile::Desired> {
    let spec: SnapshotSpec = manifest::spec_of(doc)?;
    let (_, canonical) = resolve_target(&spec.target_kind)?;
    let snap = snap_name_of(doc, &spec);
    Ok(super::reconcile::Desired {
        kind: k::SNAPSHOT.into(),
        name: doc.metadata.name.clone(),
        fields: spec_fields(canonical, &spec.target_name, &snap),
        converges: true,
        ownable: true,
    })
}

pub(crate) fn actual() -> Result<Vec<super::reconcile::Actual>> {
    Ok(snapshot_list()
        .into_iter()
        .map(|s| {
            let fields = spec_fields(&s.target_kind, &s.target_name, &s.snapshot_name);
            super::reconcile::Actual {
                kind: k::SNAPSHOT.into(),
                name: s.name.clone(),
                fields,
                owner: s.labels.get(super::reconcile::STACK_LABEL).cloned(),
                last_applied: s
                    .annotations
                    .get(super::reconcile::LAST_APPLIED)
                    .and_then(|raw| super::reconcile::decode_last_applied(raw)),
                protect: s.labels.contains_key(super::reconcile::PROTECT),
            }
        })
        .collect())
}

pub(crate) fn stamp(
    name: &str,
    stack: &str,
    fields: &BTreeMap<String, String>,
    protected: bool,
) -> Result<()> {
    let d = dir();
    let mut s = read_in(&d, name)?.ok_or_else(|| Error::NotFound(format!("snapshot: {name}")))?;
    s.labels
        .insert(super::reconcile::STACK_LABEL.into(), stack.into());
    if protected {
        s.labels
            .insert(super::reconcile::PROTECT.into(), "true".into());
    }
    s.labels
        .insert(super::reconcile::MANAGED_BY.into(), "delonix".into());
    super::reconcile::stamp_annotations(&mut s.annotations, fields, k::SNAPSHOT);
    write_in(&d, &s)
}

/// Removes a snapshot: deletes the artifact on its target, then drops the record.
/// This is the delete/prune path — a `Snapshot` IS the artifact, so taking it
/// down removes the point-in-time snapshot (unlike a `Volume`, which keeps its
/// data until its own removal).
pub(crate) fn remove_for_replace(name: &str) -> Result<()> {
    let d = dir();
    let Some(prev) = read_in(&d, name)? else {
        return Ok(());
    };
    backend_remove(&prev)?;
    let _ = std::fs::remove_file(path_in(&d, name));
    Ok(())
}

fn apply_one(doc: &ManifestDoc) -> Result<()> {
    let spec: SnapshotSpec = manifest::spec_of(doc)?;
    let name = &doc.metadata.name;
    let (target, canonical) = resolve_target(&spec.target_kind)?;
    let snap = snap_name_of(doc, &spec);
    let d = dir();
    let existing = read_in(&d, name)?;
    // A re-pointed Snapshot (now a different target or snapshot name) gives the
    // OLD artifact back before taking the new one — otherwise the previous
    // snapshot would be orphaned under a name nothing tracks.
    if let Some(prev) = &existing {
        if prev.target_kind != canonical
            || prev.target_name != spec.target_name
            || prev.snapshot_name != snap
        {
            backend_remove(prev)?;
        }
    }
    // Ensure-exists: create it only if it is not already on the target. A
    // snapshot taken earlier (by a previous apply, or by `vm snapshot`/`volume
    // snapshot`) of the same name is adopted, not duplicated.
    let already = backend_has(target, &spec.target_name, &snap)?;
    if !already {
        backend_create(target, &spec.target_name, &snap)?;
    }
    write_in(
        &d,
        &SnapshotFile {
            name: name.clone(),
            target_kind: canonical.into(),
            target_name: spec.target_name.clone(),
            snapshot_name: snap.clone(),
            // Preserve any stamp a previous apply wrote; `stamp()` re-writes it.
            labels: existing
                .as_ref()
                .map(|p| p.labels.clone())
                .unwrap_or_default(),
            annotations: existing
                .as_ref()
                .map(|p| p.annotations.clone())
                .unwrap_or_default(),
        },
    )?;
    println!(
        "{}",
        super::po::tf(
            "snapshot/{name}: {snap} of {kind} {target} ({state})",
            &[
                ("name", name),
                ("snap", &snap),
                ("kind", canonical),
                ("target", &spec.target_name),
                (
                    "state",
                    if already {
                        super::po::t("already present")
                    } else {
                        super::po::t("created")
                    },
                ),
            ],
        )
    );
    Ok(())
}

pub fn apply(docs: &[ManifestDoc]) -> Result<()> {
    for doc in manifest::of_kind(docs, k::SNAPSHOT) {
        apply_one(doc)?;
    }
    Ok(())
}

pub(crate) fn converge_doc(doc: &ManifestDoc) -> Result<()> {
    apply_one(doc)
}

pub(crate) fn presence_of(doc: &ManifestDoc) -> (String, String) {
    let name = &doc.metadata.name;
    match snapshot_get(name) {
        None => ("no".into(), "-".into()),
        Some(s) => {
            // The record exists; confirm the artifact really is on the target.
            let live = resolve_target(&s.target_kind)
                .ok()
                .and_then(|(t, _)| backend_has(t, &s.target_name, &s.snapshot_name).ok())
                .unwrap_or(false);
            let detail = super::po::tf(
                "{kind}/{target}",
                &[("kind", &s.target_kind), ("target", &s.target_name)],
            );
            if live {
                ("yes".into(), detail)
            } else {
                // Recorded but the artifact is gone from under it (deleted by
                // hand). Still «declared», with the gap called out.
                (
                    "yes".into(),
                    format!("{detail} {}", super::po::t("(missing)")),
                )
            }
        }
    }
}

pub fn spec_with_defaults(doc: &ManifestDoc) -> Result<serde_yaml::Value> {
    let spec: SnapshotSpec = manifest::spec_of(doc)?;
    // Fill the default snapshot name, so `--dry-run` shows what will be taken.
    let snap = snap_name_of(doc, &spec);
    let filled = SnapshotSpec {
        snapshot_name: Some(snap),
        ..spec
    };
    serde_yaml::to_value(filled).map_err(|e| Error::Invalid(format!("dry-run: {e}")))
}

#[derive(Serialize)]
struct LsRow {
    name: String,
    #[serde(rename = "targetKind")]
    target_kind: String,
    #[serde(rename = "targetName")]
    target_name: String,
    snapshot: String,
    stack: Option<String>,
}

pub(crate) fn cmd_ls(format: OutputFormat) -> Result<()> {
    let format = super::config::resolve_output(&super::util::state_root(), format);
    let rows: Vec<LsRow> = snapshot_list()
        .into_iter()
        .map(|s| LsRow {
            stack: s.labels.get(super::reconcile::STACK_LABEL).cloned(),
            name: s.name,
            target_kind: s.target_kind,
            target_name: s.target_name,
            snapshot: s.snapshot_name,
        })
        .collect();
    if format == OutputFormat::Json {
        return super::output::print_json(&rows);
    }
    let mut t = super::output::Table::new(&["NAME", "TARGET KIND", "TARGET", "SNAPSHOT", "STACK"]);
    for r in &rows {
        t.row(vec![
            r.name.clone(),
            r.target_kind.clone(),
            r.target_name.clone(),
            r.snapshot.clone(),
            r.stack.clone().unwrap_or_else(|| "-".into()),
        ]);
    }
    t.drop_uninformative().print();
    Ok(())
}

pub(crate) fn cmd_describe(names: &[String]) -> Result<()> {
    for name in names {
        let Some(s) = snapshot_get(name) else {
            return Err(Error::NotFound(format!("snapshot: {name}")));
        };
        let mut d = super::output::Describe::new();
        d.field("Name", &s.name);
        d.field("Target Kind", &s.target_kind);
        d.field("Target", &s.target_name);
        d.field("Snapshot", &s.snapshot_name);
        d.field_opt("Stack", s.labels.get(super::reconcile::STACK_LABEL));
        d.print();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(name: &str, spec: &str) -> ManifestDoc {
        let yaml = format!(
            "apiVersion: storage.delonix.io/v1beta1\nkind: Snapshot\n\
             metadata:\n  name: {name}\nspec:\n{spec}"
        );
        serde_yaml::from_str(&yaml).unwrap()
    }

    #[test]
    fn a_vm_target_and_a_volume_target_resolve_and_anything_else_is_refused() {
        assert_eq!(resolve_target("VirtualMachine").unwrap().0, Target::Vm);
        // casing and alias both resolve
        assert_eq!(resolve_target("vm").unwrap().0, Target::Vm);
        assert_eq!(resolve_target("Volume").unwrap().0, Target::Volume);
        let e = resolve_target("Network").unwrap_err().to_string();
        assert!(e.contains("not snapshottable"), "{e}");
    }

    #[test]
    fn the_snapshot_name_defaults_to_the_metadata_name() {
        let d = doc("nightly", "  targetKind: Volume\n  targetName: data\n");
        let spec: SnapshotSpec = manifest::spec_of(&d).unwrap();
        assert_eq!(snap_name_of(&d, &spec), "nightly");
        let d2 = doc(
            "nightly",
            "  targetKind: Volume\n  targetName: data\n  snapshotName: v1\n",
        );
        let spec2: SnapshotSpec = manifest::spec_of(&d2).unwrap();
        assert_eq!(snap_name_of(&d2, &spec2), "v1");
    }

    #[test]
    fn desired_canonicalizes_the_target_kind_and_fills_the_snapshot_name() {
        let d = doc("nightly", "  targetKind: vm\n  targetName: web\n");
        let want = desired(&d).unwrap();
        assert_eq!(want.fields.get("targetKind").unwrap(), "VirtualMachine");
        assert_eq!(want.fields.get("targetName").unwrap(), "web");
        assert_eq!(want.fields.get("snapshotName").unwrap(), "nightly");
        assert!(want.converges && want.ownable);
    }

    #[test]
    fn the_sidecar_record_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        let f = SnapshotFile {
            name: "nightly".into(),
            target_kind: k::VOLUME.into(),
            target_name: "data".into(),
            snapshot_name: "nightly".into(),
            ..Default::default()
        };
        write_in(d, &f).unwrap();
        let got = read_in(d, "nightly").unwrap().unwrap();
        assert_eq!(got.target_name, "data");
        assert_eq!(got.snapshot_name, "nightly");
        assert!(read_in(d, "absent").unwrap().is_none());
    }

    #[test]
    fn a_damaged_record_is_an_error_not_an_empty_one() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        std::fs::write(path_in(d, "nightly"), b"{ not json").unwrap();
        let e = read_in(d, "nightly").unwrap_err().to_string();
        assert!(e.contains("damaged"), "{e}");
    }
}
