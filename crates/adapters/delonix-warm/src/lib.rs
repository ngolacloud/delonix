//! `delonix-warm` — the **warm-plane pool** behind the persistent servers.
//!
//! The measured cold-start profile (ADR-0095, `reports/spawn-profile-2026-11-18/`)
//! showed the dominant cost of starting a container is **not** the container
//! mechanics (`clone` 0.4 ms, mounts ~2 ms) but the `pre_spawn`: ~58 ms (host,
//! 68 %) that a freshly `execve`'d `delonix __apirun`/`delonix start` re-pays on
//! every start — the 43 MB binary re-loaded, the image re-resolved, the rootfs
//! re-prepared. The persistent CRI/API servers hold nothing reusable; they
//! re-exec a cold child per container.
//!
//! This crate keeps a **pool of parked, single-threaded `delonix __poolrunner`
//! processes** — binary already loaded, engine stores already open, the hinted
//! image already resolved and its layers already extracted (the page cache stays
//! hot while the runner lives). A start no longer re-execs a cold child: the
//! server hands a parked runner the run spec over a socket, the runner does the
//! *same* engine work (`cmd_run`) with its warm state, and reports back.
//!
//! **Serve-one-then-exit.** A runner serves exactly one start and exits, so the
//! `libc::fork()` in the supervisor (`delonix-linux`, safe only single-threaded)
//! runs in a pristine process every time, exactly as the re-exec guaranteed. The
//! pool refills off the hot path. This crate is OS orchestration only — it spawns
//! processes and frames a line protocol over a `UnixStream`; the container
//! mechanics live entirely in the `delonix` binary it launches.
//!
//! Scope boundaries this crate deliberately keeps (ADR-0040 daemonless, ADR-0095):
//! the cold CLI (`delonix container run`) never touches a pool; the pool is opt-in
//! and only legitimate where a process is already alive (CRI/API) or in the
//! measurement harness.

use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::io::{IntoRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

/// The fd the parent dups the control socket onto in the runner, before exec.
/// A fixed, low number the runner reads back as `__poolrunner <FD>`.
pub const RUNNER_FD: RawFd = 3;

/// What a start hands a parked runner: always a run-spec path, and — on the
/// ISOLATED path — the per-container network identity the host side already
/// built (its id, the netns the holder created for it, and its leased IP).
///
/// For host-net starts the `plane` is `None` and the runner reaches the engine
/// exactly as before (no netns to enter). For an isolated start the pool parks
/// its runner INSIDE the holder (an `nsenter` prefix, see [`WarmPool::with_prefix`])
/// and the host loop has already run `attach_container` to wire the veth and
/// lease the IP; `plane` carries what the runner needs to `setns` into that
/// netns and run the engine's SECOND-pass path in-process — no cold `netns run`
/// child is re-exec'd, which is the whole point on this path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartRequest {
    /// The run-spec JSON the runner reads (a `RunOpts`).
    pub spec: PathBuf,
    /// The per-container network plane, present only for an isolated start.
    pub plane: Option<ContainerPlane>,
}

/// The per-container network identity an isolated start hands its runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerPlane {
    /// The container id (becomes `DELONIX_REEXEC_ID`, the record's id).
    pub id: String,
    /// The named netns the holder created (`/run/netns/<netns>` to `setns` into).
    pub netns: String,
    /// The IP the host leased for this container (`DELONIX_REEXEC_IP`).
    pub ip: String,
}

impl StartRequest {
    /// A host-net start: just a spec, no netns to enter.
    pub fn host(spec: impl Into<PathBuf>) -> Self {
        Self {
            spec: spec.into(),
            plane: None,
        }
    }

    /// An isolated start whose network plane the host already built.
    pub fn isolated(spec: impl Into<PathBuf>, plane: ContainerPlane) -> Self {
        Self {
            spec: spec.into(),
            plane: Some(plane),
        }
    }

    /// PURE. The one wire line the pool sends a runner: the spec alone (host), or
    /// tab-joined `spec\tid\tnetns\tip` (isolated). The paths and tokens the pool
    /// generates never contain a tab, so the split back in [`parse_request`] is
    /// unambiguous.
    fn encode(&self) -> String {
        match &self.plane {
            None => self.spec.to_string_lossy().into_owned(),
            Some(p) => format!(
                "{}\t{}\t{}\t{}",
                self.spec.to_string_lossy(),
                p.id,
                p.netns,
                p.ip
            ),
        }
    }
}

