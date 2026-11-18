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

use super::container::{cmd_run, RunOpts};
use super::util::state_root;
use delonix_model::{exitcode, Result};
use delonix_oci::ImageStore;
use delonix_state::Store;
use delonix_warm::{root_env, serve_parked, StartOutcome, WarmPool};
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
    let run = |spec: &Path| -> StartOutcome {
        match run_spec_resident(&images, &store, spec) {
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

/// Entry for `delonix __poolbench <image> <runs> <size>`. Never returns.
pub fn poolbench(image: &str, runs: usize, size: usize) -> ! {
    std::process::exit(match run_poolbench(image, runs, size) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("delonix: poolbench: {}", super::po::t_dyn(&e.to_string()));
            1
        }
    });
}

fn run_poolbench(image: &str, runs: usize, size: usize) -> Result<()> {
    let root = state_root();
    let self_exe = std::env::current_exe()?;
    let specdir = root.join("warmbench");
    std::fs::create_dir_all(&specdir)?;

    // One spec template: `<image> true`, detached + host-net + quiet, so the
    // supervised path returns right after the fork handshake (no container wait,
    // no `-d` id-print, no 400 ms death-at-birth delay) — the fairest wall for
    // comparing pre_spawn across the two ways of reaching the SAME `spawn`.
    let mk_opts = || RunOpts {
        image: image.to_string(),
        command: vec!["true".to_string()],
        net: "host".to_string(),
        detach: true,
        quiet: true,
        ..Default::default()
    };

    eprintln!("== bench == image={image} runs={runs} pool-size={size}");

    // COLD: a fresh `__apirun` child per run — what the CRI/API do today.
    eprintln!("== cold (fresh __apirun per run) ==");
    for i in 0..runs {
        let spec = write_spec(&specdir, i, &mk_opts())?;
        let t0 = Instant::now();
        let status = Command::new(&self_exe)
            .arg("__apirun")
            .arg(&spec)
            .env("DELONIX_ROOT", &root)
            .status()?;
        let wall = t0.elapsed();
        // `__apirun` removes the spec itself; only fall back if it somehow did not.
        let _ = std::fs::remove_file(&spec);
        if !status.success() {
            eprintln!("  cold run {i}: exited {status}");
        }
        append_bench("cold", wall.as_micros());
    }

    // WARM: a pool of parked, pre-warmed runners; each start reuses one.
    eprintln!("== warm (pooled __poolrunner, image pre-resolved) ==");
    let env = vec![root_env(&root)];
    let mut pool = WarmPool::new(&self_exe, image, env, size)?;
    for i in 0..runs {
        let spec = write_spec(&specdir, runs + i, &mk_opts())?;
        let t0 = Instant::now();
        let outcome = pool.start(&spec)?;
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
        let _ = writeln!(f, "{{\"role\":\"bench\",\"phase\":\"{phase}\",\"wall_us\":{wall_us}}}");
    }
}
