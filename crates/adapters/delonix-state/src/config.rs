//! Config store: named configs (key→value pairs) consumed by a workload as
//! environment variables or files — the ConfigMap-shaped sibling of
//! [`crate::secret::SecretStore`]. Persisted one file per config under
//! `<root>/configs/<name>.json` with **0644** permissions and **in CLEARTEXT**.
//!
//! That cleartext is the whole point, and the one real difference from the
//! secret store: a `kind: Config` holds NON-secret data (feature flags, log
//! levels, service URLs), so it never goes through the [`crate::cred_vault`] —
//! the values are readable on disk on purpose, which is why it is a Config and
//! not a Secret. Everything else — the atomic write, the flock-guarded
//! read-modify-write, the store-assigned version, the name/key validation, the
//! `resolve_env`/`materialize` consumption path — mirrors `SecretStore`.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use crate::{Error, Result};
pub use delonix_model::config::Config;
use delonix_model::secret::{valid_env_key, valid_name};

/// Exclusive file lock (`flock`) sequencing [`ConfigStore::update`]'s
/// read-modify-write BETWEEN PROCESSES. Same pattern as `SecretStore`'s own
/// (module-private there, so duplicated here rather than shared — matches the
/// workspace's habit of each store keeping its own copy of the idiom).
struct FileLock(fs::File);

impl FileLock {
    fn acquire(path: &Path) -> Option<FileLock> {
        let f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .ok()?;
        if !delonix_host::flock_exclusive(f.as_raw_fd()).unwrap_or(false) {
            return None;
        }
        Some(FileLock(f))
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // SAFETY: fd still open (we own the File until here). The File's `close`
        // would also release the flock; explicit so as not to depend on that.
        unsafe { libc::flock(self.0.as_raw_fd(), libc::LOCK_UN) };
    }
}

/// The values as one unambiguous byte string: length-prefixed key and value, in
/// key order, so `{a: "bc"}` and `{ab: "c"}` differ. Identical shape to the
/// secret store's, so the two fingerprints answer the same question.
fn canonical(data: &BTreeMap<String, String>) -> Vec<u8> {
    let mut out = Vec::new();
    for (k, v) in data {
        out.extend_from_slice(&(k.len() as u64).to_le_bytes());
        out.extend_from_slice(k.as_bytes());
        out.extend_from_slice(&(v.len() as u64).to_le_bytes());
        out.extend_from_slice(v.as_bytes());
    }
    out
}

/// A content fingerprint of a set of values, for the reconciler's change
/// detection. Unlike [`crate::cred_vault::CredVault::fingerprint`] it is NOT
/// keyed — a config is non-secret, and a plain content hash is both sufficient
/// (nothing is hidden) and portable (the same values fingerprint the same on
/// every node, which a keyed hash deliberately does not).
pub fn fingerprint(data: &BTreeMap<String, String>) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"delonix-config-fingerprint-v1\0");
    h.update(canonical(data));
    h.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Config store under `<root>/configs`, with 0644 files in **cleartext**.
pub struct ConfigStore {
    root: PathBuf,
}

impl ConfigStore {
    /// Opens (creating) the store. `base` = `$DELONIX_ROOT` (the `configs` dir is
    /// created there). No master key — the values are stored in clear.
    pub fn open(base: impl Into<PathBuf>) -> Result<Self> {
        let base = base.into();
        let root = base.join("configs");
        fs::create_dir_all(&root)?;
        // 0755: unlike the secret store's 0700, a config is non-secret and its
        // directory is readable — the cleartext is the point.
        let _ = fs::set_permissions(&root, fs::Permissions::from_mode(0o755));
        Ok(Self { root })
    }

    /// Opens an existing store without creating anything: `None` when this node
    /// has no `configs` directory, so no config was ever stored. Mirrors
    /// [`crate::secret::SecretStore::open_existing`]'s "nothing to read" answer.
    pub fn open_existing(base: impl Into<PathBuf>) -> Result<Option<Self>> {
        let root = base.into().join("configs");
        Ok(root.is_dir().then_some(Self { root }))
    }