/// PURE. Parse the wire line a runner reads back into a [`StartRequest`]. One
/// field → host; four tab-separated fields → isolated. Anything else keeps the
/// whole line as the spec with no plane (a host start), so a protocol drift
/// surfaces as "spec not found" rather than a silent isolated misfire.
pub fn parse_request(line: &str) -> StartRequest {
    let parts: Vec<&str> = line.split('\t').collect();
    if let [spec, id, netns, ip] = parts.as_slice() {
        return StartRequest::isolated(
            PathBuf::from(spec),
            ContainerPlane {
                id: (*id).to_string(),
                netns: (*netns).to_string(),
                ip: (*ip).to_string(),
            },
        );
    }
    StartRequest::host(PathBuf::from(line))
}

/// What a runner reports after serving one start: the engine's exit class and,
/// on failure, the reason line (so a kubelet event still names WHY, exactly as
/// the cold `delonix_detached_why` path does).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartOutcome {
    /// The engine exit code (`0` = started/ran; non-zero = the error's class).
    pub code: i32,
    /// `None` on success; the one-line reason on failure.
    pub reason: Option<String>,
}

impl StartOutcome {
    /// Did the start succeed?
    pub fn ok(&self) -> bool {
        self.code == 0
    }
}

// ---------------------------------------------------------------------------
// Runner side — the parked process (`delonix __poolrunner <fd> [hint]`).
// ---------------------------------------------------------------------------

/// Run as a parked warm runner: pre-warm, announce READY, serve exactly one run
/// spec, report the outcome, and return. Called from the `delonix` binary's
/// `__poolrunner` interception, BEFORE any thread is spawned, so the `fork` the
/// engine reaches stays single-threaded.
///
/// `fd` is the inherited control socket ([`RUNNER_FD`]). `warm` runs the
/// image/layer/store pre-warming (its cost is off the caller's hot path — it
/// finishes before READY). `run` performs the actual `cmd_run` for the one spec
/// path the pool sends, returning the engine's outcome.
///
/// The protocol is three lines:
/// 1. runner → pool: `READY\n` (once `warm` has finished).
/// 2. pool → runner: the request line ([`StartRequest::encode`] — a spec path,
///    or tab-joined `spec\tid\tnetns\tip` for an isolated start).
/// 3. runner → pool: `OK\n` | `ERR <code> <reason>\n`.
pub fn serve_parked<W, R>(fd: RawFd, warm: W, run: R) -> io::Result<()>
where
    W: FnOnce(),
    R: FnOnce(&StartRequest) -> StartOutcome,
{
    // SAFETY: `fd` is the control socket the parent duped onto RUNNER_FD before
    // exec; we are its sole owner for the life of this process.
    let stream = unsafe { unix_stream_from_fd(fd) };
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream);

    // Pre-warm BEFORE announcing readiness: a runner is only "ready" once its
    // image is resolved and layers are warm, so the start it later serves pays
    // none of that.
    warm();
    writer.write_all(b"READY\n")?;
    writer.flush()?;

    // Serve exactly one request.
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        // The pool closed without a request (shutdown): nothing to do.
        return Ok(());
    }
    let request = parse_request(line.trim_end_matches(['\n', '\r']));
    let outcome = run(&request);
    match &outcome.reason {
        None => writer.write_all(b"OK\n")?,
        Some(reason) => {
            let one_line: String = reason.replace(['\n', '\r'], " ");
            writer.write_all(format!("ERR {} {}\n", outcome.code, one_line).as_bytes())?;
        }
    }
    writer.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Pool side — the manager held by the persistent server / harness.
// ---------------------------------------------------------------------------

/// A spawned, parked runner: its process handle and the control socket to it.
struct Runner {
    child: Child,
    sock: UnixStream,
}

