//! Bounded exclusive file lock (`flock`).
//!
//! The engine serialises many read-modify-writes with `flock(LOCK_EX)` — the
//! container record store, the secret store, the SDN registries, the IP pool,
//! the ingress proxy, the hosts file. A BARE blocking `LOCK_EX` waits FOREVER
//! when a holder is stuck or leaked (a wedged netns/slirp setup that never
//! released, a crashed op that died holding it): a container network attach then
//! hangs with no log and no exit — the systemic cause behind the custom-network
//! attach hang.
//!
//! This bounds the wait. The lock itself is unchanged — still exclusive, still
//! one holder at a time; only "wait forever" becomes "wait up to a deadline,
//! then let the caller REFUSE". Every caller already has a refuse path (an
//! `Err`, a `None`), so failing closed at the deadline is strictly better than
//! hanging.

use std::os::unix::io::RawFd;
use std::time::{Duration, Instant};

/// How long to wait for an exclusive `flock` before giving up. The critical
/// sections it guards are short (decide an address, write one record), so real
/// contention clears in milliseconds — reaching this means a holder is stuck.
pub const FLOCK_DEADLINE: Duration = Duration::from_secs(30);

/// Takes `LOCK_EX` on `fd`, waiting at most `deadline`.
///
/// * `Ok(true)` — acquired (the lock is held until `fd` is closed/unlocked).
/// * `Ok(false)` — still held by another when the deadline passed; the caller
///   MUST refuse the operation, exactly as it would on any other lock failure.
/// * `Err` — a real `flock` error (not contention).
///
/// Never blocks past `deadline`: non-blocking (`LOCK_NB`) retry with a short
/// sleep, and `EINTR` is retried transparently.
pub fn flock_exclusive_until(fd: RawFd, deadline: Duration) -> std::io::Result<bool> {
    let start = Instant::now();
    loop {
        // SAFETY: the caller passes an open fd it owns; `flock` takes the fd and
        // an integer op and touches no memory of ours.
        if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(true);
        }
        let e = std::io::Error::last_os_error();
        match e.raw_os_error() {
            Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => {
                if start.elapsed() >= deadline {
                    return Ok(false);
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Some(code) if code == libc::EINTR => continue,
            _ => return Err(e),
        }
    }
}

/// [`flock_exclusive_until`] with the engine default [`FLOCK_DEADLINE`].
pub fn flock_exclusive(fd: RawFd) -> std::io::Result<bool> {
    flock_exclusive_until(fd, FLOCK_DEADLINE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::io::AsRawFd;

    /// A lock already held by another fd is NOT granted, and the call RETURNS
    /// (Ok(false)) at the deadline instead of hanging forever.
    #[test]
    fn a_held_lock_times_out_instead_of_hanging() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let held = std::fs::OpenOptions::new()
            .write(true)
            .open(tmp.path())
            .unwrap();
        assert!(
            flock_exclusive(held.as_raw_fd()).unwrap(),
            "first holder takes it"
        );

        let other = std::fs::OpenOptions::new()
            .write(true)
            .open(tmp.path())
            .unwrap();
        let t0 = Instant::now();
        let got = flock_exclusive_until(other.as_raw_fd(), Duration::from_millis(200)).unwrap();
        let waited = t0.elapsed();
        assert!(!got, "a held lock must not be granted");
        assert!(
            waited >= Duration::from_millis(200) && waited < Duration::from_secs(5),
            "must return near the deadline, not hang: {waited:?}"
        );

        // Release the first holder; now the second path can take it at once.
        drop(held);
        assert!(flock_exclusive(other.as_raw_fd()).unwrap());
    }
}
