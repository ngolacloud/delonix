//! The Pod-shaped `kind: Container` and `kind: Pod`: the k8s-like spec types and
//! their translation into the one run specification ([`crate::RunOpts`]).
//!
//! Pure: the translators read a spec and return a `RunOpts`, and a warning the
//! operator must see is pushed into a [`crate::Notice`] list instead of printed —
//! the interface that owns the terminal renders it in the operator's language.

use crate::{Notice, RunOpts};
use delonix_model::{Error, Result};
use serde::{Deserialize, Serialize};

/// The member's position in `spec.containers`, recorded at create time on each
/// member (the history of why is in the CLI's `pod` module).
pub const POD_INDEX_LABEL: &str = "delonix.io/pod-index";

pub fn default_restart() -> String {
    "no".to_string()
}

pub fn default_true() -> bool {
    true
}

pub fn default_net() -> String {
    "host".to_string()
}

/// Validates an `--add-host` entry, in the `name:ip` format (Docker's).
///
/// Returns the normalized `(name, ip)`, or the error to show. Validate HERE, at
/// the boundary, and not where the file is written: a bad entry must fail before
/// the container exists, never be dropped in silence on the next start (the trap
/// this repo has already turned into an error for `--security-opt seccomp=`,
/// `-v :z` and `--network-alias`).
///
/// The `\n` is the crux: without refusing it, an entry injects arbitrary lines
/// into `/etc/hosts` which — combined with a symlink planted by the image —
/// gave a write of chosen content outside the rootfs.
///
/// Only the `name:ip` form. The `name=ip` form was tried and removed: because
/// the `:` is looked for first, `db=2001:db8::1` split into `db=2001` + `db8::1`
/// and wrote a wrong entry without a word. Docker does not accept it.
pub fn parse_add_host(entry: &str) -> std::result::Result<(String, String), String> {
    // Split on the FIRST `:`, like Docker (`SplitN(..., 2)`). The name never
    // contains `:` (the LDH whitelist below guarantees it), so everything after
    // it is the address — and that is how an IPv6 (`db:2001:db8::1`) stays
    // whole. With `rsplit_once` the split happened on the last `:` and the IPv6
    // came out truncated; the test is what caught it.
    let Some((name, addr)) = entry.split_once(':') else {
        return Err(format!("invalid --add-host '{entry}': expected 'name:ip'"));
    };
    let (name, addr) = (name.trim(), addr.trim());
    if name.is_empty() {
        return Err(format!("invalid --add-host '{entry}': empty name"));
    }
    if name.len() > 253 {
        return Err(format!("invalid --add-host '{entry}': name too long"));
    }
    // LDH whitelist. The `.` IS allowed here — unlike `valid_container_name`,
    // which refuses it because a container name enters the holder's SHARED DNS
    // and could hijack a domain for the whole node. This is written only into
    // the container's own `/etc/hosts`.
    if !name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return Err(format!(
            "invalid --add-host '{entry}': name may only contain letters, digits, '.', '-' and '_'"
        ));
    }
    // The address is PARSED, not copied. That is what makes injection
    // structurally impossible on this side — the same thing Docker does by
    // storing a `netip.Addr` rather than a string.
    let ip: std::net::IpAddr = addr
        .parse()
        .map_err(|_| format!("invalid --add-host '{entry}': '{addr}' is not an IP address"))?;
    Ok((name.to_string(), ip.to_string()))
}

// ===========================================================================
// Pod-shaped `kind: Container` (k8s-like) — opt-in when `spec.containers` is
// present. Normalizes to the SAME internal `RunOpts` as the flat spec, so the
// engine is untouched. v1: EXACTLY ONE container (a clear error on >1). The flat
// spec stays fully supported (back-compat); the two shapes never mix.
// ===========================================================================

/// k8s `hostAliases[]` entry: one IP, N hostnames.
#[derive(Debug, Deserialize, Serialize, Clone, schemars::JsonSchema)]
pub struct HostAlias {
    pub ip: String,
    #[serde(default)]
    pub hostnames: Vec<String>,
}