/// Reaps a parked runner on drop — the single place a runner is torn down, so a
/// runner is safe to move OUT of a pool (as a [`ParkedRunner`]) and drop anywhere.
///
/// A parked runner is blocked in `read_line` waiting for its one request; closing
/// the control socket gives it EOF, which [`serve_parked`] treats as a clean
/// shutdown (`n == 0` → `Ok(())` → exit 0), so no `kill` is needed — only the
/// `wait` that keeps the exited runner from lingering as a zombie of this process.
impl Drop for Runner {
    fn drop(&mut self) {
        // ORDERING: close the control socket BEFORE the wait. The runner is blocked
        // in `read_line`; the close is what turns that block into the EOF it exits
        // on, so the subsequent `wait` cannot hang. We cannot move `sock` out of a
        // field in `drop`, so swap in a throwaway socket — that drops the live one
        // and sends the EOF. (A runner that already served has exited; the close is
        // then a no-op and the `wait` simply reaps it.)
        if let Ok((throwaway, _peer)) = UnixStream::pair() {
            drop(std::mem::replace(&mut self.sock, throwaway));
        }
        let _ = self.child.wait();
    }
}

/// A spawned, READY runner detached from any pool.
///
/// The unit the background refiller spawns OFF the plane lock ([`spawn_from_recipe`])
/// and later deposits ([`WarmPlane::deposit`]), and the unit the hot path takes to
/// serve a start OUTSIDE the lock ([`WarmPlane::take_runner`] → [`serve`](ParkedRunner::serve)).
/// `Send` — it owns a `Child` and a `UnixStream`, both `Send` — so it can move to
/// the refiller thread and back; deliberately not `Sync` (no two threads touch one
/// runner at once).
pub struct ParkedRunner(Runner);

impl ParkedRunner {
    /// Serve exactly one start on this runner and reap it. Consumes the runner (it
    /// is serve-one-then-exit). Nothing is borrowed from a pool or plane, so the
    /// caller holds NO lock across the round-trip — two images' serves run
    /// concurrently. The runner is reaped by its [`Drop`](Runner) when this returns.
    pub fn serve(mut self, request: &StartRequest) -> io::Result<StartOutcome> {
        run_on(&mut self.0, request)
    }
}

/// A pool of parked warm runners.
///
/// Thread-free by design: [`start`](WarmPool::start) is the hot path (hand a
/// parked runner the spec, read the outcome) and [`refill`](WarmPool::refill)
/// tops the pool back up OFF the hot path — the caller decides when (a benchmark
/// refills between timed runs; a server refills on a background thread). This
/// keeps the measurement deterministic and the fork invariant on the runner side
/// unaffected by any pool-side concurrency.
pub struct WarmPool {
    /// The `delonix` binary to launch runners from (its own `/proc/self/exe`).
    bin: PathBuf,
    /// A command prefix each runner is launched THROUGH (empty for host-net).
    /// The ISOLATED path sets this to the holder's `nsenter -t <pin> -U -m -n
    /// --preserve-credentials --` so the parked runner lives inside the holder's
    /// userns+mountns and can `setns` into any per-container netns the holder
    /// built — no cold `netns run` child is ever re-exec'd.
    prefix: Vec<OsString>,
    /// The image hint each runner pre-warms (`""` = no hint).
    hint: OsString,
    /// Extra environment every runner inherits beyond the process's own (e.g.
    /// `DELONIX_ROOT`). `DELONIX_PROFILE_SPAWN` flows through the inherited env.
    env: Vec<(OsString, OsString)>,
    /// Target number of parked runners.
    size: usize,
    /// The parked, READY runners.
    idle: VecDeque<Runner>,
}

impl WarmPool {
    /// Build a pool of `size` parked runners launched from `bin`, each
    /// pre-warming `hint` (an image reference, or empty for none) and inheriting
    /// `env` on top of this process's environment. Blocks until every runner has
    /// announced READY. Host-net runners (no launch prefix).
    pub fn new(
        bin: impl Into<PathBuf>,
        hint: impl Into<OsString>,
        env: Vec<(OsString, OsString)>,
        size: usize,
    ) -> io::Result<Self> {
        Self::with_prefix(bin, hint, env, size, Vec::new())
    }

