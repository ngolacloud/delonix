//! `delonix-mcp` — the Model Context Protocol server (ADR-0025): a LOCAL,
//! tenancy-free AI control surface. `delonix mcp <verb>` runs this binary.
//!
//! Verbs: `serve [--transport stdio]` (the default transport, a child process of
//! the AI client for one session, not a daemon), `doctor`, `capabilities`.

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use delonix_compute::{Container, Vm};
use delonix_engine::Engine;
use delonix_model::Error as EngineError;
use serde_json::Value;

/// The composed engine, handed to the MCP server as its port(s) (ADR-0040 A5).
/// The bin is the one place that names `delonix-engine`; the server receives the
/// engine already mounted and never learns what composes it. VMs arrive as
/// [`delonix_mcp::VmView`]; containers/volumes/network/host-resources as
/// [`delonix_mcp::EngineReads`] (R3e). A value typed only inside an adapter (a
/// usage sample, the SDN status) is serialized here, so the port speaks the
/// `serde_json::Value`s the server serves; native errors are mapped to the shared
/// engine error the interface already understands.
struct EnginePorts(Engine);

impl delonix_mcp::VmView for EnginePorts {
    fn list(&self) -> Result<Vec<Vm>, EngineError> {
        self.0.vms().list().map_err(Into::into)
    }

    fn status(&self, name: &str) -> Result<Vm, EngineError> {
        self.0.vms().status(name).map_err(Into::into)
    }
}

impl delonix_mcp::EngineReads for EnginePorts {
    fn list_containers(&self) -> Result<Vec<Container>, EngineError> {
        self.0.containers().list()
    }

    fn load_container(&self, id_or_name: &str) -> Result<Container, EngineError> {
        self.0.containers().load(id_or_name)
    }

    fn container_store_reachable(&self) -> Result<(), EngineError> {
        self.0.containers().store_reachable()
    }

    fn reconcile_status(&self, c: &mut Container) {
        self.0.containers().reconcile_status(c);
    }

    fn live_cgroup(&self, c: &Container) -> String {
        self.0.containers().live_cgroup(c)
    }

    fn cgroup_sample(&self, cgroup: &Path) -> Value {
        serde_json::to_value(self.0.containers().cgroup_sample(cgroup)).unwrap_or(Value::Null)
    }

    fn process_sample(&self, pid: i32) -> Value {
        serde_json::to_value(self.0.containers().process_sample(pid)).unwrap_or(Value::Null)
    }

    fn process_started_at_unix(&self, pid: i32) -> Option<u64> {
        self.0.containers().process_started_at_unix(pid)
    }

    fn list_volumes(&self) -> Result<Vec<Value>, EngineError> {
        self.0.volumes().list().map(|vs| {
            vs.into_iter()
                .map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
                .collect()
        })
    }

    fn volume_store_reachable(&self) -> Result<(), EngineError> {
        self.0.volumes().store_reachable()
    }

    fn network_status(&self) -> Value {
        serde_json::to_value(self.0.networks().status()).unwrap_or(Value::Null)
    }

    fn container_net_bytes(&self, id: &str) -> Option<(u64, u64)> {
        self.0.networks().container_net_bytes(id)
    }

    fn host_resources(&self) -> Value {
        self.0.host_resources()
    }

    fn dash_summary(&self) -> Value {
        // Cheap: no per-container netns reads, no disk walk (the two false flags).
        serde_json::to_value(self.0.dash_summary(false, false)).unwrap_or(Value::Null)
    }
}

fn usage() -> String {
    "usage: delonix-mcp <serve [--transport stdio] | doctor | capabilities>".to_string()
}

fn run(args: &[String]) -> Result<(), String> {
    delonix_host::dispatch::check_version("delonix-mcp", env!("CARGO_PKG_VERSION"))?;
    match args.first().map(String::as_str) {
        Some("serve") => {
            let mut transport = "stdio".to_string();
            let mut rest = args[1..].iter();
            while let Some(a) = rest.next() {
                if a == "--transport" {
                    transport = rest.next().cloned().ok_or_else(usage)?;
                } else if let Some(v) = a.strip_prefix("--transport=") {
                    transport = v.to_string();
                } else {
                    return Err(format!("unknown argument: {a}"));
                }
            }
            if transport != "stdio" {
                return Err(format!(
                    "unsupported MCP transport '{transport}' — only 'stdio' is implemented \
                     (ADR-0025 defers loopback HTTP)"
                ));
            }
            let base = delonix_mcp::state_root();
            let engine = Engine::mount(base.clone()).map_err(|e| format!("mount engine: {e}"))?;
            let ports = Arc::new(EnginePorts(engine));
            let vms: Arc<dyn delonix_mcp::VmView> = ports.clone();
            let reads: Arc<dyn delonix_mcp::EngineReads> = ports;
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("mcp runtime: {e}"))?;
            rt.block_on(delonix_mcp::serve_stdio(base, vms, reads))
        }
        Some("doctor") => {
            let base = delonix_mcp::state_root();
            let engine = Engine::mount(base.clone()).map_err(|e| format!("mount engine: {e}"))?;
            let ports = EnginePorts(engine);
            let checks = delonix_mcp::doctor_checks(&base, &ports);
            let mut report = String::new();
            let mut all_ok = true;
            for (name, ok, detail) in &checks {
                all_ok &= ok;
                report.push_str(&format!(
                    "{} {name}: {detail}\n",
                    if *ok { "✓" } else { "✗" }
                ));
            }
            print!("{report}");
            if all_ok {
                Ok(())
            } else {
                Err("one or more MCP doctor checks failed (see above)".to_string())
            }
        }
        Some("capabilities") => {
            let table = delonix_mcp::capabilities_table();
            println!(
                "{}",
                serde_json::to_string_pretty(&table).unwrap_or_default()
            );
            Ok(())
        }
        _ => Err(usage()),
    }
}

/// True when `args` asks for the version: `--version` or `-V`, alone.
fn wants_version(args: &[String]) -> bool {
    matches!(args, [a] if a == "--version" || a == "-V")
}

fn main() -> ExitCode {
    // Answered before telemetry and the release check: asking which release this
    // binary is must work even for one `check_version` would refuse.
    if wants_version(&std::env::args().skip(1).collect::<Vec<_>>()) {
        println!("delonix-mcp {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    delonix_telemetry::telemetry::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("delonix-mcp: {e}");
            ExitCode::FAILURE
        }
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
        assert!(!wants_version(&v(&["serve", "--version"])));
    }
}
