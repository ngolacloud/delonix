//! `LinuxLifecycle` — the node's **container lifecycle edge** as this interface
//! needs it (R3d-i, ADR-0040 A5). The interface holds this port instead of
//! naming the `delonix-linux` adapter directly on the container read/write path:
//! a bin composes an implementor and hands the server one, so the forbidden
//! interface→adapter edge stays in the bin — the same shape as
//! [`crate::vms::VmPort`], [`crate::store::StorePort`],
//! [`crate::run_resolver::RunResolver`], [`crate::net_ops::NetworkOps`] and
//! [`crate::image_repo::ImageRepo`].
//!
//! This Engine has no container *service* yet (the slice is PRE-R3h), so the
//! implementor is backed by `delonix-linux` DIRECTLY — the same way the bin backs
//! the record store with `delonix-state` and the SDN with `delonix-sdn` — not by
//! an engine container port. When a container service lands, this port is where
//! it is swapped in.
//!
//! The adapter's `delonix_linux::ContainerUsage`, `delonix_linux::ResourceUpdate`
//! and `delonix_linux::LimitUpdate` cross as the port's own [`ContainerUsage`],
//! [`ResourceUpdate`] and [`LimitUpdate`] — the fields the stats/update halves
//! render and build, so the adapter types never leak past the port (the
//! discipline [`crate::image_repo::StoredImage`] and [`crate::net_ops::NetRecord`]
//! use for their records). The shared [`delonix_model::Error`] both the engine
//! and this crate speak is the error on every fallible method; the
//! `delonix_compute::Container` record and the cgroup/liveness reads cross as
//! themselves. The `stop`/`remove` verbs drive over the flock-guarded record
//! store they are handed, which [`crate::store::PortRepo`] bridges to the
//! `delonix_model::ports::StateRepository` the adapter takes — so the implementor
//! never names a store adapter either.

use delonix_compute::Container;
use delonix_model::Error;

/// A container's live cgroup usage as the `Stats` read renders it: the two
/// cumulative counters `delonix_linux::ContainerUsage` carries that this
/// interface reports, crossed as a summary so the adapter type does not leak
/// past the port. Each is best-effort — a stopped container (dead/missing
/// cgroup) reads back `None`, never a fabricated zero.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContainerUsage {
    /// Cumulative CPU time in microseconds (`cpu.stat`'s `usage_usec`).
    pub cpu_usage_usec: Option<u64>,
    /// Current memory working set in bytes (`memory.current`).
    pub mem_bytes: Option<u64>,
}

/// A partial resource update the `UpdateContainer` write applies, crossed as the
/// port's own owned-field record so `delonix_linux::ResourceUpdate` (which
/// borrows) does not leak past the port. Each field follows the adapter's own
/// convention: `None`/an empty slice means "leave this one as it is", never
/// "clear it" — a partial request (a kubelet in-place resize) only ever sets
/// what changed. The implementor borrows these into the adapter's own
/// `ResourceUpdate` for the write.
#[derive(Clone, Debug, Default)]
pub struct ResourceUpdate {
    pub memory: Option<String>,
    pub cpus: Option<String>,
    pub cpu_weight: Option<String>,
    pub cpuset: Option<String>,
    pub cpuset_mems: Option<String>,
    pub hugepage_limits: Vec<(String, u64)>,
    pub unified: Vec<(String, String)>,
    pub oom_score_adj: Option<i32>,
}

/// Which of the three outcomes a live-limit write reached, crossed as the port's
/// own enum so `delonix_linux::LimitUpdate` does not leak past the port. The
/// caller MUST distinguish them: saying "updated" about a `NotEnforced` is the
/// dishonest reporting this engine hunts down everywhere else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitUpdate {
    /// Written to the live cgroup: in force now, without the PID changing.
    Applied,
    /// Stopped container — only the record changes, and the next `start` applies it.
    Deferred,
    /// Running container whose cgroup does not exist at the expected path: the
    /// request is not in force and will not become so on its own.
    NotEnforced,
}

