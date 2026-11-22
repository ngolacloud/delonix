//! **State versioning** — the `delonix.io/*` record-metadata annotations
//! (ADR-0090) and the pure rule that advances them, in the one crate BOTH the
//! CLI reconciler (`delonix-stack`) and the node API (`delonix-node-api`) depend
//! on. The counter rule lives here once, so the two writers cannot drift
//! (ADR-0099 §1).
//!
//! ADR-0090 decided three fields carried as annotations, akin to
//! [`LAST_APPLIED`]: [`GENERATION`] (a spec-version counter), [`OBSERVED_GENERATION`]
//! (the generation the machine last reconciled to), [`STORED_VERSION`] (the
//! `apiVersion` the record was written under). They are **record metadata, never
//! a user knob**; a `generation` in an input spec is ignored, so a `get`→`apply`
//! round-trip is clean.
//!
//! The CLI stamps once, post-success, so [`version_annotations`] sets
//! `observedGeneration == generation` — the ADR-0090 happy path. The node-API
//! mutation path (ADR-0099) splits the stamp in two within its single mutating
//! call: [`desired_annotations`] bumps `generation` at accept and LEAVES
//! `observedGeneration` behind, then [`observed_generation_entry`] advances it on
//! a successful reconcile. A call whose reconcile fails never reaches the second
//! write, so a record can read `generation > observedGeneration` — a pending (or
//! failed) change, which the post-success-only CLI stamp could not express.

use std::collections::BTreeMap;

/// Annotation: the last applied field map (compact JSON). An annotation and not
/// a label: it is not identifying and it is large — a whole spec inside a label
/// would wreck every listing that prints labels.
pub const LAST_APPLIED: &str = "delonix.io/last-applied";

/// Annotation: the spec-version counter (ADR-0090). A monotonic integer, bumped
/// at stamp when the applied spec changed vs the one already on the record, kept
/// otherwise; `1` on first apply. Record metadata, akin to [`LAST_APPLIED`] —
/// not a user knob (a `metadata.generation` in an input manifest is ignored).
pub const GENERATION: &str = "delonix.io/generation";
/// Annotation: the generation the machine was last reconciled to (ADR-0090).
/// On the CLI path, written at stamp (post-success), so it equals [`GENERATION`];
/// on the node-API path (ADR-0099), advanced to [`GENERATION`] only once the
/// reconcile succeeds, so a pending change reads `generation > observedGeneration`.
pub const OBSERVED_GENERATION: &str = "delonix.io/observed-generation";
/// Annotation: the `apiVersion` the record was written under (ADR-0090). One
/// version per Kind today; the field a future `manifest convert` (ADR-0086)
/// reads to migrate a stored record on read.
pub const STORED_VERSION: &str = "delonix.io/stored-version";

/// The next generation for a stamp (ADR-0090): bump when the applied spec
/// changed vs the one already stored, keep it otherwise. A record with no stored
/// generation counts as `0` (unversioned), so its first stamped apply writes `1`.
/// Pure — the one place the counter's rule lives.
pub fn next_generation(
    prev_last_applied: Option<&str>,
    prev_generation: Option<&str>,
    new_last_applied: &str,
) -> u64 {
    let prev_gen: u64 = prev_generation.and_then(|s| s.parse().ok()).unwrap_or(0);
    if prev_last_applied != Some(new_last_applied) {
        prev_gen + 1
    } else {
        prev_gen.max(1)
    }
}

/// The three ADR-0090 version annotations to write at stamp time (CLI path), as
/// `(key, value)` pairs to insert into the record's annotations ALONGSIDE
/// [`LAST_APPLIED`]. `prev` is the record's annotations BEFORE this stamp (where
/// the previous generation and last-applied are read); `new_last_applied` is
/// what this stamp writes; `api_version` the Kind's stored version. Absent-safe:
/// a `prev` with none of them yields generation `1` and the given version.
/// `observedGeneration == generation` here — the stamp runs post-success.
pub fn version_annotations(
    prev: &BTreeMap<String, String>,
    new_last_applied: &str,
    api_version: &str,
) -> [(String, String); 3] {
    let gen = next_generation(
        prev.get(LAST_APPLIED).map(String::as_str),
        prev.get(GENERATION).map(String::as_str),
        new_last_applied,
    );
    [
        (GENERATION.to_string(), gen.to_string()),
        (OBSERVED_GENERATION.to_string(), gen.to_string()),
        (STORED_VERSION.to_string(), api_version.to_string()),
    ]
}

/// The node-API **desired** stamp (ADR-0099): the entries to write at accept,
/// BEFORE the reconcile runs. Bumps [`GENERATION`] by the [`next_generation`]
/// rule for the spec just accepted, writes [`STORED_VERSION`] and
/// [`LAST_APPLIED`], and — unlike [`version_annotations`] — does **not** touch
/// [`OBSERVED_GENERATION`], so it stays at its prior value (absent ⇒ reads `0`).
/// After this, a record whose desired spec changed reads
/// `generation > observedGeneration` until [`observed_generation_entry`] advances
/// it. `prev` is the record's CURRENT annotations; `fields` the applied spec.
pub fn desired_annotations(
    prev: &BTreeMap<String, String>,
    fields: &BTreeMap<String, String>,
    api_version: &str,
) -> Vec<(String, String)> {
    let encoded = encode_last_applied(fields);
    let gen = next_generation(
        prev.get(LAST_APPLIED).map(String::as_str),
        prev.get(GENERATION).map(String::as_str),
        &encoded,
    );
    vec![
        (GENERATION.to_string(), gen.to_string()),
        (STORED_VERSION.to_string(), api_version.to_string()),
        (LAST_APPLIED.to_string(), encoded),
    ]
}

