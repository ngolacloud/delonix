//! The warm-plane pool's two ends inside the `delonix` binary (ADR-0095).
//!
//! - [`poolrunner`] — a **parked warm runner** (`delonix __poolrunner <fd> [hint]`):
//!   opens the engine stores ONCE, pre-warms the hinted image, announces READY,
//!   then serves exactly one run spec with its warm state and exits. It is the
//!   single-threaded process the supervisor's `fork` needs, exactly as the cold
//!   `__apirun` re-exec was — only kept alive and pre-warmed, so the start it
//!   serves pays neither the 43 MB re-load nor the cold image/rootfs resolution.
//!
//! - [`poolbench`] — the **measurement bench** (`delonix __poolbench <image>
//!   <runs> <size>`): a persistent process that starts identical containers two
//!   ways — cold (a fresh `__apirun` child per run, what the CRI/API do today)
//!   and warm (handed to a parked runner from a [`WarmPool`]) — timing each and
//!   emitting JSONL to `DELONIX_PROFILE_SPAWN` so the phase profiler's internal
//!   `total_us` (the real spawn mechanics) can be subtracted from the wall to
//!   expose the `pre_spawn` the warm plane eliminates. This is the persistent
//!   path the done-criterion measures, exercising the SAME [`WarmPool`] the CRI
//!   wiring uses.
//!
//! Neither path is reachable from the public CLI (`__`-prefixed, intercepted
//! before clap, like `__apirun`/`__rmtree`): the cold `delonix container run`
//! stays byte-for-byte unchanged, and the pool is opt-in on a live process only
//! (ADR-0040 daemonless, ADR-0095).

use super::container::{cmd_run, run_spec_resident_isolated, RunOpts};
use super::util::state_root;
use delonix_model::{exitcode, Result};
use delonix_oci::ImageStore;
use delonix_state::Store;
use delonix_warm::{root_env, serve_parked, ContainerPlane, StartOutcome, StartRequest, WarmPool};
use std::ffi::OsString;
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Runner (`__poolrunner <fd> [hint]`)
// ---------------------------------------------------------------------------

/// Entry for `delonix __poolrunner <fd> [hint]`. Never returns.
pub fn poolrunner(fd: RawFd, hint: &str) -> ! {
    std::process::exit(run_poolrunner(fd, hint));
}

fn run_poolrunner(fd: RawFd, hint: &str) -> i32 {
    let root = state_root();
    let images = match ImageStore::open(&root) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("delonix: poolrunner: {}", super::po::t_dyn(&e.to_string()));
            return 1;
        }
    };
    // The `containers` subdir, exactly as `__apirun` opens it (`run_from_spec_file_inner`).
    let store = match Store::open(root.join("containers")) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("delonix: poolrunner: {}", super::po::t_dyn(&e.to_string()));
            return 1;
        }
    };

    // Pre-warm: resolve the hinted image so its record read and config parse are
    // done, and its layers are warm in the page cache, before this runner is
    // declared READY. Errors are ignored — an unknown hint just means the start
    // pays its own resolve, exactly as the cold path would.
    let warm = || {
        if !hint.is_empty() {
            let _ = images.resolve(hint);
        }
    };
    let run = |req: &StartRequest| -> StartOutcome {
        // Host-net start → the engine directly, resident stores (as before).
        // Isolated start → `setns` into the holder-built netns and run the SAME
        // second-pass engine path in-process, never re-execing a cold child.
        let result = match &req.plane {
            None => run_spec_resident(&images, &store, &req.spec),
            Some(p) => {
                run_spec_resident_isolated(&images, &store, &req.spec, &p.id, &p.netns, &p.ip)
            }
        };
        match result {
            Ok(()) => StartOutcome {
                code: 0,
                reason: None,
            },
            Err(e) => StartOutcome {
                code: exitcode::for_error(&e),
                reason: Some(super::po::t_dyn(&e.to_string())),
            },
        }
    };

    match serve_parked(fd, warm, run) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("delonix: poolrunner: {e}");
            1
        }
    }
}

