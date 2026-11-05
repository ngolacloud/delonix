//! `delonix-mgmt` — the LOCAL management API (HTTP+JSON on a unix socket, the
//! calling uid only). `delonix serve api` runs this binary.
//!
//! `--addr` beats `DELONIX_API_ADDR`, which beats `unix:///run/delonix-mgmt.sock`.
//! The state root is `DELONIX_ROOT` (set by `delonix`), else `/var/lib/delonix`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use delonix_compute::Vm;
use delonix_engine::Engine;
use delonix_model::Error as EngineError;
use serde_json::Value;

/// The composed engine, handed to the management API as its port(s) (ADR-0040
/// A5). The bin is the one place that names `delonix-engine`; the server receives
/// the engine already mounted and never learns what composes it, so no
/// `interface -> adapter` edge survives. VMs arrive as [`delonix_mgmt::VmView`];
/// the image/volume/container/network use cases as [`delonix_mgmt::EngineOps`]
/// (R3f). A value typed only inside an adapter is serialized here, so the port
/// speaks the `serde_json::Value`s the server serves; native errors map to the
/// shared engine error the interface already understands.
struct EnginePorts(Engine);

impl delonix_mgmt::VmView for EnginePorts {
    fn list(&self) -> Result<Vec<Vm>, EngineError> {
        self.0.vms().list().map_err(Into::into)
    }

    fn status(&self, name: &str) -> Result<Vm, EngineError> {
        self.0.vms().status(name).map_err(Into::into)
    }
}

impl delonix_mgmt::EngineOps for EnginePorts {
    fn list_images(&self) -> Result<Value, EngineError> {
        self.0
            .images()
            .list()
            .map(|imgs| serde_json::to_value(imgs).unwrap_or(Value::Null))
    }

    fn remove_image(&self, reference: &str) -> Result<String, EngineError> {
        self.0.images().remove(reference)
    }

    fn image_sbom(&self, reference: &str) -> Result<Value, EngineError> {
        self.0.images().sbom(reference)
    }
}

/// The `--version` line: binary name and the crate version, nothing else.
fn version_line() -> String {
    format!("delonix-mgmt {}", env!("CARGO_PKG_VERSION"))
}

/// True when the first argument asks for the version. Answered before any
/// effect, including the dispatch version handshake.
fn wants_version(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("--version" | "-V"))
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    delonix_host::dispatch::check_version("delonix-mgmt", env!("CARGO_PKG_VERSION"))?;
    let mut addr = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--addr" {
            addr = Some(it.next().cloned().ok_or("--addr needs a value")?);
        } else if let Some(v) = a.strip_prefix("--addr=") {
            addr = Some(v.to_string());
        } else {
            return Err(format!("unknown argument: {a}"));
        }
    }
    let addr = addr
        .or_else(|| std::env::var("DELONIX_API_ADDR").ok())
        .unwrap_or_else(|| "unix:///run/delonix-mgmt.sock".to_string());
    let base = std::env::var_os("DELONIX_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/delonix"));
    let engine = Engine::mount(base.clone()).map_err(|e| format!("mount engine: {e}"))?;
    let ports = Arc::new(EnginePorts(engine));
    let vms: Arc<dyn delonix_mgmt::VmView> = ports.clone();
    let engine_ops: Arc<dyn delonix_mgmt::EngineOps> = ports;
    delonix_mgmt::serve_blocking(base, vms, engine_ops, &addr).map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    // `--version` must not initialise telemetry (no exporter, no side effects).
    if wants_version(&std::env::args().skip(1).collect::<Vec<_>>()) {
        println!("{}", version_line());
        return ExitCode::SUCCESS;
    }
    delonix_telemetry::telemetry::init();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("delonix-mgmt: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn version_flags_are_recognised() {
        assert!(wants_version(&args(&["--version"])));
        assert!(wants_version(&args(&["-V"])));
        assert!(!wants_version(&args(&[])));
        assert!(!wants_version(&args(&["--addr", "--version"])));
    }

    #[test]
    fn version_line_carries_the_crate_version() {
        assert_eq!(
            version_line(),
            format!("delonix-mgmt {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