    /// Like [`new`](WarmPool::new), but every runner is launched THROUGH `prefix`
    /// (argv tokens prepended before the binary). The ISOLATED path passes the
    /// holder's `nsenter` argv so the runner parks inside the holder; an empty
    /// prefix is exactly [`new`](WarmPool::new).
    pub fn with_prefix(
        bin: impl Into<PathBuf>,
        hint: impl Into<OsString>,
        env: Vec<(OsString, OsString)>,
        size: usize,
        prefix: Vec<OsString>,
    ) -> io::Result<Self> {
        let mut pool = Self {
            bin: bin.into(),
            prefix,
            hint: hint.into(),
            env,
            size,
            idle: VecDeque::with_capacity(size),
        };
        pool.refill()?;
        Ok(pool)
    }

    /// How many runners are parked and ready right now.
    pub fn ready(&self) -> usize {
        self.idle.len()
    }

    /// Spawn one runner and block until it announces READY.
    fn spawn_one(&self) -> io::Result<Runner> {
        spawn_runner(&self.bin, &self.prefix, &self.hint, &self.env)
    }

    /// Top the pool back up to `size` parked runners. Off the hot path.
    pub fn refill(&mut self) -> io::Result<()> {
        while self.idle.len() < self.size {
            let r = self.spawn_one()?;
            self.idle.push_back(r);
        }
        Ok(())
    }

    /// An EMPTY sub-pool (nothing spawned). Same launch parameters as
    /// [`with_prefix`](WarmPool::with_prefix), but it parks no runner now — the
    /// [`WarmPlane`]'s background refiller fills it later via [`deposit`](WarmPool::deposit),
    /// OFF the plane lock. Never blocks.
    pub fn empty(
        bin: impl Into<PathBuf>,
        hint: impl Into<OsString>,
        env: Vec<(OsString, OsString)>,
        size: usize,
        prefix: Vec<OsString>,
    ) -> Self {
        Self {
            bin: bin.into(),
            prefix,
            hint: hint.into(),
            env,
            size,
            idle: VecDeque::with_capacity(size),
        }
    }

    /// Take one parked runner to serve OUTSIDE any lock, or `None` when the pool is
    /// momentarily empty (a lagging refill). Does NOT spawn — the caller serves
    /// cold on a miss and the refiller tops the pool back up.
    pub fn take(&mut self) -> Option<ParkedRunner> {
        self.idle.pop_front().map(ParkedRunner)
    }

    /// Deposit a runner the refiller spawned OFF the lock, up to `size`. A runner
    /// over the target is dropped (its [`Drop`](Runner) reaps it) rather than parked.
    pub fn deposit(&mut self, runner: ParkedRunner) {
        if self.idle.len() < self.size {
            self.idle.push_back(runner.0);
        }
        // else: `runner` is dropped here → control socket closes → runner exits → reaped.
    }

    /// Start one container by handing a parked runner the [`StartRequest`] and
    /// reading back its outcome. Takes a warm runner from the pool (spawning one
    /// synchronously only if the pool is momentarily empty — the slow path a
    /// timely [`refill`](WarmPool::refill) avoids). Does NOT refill: the caller
    /// refills off its hot path.
    pub fn start(&mut self, request: &StartRequest) -> io::Result<StartOutcome> {
        let mut runner = match self.idle.pop_front() {
            Some(r) => r,
            None => self.spawn_one()?,
        };
        let outcome = run_on(&mut runner, request);
        // The runner has served its one start and is exiting; reap it so it does
        // not linger as a zombie of this process.
        let _ = runner.child.wait();
        outcome
    }

    /// The image hint every runner in this pool pre-warms (`""` = none). The
    /// [`WarmPlane`] keys its sub-pools on this, so a start can find the pool
    /// warmed for exactly its image.
    pub fn hint(&self) -> &OsStr {
        &self.hint
    }

    /// The target number of parked runners — the pool's share of a plane's
    /// node-wide cap.
    pub fn size(&self) -> usize {
        self.size
    }
}

