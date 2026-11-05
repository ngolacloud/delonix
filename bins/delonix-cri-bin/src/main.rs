//! `delonix-cri` — the CRI server's executable. The kubelet speaks to its unix
//! socket via `--container-runtime-endpoint=unix:///run/delonix-cri.sock`.
//!
//! People reach it as `delonix serve cri`, which execs THIS binary (ADR-0040
//! D2.4: one server, one executable, and `delonix` stays the only name a user
//! needs). A unit may also call it directly.
//!
//! The bin is the one layer allowed to name the assembly (ADR-0040 A5): it mounts
//! the `Engine` and hands the server its image AND container ports, so the
//! `delonix-cri` interface composes no adapter for images or for the container
//! store (R3h).
//!
//! Flags beat environment variables, which beat the defaults:
//! `--addr` / `DELONIX_CRI_ADDR`, `--cap-ceiling` / `DELONIX_CRI_CAP_CEILING`,
//! `--cap-ceiling-mode` / `DELONIX_CRI_CAP_CEILING_MODE`.

use std::path::PathBuf;
use std::sync::Arc;

use delonix_engine::Engine;
use delonix_model::Error as EngineError;

/// The composed engine behind the CRI server's ports (ADR-0040 A5, R3h): the
/// `ImageService`'s image use cases and the RuntimeService's container-store use
/// cases, served from one mounted [`Engine`]. Image records cross as the engine's
/// `{id, user, repo_tags, size}` summary (`Image` stays in the adapter); container
/// records cross as [`delonix_compute::Container`]. The CRI proto mappings are
/// `delonix-cri`'s. Wrapped once in an `Arc` and handed in as both ports.
struct EnginePorts(Engine);

impl delonix_cri::ImageOps for EnginePorts {
    fn list_images(&self) -> Result<Vec<serde_json::Value>, EngineError> {
        self.0.images().image_summaries().map(|v| match v {
            serde_json::Value::Array(a) => a,
            _ => Vec::new(),
        })
    }
    fn image_status(&self, reference: &str) -> Result<Option<serde_json::Value>, EngineError> {
        self.0.images().image_status(reference)
    }
    fn pull_image(
        &self,
        reference: &str,
        creds: Option<(String, String)>,
    ) -> Result<String, EngineError> {
        self.0.images().pull(reference, creds)
    }
    fn remove_image(&self, reference: &str) -> Result<(), EngineError> {
        self.0.images().remove(reference).map(|_| ())
    }
    fn image_fs_used_bytes(&self) -> u64 {
        self.0.images().image_fs_used_bytes()
    }
}

impl delonix_cri::ContainerOps for EnginePorts {
    fn list(&self) -> Result<Vec<delonix_compute::Container>, EngineError> {
        self.0.containers().list()
    }
    fn load(&self, id_or_name: &str) -> Result<delonix_compute::Container, EngineError> {
        self.0.containers().load(id_or_name)
    }
    fn update(
        &self,
        id_or_name: &str,
        f: &mut dyn FnMut(&mut delonix_compute::Container) -> bool,
    ) -> Result<delonix_compute::Container, EngineError> {
        // The port's `&mut dyn FnMut` forwards to the engine's generic `update`,
        // which runs it under the store's exclusive `flock` (safe read-modify-write).
        self.0.containers().update(id_or_name, |c| f(c))
    }
}

