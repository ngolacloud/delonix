//! KRN-0003 fatia 1 — host-side bulk file copy over io_uring, with a std
//! fallback. This is the engine's FIRST use of io_uring, confined to a single
//! function so the rest of the crate stays plain `std::fs`.
//!
//! io_uring is a HOST-side optimization only: the container's seccomp profile
//! keeps `io_uring_setup`/`enter`/`register` denied (see
//! `delonix-linux/src/seccomp_profile.rs`), and nothing here runs inside a
//! container — `copy_file_uring` copies between two host paths the engine
//! already resolved.
//!
//! Robustness: `io_uring_setup` can fail in the wild — an old kernel (< 5.1),
//! `sysctl kernel.io_uring_disabled`, or a low `RLIMIT_MEMLOCK` under rootless.
//! Any failure to build or drive the ring falls back to `std::fs::copy`, which
//! also preserves the file mode. So the fast path is a pure optimization: it can
//! never be the reason a copy fails.

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;

/// Copies `src` to `dst` byte-for-byte and preserves the source's permission
/// bits (like `std::fs::copy`). Uses io_uring when the kernel allows it, else
/// falls back to `std::fs::copy`. Returns the number of bytes copied.
pub fn copy_file_uring(src: &Path, dst: &Path) -> io::Result<u64> {
    match copy_with_uring(src, dst) {
        Ok(n) => Ok(n),
        // Any io_uring problem is non-fatal: the std path does exactly the same
        // copy, mode included. `Unsupported` (no ring) is the common, quiet case;
        // an error after the ring was up is worth a debug line.
        Err(UringCopyError::Unsupported) => std::fs::copy(src, dst),
        Err(UringCopyError::Io(e)) => {
            tracing::debug!("copy_file_uring: io_uring path failed ({e}); using std::fs::copy");
            std::fs::copy(src, dst)
        }
    }
}

enum UringCopyError {
    /// The ring could not be created (old kernel, disabled, MEMLOCK) — fall back.
    Unsupported,
    /// An I/O error after the ring was up — fall back rather than surface it, so
    /// the copy still succeeds via std.
    Io(io::Error),
}

impl From<io::Error> for UringCopyError {
    fn from(e: io::Error) -> Self {
        UringCopyError::Io(e)
    }
}

fn copy_with_uring(src: &Path, dst: &Path) -> Result<u64, UringCopyError> {
    use io_uring::{opcode, types, IoUring};

    // io_uring_setup — the single call that decides whether the fast path is
    // available at all. Treat any failure as "unsupported" → std fallback.
    let mut ring = IoUring::new(8).map_err(|_| UringCopyError::Unsupported)?;

    let src_f = File::open(src)?;
    let dst_f = File::create(dst)?;
    let src_fd = types::Fd(src_f.as_raw_fd());
    let dst_fd = types::Fd(dst_f.as_raw_fd());

    let mut buf = vec![0u8; 256 * 1024];
    let mut offset: u64 = 0;

    loop {
        // READ a block from `src` at `offset`.
        let n = {
            let read_e = opcode::Read::new(src_fd, buf.as_mut_ptr(), buf.len() as u32)
                .offset(offset)
                .build()
                .user_data(0x01);
            // SAFETY: `buf` outlives the operation (we wait for completion before
            // reusing it); `src_fd` is a live fd for the duration.
            unsafe {
                ring.submission()
                    .push(&read_e)
                    .map_err(|_| io::Error::other("io_uring: submission queue full"))?;
            }
            ring.submit_and_wait(1)?;
            let cqe = ring
                .completion()
                .next()
                .ok_or_else(|| io::Error::other("io_uring: no read completion"))?;
            let r = cqe.result();
            if r < 0 {
                return Err(UringCopyError::Io(io::Error::from_raw_os_error(-r)));
            }
            r as usize
        };
        if n == 0 {
            break; // EOF
        }

        // WRITE the `n` bytes to `dst`, looping over short writes.
        let mut written = 0usize;
        while written < n {
            let write_e = opcode::Write::new(
                dst_fd,
                // SAFETY: pointer into `buf` within the valid `[written, n)` range.
                unsafe { buf.as_ptr().add(written) },
                (n - written) as u32,
            )
            .offset(offset + written as u64)
            .build()
            .user_data(0x02);
            // SAFETY: `buf` outlives the operation; `dst_fd` is live.
            unsafe {
                ring.submission()
                    .push(&write_e)
                    .map_err(|_| io::Error::other("io_uring: submission queue full"))?;
            }
            ring.submit_and_wait(1)?;
            let cqe = ring
                .completion()
                .next()
                .ok_or_else(|| io::Error::other("io_uring: no write completion"))?;
            let w = cqe.result();
            if w < 0 {
                return Err(UringCopyError::Io(io::Error::from_raw_os_error(-w)));
            }
            if w == 0 {
                return Err(UringCopyError::Io(io::Error::other(
                    "io_uring: zero-length write",
                )));
            }
            written += w as usize;
        }
        offset += n as u64;
    }

    // Preserve the source mode, like `std::fs::copy`, then durably commit.
    let perm = src_f.metadata()?.permissions();
    dst_f.set_permissions(perm)?;
    dst_f.sync_all()?;
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::copy_file_uring;
    use crate::cas::sha256_file;
    use std::io::Write as _;

    /// Correctness: an io_uring copy of a multi-block file is byte-identical to
    /// the source (same sha256, same length) and keeps the mode. This exercises
    /// the REAL io_uring path on a kernel that supports it; on one that does not,
    /// the std fallback produces the same result, so the assertions hold either
    /// way — which is exactly the guarantee we want.
    #[test]
    fn a_uring_copy_is_byte_identical_and_keeps_the_mode() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = std::env::temp_dir().join(format!("dlx-uring-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.bin");
        let dst = dir.join("dst.bin");

        // ~1.5 MiB so the copy loop runs several read/write blocks.
        let mut f = std::fs::File::create(&src).unwrap();
        let block = vec![0xABu8; 64 * 1024];
        for i in 0..24u8 {
            let mut b = block.clone();
            b[0] = i; // make each block distinct
            f.write_all(&b).unwrap();
        }
        f.sync_all().unwrap();
        std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o640)).unwrap();

        let n = copy_file_uring(&src, &dst).unwrap();
        assert_eq!(n, 24 * 64 * 1024, "copied byte count");
        assert_eq!(
            sha256_file(&src).unwrap(),
            sha256_file(&dst).unwrap(),
            "the copy must be byte-identical to the source"
        );
        assert_eq!(
            std::fs::metadata(&dst).unwrap().permissions().mode() & 0o777,
            0o640,
            "the copy must keep the source mode, like std::fs::copy"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An empty file copies cleanly (the loop takes the EOF branch immediately).
    #[test]
    fn an_empty_file_copies_to_an_empty_file() {
        let dir = std::env::temp_dir().join(format!("dlx-uring-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("e.bin");
        let dst = dir.join("e-copy.bin");
        std::fs::File::create(&src).unwrap();
        assert_eq!(copy_file_uring(&src, &dst).unwrap(), 0);
        assert_eq!(std::fs::metadata(&dst).unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