/// The node-API **observed** advance (ADR-0099): the single
/// `(OBSERVED_GENERATION, value)` entry that sets `observedGeneration` equal to
/// the record's current [`GENERATION`], written AFTER a reconcile succeeded.
/// `prev` is the record's CURRENT annotations (carrying the generation the
/// desired stamp wrote). A record with no [`GENERATION`] yields
/// `observedGeneration = 0`, which equals the absent default — a no-op.
pub fn observed_generation_entry(prev: &BTreeMap<String, String>) -> (String, String) {
    let gen = prev.get(GENERATION).map(String::as_str).unwrap_or("0");
    (OBSERVED_GENERATION.to_string(), gen.to_string())
}

/// Serializes a field map for the [`LAST_APPLIED`] annotation. Compact JSON, so
/// it survives the `key=value` line format of the network record (see
/// `NetworkStore::set_metadata`) — `serde_json` escapes a newline inside a
/// string as two characters, never a literal one.
pub fn encode_last_applied(fields: &BTreeMap<String, String>) -> String {
    serde_json::to_string(fields).unwrap_or_else(|_| "{}".to_string())
}

/// Reads back what [`encode_last_applied`] wrote. A corrupt or absent value is
/// `None`, which degrades to «never applied by us» — the conservative side: the
/// reconciler then only ever adds, and never reverts a field on the basis of a
/// value it could not read.
pub fn decode_last_applied(raw: &str) -> Option<BTreeMap<String, String>> {
    serde_json::from_str(raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn next_generation_bumps_on_change_keeps_otherwise() {
        // First apply of an unversioned record writes 1.
        assert_eq!(next_generation(None, None, "la-v1"), 1);
        // Same applied spec keeps the generation.
        assert_eq!(next_generation(Some("la-v1"), Some("1"), "la-v1"), 1);
        // A changed applied spec bumps it.
        assert_eq!(next_generation(Some("la-v1"), Some("1"), "la-v2"), 2);
        assert_eq!(next_generation(Some("la-v2"), Some("2"), "la-v3"), 3);
        // A stored last-applied but no counter (hand-made record) upgrades to 1.
        assert_eq!(next_generation(Some("la-v1"), None, "la-v1"), 1);
    }

    #[test]
    fn version_annotations_sets_observed_equal_on_the_cli_path() {
        let ann = version_annotations(&BTreeMap::new(), "la-v1", "compute.delonix.io/v1beta1");
        let m: BTreeMap<_, _> = ann.into_iter().collect();
        assert_eq!(m.get(GENERATION).map(String::as_str), Some("1"));
        assert_eq!(m.get(OBSERVED_GENERATION).map(String::as_str), Some("1"));
        assert_eq!(
            m.get(STORED_VERSION).map(String::as_str),
            Some("compute.delonix.io/v1beta1")
        );
    }

    #[test]
    fn desired_stamp_bumps_generation_and_leaves_observed_behind() {
        // First apply: generation 1, observed untouched (absent ⇒ reads 0).
        let prev = BTreeMap::new();
        let entries = desired_annotations(
            &prev,
            &map(&[("image", "nginx")]),
            "compute.delonix.io/v1beta1",
        );
        let m: BTreeMap<_, _> = entries.into_iter().collect();
        assert_eq!(m.get(GENERATION).map(String::as_str), Some("1"));
        assert!(
            !m.contains_key(OBSERVED_GENERATION),
            "desired must not advance observed"
        );
        assert!(m.contains_key(LAST_APPLIED));
        assert_eq!(
            m.get(STORED_VERSION).map(String::as_str),
            Some("compute.delonix.io/v1beta1")
        );

        // A record converged at generation 1, then a CHANGED desired spec: the
        // generation bumps to 2 while the stored observed stays 1 — the pending window.
        let converged = map(&[
            (GENERATION, "1"),
            (OBSERVED_GENERATION, "1"),
            (
                LAST_APPLIED,
                &encode_last_applied(&map(&[("image", "nginx")])),
            ),
        ]);
        let entries = desired_annotations(
            &converged,
            &map(&[("image", "nginx:1.27")]),
            "compute.delonix.io/v1beta1",
        );
        let m: BTreeMap<_, _> = entries.into_iter().collect();
        assert_eq!(m.get(GENERATION).map(String::as_str), Some("2"));
        assert!(!m.contains_key(OBSERVED_GENERATION));

        // An UNCHANGED desired spec keeps the generation at 1.
        let entries = desired_annotations(
            &converged,
            &map(&[("image", "nginx")]),
            "compute.delonix.io/v1beta1",
        );
        let m: BTreeMap<_, _> = entries.into_iter().collect();
        assert_eq!(m.get(GENERATION).map(String::as_str), Some("1"));
    }

    #[test]
    fn observed_advance_equals_current_generation() {
        let after_desired = map(&[(GENERATION, "2"), (OBSERVED_GENERATION, "1")]);
        let (k, v) = observed_generation_entry(&after_desired);
        assert_eq!(k, OBSERVED_GENERATION);
        assert_eq!(v, "2");
        // Nothing stamped ⇒ observed 0, the absent default (a no-op).
        let (_, v) = observed_generation_entry(&BTreeMap::new());
        assert_eq!(v, "0");
    }

    #[test]
    fn last_applied_round_trips_and_degrades_safely() {
        let m = map(&[("image", "nginx"), ("env", "A=1\nB=2")]);
        let raw = encode_last_applied(&m);
        assert_eq!(decode_last_applied(&raw).unwrap(), m);
        assert!(decode_last_applied("not json").is_none());
    }
}