/// The container-lifecycle use cases the node API reads and writes through
/// (R3d-i). Object-safe (`dyn LinuxLifecycle`): every method names only shared
/// types — [`Container`], the shared [`Error`], the port's own
/// [`ContainerUsage`]/[`ResourceUpdate`]/[`LimitUpdate`], [`crate::store::StorePort`]
/// and primitives — never a `delonix-linux` type, so the implementor lives in the
/// composition and the interface stays clean of the adapter.
pub trait LinuxLifecycle: Send + Sync {
    // ---- liveness ----

    /// Refresh `c`'s liveness in memory (its recorded `status`/`pid` reconciled
    /// against the real process, PID-reuse-safe): a container whose init died
    /// since the last write reads `Exited`/`Dead`, not a stale `Running`. The
    /// refresh is NOT persisted by the caller — a read must not write.
    fn reconcile_status(&self, c: &mut Container);

    // ---- stats ----

    /// `c`'s live cgroup usage (best-effort per field), as the `Stats` read
    /// renders it.
    fn container_usage(&self, c: &Container) -> ContainerUsage;

    // ---- the lifecycle writes ----

    /// Graceful stop of `c` (SIGTERM, then SIGKILL after `grace_secs`), over the
    /// flock-guarded record `store` the create/CLI also drive. `c` is updated in
    /// place to its post-stop state.
    fn stop(
        &self,
        store: &dyn crate::store::StorePort,
        c: &mut Container,
        grace_secs: u64,
    ) -> Result<(), Error>;

    /// Send one signal to `c`'s init process. `NotRunning` when it has no live
    /// process to signal.
    fn send_signal(&self, c: &Container, signal: i32) -> Result<(), Error>;

    /// Remove `c` (its record under its lock, and — with `force` — the running
    /// process first), over the flock-guarded record `store`. Without `force`,
    /// removing a running container is the engine's refusal.
    fn remove(
        &self,
        store: &dyn crate::store::StorePort,
        c: &Container,
        force: bool,
    ) -> Result<(), Error>;

    /// Rewrite `c`'s LIVE cgroup limits (a `docker update`), returning which of
    /// the three [`LimitUpdate`] cases happened.
    fn update_kube_resources(
        &self,
        c: &Container,
        u: &ResourceUpdate,
    ) -> Result<LimitUpdate, Error>;

    // ---- the host reads ----

    /// Whether this node runs rootless (unprivileged user namespace).
    fn is_rootless(&self) -> bool;
    /// Whether this node can apply cgroup limits (cgroup v2 delegated).
    fn cgroup_limits_apply(&self) -> bool;
    /// The host's online CPU count (`0` when unmeasurable).
    fn host_ncpu(&self) -> u64;
    /// The host's total memory in bytes (`0` when unmeasurable).
    fn host_mem_bytes(&self) -> u64;
    /// The node's cgroup slice path (its `cpu.max`/`memory.max`/`pids.max` are
    /// the node's allocatable), `None` when there is no delegated slice.
    fn slice_path(&self) -> Option<String>;
}

/// A [`LinuxLifecycle`] with no composed engine — the empty backing
/// [`crate::router`] hands the server when no engine is injected. The liveness
/// refresh leaves the record as-is; the usage read is all-unmeasured; a mutation
/// on a missing target is the safe nothing; the host reads answer zero/false/no
/// slice. The production server never uses it — a bin hands in the
/// `delonix-linux`-backed port — exactly as [`crate::store::NoStore`] is to
/// [`crate::store::StorePort`]. The lifecycle write/stats paths that would rely on
/// these answers are not exercised through the default router (its tests drive
/// node/provider reads and refusals).
pub(crate) struct NoLinux;

impl LinuxLifecycle for NoLinux {
    fn reconcile_status(&self, _c: &mut Container) {
        // Leave the record as it was read: no engine to probe liveness against.
    }
    fn container_usage(&self, _c: &Container) -> ContainerUsage {
        ContainerUsage::default()
    }
    fn stop(
        &self,
        _store: &dyn crate::store::StorePort,
        _c: &mut Container,
        _grace_secs: u64,
    ) -> Result<(), Error> {
        Ok(())
    }
    fn send_signal(&self, _c: &Container, _signal: i32) -> Result<(), Error> {
        Ok(())
    }
    fn remove(
        &self,
        _store: &dyn crate::store::StorePort,
        _c: &Container,
        _force: bool,
    ) -> Result<(), Error> {
        Ok(())
    }
    fn update_kube_resources(
        &self,
        _c: &Container,
        _u: &ResourceUpdate,
    ) -> Result<LimitUpdate, Error> {
        // Nothing composed to write a live cgroup: the safe nothing is "deferred"
        // (the record carries the desired state; a start would rebuild it).
        Ok(LimitUpdate::Deferred)
    }
    fn is_rootless(&self) -> bool {
        false
    }
    fn cgroup_limits_apply(&self) -> bool {
        false
    }
    fn host_ncpu(&self) -> u64 {
        0
    }
    fn host_mem_bytes(&self) -> u64 {
        0
    }
    fn slice_path(&self) -> Option<String> {
        None
    }
}