/// Spawn one parked runner from the given launch parameters and block until it
/// announces READY. Shared by [`WarmPool::spawn_one`] and the lock-free
/// [`spawn_from_recipe`] the background refiller uses: the heavy work (fork, exec,
/// pre-warm) runs with NO pool or plane borrowed, so a refiller can do it off the
/// plane lock and only re-take the lock for the O(1) [`WarmPlane::deposit`].
fn spawn_runner(
    bin: &Path,
    prefix: &[OsString],
    hint: &OsStr,
    env: &[(OsString, OsString)],
) -> io::Result<Runner> {
    // A connected socket pair: the parent keeps `parent`, the child inherits
    // `child` duped onto RUNNER_FD (CLOEXEC cleared by dup2, so it survives exec).
    let (parent, child) = UnixStream::pair()?;
    let child_fd = child.into_raw_fd();
    // With a prefix (isolated), the launched program is the prefix's head
    // (`nsenter`), which then execs `bin __poolrunner …` inside the holder; the
    // control-socket fd duped onto RUNNER_FD survives both the `nsenter` exec and
    // the namespace entry (it is CLOEXEC-cleared by dup2 and a socket fd is
    // namespace-agnostic). No prefix → launch the binary directly, byte-for-byte
    // the host-net path.
    let mut cmd = match prefix.split_first() {
        Some((head, rest)) => {
            let mut c = Command::new(head);
            c.args(rest).arg(bin);
            c
        }
        None => Command::new(bin),
    };
    cmd.arg("__poolrunner").arg(RUNNER_FD.to_string()).arg(hint);
    for (k, v) in env {
        cmd.env(k, v);
    }
    // SAFETY: the closure runs in the forked child before exec and makes only
    // async-signal-safe calls (`dup2`). `child_fd` is a valid open fd.
    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(child_fd, RUNNER_FD) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = cmd.spawn();
    // The parent no longer needs the child's end; it is open here regardless of
    // whether the spawn succeeded (the fd was duped out of `child`).
    // SAFETY: `child_fd` is ours; closed exactly once.
    unsafe {
        libc::close(child_fd);
    }
    let child = child?;
    let mut runner = Runner { child, sock: parent };
    wait_ready(&mut runner)?;
    Ok(runner)
}

/// Read the `READY\n` line a freshly spawned runner sends once pre-warmed.
fn wait_ready(runner: &mut Runner) -> io::Result<()> {
    let mut reader = BufReader::new(runner.sock.try_clone()?);
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 || line.trim() != "READY" {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "runner died before announcing READY",
        ));
    }
    Ok(())
}

/// Send the request to a READY runner and read back its one-line outcome.
fn run_on(runner: &mut Runner, request: &StartRequest) -> io::Result<StartOutcome> {
    let mut writer = runner.sock.try_clone()?;
    writer.write_all(request.encode().as_bytes())?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    let mut reader = BufReader::new(runner.sock.try_clone()?);
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "runner died before reporting the start outcome",
        ));
    }
    Ok(parse_outcome(line.trim_end_matches(['\n', '\r'])))
}

/// PURE. Parse a runner's reply line into a [`StartOutcome`].
///
/// `OK` → success. `ERR <code> <reason>` → failure with that class and reason.
/// Anything else is treated as an opaque failure, so a protocol drift surfaces
/// as an error rather than a silent success.
fn parse_outcome(line: &str) -> StartOutcome {
    if line == "OK" {
        return StartOutcome {
            code: 0,
            reason: None,
        };
    }
    if let Some(rest) = line.strip_prefix("ERR ") {
        let (code, reason) = match rest.split_once(' ') {
            Some((c, r)) => (c.parse().unwrap_or(1), r.to_string()),
            None => (rest.parse().unwrap_or(1), String::new()),
        };
        return StartOutcome {
            code,
            reason: Some(if reason.is_empty() {
                "the runner reported a failure".to_string()
            } else {
                reason
            }),
        };
    }
    StartOutcome {
        code: 1,
        reason: Some(format!("unexpected runner reply: {line}")),
    }
}

/// Build an owned [`UnixStream`] from a raw fd.
///
/// # Safety
/// `fd` must be an open, connected stream socket this process owns exclusively.
unsafe fn unix_stream_from_fd(fd: RawFd) -> UnixStream {
    use std::os::unix::io::FromRawFd;
    unsafe { UnixStream::from_raw_fd(fd) }
}

/// A convenience for callers assembling the runner env: `DELONIX_ROOT=<root>`.
pub fn root_env(root: &Path) -> (OsString, OsString) {
    (
        OsStr::new("DELONIX_ROOT").to_os_string(),
        root.as_os_str().to_os_string(),
    )
}

