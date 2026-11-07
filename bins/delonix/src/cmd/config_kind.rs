//! `kind: Config` — the DECLARATIVE, ConfigMap-shaped sibling of `kind: Secret`
//! (ADR-0086 D3 / ADR-0090). A named bag of key/value pairs a workload consumes
//! as environment variables or files, stored in CLEARTEXT (that is what makes it
//! a Config and not a Secret).
//!
//! **No imperative `config` verb.** The engine already owns `delonix config` (the
//! local CLI preference). So a Config is applied with `delonix stack apply -f`
//! and read/removed through the GENERIC verbs — `delonix get configs`,
//! `delonix describe config <name>`, `delonix delete config <name>` — which route
//! into the [`cmd_ls`]/[`cmd_describe`]/[`cmd_rm`] helpers here. There is no
//! `ConfigCmd` subcommand, on purpose: the imperative surface `Secret` has
//! (`delonix secret …`) cannot be mirrored without colliding with the engine
//! config command, and the Kind is declarative anyway.
//!
//! Everything else mirrors `secret.rs`: the apply, the per-document convergence,
//! the dry-run, the reconcile `desired`/`actual`. The env-loading helpers
//! (`fromEnvFile`/`fromEnv`) are shared from [`super::secret`] rather than copied.

use super::kinds as k;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delonix_model::{Error, Result};
use delonix_node::now_unix;
use delonix_state::{Config, ConfigStore};
use serde::Deserialize;

use super::manifest::{self, ManifestDoc};
use super::output;
use super::secret::{load_env_file, read_env_keys, FromEnv};
use super::util::state_root;

/// `spec` of `kind: Config` — a bag of key/value pairs in CLEARTEXT, consumed by
/// `Container.config` (env/files). The ConfigMap shape: the inline field is
/// `data` (not `stringData`), because the values are not secret.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ConfigSpec {
    /// Inline `KEY: value` pairs — the ConfigMap `data`. In cleartext in the
    /// manifest, which is fine: a Config is non-secret by definition (that is the
    /// difference from a `Secret`'s `stringData`, which warns).
    #[serde(default)]
    data: BTreeMap<String, String>,
    /// Path to a `KEY=value` file (e.g. `.env`) — keeps the values out of the
    /// manifest. Applied BEFORE `data` (inline overrides the file). Same shape as
    /// a Secret's `fromEnvFile`.
    #[serde(default, rename = "fromEnvFile")]
    from_env_file: Option<PathBuf>,
    /// Keys read from the PROCESS's environment at apply time — the same two
    /// shapes a Secret's `fromEnv` accepts (a list of names, or a `key: VAR`
    /// mapping).
    #[serde(default, rename = "fromEnv")]
    from_env: Option<FromEnv>,
}

/// Names accepted in the `kind: Config` `spec`, for the unknown-field warning.
pub(crate) const CONFIG_SPEC_FIELDS: &[&str] = &["data", "fromEnvFile", "fromEnv"];

/// `get configs -o json` row: name + key NAMES + count. A config is cleartext, so
/// — unlike a secret — the VALUES are shown by `describe`; the list still keeps to
/// the names, as every other `get` listing does.
#[derive(serde::Serialize)]
struct ConfigLsRow {
    name: String,
    keys: usize,
    names: Vec<String>,
    updated_unix: u64,
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    used_by: Option<Vec<String>>,
}

/// Containers that reference a config by NAME (`Container.config`). `None` means
/// the container store could not be read — matching the same distinction
/// `secret_user_names` draws.
fn config_user_names(name: &str) -> Option<Vec<String>> {
    let store = delonix_state::Store::open(state_root().join("containers")).ok()?;
    let cs = store.list().ok()?;
    Some(
        cs.into_iter()
            .filter(|c| c.configs.iter().any(|s| s == name))
            .map(|c| c.name)
            .collect(),
    )
}

