//! The node's admission GATE as the ENGINE reaches it: read the policy file,
//! record the outcome on the event log, and hand back the decision.
//!
//! # Why this is here and not only in the `-bin`
//!
//! Measured on this branch: `cmd/policy.rs::enforce` — the CLI's admission
//! point — is the ONLY place that reads `policy.json` and evaluates a request.
//! It lives in the `delonix` binary, which the interface crates cannot depend
//! on. So the programmatic surfaces that also create workloads —
//! `delonix-node-api` (`CreateVirtualMachine` over gRPC) and `delonix-cri`
//! (`create_container`, the kubelet's path) — created them WITHOUT consulting
//! the node policy at all. A node that set `denyLatestVmImage` or
//! `allowedRegistries` refused the CLI and admitted the very same workload the
//! moment it arrived through the API or the CRI socket: the multi-tenant surface
//! was the one with the hole.
//!
//! This module is the gate on THIS side of the `-bin` boundary. The decision
//! itself is [`crate::admission::evaluate`] (pure, already here); what was
//! trapped in the binary was the glue around it — reading the file and emitting
//! the events — and that is what moves here, so every entry point runs the SAME
//! gate on the SAME file.
//!
//! # Fail-closed, and the same file
//!
//! [`load`] reads `<root>/policy.json`, the exact file the CLI reads. Absent
//! means absent (no ceiling, the engine's documented default); a file that
//! EXISTS and cannot be read or parsed is an error, never a missing policy — a
//! truncated or mistyped policy means somebody's intent is unknown, and running
//! anyway is the silent degradation the engine refuses. The parse itself is
//! [`SecurityPolicy::parse`], which rejects unknown fields, so `denyPriviledged`
//! misspelt does not read as permissive.
//!
//! The CLI keeps its OWN loader (`cmd/policy.rs::load`) so its refusals stay in
//! the operator's language; both funnel through the same `SecurityPolicy::parse`,
//! which is the security invariant. [`decide`] takes an already-loaded policy so
//! the two loaders never diverge on the ONE thing that matters — where the
//! admission events are emitted — which is here, once.

use std::path::{Path, PathBuf};

use delonix_model::{Error, Result};

use crate::admission::{self, Decision, Request};
use crate::event::{self, SecurityEvent};
use crate::policy::SecurityPolicy;
use crate::severity::Severity;

/// `<root>/policy.json` — the node's runtime policy file.
pub fn policy_path(root: &Path) -> PathBuf {
    root.join("policy.json")
}

/// Reads the node policy. `Ok(None)` when there is no file — no ceiling, the
/// engine's documented default. A file that EXISTS and cannot be read or parsed
/// is an `Err`: a policy that cannot be read is not an absent policy.
///
/// English messages, on purpose: the callers of this loader are the engine's
/// programmatic surfaces (gRPC/CRI), which answer in English. The CLI has its
/// own translated loader; both share [`SecurityPolicy::parse`].
pub fn load(root: &Path) -> Result<Option<SecurityPolicy>> {
    let p = policy_path(root);
    let raw = match std::fs::read_to_string(&p) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(Error::Invalid(format!(
                "runtime policy {}: {e} — a policy that cannot be read is not an absent policy",
                p.display()
            )))
        }
    };
    SecurityPolicy::parse(&raw)
        .map(Some)
        .map_err(|e| Error::Invalid(format!("runtime policy {}: {e}", p.display())))
}

/// Evaluates `r` against an already-loaded policy, RECORDS the outcome on the
/// node's event log, and returns the decision.
///
/// This is the single place the admission events are emitted, so the CLI and the
/// programmatic surfaces cannot drift on what gets logged:
/// - no policy at all → one `NO_RUNTIME_POLICY` event, and [`Decision::Allow`]
///   (no ceiling);
/// - a policy that does not constrain this workload's own path → one
///   `WORKLOAD_POLICY_SILENT` event, then the normal evaluation;
/// - the decision's own events (`from_decision`), so a refusal lands on the log
///   whether the request was stopped (`enforce`) or let through (`warn`).
///
/// Rendering the refusal is the CALLER's: the CLI translates it, the gRPC/CRI
/// surfaces map it to a status. See [`Decision::violations`] and
/// [`crate::Violation::summary`].
pub fn decide(
    root: &Path,
    resource: &str,
    policy: Option<&SecurityPolicy>,
    r: &Request<'_>,
) -> Decision {
    let Some(p) = policy else {
        SecurityEvent::unconstrained(
            event::unconstrained::NO_RUNTIME_POLICY,
            Severity::Low,
            r.workload,
            resource,
            None,
        )
        .emit(root);
        return Decision::Allow;
    };
    if !p.constrains(r.workload) {
        SecurityEvent::unconstrained(
            event::unconstrained::WORKLOAD_POLICY_SILENT,
            Severity::Low,
            r.workload,
            resource,
            None,
        )
        .emit(root);
    }
    let decision = admission::evaluate(p, r);
    for ev in SecurityEvent::from_decision(&decision, r, resource) {
        ev.emit(root);
    }
    decision
}