impl delonix_cri::RuntimeOps for EnginePorts {
    // The Linux-mechanism port (R3h). These are the adapter's own stateless
    // host/cgroup primitives and its capability dictionary: the bin backs them
    // with `delonix-linux` directly (a bin may name an adapter, ADR-0040 A1), so
    // the `delonix-cri` interface names no adapter type of its own.
    fn is_rootless(&self) -> bool {
        delonix_linux::is_rootless()
    }
    fn safe_to_signal(&self, pid: i32, starttime: Option<u64>) -> bool {
        delonix_linux::safe_to_signal(pid, starttime)
    }
    fn leaf_controllers(&self) -> Vec<String> {
        delonix_linux::leaf_controllers()
    }
    fn reconcile_status(&self, c: &mut delonix_compute::Container) -> bool {
        delonix_linux::reconcile_status(c)
    }
    fn oom_killed_reason(&self) -> &'static str {
        delonix_linux::OOM_KILLED
    }
    fn seccomp_validate(&self, json: &str) -> Result<(), String> {
        delonix_linux::seccomp_profile::parse(json).map(|_| ())
    }
    fn update_kube_resources(
        &self,
        c: &delonix_compute::Container,
        update: &delonix_cri::ResourceUpdateSpec<'_>,
    ) -> Result<bool, EngineError> {
        let u = delonix_linux::ResourceUpdate {
            memory: update.memory,
            cpus: update.cpus,
            cpu_weight: update.cpu_weight,
            cpuset: update.cpuset,
            cpuset_mems: update.cpuset_mems,
            hugepage_limits: update.hugepage_limits,
            unified: update.unified,
            oom_score_adj: update.oom_score_adj,
        };
        // `NotEnforced` → `false` (no live cgroup); applied/deferred → `true`. The
        // adapter's `Result` error becomes the shared `delonix_model::Error`.
        delonix_linux::update_kube_resources(c, &u)
            .map(|lu| lu != delonix_linux::LimitUpdate::NotEnforced)
            .map_err(EngineError::from)
    }
    fn all_caps_mask(&self) -> u64 {
        delonix_linux::capabilities::all_caps_mask()
    }
    fn cap_bit(&self, name: &str) -> Option<u8> {
        delonix_linux::capabilities::cap_num(name)
    }
    fn default_kept_mask(&self) -> u64 {
        delonix_linux::capabilities::default_kept_mask()
    }
    fn names_from_mask(&self, mask: u64) -> Vec<&'static str> {
        delonix_linux::capabilities::names_from_mask(mask)
    }
    fn resolve_cap_keep(&self, cap_drop: &[String], cap_add: &[String]) -> u64 {
        delonix_linux::capabilities::resolve_cap_keep(cap_drop, cap_add)
    }
}

/// A CRI [`CniPort`](delonix_cri::CniPort) → the adapter's `cni::PortMapping`.
fn to_sdn_ports(ports: &[delonix_cri::CniPort]) -> Vec<delonix_sdn::cni::PortMapping> {
    ports
        .iter()
        .map(|p| delonix_sdn::cni::PortMapping {
            host_port: p.host_port,
            container_port: p.container_port,
            protocol: p.protocol.clone(),
            host_ip: p.host_ip.clone(),
        })
        .collect()
}

/// The adapter's resolved chain → the CRI's opaque [`CniConf`](delonix_cri::CniConf).
fn conf_to_cni(conf: &delonix_sdn::cni::NetConfList) -> delonix_cri::CniConf {
    delonix_cri::CniConf {
        name: conf.name.clone(),
        publishes_host_ports: delonix_sdn::cni::publishes_host_ports(conf),
        json: serde_json::to_string(conf).unwrap_or_default(),
    }
}