/// The engine work for one spec, reusing the runner's resident stores — the SAME
/// body as `dockerapi::run_from_spec_file_inner`, minus the per-call
/// `ImageStore::open`/`Store::open` the runner has already paid.
fn run_spec_resident(images: &ImageStore, store: &Store, spec: &Path) -> Result<()> {
    let data = std::fs::read(spec)?;
    let opts: RunOpts = serde_json::from_slice(&data)?;
    cmd_run(images, store, opts)
}

// ---------------------------------------------------------------------------
// Bench (`__poolbench <image> <runs> <size>`)
// ---------------------------------------------------------------------------

/// Entry for `delonix __poolbench <image> <runs> <size> [net]`. Never returns.
///
/// `net` is the network the benched containers run on: `host` (the default,
/// line 4a of the profile — no per-container network plane) or a custom network
/// name (line 4b, the ISOLATED path whose +322 ms `pre_spawn` the warm plane is
/// aimed at). The caller creates the network and warms the holder beforehand
/// (`scripts/warm_profile.sh --net <name>`), so the bench measures the start
/// cost, not the one-time plane bring-up.
pub fn poolbench(image: &str, runs: usize, size: usize, net: &str) -> ! {
    std::process::exit(match run_poolbench(image, runs, size, net) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("delonix: poolbench: {}", super::po::t_dyn(&e.to_string()));
            1
        }
    });
}

fn run_poolbench(image: &str, runs: usize, size: usize, net: &str) -> Result<()> {
    let root = state_root();
    let self_exe = std::env::current_exe()?;
    let specdir = root.join("warmbench");
    std::fs::create_dir_all(&specdir)?;

    // The network runtime dir (control/slirp sockets) must reach every child —
    // the cold `__apirun`, the pooled runner, and the runner's own re-exec — or
    // an isolated run resolves a different holder than the one the caller warmed
    // and brings a cold plane up mid-measurement. Inherited on every path so the
    // two ways reach the SAME holder; a no-op for host-net.
    let (net_rt_var, net_rt_dir) = delonix_sdn::infra::runtime_dir_env();
    let isolated = net != "host" && net != "none" && !net.is_empty();

    // One spec template: `<image> true`, detached + quiet, so the supervised
    // path returns right after the fork handshake (no container wait, no `-d`
    // id-print, no 400 ms death-at-birth delay) — the fairest wall for comparing
    // pre_spawn across the two ways of reaching the SAME `spawn`. `net` is the
    // one knob that moves it between the host (4a) and isolated (4b) lines.
    let mk_opts = || RunOpts {
        image: image.to_string(),
        command: vec!["true".to_string()],
        net: net.to_string(),
        detach: true,
        quiet: true,
        ..Default::default()
    };

    eprintln!("== bench == image={image} runs={runs} pool-size={size} net={net}");

    // COLD: a fresh `__apirun` child per run — what the CRI/API do today.
    eprintln!("== cold (fresh __apirun per run) ==");
    for i in 0..runs {
        let spec = write_spec(&specdir, i, &mk_opts())?;
        let t0 = Instant::now();
        let status = Command::new(&self_exe)
            .arg("__apirun")
            .arg(&spec)
            .env("DELONIX_ROOT", &root)
            .env(net_rt_var, &net_rt_dir)
            .status()?;
        let wall = t0.elapsed();
        // `__apirun` removes the spec itself; only fall back if it somehow did not.
        let _ = std::fs::remove_file(&spec);
        if !status.success() {
            eprintln!("  cold run {i}: exited {status}");
        }
        append_bench("cold", wall.as_micros());
    }

    // WARM: a pool of parked, pre-warmed runners; each start reuses one. On the
    // ISOLATED path the runners are parked INSIDE the holder (an `nsenter` prefix)
    // and the host loop runs `attach_container` before each start, so the runner
    // only `setns`'s into the holder-built netns and launches in-process — it
    // never re-execs the cold `netns run` child the cold path above pays for.
    eprintln!("== warm (pooled __poolrunner, image pre-resolved) ==");
    let env = vec![
        root_env(&root),
        (net_rt_var.into(), net_rt_dir.clone().into_os_string()),
    ];
    let prefix = if isolated {
        isolated_runner_prefix()?
    } else {
        Vec::new()
    };
    let mut pool = WarmPool::with_prefix(&self_exe, image, env, size, prefix)?;
    for i in 0..runs {
        let spec = write_spec(&specdir, runs + i, &mk_opts())?;
        // Isolated: wire the per-container plane on the HOST (IPAM lease + the
        // holder's veth/netns via the control socket), exactly as the cold 1st
        // pass does, then hand the runner only what it needs to `setns` in.
        let request = if isolated {
            let id = delonix_node::generate_id();
            let (netns, ip) = delonix_sdn::infra::attach_container(&id, net, "")?;
            StartRequest::isolated(&spec, ContainerPlane { id, netns, ip })
        } else {
            StartRequest::host(&spec)
        };
        let t0 = Instant::now();
        let outcome = pool.start(&request)?;
        let wall = t0.elapsed();
        let _ = std::fs::remove_file(&spec);
        if !outcome.ok() {
            eprintln!(
                "  warm run {i}: code {} {}",
                outcome.code,
                outcome.reason.as_deref().unwrap_or("")
            );
        }
        append_bench("warm", wall.as_micros());
        // Off the hot path (not timed): top the pool back up for the next run.
        pool.refill()?;
    }

    eprintln!("== done; records in DELONIX_PROFILE_SPAWN ==");
    Ok(())
}

