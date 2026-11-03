//! Exclusive file lock (`flock`), to serialize the
//! read-modify-writes that decide addresses.
//!
//! It exists because this crate has MORE THAN ONE registry of that shape — the IPAM,
//! the ingress-network registry and the `NetworkStore` — and every copy of the same
//! `open`+`flock` is one more chance for one of them to drift. The
//! [`crate::ipam`] has its own (`IpamLock`), older than this one and left
//! as is on purpose: touching it means changing the path that hands addresses to
//! live containers, and its comment is the only written memory of an already
//! fixed bug — worth more read than rewritten.
//!
//! **Fallible on purpose.** A lock that guards uniqueness and fails OPEN is
//! worse than no lock at all: the caller believes itself serialized and is not. So
//! `acquire` returns `Option` and the caller must REFUSE the operation —
//! never continue without it.

use crate::Error;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long to wait for the lock before REFUSING. The critical sections it
/// serializes are short read-modify-writes (decide an address, write a
/// registry) — normal contention clears in milliseconds. If it does not clear in
/// this, a holder got stuck/leaked (seen when a netns/slirp setup wedged
/// and never released the lock): the caller's contract is to REFUSE, and refusing at
/// a deadline is infinitely better than hanging forever — that was
/// the hang of the custom-network attach that had no timeout.
const LOCK_DEADLINE: Duration = Duration::from_secs(30);

/// Exclusive lock alive while the value exists. Released on `Drop`.
pub(crate) struct ExclusiveLock(i32);

impl ExclusiveLock {
    /// Takes the lock at `path`, waiting at most [`LOCK_DEADLINE`]. `None`
    /// when it was not possible (open failed, OR a stuck holder did not release in
    /// time) — the caller MUST give up the operation.
    ///
    /// The file is created if it does not exist; its content is irrelevant (only
    /// the `flock` on the descriptor counts).
    pub(crate) fn acquire(path: &Path) -> Option<ExclusiveLock> {
        Self::acquire_until(path, LOCK_DEADLINE)
    }

    /// [`Self::acquire`] with an explicit deadline (for testing).
    pub(crate) fn acquire_until(path: &Path, deadline: Duration) -> Option<ExclusiveLock> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let c =
            std::ffi::CString::new(path.as_os_str().to_string_lossy().as_bytes().to_vec()).ok()?;
        // SAFETY: open with a valid NUL-terminated path; -1 is handled.
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o600) };
        if fd < 0 {
            return None;
        }
        // NON-blocking with retry up to the deadline: keeps exclusion (only one
        // holding at a time) but fails CLOSED if a holder is stuck,
        // instead of sitting in `flock(LOCK_EX)` forever.
        let start = Instant::now();
        loop {
            // SAFETY: the fd is ours and is open.
            if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Some(ExclusiveLock(fd));
            }
            let err = std::io::Error::last_os_error();
            // Only `EWOULDBLOCK` means "another holds it"; any other error is real.
            if err.raw_os_error() != Some(libc::EWOULDBLOCK) || start.elapsed() >= deadline {
                // SAFETY: we close our own fd, which we are going to discard.
                unsafe { libc::close(fd) };
                return None;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// The error the caller reports when it could not get the lock.
    ///
    /// `consequence` is what would have happened without it, in text. Naming the
    /// consequence matters: "could not lock" reads as a passing annoyance,
    /// when what it prevents is two networks in the same `/16`.
    pub(crate) fn unavailable(path: &Path, consequence: &str) -> Error {
        Error::Command {
            context: "lock",
            message: format!(
                "could not lock {} — refusing to continue, since {consequence}",
                path.display()
            ),
        }
    }
}

impl Drop for ExclusiveLock {
    fn drop(&mut self) {
        if self.0 >= 0 {
            // SAFETY: own fd, opened in `acquire`.
            unsafe {
                libc::flock(self.0, libc::LOCK_UN);
                libc::close(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_actually_excludes() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("f.lock");

        let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let hs: Vec<_> = (0..8)
            .map(|_| {
                let (p, c, m, b) = (p.clone(), counter.clone(), max.clone(), barrier.clone());
                std::thread::spawn(move || {
                    b.wait();
                    let _l = ExclusiveLock::acquire(&p).expect("lock");
                    // Inside the critical section only ONE thread may be: if
                    // `flock` did not exclude, the counter would go past 1.
                    let n = c.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    m.fetch_max(n, std::sync::atomic::Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    c.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(max.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    /// REGRESSION: a lock HELD by another makes `acquire_until` REFUSE at
    /// the deadline (returns `None`), never hang forever — this was the
    /// hang of the custom-network attach without a timeout.
    #[test]
    fn held_lock_refuses_at_deadline_instead_of_hanging() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("f.lock");
        let held = ExclusiveLock::acquire(&p).expect("first take");
        let t0 = std::time::Instant::now();
        let second = ExclusiveLock::acquire_until(&p, Duration::from_millis(200));
        let elapsed = t0.elapsed();
        assert!(second.is_none(), "should not obtain an already-held lock");
        assert!(
            elapsed >= Duration::from_millis(200) && elapsed < Duration::from_secs(5),
            "should refuse near the deadline (200ms), not hang: {elapsed:?}"
        );
        drop(held);
        // Release it and the next take should work immediately.
        assert!(ExclusiveLock::acquire_until(&p, Duration::from_millis(200)).is_some());
    }

    #[test]
    fn impossible_path_returns_none_and_does_not_fail_open() {
        // A directory where `open` cannot create anything: the lock does NOT
        // invent itself, it returns `None`, and the caller refuses the operation.
        let p = std::path::Path::new("/proc/1/impossivel/f.lock");
        assert!(ExclusiveLock::acquire(p).is_none());
        let e = ExclusiveLock::unavailable(p, "two networks could end up in the same /16");
        assert!(e.to_string().contains("refusing to continue"));
    }
}
