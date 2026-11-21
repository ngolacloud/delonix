//! The CRI server's warm-plane port (ADR-0095, `delonix-warm`).
//!
//! The bin is the one layer allowed to name both adapters (ADR-0040 A1), so the
//! `delonix-cri` interface names only its [`WarmOps`](delonix_cri::WarmOps) port
//! while the real plane here composes `delonix-warm` (the runner orchestrator)
//! with `delonix-sdn` (the holder PID, the per-container netns name, the pod IP,
//! and the ref-count that keeps the netns alive under parked runners).
//!
//! It serves ONLY the isolated native-SDN path — a rootless pod member whose
//! shared netns the holder built — which is where the `+322 ms` cold `netns run`
//! child lives (see `crates/adapters/delonix-warm/PER-IMAGE-CRI-WARMING.md`).
//! Host-net, CNI and root starts never reach here; they keep the cold path.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use delonix_cri::WarmOps;
use delonix_warm::{root_env, spawn_from_recipe, ContainerPlane, StartRequest, WarmPlane};

/// The plane the hot path and the background refiller share. The heavy work
/// (spawn/warm, serve) runs with this lock RELEASED; the lock is taken only for
/// O(1) bookkeeping (take a runner, snapshot a recipe, deposit a runner).
type SharedPlane = Arc<Mutex<Option<WarmPlane>>>;

/// A request to the background refiller: top `image`'s sub-pool back up to its
/// target size. Coalesces naturally — a redundant message finds the pool already
/// full (or gone) and is a no-op.
enum RefillMsg {
    Topup(String),
}

/// The ref-count marker the plane holds for its whole life. The holder is
/// ref-counted (`acquire`/`release`) and the LAST `release` tears the infra down
/// (`infra::release`); a plane that parks runners in the holder's netns must keep
/// one ref, or a last-container release would pull the netns out from under them.
/// Distinct from any container id (double underscore, not hex), and re-acquired on
/// every `seed` so a `system prune` sweep cannot reap it while the plane is active.
const WARM_REF_ID: &str = "__warmplane";

/// The real [`WarmOps`]: a per-image [`WarmPlane`] rebuilt against the live
/// holder, consulted on every isolated start.
pub struct CriWarmPlane {
    /// The server's state root — the runners inherit it as `DELONIX_ROOT` so they
    /// open the SAME stores the cold `__apirun` would.
    base: PathBuf,
    /// Target parked runners per hot image (`DELONIX_CRI_WARM_SIZE`, default 1).
    size_per_image: usize,
    /// Ceiling on parked runners node-wide (`DELONIX_CRI_WARM_CAP`, default 8) —
    /// the memory lever, one resident process per parked runner.
    total_cap: usize,
    /// The plane, rebuilt when the holder is first seen and whenever its pin
    /// restarts. `None` until the holder is up (the first SDN start is cold and
    /// brings it up; every start after that can warm). Shared (`Arc`) with the
    /// background refiller, which spawns OFF this lock.
    plane: SharedPlane,
    /// Sender to the background refiller. `None` after [`Drop`] takes it to close
    /// the channel and join the thread.
    refill_tx: Option<Sender<RefillMsg>>,
    /// The background refiller thread. Joined on [`Drop`] (after the channel is
    /// closed) so no refill runs while the plane is being torn down.
    refiller: Option<JoinHandle<()>>,
}

impl CriWarmPlane {
    /// Build the plane from the environment, or `None` when warming is not opted
    /// in (`DELONIX_CRI_WARM` unset/false) — the bin then hands the server
    /// [`delonix_cri::NoWarm`] and every start takes the cold path, exactly as a
    /// server without this change.
    pub fn from_env(base: PathBuf) -> Option<Self> {
        let on = std::env::var("DELONIX_CRI_WARM")
            .map(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(false);
        if !on {
            return None;
        }
        let size_per_image = env_usize("DELONIX_CRI_WARM_SIZE", 1).max(1);
        let total_cap = env_usize("DELONIX_CRI_WARM_CAP", 8).max(size_per_image);
        let plane: SharedPlane = Arc::new(Mutex::new(None));
        let (tx, rx) = std::sync::mpsc::channel::<RefillMsg>();
        let refiller = spawn_refiller(plane.clone(), rx);
        Some(Self {
            base,
            size_per_image,
            total_cap,
            plane,
            refill_tx: Some(tx),
            refiller: Some(refiller),
        })
    }

    /// Ask the background refiller to top `image`'s sub-pool back up. Non-blocking
    /// (the channel is unbounded); a send failure (refiller gone, only during
    /// shutdown) is ignored — the next start serves cold and re-asks.
    fn ask_refill(&self, image: &str) {
        if let Some(tx) = &self.refill_tx {
            let _ = tx.send(RefillMsg::Topup(image.to_string()));
        }
    }

    /// Ensure the plane matches the LIVE holder, building it on first use (and
    /// taking the holder ref then) and reconciling a pin restart. Returns `None`
    /// when the holder is down — the caller then serves cold, and the first SDN
    /// start brings the holder up so the next one can warm.
    fn ensure<'a>(&self, guard: &'a mut Option<WarmPlane>) -> Option<&'a mut WarmPlane> {
        let pin = delonix_sdn::infra::status().holder_pid?;
        match guard {
            Some(plane) => {
                // A restarted pin means every parked runner is in a dead netns;
                // `reconcile_holder` drops them and adopts the new prefix.
                plane.reconcile_holder(pin, isolated_prefix(pin));
            }
            None => {
                // First use: take the holder ref BEFORE parking any runner, so a
                // concurrent last-container release cannot tear the netns out.
                if delonix_sdn::infra::acquire(WARM_REF_ID).is_err() {
                    return None;
                }
                let (rt_k, rt_v) = delonix_sdn::infra::runtime_dir_env();
                let env = vec![
                    root_env(&self.base),
                    (OsString::from(rt_k), rt_v.into_os_string()),
                ];
                *guard = Some(WarmPlane::new(
                    delonix_host::dispatch::cli_bin(),
                    env,
                    isolated_prefix(pin),
                    pin,
                    self.size_per_image,
                    self.total_cap,
                ));
            }
        }
        guard.as_mut()
    }
}