/// The `nsenter` argv that parks a warm runner INSIDE the running holder's
/// userns+mountns+netns (so it can `setns` into any per-container netns the
/// holder builds). The holder must already be up — the cold bench loop brings it
/// up on its first isolated attach, and `scripts/warm_profile.sh --net` warms it
/// before the bench. Mirrors `infra::join_argv` minus its trailing
/// `ip netns exec <netns>` (the runner enters the container netns per start).
fn isolated_runner_prefix() -> Result<Vec<OsString>> {
    let pin =
        delonix_sdn::infra::status()
            .holder_pid
            .ok_or_else(|| delonix_model::Error::Runtime {
                context: "warm isolated",
                message: super::po::t(
                    "no network holder is up to park warm runners in — create the network \
                 and start one isolated container first",
                )
                .into(),
            })?;
    Ok([
        "nsenter",
        "-t",
        &pin.to_string(),
        "-U",
        "-m",
        "-n",
        "--preserve-credentials",
        "--",
    ]
    .iter()
    .map(OsString::from)
    .collect())
}

/// Write one run spec (`0600`) and return its path.
fn write_spec(dir: &Path, idx: usize, opts: &RunOpts) -> Result<PathBuf> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let path = dir.join(format!("spec-{}-{idx}.json", std::process::id()));
    let f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&path)?;
    serde_json::to_writer(f, opts)?;
    Ok(path)
}

/// Append one bench wall record to the `DELONIX_PROFILE_SPAWN` JSONL file,
/// alongside the profiler's own `{"role":"parent",...}` lines. A no-op when the
/// env var is a flag (`1`/`true`/…) rather than a path — same rule the profiler's
/// `append_jsonl` uses, so the two share one sink without colliding.
fn append_bench(phase: &str, wall_us: u128) {
    let Some(val) = std::env::var_os("DELONIX_PROFILE_SPAWN") else {
        return;
    };
    let s = val.to_string_lossy();
    if matches!(s.as_ref(), "1" | "true" | "yes" | "on" | "") {
        return;
    }
    use std::io::Write as _;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(s.as_ref())
    {
        let _ = writeln!(
            f,
            "{{\"role\":\"bench\",\"phase\":\"{phase}\",\"wall_us\":{wall_us}}}"
        );
    }
}
