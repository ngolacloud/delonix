//! Scrape-path plumbing for the resource summary. The AGGREGATION itself moved
//! to the `delonix-engine` assembly in R3f-B (`Engine::dash_summary`/
//! `container_net`/`workload_series`): this interface no longer reaches the
//! `delonix-state`/`delonix-linux`/`delonix-sdn`/`delonix-volume`/`delonix-oci`
//! adapters. What stays here is the half that is genuinely the SCRAPE's concern,
//! not the engine's:
//!
//!  * the VM port ([`VmView`]) this crate shares with `delonix-mcp`;
//!  * the wall-clock ceiling + circuit breaker ([`collect_with_timeout`],
//!    [`run_bounded`]) that keep a hung I/O path from freezing the `/metrics`
//!    background refresh or leaking threads;
//!  * the Prometheus gauge publishers ([`publish_to_metrics`],
//!    [`publish_workloads_to_metrics`]) — reading the summary's fields and the
//!    workload series into the shared `delonix-telemetry` registry;
//!  * the per-container network cache the workload series overlay reads
//!    ([`workload_series_with_net_cache`]).
//!
//! Consumers call the engine through the [`EngineOps`](crate::EngineOps) port
//! (`delonix-mgmt`, `delonix-mcp`) or the composed `Engine` directly (the
//! `delonix` CLI's `dash`/`system`); `DashSummary` is re-exported here unchanged
//! so their type references keep resolving.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::Duration;

use delonix_telemetry::workload::{WorkloadKind, WorkloadSeries};

pub use delonix_model::dash::DashSummary;

/// Read-only view of the node's VMs — the one VM use case a collector needs.
/// The interface holds this port instead of calling the `delonix-vm` adapter
/// directly (ADR-0040 A5): the `delonix-engine` assembly composes the service
/// and the bin hands it in, so no `interface -> adapter` edge survives. Shared
/// with `delonix-mcp`, which reuses this exact trait for its
/// `resource.list`/`resource.get vm` path (it already depends on `delonix-mgmt`),
/// so the two surfaces cannot drift.
pub trait VmView: Send + Sync {
    /// Every VM the node has.
    fn list(&self) -> Result<Vec<delonix_compute::Vm>, delonix_model::Error>;
    /// One VM's full record by name.
    fn status(&self, name: &str) -> Result<delonix_compute::Vm, delonix_model::Error>;
}

/// Outcome of a bounded collection — three distinct states, because the
/// caller's log line must not claim a timeout when nothing was even attempted.
#[derive(Debug)]
pub enum Bounded<T> {
    /// The work finished within the deadline.
    Done(T),
    /// The work was started but did not finish in time. Its thread is now
    /// leaked (see [`run_bounded`]).
    TimedOut,
    /// Nothing was started: a PREVIOUS attempt is still stuck in the same
    /// underlying operation. This is the circuit breaker, not a failure of
    /// this attempt.
    Skipped,
}

impl<T> Bounded<T> {
    /// The value, if any — for callers that only care about "did I get data".
    pub fn ok(self) -> Option<T> {
        match self {
            Bounded::Done(v) => Some(v),
            _ => None,
        }
    }
}

/// A one-slot "is a worker still stuck in there?" latch, guarding a single
/// call site. Lives in a `static` at each call site so the breaker is
/// per-operation rather than global.
pub struct InFlight(std::sync::atomic::AtomicBool);

impl InFlight {
    pub const fn new() -> Self {
        InFlight(std::sync::atomic::AtomicBool::new(false))
    }
}

impl Default for InFlight {
    fn default() -> Self {
        Self::new()
    }
}

/// Clears the latch when the worker leaves `work` — by return OR by unwind.
///
/// The `Drop` matters: with a bare `store(false)` after the call, a panicking
/// collection would leave the latch stuck at `true` and every future attempt
/// would be `Skipped` forever. That trades a thread leak for a permanently
/// dead metric, which is worse — the breaker must only ever trip on a real
/// hang.
struct InFlightGuard(&'static InFlight);

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.0 .0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Runs `work` on a throwaway thread with a hard wall-clock ceiling, and —
/// this is the part that matters — **refuses to start a second worker while
/// the first is still stuck**.
///
/// BUG FIXED HERE (the thread leak was unbounded). The deadline was added
/// earlier to stop a hung I/O path from freezing the caller's loop forever,
/// and the worker thread is necessarily leaked on timeout (Rust cannot cancel
/// a thread blocked in a syscall). What was missed is that the caller is an
/// *infinite periodic loop*: `spawn_expensive_metrics_refresh` retries every
/// 120s deadline + 30s sleep, so a genuinely stuck operation leaked a thread
/// every ~150s — ~576 per day, each one parked in the same syscall, growing
/// without limit until the process runs out of threads.
///
/// With the latch, the leak is bounded at **exactly one thread** for as long
/// as the hang lasts, however long that is; the moment the underlying
/// operation unblocks, the latch clears and collection resumes on its own.
fn run_bounded<T: Send + 'static>(
    flag: &'static InFlight,
    timeout: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Bounded<T> {
    // `swap` is the whole breaker: if it was already `true`, a previous worker
    // never came back and we must not add another.
    if flag.0.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return Bounded::Skipped;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _guard = InFlightGuard(flag);
        let _ = tx.send(work());
    });
    match rx.recv_timeout(timeout) {
        Ok(v) => Bounded::Done(v),
        Err(_) => Bounded::TimedOut,
    }
}