impl WarmOps for CriWarmPlane {
    fn try_start_warm(
        &self,
        image: &str,
        pod: &str,
        container_id: &str,
        spec: &Path,
    ) -> Option<std::result::Result<(), String>> {
        // PHASE 1 — under the plane lock, do ONLY O(1) work: ensure the plane,
        // apply the first-member guard, and TAKE a parked runner out of the image's
        // sub-pool. No spawn and no serve happen here, so a concurrent start for
        // another image waits on the lock for microseconds, not for this start's
        // round-trip. The lock is dropped at the end of this block (the `?`-early
        // returns all run the cold path).
        let (runner, request) = {
            let mut guard = self.plane.lock().ok()?;
            let plane = self.ensure(&mut guard)?;
            // The pod's SHARED netns the runner `setns`'s into: `sanitize(cri-<id>)`,
            // exactly what `join_argv`/`reexec_into_netns` pin inside the holder.
            let netns = delonix_sdn::infra::sanitize(pod);
            // FIRST-MEMBER GUARD. The pod netns is built lazily by the pod's FIRST
            // container start (the cold `reexec_into_netns` pod attach); until it
            // exists there is nothing for a runner to join, and entering a missing
            // netns would FAIL the start — worse than today. The holder pins the
            // netns at `/run/netns/<name>` inside its own mount namespace, visible
            // here as `/proc/<pin>/root/run/netns/<name>`. Absent → serve this
            // member cold (it builds the netns): only the SECOND+ member serves warm.
            let pinned = format!("/proc/{}/root/run/netns/{}", plane.holder_pid(), netns);
            if !std::path::Path::new(&pinned).exists() {
                return None;
            }
            // No sub-pool for this image yet → cold path; the StartContainer handler
            // then calls `seed`, which creates the sub-pool and the refiller fills it.
            if !plane.has(image) {
                return None;
            }
            // The host built the netns for the pod's first member; this member only
            // joins it (the pod IP, for the record), so there is no per-member attach.
            let request = StartRequest::isolated(
                spec,
                ContainerPlane {
                    id: container_id.to_string(),
                    netns,
                    ip: delonix_sdn::infra::container_ip(pod),
                },
            );
            match plane.take_runner(image) {
                Some(runner) => (runner, request),
                None => {
                    // The sub-pool exists but is momentarily empty (a lagging
                    // refill): serve cold, and ask the refiller to top it up so the
                    // next member serves warm.
                    drop(guard);
                    self.ask_refill(image);
                    return None;
                }
            }
        }; // <-- plane lock RELEASED here, BEFORE the serve.

        // PHASE 2 — serve OUTSIDE the lock. Two images' serves now run concurrently.
        // Time the warm wall ONLY when the profiler is on — a no-op otherwise, so
        // the hot path pays nothing in production. The record is emitted only when a
        // runner actually SERVED the start, and `warm_profile_report.py` pairs it
        // with the runner's own `parent` record (`spawn_total`) to give
        // `pre_spawn = wall − spawn_total` — the SAME definition `__poolbench` uses.
        // Crucially, `wall` no longer includes the refill (it is now on the
        // background thread), so a concurrent start never inflates this measurement.
        let t0 = std::time::Instant::now();
        let result = runner.serve(&request);
        let wall = t0.elapsed().as_micros();

        // PHASE 3 — refill OFF the hot path, on the background thread (never under
        // the serving lock). The consumed runner is replaced asynchronously.
        self.ask_refill(image);

        match result {
            // Served: the engine's own verdict is authoritative (success, or the
            // same refusal the cold path would surface).
            Ok(outcome) => {
                append_bench_wall("warm", wall);
                Some(if outcome.ok() {
                    Ok(())
                } else {
                    Err(outcome
                        .reason
                        .unwrap_or_else(|| "warm start failed".to_string()))
                })
            }
            // The runner died before the engine ran (socket EOF): fall back to the
            // cold path, which runs the engine cleanly — never worse than today.
            Err(_) => None,
        }
    }