impl HostAlias {
    /// k8s (`{ip, hostnames[]}`) → docker (`name:ip`), validated by the same
    /// `--add-host` parser. A k8s entry with N names becomes N entries.
    pub fn to_add_host(&self) -> std::result::Result<Vec<String>, String> {
        let mut out = Vec::with_capacity(self.hostnames.len());
        for name in &self.hostnames {
            let (n, ip) = parse_add_host(&format!("{}:{}", name.trim(), self.ip.trim()))?;
            out.push(format!("{n}:{ip}"));
        }
        Ok(out)
    }
}

/// k8s-like Pod spec: `spec.containers[]`. Used by `kind: Container` (Pod shape,
/// 1 container) AND by `kind: Pod` (N containers sharing the pod's namespaces —
/// see `cmd::pod`).
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodSpec {
    pub containers: Vec<PodContainer>,
    #[serde(default)]
    pub volumes: Vec<PodVolume>,
    /// delonix extension: the SDN network the POD's shared netns attaches to.
    ///
    /// A `<custom>` name selects that network's bridge. `host`/`none` (the default) mean
    /// "the pod's own netns on the default bridge" — a pod IS a shared netns, so it never
    /// gets the host's. They are kept as the default because every existing manifest relies
    /// on it; only a custom name changes anything.
    ///
    /// Was parsed and **entirely ignored** until v0.47.0: `create_pod` hardcoded `ingress`,
    /// so a pod declared on a custom network landed on the default bridge in silence.
    #[serde(default = "default_net")]
    pub network: String,
    /// k8s `restartPolicy`: `Always`|`OnFailure`|`Never` (delonix values also accepted).
    #[serde(default = "default_restart", rename = "restartPolicy")]
    pub restart_policy: String,
    #[serde(default)]
    pub hostname: Option<String>,
    /// delonix extension: auto-register an HTTP port in the L7 proxy.
    #[serde(default)]
    pub expose: Option<u16>,
    #[serde(default = "default_true")]
    pub detach: bool,
    /// k8s `hostAliases`: extra `/etc/hosts` entries, in the k8s shape
    /// (`{ip, hostnames[]}`) rather than docker's `name:ip`. Same effect as
    /// `--add-host`; normalized below.
    ///
    /// Wired on purpose: without it, the SAME `kind: Container` gained or lost
    /// the feature depending on which shape of spec was used — flat had it,
    /// k8s silently did not.
    #[serde(default, rename = "hostAliases")]
    pub host_aliases: Vec<HostAlias>,
    /// k8s `shareProcessNamespace`: the pod's containers see each other's
    /// processes (shared PID namespace). Default `false`, like k8s. Honored by
    /// `kind: Pod` (see `cmd::pod`); ignored for a single `kind: Container`.
    #[serde(default, rename = "shareProcessNamespace")]
    pub share_process_namespace: bool,
}