/// Applies the `kind: Config` documents (called by `stack apply`). Idempotent:
/// `ConfigStore::save` creates or replaces. `base` is the manifest folder, to
/// resolve `fromEnvFile` relative to it. Mirrors `secret::apply` — minus the
/// cleartext warning, because a Config is cleartext on purpose.
pub fn apply(docs: &[ManifestDoc], base: &Path) -> Result<()> {
    let store = ConfigStore::open(state_root())?;
    for doc in manifest::of_kind(docs, k::CONFIG) {
        let name = &doc.metadata.name;
        let spec: ConfigSpec = manifest::spec_of(doc)?;

        let mut data = BTreeMap::new();
        if let Some(f) = &spec.from_env_file {
            data.extend(load_env_file(base, f)?);
        }
        if let Some(fe) = &spec.from_env {
            data.extend(read_env_keys(name, fe)?);
        }
        // Inline overrides the file. No cleartext warning: a Config is cleartext.
        data.extend(spec.data.clone());
        if data.is_empty() {
            return Err(Error::Invalid(super::po::tf(
                "Config '{name}': empty — provide data, fromEnvFile and/or fromEnv",
                &[("name", name)],
            )));
        }
        let n = data.len();
        let previous = store.load(name).ok();
        store.save(&Config {
            name: name.clone(),
            data,
            updated_unix: now_unix(),
            version: 0,
        })?;
        let now = store.load(name)?;
        let line = match previous {
            None => super::po::tf(
                "config/{name}: created, version 1 ({n} key(s))",
                &[("name", name), ("n", &n.to_string())],
            ),
            Some(prev) if prev.data == now.data => super::po::tf(
                "config/{name}: unchanged, version {v} ({n} key(s))",
                &[
                    ("name", name),
                    ("v", &now.version.to_string()),
                    ("n", &n.to_string()),
                ],
            ),
            Some(prev) => {
                let mut changed: Vec<&str> = now
                    .data
                    .iter()
                    .filter(|(k, v)| prev.data.get(*k) != Some(*v))
                    .map(|(k, _)| k.as_str())
                    .chain(
                        prev.data
                            .keys()
                            .filter(|k| !now.data.contains_key(*k))
                            .map(String::as_str),
                    )
                    .collect();
                changed.sort_unstable();
                super::po::tf(
                    "config/{name}: updated to version {v} (changed: {keys})",
                    &[
                        ("name", name),
                        ("v", &now.version.to_string()),
                        ("keys", &changed.join(", ")),
                    ],
                )
            }
        };
        println!("{line}");
    }
    Ok(())
}

/// Dry-run of a `kind: Config`. A config is cleartext, so the values ARE shown
/// (the opposite of `secret::spec_with_defaults`, which redacts) — but, like the
/// secret path, `fromEnvFile` is NOT opened and `fromEnv` is NOT resolved:
/// planning must stay free of I/O and of a dependency on the caller's
/// environment, so those render as the path/pairing only.
pub fn spec_with_defaults(doc: &ManifestDoc) -> Result<serde_yaml::Value> {
    let spec: ConfigSpec = manifest::spec_of(doc)?;
    let mut out = serde_yaml::Mapping::new();
    let mut data = serde_yaml::Mapping::new();
    for (k, v) in &spec.data {
        data.insert(
            serde_yaml::Value::from(k.clone()),
            serde_yaml::Value::from(v.clone()),
        );
    }
    out.insert(
        serde_yaml::Value::from("data"),
        serde_yaml::Value::Mapping(data),
    );
    out.insert(
        serde_yaml::Value::from("fromEnvFile"),
        match &spec.from_env_file {
            Some(p) => serde_yaml::Value::from(p.display().to_string()),
            None => serde_yaml::Value::Null,
        },
    );
    out.insert(
        serde_yaml::Value::from("fromEnv"),
        match &spec.from_env {
            Some(fe) => {
                let mut m = serde_yaml::Mapping::new();
                for (key, var) in fe.pairs() {
                    m.insert(serde_yaml::Value::from(key), serde_yaml::Value::from(var));
                }
                serde_yaml::Value::Mapping(m)
            }
            None => serde_yaml::Value::Null,
        },
    );
    Ok(serde_yaml::Value::Mapping(out))
}

/// The fields `stack plan` compares for a Config: its key NAMES and a content
/// fingerprint of its values. Same pair a Secret uses.
pub(crate) const RECONCILED_CONFIG_FIELDS: &[&str] = &["keys", "fingerprint"];

/// A Config document as the reconciler sees it. Converges only when the manifest
/// CARRIES its values (`data`): a plan that resolved `fromEnv` or read
/// `fromEnvFile` would report differently depending on who ran it and from where,
/// so those stay ensure-present — identical to `secret::desired`.
pub(crate) fn desired(doc: &ManifestDoc) -> Result<super::reconcile::Desired> {
    let spec: ConfigSpec = manifest::spec_of(doc)?;
    let carried = spec.from_env.is_none() && spec.from_env_file.is_none() && !spec.data.is_empty();
    let mut fields = BTreeMap::new();
    if carried {
        let data: BTreeMap<String, String> = spec.data.clone();
        fields.insert(
            "keys".into(),
            data.keys().cloned().collect::<Vec<_>>().join(","),
        );
        fields.insert(
            "fingerprint".into(),
            delonix_state::config::fingerprint(&data),
        );
    }
    Ok(super::reconcile::Desired {
        kind: k::CONFIG.into(),
        name: doc.metadata.name.clone(),
        fields,
        converges: carried,
        ownable: false,
    })
}