// ---------------------------------------------------------------------------
// Warm plane — a keyed set of per-image sub-pools for a live server (the CRI).
// ---------------------------------------------------------------------------

/// A bounded, per-IMAGE set of warm sub-pools for a long-lived server.
///
/// `PER-IMAGE-CRI-WARMING.md` is the design: a kubelet runs dozens of distinct
/// images on one node, and a single-hint [`WarmPool`] only serves the per-image
/// half of the win (image resolve + hot layers) for the ONE image it warmed. The
/// plane keeps one sub-pool per hot image, each a `WarmPool` warmed for exactly
/// that image and launched through the holder's `nsenter` prefix (the ISOLATED
/// path), so a start for image X finds a runner warmed for X.
///
/// **Keyed by image, not by network** (the decisive observation of the note): the
/// host runs `attach_container` per container (its own veth + leased IP) and the
/// runner only `setns`'s into the holder-built netns, so one holder-resident
/// runner warmed for X serves a start of X on any network — the fan-out is
/// `O(images)`, not `O(images × networks)`.
///
/// **Bounded.** `total_cap` caps parked runners across all images; a least-recently
/// -used order over the images evicts the coldest image's sub-pool (reaping its
/// runners, see [`WarmPool`]'s `Drop`) when a new hot image needs room. Parked
/// runners cost one resident process each, so the cap is a real memory lever.
///
/// **Holder-bound.** The isolated prefix depends on the pin PID; the plane records
/// which pin it was built for so a caller can detect a pin restart
/// ([`reconcile_holder`](WarmPlane::reconcile_holder)) and rebuild. The plane does
/// NOT itself take the holder's ref-count (that lives in the SDN adapter, which
/// this crate deliberately does not name) — the owner holds the ref for the
/// plane's life, exactly as the note requires.
///
/// Thread-free ITSELF, like [`WarmPool`] — it spawns no thread — but built for an
/// owner that refills OFF the lock. Every method is O(1) and the heavy work is
/// factored out: the hot path [`take_runner`](WarmPlane::take_runner)s a parked
/// runner under a brief lock and serves it with [`ParkedRunner::serve`] holding
/// NO lock (two images' serves run concurrently); a background refiller snapshots
/// a [`refill_recipe`](WarmPlane::refill_recipe) under the lock, runs the expensive
/// [`spawn_from_recipe`] with the lock released, and re-takes the lock only for the
/// O(1) [`deposit`](WarmPlane::deposit). So a spawn/warm NEVER blocks a concurrent
/// start — the mutex is held only for microseconds of bookkeeping.
pub struct WarmPlane {
    /// The `delonix` binary runners are launched from — the one that intercepts
    /// `__poolrunner` (NOT the server's own `delonix-cri` exe).
    bin: PathBuf,
    /// Extra environment every runner inherits (`DELONIX_ROOT`, the SDN runtime
    /// dir) — the same vector [`WarmPool::with_prefix`] takes.
    env: Vec<(OsString, OsString)>,
    /// The holder `nsenter` prefix every sub-pool launches through (parks the
    /// runner INSIDE the holder so it can `setns` into any per-container netns).
    prefix: Vec<OsString>,
    /// The pin PID the `prefix` was derived from; a change means the holder
    /// restarted and every parked runner is stale (see [`reconcile_holder`]).
    holder_pid: i32,
    /// Target parked runners per hot image.
    size_per_image: usize,
    /// Ceiling on parked runners across ALL images (the memory lever).
    total_cap: usize,
    /// The sub-pools, most-recently-used FIRST. One per hot image.
    pools: Vec<(String, WarmPool)>,
}

impl WarmPlane {
    /// Build an empty plane that will launch runners from `bin`, through the
    /// holder `prefix` derived from pin `holder_pid`, each inheriting `env`.
    /// `size_per_image` runners per hot image, `total_cap` parked runners
    /// node-wide (clamped so one image always fits).
    pub fn new(
        bin: impl Into<PathBuf>,
        env: Vec<(OsString, OsString)>,
        prefix: Vec<OsString>,
        holder_pid: i32,
        size_per_image: usize,
        total_cap: usize,
    ) -> Self {
        let size_per_image = size_per_image.max(1);
        Self {
            bin: bin.into(),
            env,
            prefix,
            holder_pid,
            size_per_image,
            total_cap: total_cap.max(size_per_image),
            pools: Vec::new(),
        }
    }

