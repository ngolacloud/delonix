//! Remote execution via SSH/SCP — shell-out to the system's `ssh`/`scp` (the
//! same pattern already used in this repo for `ip`/`nft`/`qemu-img`/`virt-customize`:
//! never reimplement protocols in Rust, reuse the host's client).
//! Used by `cmd::cluster` for the idempotent `kubeadm` bootstrap.

use std::path::{Path, PathBuf};
use std::process::Command;

use delonix_model::{Error, Result};

#[derive(Debug, Clone)]
pub struct SshTarget {
    pub host: String,
    pub user: String,
    pub key: Option<PathBuf>,
    /// SSH port. `None` = the client's default (22).
    ///
    /// `ssh.port` was in the `kind: KubernetesCluster` schema, was parsed, and reached
    /// nothing — every connection went to 22 regardless. A bastion on a
    /// non-standard port failed with a timeout that named no cause; worse, if
    /// something else answered on 22 (another service, another machine behind
    /// NAT) the bootstrap would run against the wrong host. Accepted-and-ignored
    /// is the failure mode this project refuses by policy — same family as
    /// `--security-opt seccomp=`, `-v :z` and `--network-alias`.
    pub port: Option<u16>,
}

impl SshTarget {
    /// Connection arguments shared by `ssh` and `scp`.
    ///
    /// `port_flag` is the port option **of the tool this argv is for**: `-p` for
    /// `ssh`, `-P` for `scp`. They are genuinely different, and this is the trap
    /// worth naming: `-p` handed to `scp` is not a port at all, it is "preserve
    /// modification times" — the copy would still go to 22, and the only symptom
    /// would be that it worked everywhere except where the port matters. Asking
    /// the caller makes the difference visible at each call site instead of
    /// hidden in here.
    fn conn_args(&self, port_flag: &str) -> Vec<String> {
        let mut a = vec!["-o".to_string(), "BatchMode=yes".to_string()];
        // Host-key verification. Default stays `accept-new` so a greenfield
        // bootstrap of a host never seen before still connects; the two env opt-ins
        // let an operator pin trust and close the first-contact MITM window. See
        // `host_key_opts` and issue #790.
        let strict = std::env::var("DELONIX_SSH_STRICT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let known = std::env::var("DELONIX_SSH_KNOWN_HOSTS")
            .ok()
            .filter(|s| !s.is_empty());
        a.extend(host_key_opts(strict, known.as_deref()));
        a.push("-o".to_string());
        a.push("ConnectTimeout=10".to_string());
        if let Some(p) = self.port {
            a.push(port_flag.to_string());
            a.push(p.to_string());
        }
        if let Some(k) = &self.key {
            a.push("-i".to_string());
            a.push(k.to_string_lossy().into_owned());
        }
        a
    }

    fn user_host(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }
}

/// The host-key verification `-o` options.
///
/// TOFU (`accept-new`) is the default so a greenfield bootstrap of a host never
/// seen before still connects. But that first contact is exactly where a MITM
/// becomes the "known" host for every later `sudo -n bash -c` session, so an
/// operator who can pin trust should be able to:
///
/// - `DELONIX_SSH_KNOWN_HOSTS=<path>` pins trust to that file —
///   `StrictHostKeyChecking=yes` against it, so an unknown or changed key is
///   refused instead of silently trusted.
/// - `DELONIX_SSH_STRICT=1` alone sets `yes` against the user's default
///   known_hosts.
/// - neither: `accept-new`, byte-for-byte the historical behavior.
///
/// Pure (env is read by the caller) so the three branches are tested without
/// touching the process environment.
fn host_key_opts(strict: bool, known_hosts: Option<&str>) -> Vec<String> {
    let mut a = vec!["-o".to_string()];
    match known_hosts {
        Some(path) => {
            a.push("StrictHostKeyChecking=yes".to_string());
            a.push("-o".to_string());
            a.push(format!("UserKnownHostsFile={path}"));
        }
        None if strict => a.push("StrictHostKeyChecking=yes".to_string()),
        None => a.push("StrictHostKeyChecking=accept-new".to_string()),
    }
    a
}

pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Runs `cmd` on the host, as root (`sudo -n` — the SSH user must already be
/// in sudoers without a password; `BatchMode=yes` refuses any interactive
/// prompt, including a password one) OR, when `as_root` is false, as the
/// SSH-connecting user with no `sudo` at all — see [`ssh_run_as_user`] for
/// why that variant exists. Returns `(success, stdout+stderr)`.
fn ssh_run_raw(t: &SshTarget, cmd: &str, as_root: bool) -> Result<(bool, String)> {
    let mut args = t.conn_args("-p");
    // `--` separates options from positional arguments — defense in depth
    // against a `host` starting with `-` being interpreted as an `ssh` flag
    // (e.g. `-oProxyCommand=...`). `valid_endpoint` (cmd::cluster) already
    // refuses such values at the source; this is the second layer. Security
    // audit finding, see AGENTS.md.
    args.push("--".to_string());
    args.push(t.user_host());
    let remote_cmd = if as_root {
        format!("sudo -n bash -c {}", shell_quote(cmd))
    } else {
        format!("bash -c {}", shell_quote(cmd))
    };
    args.push(remote_cmd);
    let out = Command::new("ssh")
        .args(&args)
        .output()
        .map_err(|e| Error::Invalid(format!("running ssh to {}: {e}", t.host)))?;
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok((out.status.success(), combined))
}

/// `true` if `check_cmd` exits successfully on the host (condition already met).
pub fn ssh_check(t: &SshTarget, check_cmd: &str) -> bool {
    ssh_run_raw(t, check_cmd, true)
        .map(|(ok, _)| ok)
        .unwrap_or(false)
}

/// Runs `cmd`; a clear error (with the host and captured output) if it fails.
pub fn ssh_run(t: &SshTarget, cmd: &str) -> Result<String> {
    let (ok, out) = ssh_run_raw(t, cmd, true)?;
    if ok {
        Ok(out)
    } else {
        Err(Error::Invalid(format!(
            "[{}] command failed: {cmd}\n{out}",
            t.host
        )))
    }
}

/// Like [`ssh_run`], but WITHOUT the `sudo -n` wrapping — runs `cmd` as the
/// SSH-connecting user, in their own context. `ssh_run`'s `sudo` is right for
/// `cluster apply`'s job (installing system packages, writing `/etc` as
/// root); it is wrong for invoking a ROOTLESS tool on the target — the
/// `delonix` CLI itself resolves its state root from `geteuid()`/`$HOME`
/// (`Store::default_root`), so running it via `sudo` would silently operate
/// against ROOT's state root instead of the SSH user's own rootless one — a
/// different, invisible VM/volume/store than the one `vm ls` on that host
/// would ever show. `vm migrate` is the first caller.
pub fn ssh_run_as_user(t: &SshTarget, cmd: &str) -> Result<String> {
    let (ok, out) = ssh_run_raw(t, cmd, false)?;
    if ok {
        Ok(out)
    } else {
        Err(Error::Invalid(format!(
            "[{}] command failed: {cmd}\n{out}",
            t.host
        )))
    }
}

/// Copies a local file to the host — WITHOUT elevated privilege (`scp` itself
/// runs as the normal SSH user; for destinations that require root, it copies
/// to `/tmp` and moves it with a following `ssh_run`, as
/// `cmd::cluster::prepare_host` does for `delonix-cri`).
pub fn scp_to(t: &SshTarget, local: &Path, remote_path: &str) -> Result<()> {
    let mut args = t.conn_args("-P");
    args.push("--".to_string());
    args.push(local.to_string_lossy().into_owned());
    args.push(format!("{}:{}", t.user_host(), remote_path));
    let status = Command::new("scp")
        .args(&args)
        .status()
        .map_err(|e| Error::Invalid(format!("running scp to {}: {e}", t.host)))?;
    if !status.success() {
        return Err(Error::Invalid(format!(
            "scp to {}:{remote_path} failed",
            t.host
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{host_key_opts, shell_quote, SshTarget};

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("echo hi"), "'echo hi'");
        assert_eq!(shell_quote("echo 'hi'"), "'echo '\\''hi'\\'''");
    }

    /// Default is TOFU (the greenfield bootstrap still connects); a known_hosts file
    /// pins trust with strict checking; `strict` alone tightens the default file.
    #[test]
    fn host_key_opts_default_is_tofu_and_a_known_hosts_pins_it() {
        assert_eq!(
            host_key_opts(false, None),
            vec!["-o", "StrictHostKeyChecking=accept-new"]
        );
        assert_eq!(
            host_key_opts(true, None),
            vec!["-o", "StrictHostKeyChecking=yes"]
        );
        assert_eq!(
            host_key_opts(false, Some("/etc/delonix/known_hosts")),
            vec![
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                "UserKnownHostsFile=/etc/delonix/known_hosts"
            ]
        );
        // A known_hosts wins over `strict` alone: both pin, and the file is what pins.
        assert_eq!(
            host_key_opts(true, Some("/k")),
            host_key_opts(false, Some("/k"))
        );
    }

    fn target(port: Option<u16>) -> SshTarget {
        SshTarget {
            host: "10.0.0.5".into(),
            user: "delonix".into(),
            key: None,
            port,
        }
    }

    /// `ssh.port` was in the schema and reached nothing. Without a port the argv
    /// has to stay byte-for-byte what it always was — every cluster already out
    /// there omits it, and a change there would be a change for everybody.
    #[test]
    fn without_port_the_argv_stays_exactly_as_it_was() {
        let a = target(None);
        assert_eq!(
            a.conn_args("-p"),
            vec![
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-o",
                "ConnectTimeout=10"
            ]
        );
        assert_eq!(a.conn_args("-p"), a.conn_args("-P"));
    }

    /// The one that matters: `ssh` takes `-p`, `scp` takes `-P`, and `-p` given
    /// to `scp` is "preserve modification times" — it would connect to 22 and
    /// report success, so the symptom would appear only where the port is not
    /// the default. This test is what stops the two call sites from being
    /// "simplified" back into one.
    #[test]
    fn the_scp_port_flag_is_not_the_ssh_one() {
        let a = target(Some(2222));
        assert!(a.conn_args("-p").windows(2).any(|w| w == ["-p", "2222"]));
        assert!(a.conn_args("-P").windows(2).any(|w| w == ["-P", "2222"]));
        assert!(!a.conn_args("-P").iter().any(|x| x == "-p"));
    }
}