/// The latch guarding [`collect_with_timeout`]. One process only ever needs a
/// single expensive collection in flight at a time.
static COLLECT_IN_FLIGHT: InFlight = InFlight::new();

/// The composed engine's [`dash_summary`](crate::EngineOps::dash_summary), but
/// with a hard wall-clock ceiling AND a circuit breaker so a permanently-hung
/// I/O path cannot leak an unbounded number of threads. See [`run_bounded`] for
/// both mechanisms and why each is needed. The port is cloned into the worker
/// thread (it must outlive this frame), so callers pass an owned `Arc`.
pub fn collect_with_timeout(
    engine: Arc<dyn crate::EngineOps>,
    include_network: bool,
    include_storage: bool,
    timeout: Duration,
) -> Bounded<DashSummary> {
    run_bounded(&COLLECT_IN_FLIGHT, timeout, move || {
        engine.dash_summary(include_network, include_storage)
    })
}

/// The latch guarding [`collect_container_net_with_timeout`] — its OWN latch
/// (not [`COLLECT_IN_FLIGHT`]), so a stuck netns read here never blocks the
/// unrelated `DashSummary` collection, and vice-versa.
static CONTAINER_NET_IN_FLIGHT: InFlight = InFlight::new();

/// Bounded/circuit-broken form of [`container_net`](crate::EngineOps::container_net)
/// — same two mechanisms as [`collect_with_timeout`] (see [`run_bounded`]).
pub fn collect_container_net_with_timeout(
    engine: Arc<dyn crate::EngineOps>,
    timeout: Duration,
) -> Bounded<HashMap<String, (u64, u64)>> {
    run_bounded(&CONTAINER_NET_IN_FLIGHT, timeout, move || {
        engine.container_net()
    })
}

/// Pushes a [`DashSummary`] into the shared Prometheus registry
/// (`delonix_telemetry::metrics`). Fields collected as `None` (an
/// `include_network`/`include_storage` opt-out) simply leave that gauge at
/// its last-published value — callers publish the cheap fields often and the
/// expensive ones from a slower background refresh, so a gauge is stale, never
/// wrong-and-zero.
pub fn publish_to_metrics(s: &DashSummary) {
    delonix_telemetry::metrics::set_containers(s.containers_running, s.containers_total);
    delonix_telemetry::metrics::set_vms(s.vms_running, s.vms_total);
    delonix_telemetry::metrics::set_memory(s.memory_bytes_used, s.memory_bytes_limit);
    if let (Some(rx), Some(tx)) = (s.network_rx_bytes, s.network_tx_bytes) {
        delonix_telemetry::metrics::set_network(rx, tx, s.network_unmeasured_containers);
    }
    if let (Some(images), Some(volumes), Some(vm_images), Some(containers)) = (
        s.storage_bytes_images,
        s.storage_bytes_volumes,
        s.storage_bytes_vm_images,
        s.storage_bytes_containers,
    ) {
        delonix_telemetry::metrics::set_storage(images, volumes, vm_images, containers);
    }
}

/// Last-known per-container network bytes (`id -> (rx, tx)`), refreshed by the
/// background loop (`spawn_expensive_metrics_refresh`) via
/// [`publish_container_net_cache`] and merged into the inline per-workload
/// snapshot by [`workload_series_with_net_cache`].
///
/// WHY A CACHE (and not read inline like cpu/mem): a container's rx/tx live in
/// its netns, reachable only by an `nsenter`+`cat` per container — the exact
/// cost the `/metrics` handler keeps OFF the scrape path. So network follows the
/// aggregate's pattern: refreshed on the slow background cadence and served
/// stale-but-never-wrong, while cpu/mem stay fresh-every-scrape.
static CONTAINER_NET_CACHE: LazyLock<Mutex<HashMap<String, (u64, u64)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Replaces the per-container network cache read by
/// [`workload_series_with_net_cache`]. Called from the background refresh loop
/// with the latest netns reads.
pub fn publish_container_net_cache(map: HashMap<String, (u64, u64)>) {
    *CONTAINER_NET_CACHE
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = map;
}

fn container_net_from_cache(id: &str) -> Option<(u64, u64)> {
    CONTAINER_NET_CACHE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(id)
        .copied()
}