    /// The pin PID this plane's runners are parked in.
    pub fn holder_pid(&self) -> i32 {
        self.holder_pid
    }

    /// How many images currently have a hot sub-pool.
    pub fn hot_images(&self) -> usize {
        self.pools.len()
    }

    /// Total parked runners across all sub-pools (what `total_cap` bounds).
    pub fn parked(&self) -> usize {
        self.pools.iter().map(|(_, p)| p.ready()).sum()
    }

    /// Does `image` have a hot sub-pool right now?
    pub fn has(&self, image: &str) -> bool {
        self.pools.iter().any(|(k, _)| k == image)
    }

    /// Take one parked runner for `image` to serve a start OUTSIDE the plane lock,
    /// moving `image` to most-recently-used.
    ///
    /// `None` when there is no sub-pool for the image (the caller seeds via
    /// [`ensure_pool`](WarmPlane::ensure_pool)) OR the sub-pool is momentarily empty
    /// (a lagging refill — the caller serves cold and the refiller tops it up).
    /// Distinguish the two with [`has`](WarmPlane::has). The caller serves the
    /// returned runner with [`ParkedRunner::serve`] holding no lock, so two images'
    /// serves run concurrently — the whole point of the async plane.
    pub fn take_runner(&mut self, image: &str) -> Option<ParkedRunner> {
        let idx = self.pools.iter().position(|(k, _)| k == image)?;
        // Move to front (most-recently-used) regardless of whether a runner is
        // parked, so recency reflects the start attempt.
        let (key, mut pool) = self.pools.remove(idx);
        let runner = pool.take();
        self.pools.insert(0, (key, pool));
        runner
    }

    /// Ensure an EMPTY sub-pool exists for `image`, evicting least-recently-used
    /// sub-pools first if the node-wide cap leaves no room, and move it to
    /// most-recently-used. Returns whether a NEW sub-pool was created.
    ///
    /// Idempotent and O(1): creates NO runners — the background refiller fills it
    /// via [`refill_recipe`](WarmPlane::refill_recipe) + [`deposit`](WarmPlane::deposit),
    /// OFF the lock. `PullImage` and a cold miss call it, then enqueue a refill so
    /// the next start of this image is warm (warm-on-second-use).
    pub fn ensure_pool(&mut self, image: &str) -> bool {
        if let Some(idx) = self.pools.iter().position(|(k, _)| k == image) {
            // Already present: only promote to most-recently-used.
            let entry = self.pools.remove(idx);
            self.pools.insert(0, entry);
            return false;
        }
        // New image: reserve room against the cap (evicting the coldest), then
        // insert an empty sub-pool for the refiller to fill.
        self.evict_for(self.size_per_image);
        let pool = WarmPool::empty(
            &self.bin,
            image,
            self.env.clone(),
            self.size_per_image,
            self.prefix.clone(),
        );
        self.pools.insert(0, (image.to_string(), pool));
        true
    }

    /// A recipe to spawn ONE more runner for `image` OFF the lock, or `None` when
    /// the sub-pool is gone or already at its target size. The refiller loops on
    /// this: snapshot (under the lock) → [`spawn_from_recipe`] (off the lock) →
    /// [`deposit`](WarmPlane::deposit) (under the lock), until it returns `None`.
    /// Read-only and O(1) — the lock is held only for the clone, never a spawn.
    pub fn refill_recipe(&self, image: &str) -> Option<RefillRecipe> {
        let (_, pool) = self.pools.iter().find(|(k, _)| k == image)?;
        if pool.ready() >= pool.size() || self.parked() >= self.total_cap {
            return None;
        }
        Some(RefillRecipe {
            bin: self.bin.clone(),
            prefix: self.prefix.clone(),
            hint: pool.hint().to_os_string(),
            env: self.env.clone(),
            holder_pid: self.holder_pid,
        })
    }

