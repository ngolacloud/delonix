//! `delonix-cri` — the CRI server's executable. The kubelet speaks to its unix
//! socket via `--container-runtime-endpoint=unix:///run/delonix-cri.sock`.
//!
//! People reach it as `delonix serve cri`, which execs THIS binary (ADR-0040
//! D2.4: one server, one executable, and `delonix` stays the only name a user
//! needs). A unit may also call it directly.
//!
//! The bin is the one layer allowed to name the assembly (ADR-0040 A5): it mounts
//! the `Engine` and hands the server its image port, so the `delonix-cri`
//! interface composes no adapter for images (R3h).
//!
//! Flags beat environment variables, which beat the defaults:
//! `--addr` / `DELONIX_CRI_ADDR`, `--cap-ceiling` / `DELONIX_CRI_CAP_CEILING`,
//! `--cap-ceiling-mode` / `DELONIX_CRI_CAP_CEILING_MODE`.

use std::path::PathBuf;
use std::sync::Arc;

use delonix_engine::Engine;
use delonix_model::Error as EngineError;

/// The composed engine as the CRI `ImageService`'s port (ADR-0040 A5, R3h).
/// Image records cross as the engine's `{id, user, repo_tags, size}` summary
/// (`Image` stays in the adapter); the CRI proto mapping is `delonix-cri`'s.
struct EngineImages(Engine);

impl delonix_cri::ImageOps for EngineImages {
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
    let ceiling = match delonix_cri::CapCeiling::parse(&spec, &mode) {
        Ok(c) => c,
        Err(e) => fail(&e),
    };

    // The bin mounts the composed engine and hands the server its image port.
    let engine = match Engine::mount(base.clone()) {
        Ok(e) => e,
        Err(e) => fail(&format!("mount engine: {e}")),
    };
    let images: Arc<dyn delonix_cri::ImageOps> = Arc::new(EngineImages(engine));

    tracing::info!(%addr, root = %base.display(), "delonix-cri starting");
    if let Err(e) = delonix_cri::serve_blocking(base, images, &addr, ceiling) {
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
