//! `kind: Application` — the Kind you WRITE for a container service (ADR-0086 D4).
//!
//! An `Application` lowers at load into its `replicas` `kind: Pod` INSTANCES, the
//! same way `kind: Workload` lowers into its child (ADR-0001). The Pod is the
//! instance an Application creates — it shows in `delonix get pods`, it is not a
//! thing a person writes directly going forward — and reusing the Pod executor
//! means an Application needs no apply, stamp or presence of its own: each
//! instance flows through the normal per-Kind apply like any other Pod.
//!
//! Scaling is a re-apply with a different `replicas`: the instances that fall
//! away become `Delete`s the normal `--prune` removes, so there is no new
//! reconcile path — the property the lowering approach buys.

use delonix_model::{Error, Result};
use serde::Deserialize;

use super::kinds as k;
use super::manifest::ManifestDoc;

/// A guard against a typo (`replicas: 1000000`) turning into a million Pod
/// documents at load. Not a product limit — a sanity ceiling.
const MAX_REPLICAS: u32 = 1024;

fn one() -> u32 {
    1
}

/// `spec` of `kind: Application`: a [`super::container::PodSpec`] (`containers[]`,
/// `network`, …) plus `replicas`. Each instance is one `kind: Pod` whose spec is
/// this one with `replicas` removed.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ApplicationSpec {
    /// How many Pod instances to run (default 1). Each is a `<name>-<i>` Pod.
    #[serde(default = "one")]
    pub replicas: u32,
    /// The rest of the spec IS a `PodSpec`. Read by serde to VALIDATE it on
    /// `lower_application` (so a missing `containers[]` is caught at load) and to
    /// shape the schema via `#[serde(flatten)]`; the lowering copies the raw spec
    /// (minus `replicas`), so the parsed value itself is never read back.
    #[serde(flatten)]
    #[allow(dead_code)]
    pub pod: super::container::PodSpec,
}

/// Known `spec` keys: `replicas` plus every `PodSpec` key. Hand-written because a
/// `&'static` list cannot concatenate `POD_SPEC_FIELDS` at compile time; a test
/// below keeps it in step with that list so the two cannot drift.
pub const APPLICATION_SPEC_FIELDS: &[&str] = &[
    "replicas",
    "containers",
    "volumes",
    "network",
    "restartPolicy",
    "hostname",
    "expose",
    "detach",
    "shareProcessNamespace",
    "hostAliases",
];

/// Lowers a `kind: Application` into its `replicas` `kind: Pod` INSTANCES
/// (ADR-0086 D4). Each instance is `<name>-<i>` (`i` from 0), inherits the
/// Application's namespace/labels/annotations, and carries the Application spec
/// with `replicas` removed — which is exactly a `PodSpec`.
pub fn lower_application(doc: &ManifestDoc) -> Result<Vec<ManifestDoc>> {
    // Parse through the typed spec so a bad `replicas` or a missing
    // `containers[]` is caught HERE, not halfway through the apply.
    let spec: ApplicationSpec = super::manifest::spec_of(doc)?;
    let name = &doc.metadata.name;
    if spec.replicas == 0 {
        return Err(Error::Invalid(super::po::tf(
            "application '{name}': replicas must be at least 1",
            &[("name", name)],
        )));
    }
    if spec.replicas > MAX_REPLICAS {
        return Err(Error::Invalid(super::po::tf(
            "application '{name}': replicas {n} exceeds the limit of {max}",
            &[
                ("name", name),
                ("n", &spec.replicas.to_string()),
                ("max", &MAX_REPLICAS.to_string()),
            ],
        )));
    }
    // The instance spec is the Application spec minus `replicas` — a `PodSpec`.
    let mut pod_spec = doc.spec.clone();
    if let Some(m) = pod_spec.as_mapping_mut() {
        m.remove(serde_yaml::Value::from("replicas"));
    }
    let mut out = Vec::with_capacity(spec.replicas as usize);
    for i in 0..spec.replicas {
        let mut metadata = doc.metadata.clone();
        metadata.name = format!("{name}-{i}");
        out.push(ManifestDoc {
            api_version: doc.api_version.clone(),
            kind: k::POD.to_string(),
            metadata,
            spec: pod_spec.clone(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(yaml: &str) -> ManifestDoc {
        serde_yaml::from_str(yaml).unwrap()
    }

    /// The hand-written field list is exactly `replicas` + every `PodSpec` key,
    /// so the unknown-field check cannot drift from what a Pod instance accepts.
    #[test]
    fn the_field_list_is_replicas_plus_the_pod_fields() {
        let mut want: Vec<&str> = std::iter::once("replicas")
            .chain(delonix_compute::pod::POD_SPEC_FIELDS.iter().copied())
            .collect();
        want.sort_unstable();
        let mut got: Vec<&str> = APPLICATION_SPEC_FIELDS.to_vec();
        got.sort_unstable();
        assert_eq!(got, want);
    }

    /// Default `replicas` is 1 — one Pod instance `<name>-0`, carrying the spec
    /// with `replicas` gone, the Kind flipped to `Pod`, and the metadata inherited.
    #[test]
    fn a_single_replica_lowers_to_one_pod_instance() {
        let pods = lower_application(&doc(
            "apiVersion: compute.delonix.io/v1beta1\n\
             kind: Application\n\
             metadata: { name: web, namespace: shop }\n\
             spec: { containers: [{ name: web, image: nginx }] }\n",
        ))
        .unwrap();
        assert_eq!(pods.len(), 1);
        assert_eq!(pods[0].kind, k::POD);
        assert_eq!(pods[0].metadata.name, "web-0");
        assert_eq!(pods[0].metadata.namespace.as_deref(), Some("shop"));
        assert!(pods[0].spec.get("replicas").is_none());
        assert!(pods[0].spec.get("containers").is_some());
    }

    /// `replicas: N` lowers to N instances `<name>-0..N-1`.
    #[test]
    fn n_replicas_lower_to_n_numbered_instances() {
        let pods = lower_application(&doc(
            "apiVersion: compute.delonix.io/v1beta1\n\
             kind: Application\n\
             metadata: { name: api }\n\
             spec: { replicas: 3, containers: [{ name: api, image: nginx }] }\n",
        ))
        .unwrap();
        let names: Vec<&str> = pods.iter().map(|p| p.metadata.name.as_str()).collect();
        assert_eq!(names, ["api-0", "api-1", "api-2"]);
        assert!(pods.iter().all(|p| p.kind == k::POD));
    }

    /// `replicas: 0` is a mistake, refused — never a silent no-op.
    #[test]
    fn zero_replicas_is_refused() {
        assert!(lower_application(&doc(
            "apiVersion: compute.delonix.io/v1beta1\n\
             kind: Application\n\
             metadata: { name: x }\n\
             spec: { replicas: 0, containers: [{ name: x, image: a }] }\n",
        ))
        .is_err());
    }
}