    /// Deposit a runner the refiller spawned OFF the lock into `image`'s sub-pool.
    ///
    /// Dropped (reaped) rather than parked when the holder restarted since the
    /// recipe was taken (`holder_pid` mismatch → the runner is in a since-dead
    /// namespace), the sub-pool was evicted meanwhile, it is already full, or the
    /// node-wide cap is hit. O(1); never spawns.
    pub fn deposit(&mut self, image: &str, holder_pid: i32, runner: ParkedRunner) {
        if holder_pid != self.holder_pid || self.parked() >= self.total_cap {
            return; // stale pin or cap hit → drop (reaps)
        }
        if let Some((_, pool)) = self.pools.iter_mut().find(|(k, _)| k == image) {
            pool.deposit(runner); // the per-pool `size` is enforced inside deposit
        }
        // no sub-pool (evicted) → `runner` dropped here → reaped
    }

    /// Evict least-recently-used sub-pools (the TAIL) until parking `incoming`
    /// more runners stays within `total_cap`. Dropping a sub-pool reaps its
    /// runners (see [`WarmPool`]'s `Drop`).
    fn evict_for(&mut self, incoming: usize) {
        while !self.pools.is_empty()
            && self.parked() + incoming > self.total_cap
        {
            // Pop the least-recently-used (last) sub-pool; its `Drop` reaps it.
            self.pools.pop();
        }
    }

    /// Reconcile against the live pin PID. If the holder restarted (`current` ≠
    /// the pin this plane was built for), every parked runner is in a dead
    /// namespace: drop ALL sub-pools (reaping them), adopt the new prefix and pin,
    /// and return `true` so the caller knows the plane was rebuilt empty (the next
    /// start seeds afresh). A matching pin is a no-op returning `false`.
    pub fn reconcile_holder(&mut self, current: i32, prefix: Vec<OsString>) -> bool {
        if current == self.holder_pid {
            return false;
        }
        self.pools.clear(); // each sub-pool's Drop reaps its stale runners
        self.holder_pid = current;
        self.prefix = prefix;
        true
    }
}

/// A snapshot of what the background refiller needs to spawn ONE more runner for
/// an image OFF the plane lock: the launch parameters plus the holder pin they
/// were taken under. [`WarmPlane::refill_recipe`] produces it under the lock;
/// [`spawn_from_recipe`] consumes it without any lock; [`WarmPlane::deposit`] takes
/// the pin back ([`holder_pid`](RefillRecipe::holder_pid)) to detect a holder
/// restart that happened while the runner was spawning.
pub struct RefillRecipe {
    bin: PathBuf,
    prefix: Vec<OsString>,
    hint: OsString,
    env: Vec<(OsString, OsString)>,
    holder_pid: i32,
}

impl RefillRecipe {
    /// The holder pin this recipe was taken under — passed back to
    /// [`WarmPlane::deposit`] so a runner parked in a since-dead namespace is
    /// discarded instead of deposited.
    pub fn holder_pid(&self) -> i32 {
        self.holder_pid
    }
}

/// Spawn one parked runner from a [`RefillRecipe`] with NOTHING borrowed from the
/// plane — the heavy fork/exec/pre-warm the background refiller runs OFF the plane
/// lock. Blocks until the runner announces READY. The resulting [`ParkedRunner`] is
/// deposited back under the lock with [`WarmPlane::deposit`].
pub fn spawn_from_recipe(recipe: &RefillRecipe) -> io::Result<ParkedRunner> {
    spawn_runner(&recipe.bin, &recipe.prefix, &recipe.hint, &recipe.env).map(ParkedRunner)
}

/// PURE. How many least-recently-used sub-pools must be evicted to fit `incoming`
/// more parked runners under `cap`, given the current per-image parked counts
/// (most-recently-used first). The policy [`WarmPlane::evict_for`] runs, factored
/// out so the bookkeeping is a data test independent of any live runner.
pub fn evictions_for(parked_mru_first: &[usize], incoming: usize, cap: usize) -> usize {
    let mut total: usize = parked_mru_first.iter().sum();
    let mut evicted = 0;
    // Evict from the tail (least-recently-used) until there is room.
    for &n in parked_mru_first.iter().rev() {
        if total + incoming <= cap {
            break;
        }
        total -= n;
        evicted += 1;
    }
    evicted
}

#[cfg(test)]
mod tests;