/// One entry of `spec.containers[]`.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodContainer {
    /// k8s member name. Absent → `c<i>` by position, the fallback
    /// `pod_member_run_opts` uses to build the container name `<pod>-<member>`;
    /// the reconciler has to reproduce it or every pod would diff against
    /// itself.
    #[serde(default)]
    pub name: Option<String>,
    pub image: String,
    /// k8s `command` — overrides the image ENTRYPOINT.
    #[serde(default)]
    pub command: Vec<String>,
    /// k8s `args` — overrides the image CMD.
    #[serde(default)]
    pub args: Vec<String>,
    /// k8s `workingDir` — the directory the process starts in (same as
    /// `container run -w`). Omitted: the image's own working directory, or `/`.
    // It used to be accepted, warned about as "not applied yet" and dropped, long
    // after `RunOpts.workdir` existed and compose used it.
    #[serde(default, rename = "workingDir")]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub ports: Vec<PodPort>,
    #[serde(default)]
    pub env: Vec<PodEnvVar>,
    #[serde(default, rename = "volumeMounts")]
    pub volume_mounts: Vec<PodVolumeMount>,
    #[serde(default)]
    pub resources: Option<PodResources>,
    #[serde(default, rename = "securityContext")]
    pub security_context: Option<PodSecurityContext>,
    #[serde(default)]
    #[allow(dead_code)] // accepted; delonix decides tty by attach/detach
    pub tty: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodPort {
    #[serde(rename = "containerPort")]
    pub container_port: u16,
    #[serde(default, rename = "hostPort")]
    pub host_port: Option<u16>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default, rename = "hostIP")]
    pub host_ip: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodEnvVar {
    pub name: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodVolumeMount {
    pub name: String,
    #[serde(rename = "mountPath")]
    pub mount_path: String,
    #[serde(default, rename = "readOnly")]
    pub read_only: bool,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodResources {
    #[serde(default)]
    pub limits: Option<PodResourceList>,
    #[serde(default)]
    #[allow(dead_code)] // requests are advisory; delonix enforces limits
    pub requests: Option<PodResourceList>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodResourceList {
    #[serde(default)]
    pub cpu: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodSecurityContext {
    #[serde(default)]
    pub privileged: bool,
    #[serde(default, rename = "runAsUser")]
    pub run_as_user: Option<i64>,
    #[serde(default, rename = "readOnlyRootFilesystem")]
    pub read_only_root_filesystem: bool,
    #[serde(default)]
    pub capabilities: Option<PodCapabilities>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodCapabilities {
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub drop: Vec<String>,
}

/// One entry of the Pod-level `spec.volumes[]` (referenced by `volumeMounts`).
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodVolume {
    pub name: String,
    #[serde(default, rename = "hostPath")]
    pub host_path: Option<PodHostPath>,
    #[serde(default, rename = "emptyDir")]
    pub empty_dir: Option<PodEmptyDir>,
    #[serde(default, rename = "persistentVolumeClaim")]
    pub pvc: Option<PodPvc>,
    /// delonix extension: a named `Volume`/`Storage` directly by source string.
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodHostPath {
    pub path: String,
}
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodEmptyDir {
    // Internal note (deliberately `//` and not `///`): a doc-comment here is
    // published as `description` in `docs/schema/v1/delonix.json` and is the text
    // the manifest author's IDE shows. User surface, therefore — EN and about the
    // BEHAVIOUR, never about the internals. The gate
    // `the_published_schema_is_up_to_date_with_the_code` caught it in the first version,
    // which had a PT note here about `#[allow(dead_code)]`.
    //
    // The field was `#[allow(dead_code)]` — that is how an accepted-and-ignored
    // field slips by unnoticed: `warn_unknown_fields` lets it in because it is in
    // the schema, and nobody reads it. It is now read in `pod_to_run_opts`, to warn.
    /// `""` (default) or `"Memory"`. This engine always backs an `emptyDir` with
    /// tmpfs (host RAM); Kubernetes uses node disk unless `Memory` is set, so a
    /// manifest that omits this gets a warning. Prefer a named volume for large
    /// scratch space.
    #[serde(default)]
    pub medium: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PodPvc {
    #[serde(rename = "claimName")]
    pub claim_name: String,
}

/// Top-level field names accepted in a Pod-shaped `spec` (for the unknown-field warning).
pub const POD_SPEC_FIELDS: &[&str] = &[
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

/// Keys accepted in one `spec.containers[]` entry. Anything else (`livenessProbe`,
/// `readinessProbe`, `startupProbe`, `lifecycle`, a typo) has no effect in this
/// engine, so it is refused instead of silently dropped.
pub const POD_CONTAINER_FIELDS: &[&str] = &[
    "name",
    "image",
    "command",
    "args",
    "workingDir",
    "ports",
    "env",
    "volumeMounts",
    "resources",
    "securityContext",
    "tty",
];

/// k8s CPU quantity → docker-style core count: `"500m"` → `"0.5"`, `"2"` → `"2"`.
pub fn cpu_quantity_to_cores(q: &str) -> String {
    if let Some(m) = q.strip_suffix('m') {
        if let Ok(milli) = m.trim().parse::<f64>() {
            return format!("{}", milli / 1000.0);
        }
    }
    q.trim().to_string()
}

/// Normalizes a Pod-shaped spec (k8s-like) into the flat [`RunOpts`]. `kind:
/// Container` accepts exactly one container (multi-container is `kind: Pod` — see
/// `cmd::pod`); the per-container mapping lives in [`container_to_run_opts`].
pub fn pod_to_run_opts(
    name: &str,
    namespace: Option<String>,
    pod: PodSpec,
    notices: &mut Vec<Notice>,
) -> Result<RunOpts> {
    if pod.containers.is_empty() {
        return Err(Error::Invalid(format!(
            "Container '{name}': spec.containers is empty"
        )));
    }
    if pod.containers.len() > 1 {
        return Err(Error::Invalid(format!(
            "Container '{name}': a `kind: Container` runs a single container — use `kind: Pod` for {} containers sharing a namespace",
            pod.containers.len()
        )));
    }
    let c = pod.containers.into_iter().next().unwrap();
    let add_host = pod_add_host(&pod.host_aliases)?;
    let mut opts = container_to_run_opts(
        name,
        namespace,
        c,
        &pod.volumes,
        pod.network,
        &pod.restart_policy,
        pod.hostname,
        pod.expose,
        pod.detach,
        notices,
    )?;
    opts.add_host = add_host;
    Ok(opts)
}

/// `hostAliases` (k8s) → `--add-host` (docker), validated. Shared by the
/// `kind: Container` pod shape and by each member of a `kind: Pod`, so the two
/// shapes cannot diverge.
pub fn pod_add_host(aliases: &[HostAlias]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for a in aliases {
        out.extend(a.to_add_host().map_err(Error::Invalid)?);
    }
    Ok(out)
}

/// Builds the [`RunOpts`] for EACH container of a `kind: Pod`, wired to the shared
/// pod netns `pod_netns` (via `--pod`) and labelled for membership
/// (`delonix.io/pod=<name>`). All containers share the pod's network (same IP,
/// localhost between them) and hostname. Reuses [`container_to_run_opts`] so the
/// k8s→docker mapping is identical to the single-container path.
pub fn pod_member_run_opts(
    pod_name: &str,
    namespace: Option<String>,
    pod: PodSpec,
    pod_netns: &str,
    notices: &mut Vec<Notice>,
) -> Result<Vec<RunOpts>> {
    if pod.containers.is_empty() {
        return Err(Error::Invalid(format!(
            "Pod '{pod_name}': spec.containers is empty"
        )));
    }
    let hostname = pod.hostname.clone().unwrap_or_else(|| pod_name.to_string());
    // k8s `shareProcessNamespace`: a pod-level fact, read before the loop
    // consumes `pod`. Carried on EVERY member's `RunOpts`; the member join only
    // takes effect once `cmd::pod` has filled in `pod_infra_pid` (the head
    // member, which holds the shared PID namespace, gets none and keeps its
    // own).
    let share_process_namespace = pod.share_process_namespace;
    // The members share the netns, so they also share the pod's `/etc/hosts`
    // entries — but each has its own rootfs, so they must be written into every
    // one.
    let add_host = pod_add_host(&pod.host_aliases)?;
    let mut out = Vec::with_capacity(pod.containers.len());
    for (i, c) in pod.containers.into_iter().enumerate() {
        let member = c.name.clone().unwrap_or_else(|| format!("c{i}"));
        let cname = format!("{pod_name}-{member}");
        // `network = "host"`: irrelevant here — the `pod` field makes the
        // container JOIN the pod's shared netns regardless (see `cmd_run`).
        let mut opts = container_to_run_opts(
            &cname,
            namespace.clone(),
            c,
            &pod.volumes,
            "host".to_string(),
            &pod.restart_policy,
            Some(hostname.clone()),
            None,
            true,
            notices,
        )?;
        opts.pod = Some(pod_netns.to_string());
        opts.pod_shared_pid = share_process_namespace;
        opts.add_host = add_host.clone();
        opts.labels.push(format!("delonix.io/pod={pod_name}"));
        opts.labels
            .push(format!("delonix.io/pod-role=app.{member}"));
        // The member's position in `spec.containers`. Nothing else carries it —
        // the container record keeps only the `<pod>-<member>` name — and
        // without it «the pod's first member» resolved to whatever `read_dir`
        // returned first (ACH-011). See `pod::POD_INDEX_LABEL`.
        opts.labels.push(format!("{}={i}", POD_INDEX_LABEL));
        out.push(opts);
    }
    Ok(out)
}

/// Canonical (delonix/docker) restart-policy keyword for a manifest value: maps
/// the k8s spellings (`Always`/`OnFailure`/`Never`) to the engine's own
/// (`always`/`on-failure`/`no`); delonix values pass through. The single source
/// of truth for this mapping — used both when lowering a Pod AND when stamping
/// `restartPolicy` for the reconciler, so the DESIRED value matches the OBSERVED
/// one. Without it, `restartPolicy: Always` kept drift red forever against the
/// stored `always` (BUG-0009).
pub fn canonical_restart_policy(value: &str) -> &str {
    match value {
        "Always" => "always",
        "OnFailure" => "on-failure",
        "Never" => "no",
        other => other,
    }
}

/// Normalizes ONE Pod container (k8s-shaped) into the flat [`RunOpts`], resolving
/// its `volumeMounts` against the pod-level `volumes`. Shared by the single
/// `kind: Container` and each member of a `kind: Pod`.
#[allow(clippy::too_many_arguments)]
pub fn container_to_run_opts(
    name: &str,
    namespace: Option<String>,
    c: PodContainer,
    pod_volumes: &[PodVolume],
    network: String,
    restart_policy: &str,
    hostname: Option<String>,
    expose: Option<u16>,
    detach: bool,
    notices: &mut Vec<Notice>,
) -> Result<RunOpts> {
    // `command` (k8s) IS the entrypoint override and `args` (k8s) ARE its
    // arguments — the same split this engine's own two fields make, so each maps
    // straight across. This used to peel `command[0]` off as the entrypoint and
    // push the remainder in front of `args`, because the entrypoint could hold
    // only one element; it composed to the same argv, but by reassembling what
    // the manifest had already separated.
    let (entrypoint, command) = if c.command.is_empty() {
        (None, c.args)
    } else {
        (Some(c.command), c.args)
    };

    // ports: publish only those declaring a hostPort (a bare containerPort is
    // informational in k8s and does not publish).
    let mut ports = Vec::new();
    for p in &c.ports {
        if let Some(hp) = p.host_port {
            let proto = p.protocol.as_deref().unwrap_or("tcp").to_lowercase();
            let base = format!("{hp}:{}/{}", p.container_port, proto);
            ports.push(match &p.host_ip {
                Some(ip) => format!("{ip}:{base}"),
                None => base,
            });
        }
    }

    let env = c
        .env
        .iter()
        .map(|e| format!("{}={}", e.name, e.value))
        .collect();

    // volumeMounts resolved against the pod volumes; emptyDir → tmpfs (ephemeral).
    let vmap: std::collections::HashMap<&str, &PodVolume> =
        pod_volumes.iter().map(|v| (v.name.as_str(), v)).collect();
    let mut volumes = Vec::new();
    let mut tmpfs = Vec::new();
    for m in &c.volume_mounts {
        let ro = if m.read_only { ":ro" } else { "" };
        let vol = vmap.get(m.name.as_str()).ok_or_else(|| {
            Error::Invalid(format!(
                "Container '{name}': volumeMount '{}' has no matching entry in spec.volumes",
                m.name
            ))
        })?;
        if let Some(hp) = &vol.host_path {
            volumes.push(format!("{}:{}{ro}", hp.path, m.mount_path));
        } else if let Some(pvc) = &vol.pvc {
            volumes.push(format!("{}:{}{ro}", pvc.claim_name, m.mount_path));
        } else if let Some(src) = &vol.source {
            volumes.push(format!("{}:{}{ro}", src, m.mount_path));
        } else if let Some(ed) = &vol.empty_dir {
            // **Here an `emptyDir` is ALWAYS tmpfs, and in k8s it is not.** There,
            // `medium` absent or `""` means node disk, and only `medium: Memory`
            // is RAM. An imported manifest that uses `emptyDir` as build or upload
            // scratch — several GiB is common — starts consuming HOST RAM, and the
            // field that expresses the difference was `#[allow(dead_code)]`:
            // accepted and thrown away.
            //
            // Warn instead of changing the behaviour, and the choice is deliberate:
            // going to disk would require a per-container directory with a lifecycle
            // of its own (when is it deleted? on `rm`? on `stop`?), which is design
            // that deserves its own session — and changing it by drift would break
            // whoever relies on today's semantics. What cannot stay is the silence.
            match ed.medium.as_deref() {
                Some("Memory") => {}
                Some("") | None => notices.push(Notice::new(
                    "warning: volume '{vol}': emptyDir without `medium: Memory` is \
                     node DISK in Kubernetes, but this engine always backs it with \
                     tmpfs (host RAM, no `size=` — up to half the RAM). Declare \
                     `medium: Memory` to say so explicitly, or use a named volume \
                     for large scratch space.",
                    &[("vol", &vol.name)],
                )),
                Some(outro) => notices.push(Notice::new(
                    "warning: volume '{vol}': unknown emptyDir medium '{medium}' \
                     (Kubernetes defines \"\" and \"Memory\"); treated as tmpfs.",
                    &[("vol", &vol.name), ("medium", outro)],
                )),
            }
            tmpfs.push(m.mount_path.clone());
        } else {
            volumes.push(format!("{}:{}{ro}", vol.name, m.mount_path));
        }
    }

    // resources.limits → memory/cpus (requests are advisory, ignored).
    let (mut memory, mut cpus) = (None, None);
    if let Some(res) = &c.resources {
        if let Some(lim) = &res.limits {
            memory = lim.memory.clone();
            cpus = lim.cpu.as_deref().map(cpu_quantity_to_cores);
        }
    }

    // securityContext → privileged/user/read_only/cap_add/cap_drop.
    let (mut privileged, mut user, mut read_only, mut cap_add, mut cap_drop) =
        (false, None, false, Vec::new(), Vec::new());
    if let Some(sc) = &c.security_context {
        privileged = sc.privileged;
        read_only = sc.read_only_root_filesystem;
        user = sc.run_as_user.map(|u| u.to_string());
        if let Some(caps) = &sc.capabilities {
            cap_add = caps.add.clone();
            cap_drop = caps.drop.clone();
        }
    }

    // restartPolicy: k8s → delonix (delonix values pass through).
    let restart = canonical_restart_policy(restart_policy).to_string();

    Ok(RunOpts {
        detach,
        name: Some(name.to_string()),
        hostname,
        user,
        net: network,
        namespace,
        expose,
        volumes,
        ports,
        privileged,
        entrypoint,
        restart,
        env,
        image: c.image,
        command,
        memory,
        cpus,
        read_only,
        cap_add,
        cap_drop,
        tmpfs,
        workdir: c.working_dir,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::canonical_restart_policy;

    /// BUG-0009: the reconciler stamps the DESIRED policy through this mapping
    /// so it matches the OBSERVED (engine-canonical) value. The k8s spellings
    /// must fold to the delonix ones, and delonix values must pass through —
    /// otherwise `restartPolicy` diffs forever against the stored value.
    #[test]
    fn restart_policy_maps_k8s_to_canonical_and_passes_delonix_through() {
        assert_eq!(canonical_restart_policy("Always"), "always");
        assert_eq!(canonical_restart_policy("OnFailure"), "on-failure");
        assert_eq!(canonical_restart_policy("Never"), "no");
        // delonix/docker spellings are already canonical.
        for p in [
            "always",
            "on-failure",
            "unless-stopped",
            "no",
            "on-failure:3",
        ] {
            assert_eq!(canonical_restart_policy(p), p, "{p} should pass through");
        }
    }
}
