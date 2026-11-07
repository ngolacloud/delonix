//! `kind: Job` — a run-to-completion workload (ADR-0086 D3).
//!
//! Like `Application`, a Job lowers at load into a `kind: Pod` and reuses the Pod
//! executor — it needs no apply of its own. The one behavioural difference from a
//! bare Pod is the default `restartPolicy`: a Pod defaults to `no` (never restart),
//! while a Job defaults to `OnFailure` — the container is retried until it exits
//! successfully, which is what "run to completion" means. A daemonless engine has
//! no controller to track `completions`/`backoffLimit`/`parallelism`, so those
//! k8s-Job fields are deliberately absent; the retry is the container runtime's
//! restart policy, not a controller's.
//!
//! A Job is a single Pod (no `replicas`): a batch task runs once, retried on
//! failure. The user may still set `restartPolicy: Never` to fail once and stop.

use delonix_model::Result;

use super::kinds as k;
use super::manifest::ManifestDoc;

/// `spec` of `kind: Job`: a [`super::container::PodSpec`]. A newtype over it (not
/// a reuse of the `Pod` schema) so the published schema names it `JobSpec` and a
/// future `backoffLimit`/`completions` has somewhere to land.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub(crate) struct JobSpec {
    /// A Job's spec IS a `PodSpec`. Read by serde to VALIDATE it on `lower_job`
    /// (a missing `containers[]` is caught at load) and to shape the schema; the
    /// lowering copies the raw spec, so the parsed value itself is never read back.
    #[serde(flatten)]
    #[allow(dead_code)]
    pub pod: super::container::PodSpec,
}

/// Lowers a `kind: Job` into its single `kind: Pod`, defaulting `restartPolicy`
/// to `OnFailure` (ADR-0086 D3) when the author did not set one — the run-to-
/// completion-with-retry behaviour that separates a Job from a bare Pod.
pub fn lower_job(doc: &ManifestDoc) -> Result<ManifestDoc> {
    // Validate through the typed spec (containers required) before lowering.
    let _spec: JobSpec = super::manifest::spec_of(doc)?;
    let mut spec = doc.spec.clone();
    if let Some(m) = spec.as_mapping_mut() {
        let key = serde_yaml::Value::from("restartPolicy");
        if !m.contains_key(&key) {
            m.insert(key, serde_yaml::Value::from("OnFailure"));
        }
    }
    Ok(ManifestDoc {
        api_version: doc.api_version.clone(),
        kind: k::POD.to_string(),
        metadata: doc.metadata.clone(),
        spec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(yaml: &str) -> ManifestDoc {
        serde_yaml::from_str(yaml).unwrap()
    }

    /// A Job lowers to one Pod whose `restartPolicy` defaults to `OnFailure` —
    /// the behaviour a bare Pod (default `no`) does not have.
    #[test]
    fn a_job_lowers_to_a_pod_that_retries_on_failure() {
        let pod = lower_job(&doc("apiVersion: compute.delonix.io/v1beta1\n\
             kind: Job\n\
             metadata: { name: migrate, namespace: shop }\n\
             spec: { containers: [{ name: migrate, image: flyway }] }\n"))
        .unwrap();
        assert_eq!(pod.kind, k::POD);
        assert_eq!(pod.metadata.name, "migrate");
        assert_eq!(pod.metadata.namespace.as_deref(), Some("shop"));
        assert_eq!(
            pod.spec.get("restartPolicy").and_then(|v| v.as_str()),
            Some("OnFailure")
        );
    }

    /// An explicit `restartPolicy` is kept — a Job that should fail once and stop
    /// says `Never`, and the lowering must not overwrite it.
    #[test]
    fn an_explicit_restart_policy_is_kept() {
        let pod = lower_job(&doc("apiVersion: compute.delonix.io/v1beta1\n\
             kind: Job\n\
             metadata: { name: once }\n\
             spec: { restartPolicy: Never, containers: [{ name: once, image: a }] }\n"))
        .unwrap();
        assert_eq!(
            pod.spec.get("restartPolicy").and_then(|v| v.as_str()),
            Some("Never")
        );
    }
}
