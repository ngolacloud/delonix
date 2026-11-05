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
    let runtime: Arc<dyn delonix_cri::RuntimeOps> = ports;

    // Resolved through the engine's capability dictionary (the port), so a
    // malformed value REFUSES TO START — a security bound a typo could silently
    // turn into "unlimited" is the exact failure this prevents.
    let ceiling = match delonix_cri::CapCeiling::parse(&spec, &mode, runtime.as_ref()) {
        Ok(c) => c,
        Err(e) => fail(&e),
    };

    tracing::info!(%addr, root = %base.display(), "delonix-cri starting");
    if let Err(e) = delonix_cri::serve_blocking(base, images, containers, runtime, &addr, ceiling) {
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