impl delonix_cri::NetworkOps for EnginePorts {
    // The SDN/CNI-mechanism port (R3h-D): the bin backs it with `delonix-sdn`
    // directly (a bin may name an adapter, ADR-0040 A1), so the `delonix-cri`
    // interface names no SDN type. The CRI keeps its networking policy; only the
    // leaf mechanism crosses, with the CNI chain as opaque JSON.
    fn infra_status(&self) -> delonix_cri::InfraStatus {
        let s = delonix_sdn::infra::status();
        delonix_cri::InfraStatus {
            up: s.up,
            refcount: s.refcount,
            holder_pid: s.holder_pid,
            slirp_pid: s.slirp_pid,
        }
    }
    fn root_network_ready(&self) -> Result<(), (String, String)> {
        let dirs = delonix_sdn::cni::plugin_dirs();
        let r = delonix_sdn::cni::readiness(
            std::path::Path::new(delonix_sdn::cni::DEFAULT_CONF_DIR),
            &dirs,
        );
        match r.not_ready(&dirs) {
            None => Ok(()),
            Some((reason, message)) => Err((reason.to_string(), message)),
        }
    }
    fn attached_refs_in(&self, base: &std::path::Path) -> Vec<String> {
        delonix_sdn::infra::attached_refs_in(base)
    }
    fn orphan_ref_count(&self, attached: &[String], live: &[String]) -> usize {
        let live: std::collections::HashSet<String> = live.iter().cloned().collect();
        delonix_sdn::infra::orphan_refs(attached, &live).len()
    }
    fn cni_enabled(&self) -> bool {
        delonix_sdn::cni::enabled_conf().is_some()
    }
    fn cni_resolve_conf(&self, rootless: bool) -> delonix_cri::CniConfResult {
        if rootless {
            match delonix_sdn::cni::enabled_conf() {
                Some(conf) => delonix_cri::CniConfResult::Ready(conf_to_cni(&conf)),
                None => delonix_cri::CniConfResult::Disabled,
            }
        } else {
            let dirs = delonix_sdn::cni::plugin_dirs();
            match delonix_sdn::cni::readiness(
                std::path::Path::new(delonix_sdn::cni::DEFAULT_CONF_DIR),
                &dirs,
            ) {
                delonix_sdn::cni::Readiness::Ready(conf) => {
                    delonix_cri::CniConfResult::Ready(conf_to_cni(&conf))
                }
                other => {
                    let (reason, message) = other.not_ready(&dirs).unwrap_or_default();
                    delonix_cri::CniConfResult::NotReady {
                        reason: reason.to_string(),
                        message,
                    }
                }
            }
        }
    }
    fn cni_with_port_mappings(&self, conf_json: &str, ports: &[delonix_cri::CniPort]) -> String {
        match delonix_sdn::cni::parse_config(conf_json) {
            Ok(conf) => {
                let mapped = delonix_sdn::cni::with_port_mappings(&conf, &to_sdn_ports(ports));
                serde_json::to_string(&mapped).unwrap_or_else(|_| conf_json.to_string())
            }
            Err(_) => conf_json.to_string(),
        }
    }
    fn cni_attach(
        &self,
        pod: &str,
        rootless: bool,
        conf_json: &str,
        cni_args: Option<&str>,
    ) -> Result<delonix_cri::CniAttached, String> {
        if rootless {
            let (netns, cidr) = delonix_sdn::infra::cni_attach_container(pod, conf_json, cni_args)
                .map_err(|e| e.to_string())?;
            Ok(delonix_cri::CniAttached {
                ip: cidr,
                netns_path: netns,
            })
        } else {
            let conf = delonix_sdn::cni::parse_config(conf_json).map_err(|e| e.to_string())?;
            let cidr = delonix_sdn::cni::attach_named_netns(
                &conf,
                pod,
                pod,
                delonix_sdn::cni::DEFAULT_IFNAME,
                cni_args,
            )
            .map_err(|e| e.to_string())?;
            Ok(delonix_cri::CniAttached {
                ip: cidr,
                netns_path: delonix_sdn::cni::named_netns_path(pod),
            })
        }
    }
    fn cni_set_netns_sysctls(
        &self,
        netns_path: &str,
        sysctls: &[(String, String)],
    ) -> Result<(), String> {
        delonix_sdn::cni::set_netns_sysctls(netns_path, sysctls).map_err(|e| e.to_string())
    }
    fn cni_detach(&self, pod: &str, rootless: bool, conf_json: &str) {
        if rootless {
            if let Some(conf) = delonix_sdn::cni::enabled_conf() {
                let cj = serde_json::to_string(&conf).unwrap_or_default();
                let _ = delonix_sdn::infra::cni_detach_container(pod, &cj);
            }
        } else {
            let conf = delonix_sdn::cni::parse_config(conf_json).ok();
            delonix_sdn::cni::detach_named_netns(
                conf.as_ref(),
                pod,
                pod,
                delonix_sdn::cni::DEFAULT_IFNAME,
            );
        }
    }
    fn container_ip(&self, name: &str) -> String {
        delonix_sdn::infra::container_ip(name)
    }
    fn publishable_proto(&self, name: &str) -> Option<&'static str> {
        match delonix_sdn::Proto::parse(name).ok()? {
            delonix_sdn::Proto::Tcp => Some("tcp"),
            delonix_sdn::Proto::Udp => Some("udp"),
        }
    }
}