    /// The content fingerprint of a set of values (see [`fingerprint`]).
    pub fn fingerprint(&self, data: &BTreeMap<String, String>) -> String {
        fingerprint(data)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.json"))
    }

    fn lock_path(&self, name: &str) -> PathBuf {
        self.root.join(format!(".{name}.lock"))
    }

    /// Persists a config (atomic write + chmod 0644, in cleartext).
    pub fn save(&self, c: &Config) -> Result<()> {
        if !valid_name(&c.name) {
            return Err(Error::InvalidConfigName(c.name.clone()));
        }
        for k in c.data.keys() {
            if !valid_env_key(k) {
                return Err(Error::InvalidEnvKey(k.clone()));
            }
        }
        // The version is the STORE's to assign (ADR-0069 item 6, as for a
        // secret): same values → same version; different values → previous + 1;
        // first save → 1. The caller cannot forget to bump it or bump it without
        // a change.
        let mut stored = c.clone();
        stored.version = match self.load(&c.name) {
            Ok(prev) if prev.data == c.data => prev.version.max(1),
            Ok(prev) => prev.version.max(1) + 1,
            Err(_) => 1,
        };
        // Cleartext JSON, 0644. Durable AND mode-atomic (temp + fsync + rename),
        // the same primitive the secret store uses — the only difference is the
        // bytes are not sealed and the mode is world-readable.
        crate::write_atomic_mode(
            &self.path(&c.name),
            &serde_json::to_vec(&stored)?,
            Some(0o644),
        )
    }

    /// **Safe read-modify-write** of a config, under an exclusive flock — mirrors
    /// [`crate::secret::SecretStore::update`]. `f` receives the CURRENT config (a
    /// fresh, empty one if it does not exist yet) and returns `false` to abort
    /// without writing.
    pub fn update<F>(&self, name: &str, f: F) -> Result<Config>
    where
        F: FnOnce(&mut Config) -> bool,
    {
        if !valid_name(name) {
            return Err(Error::InvalidConfigName(name.to_string()));
        }
        let _lock = FileLock::acquire(&self.lock_path(name));
        let mut c = self.load(name).unwrap_or_else(|_| Config {
            name: name.to_string(),
            data: BTreeMap::new(),
            updated_unix: 0,
            version: 0,
        });
        if !f(&mut c) {
            return Ok(c);
        }
        self.save(&c)?;
        Ok(c)
    }

    /// Loads a config by name. The name is validated first — the same
    /// path-traversal fix the secret store carries: `load`/`remove` (and
    /// everything that funnels through them: `resolve_env`, `materialize`) must
    /// not build `<root>/<name>.json` from a raw `../../etc/x`.
    pub fn load(&self, name: &str) -> Result<Config> {
        if !valid_name(name) {
            return Err(Error::InvalidConfigName(name.to_string()));
        }
        let p = self.path(name);
        if !p.exists() {
            return Err(Error::NoSuchConfig(name.to_string()));
        }
        Ok(serde_json::from_slice(&fs::read(p)?)?)
    }