// The real `delonix-linux`-backed implementor lives in the composition
// (`delonix-node-api-bin`'s `HostLinux`) and, for the integration tests, in the
// harness's `tests/common::AdapterLinux` over the `delonix-linux` dependency. The
// node-api crate names no linux adapter on the container lifecycle path — except
// the in-crate [`LocalLinux`] below, for the crate's own unit tests.

/// A [`LinuxLifecycle`] over the real `delonix-linux` adapter, for the crate's own
/// tests only (R3d-i) — the same shape as [`crate::net_ops::LocalNet`] and
/// [`crate::store::LocalStore`]. The server never builds this; production hands in
/// a bin's `delonix-linux`-backed port. `delonix-linux` is a dependency of this
/// crate (providers.rs still names its provider report), so this (and the
/// lifecycle unit tests) may name it while the container-path production code does
/// not.
#[cfg(test)]
pub(crate) struct LocalLinux;

#[cfg(test)]
impl LinuxLifecycle for LocalLinux {
    fn reconcile_status(&self, c: &mut Container) {
        delonix_linux::reconcile_status(c);
    }
    fn container_usage(&self, c: &Container) -> ContainerUsage {
        let u = delonix_linux::container_usage(c);
        ContainerUsage {
            cpu_usage_usec: u.cpu_usage_usec,
            mem_bytes: u.mem_bytes,
        }
    }
    fn stop(
        &self,
        store: &dyn crate::store::StorePort,
        c: &mut Container,
        grace_secs: u64,
    ) -> Result<(), Error> {
        delonix_linux::stop(&crate::store::PortRepo(store), c, grace_secs).map_err(Into::into)
    }
    fn send_signal(&self, c: &Container, signal: i32) -> Result<(), Error> {
        delonix_linux::send_signal(c, signal).map_err(Into::into)
    }
    fn remove(
        &self,
        store: &dyn crate::store::StorePort,
        c: &Container,
        force: bool,
    ) -> Result<(), Error> {
        delonix_linux::remove(&crate::store::PortRepo(store), c, force).map_err(Into::into)
    }
    fn update_kube_resources(
        &self,
        c: &Container,
        u: &ResourceUpdate,
    ) -> Result<LimitUpdate, Error> {
        let du = delonix_linux::ResourceUpdate {
            memory: u.memory.as_deref(),
            cpus: u.cpus.as_deref(),
            cpu_weight: u.cpu_weight.as_deref(),
            cpuset: u.cpuset.as_deref(),
            cpuset_mems: u.cpuset_mems.as_deref(),
            hugepage_limits: &u.hugepage_limits,
            unified: &u.unified,
            oom_score_adj: u.oom_score_adj,
        };
        delonix_linux::update_kube_resources(c, &du)
            .map(|l| match l {
                delonix_linux::LimitUpdate::Applied => LimitUpdate::Applied,
                delonix_linux::LimitUpdate::Deferred => LimitUpdate::Deferred,
                delonix_linux::LimitUpdate::NotEnforced => LimitUpdate::NotEnforced,
            })
            .map_err(Into::into)
    }
    fn is_rootless(&self) -> bool {
        delonix_linux::is_rootless()
    }
    fn cgroup_limits_apply(&self) -> bool {
        delonix_linux::cgroup_limits_apply()
    }
    fn host_ncpu(&self) -> u64 {
        delonix_linux::host_ncpu()
    }
    fn host_mem_bytes(&self) -> u64 {
        delonix_linux::host_mem_bytes()
    }
    fn slice_path(&self) -> Option<String> {
        delonix_linux::slice_path()
    }
}
