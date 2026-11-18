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
/// 2. pool → runner: `<spec-path>\n`.
/// 3. runner → pool: `OK\n` | `ERR <code> <reason>\n`.
pub fn serve_parked<W, R>(fd: RawFd, warm: W, run: R) -> io::Result<()>
where
    W: FnOnce(),
    R: FnOnce(&Path) -> StartOutcome,
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

    // Serve exactly one spec.
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        // The pool closed without a request (shutdown): nothing to do.
        return Ok(());
    }
    let spec = Path::new(line.trim_end_matches(['\n', '\r']));
    let outcome = run(spec);
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
    /// announced READY.
    pub fn new(
        bin: impl Into<PathBuf>,
        hint: impl Into<OsString>,
        env: Vec<(OsString, OsString)>,
        size: usize,
    ) -> io::Result<Self> {
        let mut pool = Self {
            bin: bin.into(),
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
        // A connected socket pair: the parent keeps `parent`, the child inherits
        // `child` duped onto RUNNER_FD (CLOEXEC cleared by dup2, so it survives
        // exec).
        let (parent, child) = UnixStream::pair()?;
        let child_fd = child.into_raw_fd();
        let mut cmd = Command::new(&self.bin);
        cmd.arg("__poolrunner")
            .arg(RUNNER_FD.to_string())
            .arg(&self.hint);
        for (k, v) in &self.env {
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
        // The parent no longer needs the child's end; it is open here regardless
        // of whether the spawn succeeded (the fd was duped out of `child`).
        // SAFETY: `child_fd` is ours; closed exactly once.
        unsafe {
            libc::close(child_fd);
        }
        let child = child?;
        let mut runner = Runner { child, sock: parent };
        wait_ready(&mut runner)?;
        Ok(runner)
    }

    /// Top the pool back up to `size` parked runners. Off the hot path.
    pub fn refill(&mut self) -> io::Result<()> {
        while self.idle.len() < self.size {
            let r = self.spawn_one()?;
            self.idle.push_back(r);
        }
        Ok(())
    }

    /// Start one container by handing a parked runner the run spec at `spec` and
    /// reading back its outcome. Takes a warm runner from the pool (spawning one
    /// synchronously only if the pool is momentarily empty — the slow path a
    /// timely [`refill`](WarmPool::refill) avoids). Does NOT refill: the caller
    /// refills off its hot path.
    pub fn start(&mut self, spec: &Path) -> io::Result<StartOutcome> {
        let mut runner = match self.idle.pop_front() {
            Some(r) => r,
            None => self.spawn_one()?,
        };
        let outcome = run_on(&mut runner, spec);
        // The runner has served its one start and is exiting; reap it so it does
        // not linger as a zombie of this process.
        let _ = runner.child.wait();
        outcome
    }
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

/// Send the spec path to a READY runner and read back its one-line outcome.
fn run_on(runner: &mut Runner, spec: &Path) -> io::Result<StartOutcome> {
    let mut writer = runner.sock.try_clone()?;
    writer.write_all(spec.as_os_str().as_encoded_bytes())?;
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

#[cfg(test)]
mod tests;