    /// Lists the existing configs.
    pub fn list(&self) -> Vec<Config> {
        let mut out = Vec::new();
        if let Ok(rd) = fs::read_dir(&self.root) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
                    if let Ok(c) = serde_json::from_slice(&fs::read(&p).unwrap_or_default()) {
                        out.push(c);
                    }
                }
            }
        }
        out.sort_by(|a: &Config, b: &Config| a.name.cmp(&b.name));
        out
    }

    /// Removes a config. Name validated first (same fix as `load`).
    pub fn remove(&self, name: &str) -> Result<()> {
        if !valid_name(name) {
            return Err(Error::InvalidConfigName(name.to_string()));
        }
        let p = self.path(name);
        if !p.exists() {
            return Err(Error::NoSuchConfig(name.to_string()));
        }
        fs::remove_file(p)?;
        Ok(())
    }

    /// Resolves a list of config names into their `KEY=value` pairs (env), in the
    /// given order (later ones override earlier ones on the same key).
    /// Nonexistent names are ignored (best-effort at startup) — same contract as
    /// [`crate::secret::SecretStore::resolve_env`].
    pub fn resolve_env(&self, names: &[String]) -> Vec<String> {
        let mut env: BTreeMap<String, String> = BTreeMap::new();
        for n in names {
            if let Ok(c) = self.load(n) {
                for (k, v) in c.data {
                    env.insert(k, v);
                }
            }
        }
        env.into_iter().map(|(k, v)| format!("{k}={v}")).collect()
    }

    /// Materializes the configs as **files** in `dir` (one per key, 0644; `dir`
    /// 0755) — for injection via bind-mount into `/run/config`, instead of
    /// environment variables. Idempotent. Unlike the secret equivalent the files
    /// are 0644, not 0600: a config is not a credential.
    pub fn materialize(&self, names: &[String], dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
        for n in names {
            let c = self.load(n)?;
            for (k, v) in &c.data {
                if !valid_env_key(k) {
                    continue;
                }
                let p = dir.join(k);
                fs::write(&p, v)?;
                let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o644));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The value is on disk in **cleartext** — the one property that separates a
    /// Config from a Secret. The sibling secret test asserts the opposite
    /// (`value_encrypted_at_rest_and_legacy_plaintext_readable`).
    #[test]
    fn value_is_cleartext_on_disk_unlike_a_secret() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ConfigStore::open(tmp.path()).unwrap();
        let mut data = BTreeMap::new();
        data.insert("LOG_LEVEL".to_string(), "debug".to_string());
        store
            .save(&Config {
                name: "app".into(),
                data,
                updated_unix: 0,
                version: 0,
            })
            .unwrap();
        let raw = std::fs::read_to_string(tmp.path().join("configs/app.json")).unwrap();
        assert!(
            raw.contains("debug"),
            "the value must be readable on disk (that is what makes it a Config): {raw}"
        );
        // And the file is world-readable 0644, not the secret store's 0600.
        let mode = std::fs::metadata(tmp.path().join("configs/app.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o644, "a config file is cleartext and world-readable");
        // hold the guard to the end (tmp-hygiene gate).
        drop(tmp);
    }

    /// The store assigns the version: first save is 1, an unchanged re-save keeps
    /// it, a changed value bumps it — the same discipline as the secret store.
    #[test]
    fn the_store_assigns_the_version_from_the_values() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ConfigStore::open(tmp.path()).unwrap();
        let mk = |v: &str| {
            let mut d = BTreeMap::new();
            d.insert("K".to_string(), v.to_string());
            Config {
                name: "c".into(),
                data: d,
                updated_unix: 0,
                version: 0,
            }
        };
        store.save(&mk("a")).unwrap();
        assert_eq!(store.load("c").unwrap().version, 1);
        store.save(&mk("a")).unwrap();
        assert_eq!(
            store.load("c").unwrap().version,
            1,
            "unchanged keeps version"
        );
        store.save(&mk("b")).unwrap();
        assert_eq!(store.load("c").unwrap().version, 2, "a change bumps it");
        drop(tmp);
    }

    /// `resolve_env` and `materialize` are the two consumption paths — env and
    /// files — the same pair the secret store serves.
    #[test]
    fn resolve_env_and_materialize_deliver_the_values() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ConfigStore::open(tmp.path()).unwrap();
        let mut data = BTreeMap::new();
        data.insert("A".to_string(), "1".to_string());
        data.insert("B".to_string(), "2".to_string());
        store
            .save(&Config {
                name: "c".into(),
                data,
                updated_unix: 0,
                version: 0,
            })
            .unwrap();
        let env = store.resolve_env(&["c".to_string()]);
        assert!(env.contains(&"A=1".to_string()) && env.contains(&"B=2".to_string()));
        let dir = tmp.path().join("mat");
        store.materialize(&["c".to_string()], &dir).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("A")).unwrap(), "1");
        assert_eq!(std::fs::read_to_string(dir.join("B")).unwrap(), "2");
        drop(tmp);
    }

    /// A traversal name is refused by `load`/`remove`, the same fix the secret
    /// store carries.
    #[test]
    fn a_traversal_name_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ConfigStore::open(tmp.path()).unwrap();
        assert!(store.load("../../etc/passwd").is_err());
        assert!(store.remove("../../etc/passwd").is_err());
        drop(tmp);
    }
}