/// Ends the process with `code`, saying why on stderr and in the log. The one
/// place this binary writes to the terminal.
fn exit_with(code: i32, message: &str) -> ! {
    tracing::error!(error = %message, code, "delonix-cri: exiting");
    eprintln!("delonix-cri: {message}");
    std::process::exit(code);
}

/// Refuses to start: a bad argument, ceiling or version.
fn fail(message: &str) -> ! {
    exit_with(2, message)
}

/// The value of `--name <v>` or `--name=<v>` in `args`, if given.
fn flag(args: &[String], name: &str) -> Option<String> {
    let long = format!("--{name}");
    let eq = format!("--{name}=");
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if *a == long {
            return it.next().cloned();
        }
        if let Some(v) = a.strip_prefix(&eq) {
            return Some(v.to_string());
        }
    }
    None
}

/// True when `args` asks for the version: `--version` or `-V`, alone.
fn wants_version(args: &[String]) -> bool {
    matches!(args, [a] if a == "--version" || a == "-V")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Answered before telemetry, the release check and any socket: asking which
    // release this binary is must work even for one `check_version` would refuse.
    if wants_version(&args) {
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), "delonix-cri {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    delonix_telemetry::telemetry::init();
    if let Err(e) = delonix_host::dispatch::check_version("delonix-cri", env!("CARGO_PKG_VERSION"))
    {
        fail(&e);
    }
    for a in &args {
        let name = a.split('=').next().unwrap_or(a);
        if a.starts_with("--") && !matches!(name, "--addr" | "--cap-ceiling" | "--cap-ceiling-mode")
        {
            fail(&format!("unknown argument: {a}"));
        }
    }
    let base = std::env::var_os("DELONIX_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/delonix"));
    let addr = flag(&args, "addr")
        .or_else(|| std::env::var("DELONIX_CRI_ADDR").ok())
        .unwrap_or_else(|| "unix:///run/delonix-cri.sock".to_string());
    // Node capability ceiling. A malformed value REFUSES TO START, before the
    // socket is bound: this is a security bound, and a typo that fell back to
    // "unlimited" would be the exact silent failure it exists to prevent.
    let spec = flag(&args, "cap-ceiling")
        .or_else(|| std::env::var(delonix_cri::cap_ceiling::CEILING_ENV).ok())
        .unwrap_or_default();
    let mode = flag(&args, "cap-ceiling-mode")
        .or_else(|| std::env::var(delonix_cri::cap_ceiling::MODE_ENV).ok())
        .unwrap_or_default();
    // The bin mounts the composed engine ONCE and hands the server all three
    // ports from the same `Arc`: the image port, the container port and the
    // Linux-mechanism port (R3h).
    let engine = match Engine::mount(base.clone()) {
        Ok(e) => e,
        Err(e) => fail(&format!("mount engine: {e}")),
    };
    let ports = Arc::new(EnginePorts(engine));
    let images: Arc<dyn delonix_cri::ImageOps> = ports.clone();
    let containers: Arc<dyn delonix_cri::ContainerOps> = ports.clone();
    let runtime: Arc<dyn delonix_cri::RuntimeOps> = ports.clone();
    let network: Arc<dyn delonix_cri::NetworkOps> = ports;

    // Resolved through the engine's capability dictionary (the port), so a
    // malformed value REFUSES TO START — a security bound a typo could silently
    // turn into "unlimited" is the exact failure this prevents.
    let ceiling = match delonix_cri::CapCeiling::parse(&spec, &mode, runtime.as_ref()) {
        Ok(c) => c,
        Err(e) => fail(&e),
    };

    tracing::info!(%addr, root = %base.display(), "delonix-cri starting");
    if let Err(e) =
        delonix_cri::serve_blocking(base, images, containers, runtime, network, &addr, ceiling)
    {
        exit_with(1, &e.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::wants_version;

    fn v(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn version_flag_is_recognised_only_alone() {
        assert!(wants_version(&v(&["--version"])));
        assert!(wants_version(&v(&["-V"])));
        assert!(!wants_version(&v(&[])));
        assert!(!wants_version(&v(&["--addr", "--version"])));
        assert!(!wants_version(&v(&["--versions"])));
    }
}