/// What the node holds, by name. Reads nothing when there is no `configs` dir.
pub(crate) fn actual() -> Result<Vec<super::reconcile::Actual>> {
    let Some(store) = ConfigStore::open_existing(state_root())? else {
        return Ok(Vec::new());
    };
    Ok(store
        .list()
        .into_iter()
        .map(|c| {
            let mut f = BTreeMap::new();
            f.insert(
                "keys".into(),
                c.data.keys().cloned().collect::<Vec<_>>().join(","),
            );
            f.insert("fingerprint".into(), store.fingerprint(&c.data));
            super::reconcile::Actual {
                kind: k::CONFIG.into(),
                name: c.name.clone(),
                fields: f,
                owner: None,
                last_applied: None,
                protect: false,
            }
        })
        .collect())
}

// ---- generic-verb surface (get/describe/delete route here) ----

/// `delonix get configs` — the listing. Opens nothing if no config was ever
/// stored (empty table / empty JSON array).
pub fn cmd_ls(output: output::OutputFormat) -> Result<()> {
    let output = super::config::resolve_output(&state_root(), output);
    let configs = match ConfigStore::open_existing(state_root())? {
        Some(store) => store.list(),
        None => Vec::new(),
    };
    if output == output::OutputFormat::Json {
        let rows: Vec<ConfigLsRow> = configs
            .into_iter()
            .map(|c| ConfigLsRow {
                used_by: config_user_names(&c.name),
                name: c.name.clone(),
                keys: c.data.len(),
                names: c.data.keys().cloned().collect(),
                updated_unix: c.updated_unix,
                version: c.version.max(1),
            })
            .collect();
        return output::print_json(&rows);
    }
    let mut t = output::Table::new(&["NAME", "VERSION", "KEYS", "NAMES", "AGE", "USED BY"])
        .right_align(1)
        .right_align(2);
    for c in configs {
        let keys: Vec<&str> = c.data.keys().map(String::as_str).collect();
        let used_by = match config_user_names(&c.name) {
            Some(names) if names.is_empty() => "-".to_string(),
            Some(names) => names.join(", "),
            None => "?".to_string(),
        };
        t.row(vec![
            c.name.clone(),
            c.version.max(1).to_string(),
            c.data.len().to_string(),
            keys.join(", "),
            output::fmt_age(c.updated_unix),
            used_by,
        ]);
    }
    t.print();
    Ok(())
}

/// `delonix describe config <name>` — the detail view. Shows the keys AND the
/// VALUES, in cleartext: a config is non-secret, so there is nothing to redact
/// (this is exactly where it differs from `secret inspect`, which hides values
/// behind `--reveal`).
pub fn cmd_describe(names: &[String]) -> Result<()> {
    let store = ConfigStore::open_existing(state_root())?
        .ok_or_else(|| Error::NotFound("config".into()))?;
    for name in names {
        let c = store.load(name).map_err(delonix_model::Error::from)?;
        println!("Name:  {}", c.name);
        println!("Version: {}", c.version.max(1));
        for (key, value) in &c.data {
            println!("  {key}={value}");
        }
    }
    Ok(())
}

/// `delonix delete config <name>` — removes the config. A config has no owner to
/// stamp (ADR-0090: shared content), so no stack prunes it; this is the way one
/// comes down on purpose.
pub fn cmd_rm(names: &[String]) -> Result<()> {
    let store = ConfigStore::open(state_root())?;
    for name in names {
        store.remove(name).map_err(delonix_model::Error::from)?;
        println!(
            "{}",
            super::po::tf("config '{name}' removed", &[("name", name)])
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dry-run of a Config shows the values (cleartext), unlike a Secret's.
    #[test]
    fn the_dry_run_of_a_config_shows_the_values() {
        let doc: ManifestDoc = serde_yaml::from_str(
            "apiVersion: core.delonix.io/v1beta1\nkind: Config\nmetadata: { name: c }\nspec:\n  data: { LOG_LEVEL: debug, FEATURE_X: \"on\" }\n",
        )
        .unwrap();
        let out = serde_yaml::to_string(&spec_with_defaults(&doc).unwrap()).unwrap();
        assert!(out.contains("LOG_LEVEL"), "{out}");
        assert!(out.contains("debug"), "a config value is cleartext: {out}");
    }

    /// `fromEnvFile` is NOT opened by the dry-run — planning stays free of I/O.
    #[test]
    fn the_dry_run_does_not_open_the_fromenvfile() {
        let doc: ManifestDoc = serde_yaml::from_str(
            "apiVersion: core.delonix.io/v1beta1\nkind: Config\nmetadata: { name: c }\nspec: { fromEnvFile: /no/such/app.env }\n",
        )
        .unwrap();
        let out = serde_yaml::to_string(&spec_with_defaults(&doc).unwrap()).unwrap();
        assert!(out.contains("/no/such/app.env"), "{out}");
    }
}