/// Loads the node policy and decides in one call — the engine surfaces' entry
/// point (`delonix-node-api`, `delonix-cri`). An unreadable policy is an `Err`,
/// which those surfaces turn into a refusal of the create: fail-closed, never a
/// workload admitted past a policy nobody could read.
///
/// The CLI uses [`load`] + [`decide`] directly instead, because it also renders
/// lints from the loaded policy and translates its own refusals.
pub fn admit(root: &Path, resource: &str, r: &Request<'_>) -> Result<Decision> {
    let policy = load(root)?;
    Ok(decide(root, resource, policy.as_ref(), r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::Violation;

    fn root_with(policy_json: Option<&str>) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        if let Some(j) = policy_json {
            std::fs::write(policy_path(tmp.path()), j).unwrap();
        }
        tmp
    }

    fn security_events(root: &Path, action: &str) -> usize {
        delonix_node::events::read(root)
            .iter()
            .filter(|e| e.kind == event::EVENT_KIND && e.action == action)
            .count()
    }

    /// No file at all: no ceiling, `Allow`, and a `NO_RUNTIME_POLICY` line so a
    /// caller can SEE the posture without reading the filesystem by hand.
    #[test]
    fn no_file_admits_and_records_no_runtime_policy() {
        let tmp = root_with(None);
        let d = admit(
            tmp.path(),
            "web",
            &Request::container("alpine:latest", true, true),
        )
        .unwrap();
        assert_eq!(d, Decision::Allow);
        assert_eq!(
            security_events(tmp.path(), event::unconstrained::NO_RUNTIME_POLICY),
            1
        );
    }

    /// The node-API gap, end to end through `admit`: `denyLatestVmImage` refuses
    /// a `:latest` VM image that the programmatic path used to admit.
    #[test]
    fn deny_latest_vm_image_refuses_a_latest_vm_through_admit() {
        let tmp = root_with(Some(r#"{"denyLatestVmImage": true}"#));
        let d = admit(
            tmp.path(),
            "db-01",
            &Request::virtual_machine(Some("golden:latest"), &[], None),
        )
        .unwrap();
        assert!(d.is_denied());
        assert!(matches!(
            &d.violations()[0],
            Violation::LatestVmImage { image } if image == "golden:latest"
        ));
    }

    /// The CRI gap, end to end through `admit`: an image off the registry
    /// allowlist is refused where only the capability ceiling used to apply.
    #[test]
    fn allowed_registries_refuses_an_image_off_the_list_through_admit() {
        let tmp = root_with(Some(r#"{"allowedRegistries": ["ghcr.io"]}"#));
        let d = admit(
            tmp.path(),
            "pod-x",
            &Request::container("docker.io/library/nginx:1.27", false, false),
        )
        .unwrap();
        assert!(d.is_denied());
        assert!(matches!(
            &d.violations()[0],
            Violation::Registry { host, .. } if host == "docker.io"
        ));
    }

    /// An existing but unparseable policy is an `Err` from `admit`, so the
    /// surfaces fail the create closed — a typo never silently opens the node.
    #[test]
    fn an_unparseable_policy_is_an_error_from_admit() {
        let tmp = root_with(Some("{ not json"));
        assert!(admit(tmp.path(), "web", &Request::container("a:1", false, false)).is_err());
        let tmp = root_with(Some(r#"{"denyPriviledged": true}"#));
        assert!(
            admit(tmp.path(), "web", &Request::container("a:1", false, false)).is_err(),
            "a misspelt field must not read as permissive"
        );
    }

    /// `decide` emits `WORKLOAD_POLICY_SILENT` when a policy exists but none of
    /// its fields touch this workload's path — the asymmetry `lint` warns about.
    #[test]
    fn decide_records_a_policy_that_is_silent_for_this_path() {
        let tmp = root_with(None);
        let p = SecurityPolicy::parse(r#"{"denyPrivileged": true}"#).unwrap();
        let devices = vec!["0000:01:00.0".to_string()];
        let d = decide(
            tmp.path(),
            "db-01",
            Some(&p),
            &Request::virtual_machine(None, &devices, None),
        );
        assert_eq!(d, Decision::Allow);
        assert_eq!(
            security_events(tmp.path(), event::unconstrained::WORKLOAD_POLICY_SILENT),
            1
        );
    }
}