/// The engine's [`workload_series`](crate::EngineOps::workload_series) with each
/// CONTAINER's network bytes overlaid from the background-refreshed cache. The
/// engine leaves container net `None` to keep the inline scrape path free of
/// netns enters (a Cloud Hypervisor VM's tap bytes are already filled there).
pub fn workload_series_with_net_cache(engine: &dyn crate::EngineOps) -> Vec<WorkloadSeries> {
    let mut series = engine.workload_series();
    for ws in &mut series {
        if matches!(ws.kind, WorkloadKind::Container) {
            if let Some((rx, tx)) = container_net_from_cache(&ws.id) {
                ws.net_rx_bytes = Some(rx);
                ws.net_tx_bytes = Some(tx);
            }
        }
    }
    series
}

/// Pushes the per-workload series into the shared Prometheus registry. Replaces
/// the whole set, so a workload that stopped is no longer exported.
pub fn publish_workloads_to_metrics(series: Vec<WorkloadSeries>) {
    delonix_telemetry::metrics::publish_workloads(series);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REGRESSION (unbounded resource leak): while a worker is STILL STUCK,
    /// further attempts must be `Skipped` without spawning anything.
    ///
    /// This is the bug the circuit breaker exists for: the real caller is an
    /// infinite periodic loop, so without the latch a permanently-hung NFS
    /// volume leaked one parked thread per cycle (~576/day) until the process
    /// ran out of threads. Removing the `swap` guard in `run_bounded` makes the
    /// spawn counter below come out at 5 instead of 1, and this test fails.
    #[test]
    fn run_bounded_nao_lanca_uma_2a_thread_enquanto_a_1a_esta_presa() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        static TEST_FLAG: InFlight = InFlight::new();

        let spawns = Arc::new(AtomicUsize::new(0));
        // Released only at the end of the test — until then every worker that
        // actually starts stays parked, exactly like a wedged `read` on a dead
        // NFS mount.
        let unblock = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));

        let hang = {
            let spawns = Arc::clone(&spawns);
            let unblock = Arc::clone(&unblock);
            move || {
                spawns.fetch_add(1, Ordering::SeqCst);
                let (lock, cv) = &*unblock;
                let mut done = lock.lock().unwrap();
                while !*done {
                    done = cv.wait(done).unwrap();
                }
                42u32
            }
        };

        let deadline = Duration::from_millis(150);
        // 1st attempt: starts a worker, which hangs → TimedOut, thread leaked.
        assert!(matches!(
            run_bounded(&TEST_FLAG, deadline, hang.clone()),
            Bounded::TimedOut
        ));
        // Every subsequent attempt, for as long as the hang lasts, must be
        // SKIPPED — this is the whole fix.
        for i in 0..4 {
            assert!(
                matches!(
                    run_bounded(&TEST_FLAG, deadline, hang.clone()),
                    Bounded::Skipped
                ),
                "tentativa {i} devia ter sido saltada, não relançada"
            );
        }
        assert_eq!(
            spawns.load(Ordering::SeqCst),
            1,
            "a fuga tem de ficar limitada a UMA thread, por muito que o hang dure"
        );

        // Assim que a operação desbloqueia, o disjuntor rearma-se sozinho.
        {
            let (lock, cv) = &*unblock;
            *lock.lock().unwrap() = true;
            cv.notify_all();
        }
        // Espera o worker sair (o guard limpa a latch no Drop).
        let rearmed = (0..200).any(|_| {
            std::thread::sleep(Duration::from_millis(10));
            matches!(run_bounded(&TEST_FLAG, deadline, || 7u32), Bounded::Done(7))
        });
        assert!(
            rearmed,
            "depois do hang resolver, a colheita tem de voltar a correr sozinha"
        );
    }

    /// O disjuntor NÃO pode ficar preso por um `work` que entra em pânico —
    /// senão trocava-se uma fuga de threads por uma métrica morta para sempre.
    /// É para isto que a latch é limpa por `Drop` e não por um `store` a seguir
    /// à chamada.
    #[test]
    fn run_bounded_rearma_depois_de_um_panico_no_worker() {
        static TEST_FLAG: InFlight = InFlight::new();
        let deadline = Duration::from_secs(5);

        // O worker entra em pânico; o `recv` vê o canal fechado → TimedOut.
        assert!(matches!(
            run_bounded(&TEST_FLAG, deadline, || -> u32 { panic!("boom") }),
            Bounded::TimedOut
        ));
        // A latch tem de ter sido limpa no unwind: a chamada seguinte corre.
        let rearmed = (0..200).any(|_| {
            std::thread::sleep(Duration::from_millis(10));
            matches!(run_bounded(&TEST_FLAG, deadline, || 9u32), Bounded::Done(9))
        });
        assert!(rearmed, "um pânico no worker deixou o disjuntor preso");
    }
}