    fn seed(&self, image: &str) {
        // Create the EMPTY sub-pool under a BRIEF lock (O(1): evict + insert, no
        // spawn), then hand the filling to the background refiller — the spawn/warm
        // never runs under the serving lock.
        {
            if let Ok(mut guard) = self.plane.lock() {
                if let Some(plane) = self.ensure(&mut guard) {
                    plane.ensure_pool(image);
                }
            }
        }
        self.ask_refill(image);
        // Keep the holder ref fresh (idempotent, bumps the marker mtime) so a
        // `system prune` sweep does not reap it while the plane is in use.
        let _ = delonix_sdn::infra::acquire(WARM_REF_ID);
    }
}

impl Drop for CriWarmPlane {
    fn drop(&mut self) {
        // Close the refill channel FIRST (drop the only sender), so the refiller's
        // `recv` returns `Err` and the loop ends; then join it, so no refill is
        // mid-flight when we tear the plane down. Only after that reap the parked
        // runners and give back the holder ref — if the server is the last user the
        // infra tears down, which is correct (no workload is left to keep it up).
        self.refill_tx.take();
        if let Some(h) = self.refiller.take() {
            let _ = h.join();
        }
        if let Ok(mut guard) = self.plane.lock() {
            *guard = None;
        }
        delonix_sdn::infra::release(WARM_REF_ID);
    }
}

/// Spawn the background refiller thread. It lives in the CRI SERVER process (never
/// in a runner — ADR-0095: the runner's `fork` must stay single-threaded), owns the
/// receiving end of the refill channel and a shared handle to the plane, and tops
/// sub-pools back up OFF the plane lock.
fn spawn_refiller(plane: SharedPlane, rx: Receiver<RefillMsg>) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("warm-refiller".into())
        .spawn(move || refiller_loop(&plane, rx))
        .expect("spawn warm-refiller thread")
}

/// The refiller loop: each [`RefillMsg::Topup`] tops ONE image back up to its
/// target size, one runner at a time. The plane lock is re-taken only for the O(1)
/// recipe snapshot and the O(1) deposit; the fork/exec/pre-warm in between holds NO
/// lock, so a concurrent serve or take never waits on a refill. Ends when the
/// sender (the [`CriWarmPlane`]) drops and `recv` returns `Err`.
fn refiller_loop(plane: &SharedPlane, rx: Receiver<RefillMsg>) {
    while let Ok(RefillMsg::Topup(image)) = rx.recv() {
        loop {
            // Snapshot under the lock: is another runner wanted, with what params?
            let recipe = {
                let Ok(guard) = plane.lock() else { return };
                let Some(plane) = guard.as_ref() else { break };
                match plane.refill_recipe(&image) {
                    Some(r) => r,
                    None => break, // sub-pool gone, already full, or cap hit
                }
            };
            // Spawn OFF the lock — the ~tens-of-ms cost a concurrent start must not
            // wait on.
            let runner = match spawn_from_recipe(&recipe) {
                Ok(r) => r,
                // Holder gone / binary missing: stop topping this image up; a later
                // message (or a holder reconcile) retries. A failed spawn parks
                // nothing, so the plane is never poisoned.
                Err(_) => break,
            };
            // Deposit under a brief lock; `deposit` discards the runner if the
            // sub-pool vanished or the holder restarted while we were spawning.
            let Ok(mut guard) = plane.lock() else { return };
            if let Some(plane) = guard.as_mut() {
                plane.deposit(&image, recipe.holder_pid(), runner);
            }
            // else: plane torn down → `runner` dropped here → reaped.
        }
    }
}

/// The `nsenter` argv that parks a runner INSIDE the holder (its userns, mountns
/// and netns), so it can `setns` into any per-container netns the holder built.
/// Mirrors `infra::join_argv` minus its trailing `ip netns exec <netns>` — the
/// runner enters the container netns per start, from [`CriWarmPlane::try_start_warm`].
fn isolated_prefix(pin: i32) -> Vec<OsString> {
    [
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
    .collect()
}

/// Append one warm-start wall record to the `DELONIX_PROFILE_SPAWN` JSONL file,
/// alongside the runner's own `{"role":"parent",…}`/`{"role":"prespawn",…}` lines,
/// so `scripts/warm_profile_report.py` pairs them into the CRI warm `pre_spawn`.
/// A strict no-op unless the env var names a PATH (a flag value writes nowhere) —
/// the SAME rule the profiler's `append_jsonl` and the bench's `append_bench` use,
/// so the three share one sink without colliding, and production pays nothing.
fn append_bench_wall(phase: &str, wall_us: u128) {
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

/// Parse a `usize` environment knob, falling back to `default` on absence or a
/// malformed value.
fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
