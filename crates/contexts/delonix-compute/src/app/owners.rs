//! The ownership an image records for its files, as data (ADR-0062).
//!
//! Written by the image store when a layer is unpacked and read by the
//! container's init, which gives back exactly the entries the image gave to a
//! user or group other than root. The two sides live in different adapters, so
//! the record and its on-disk form live here, once.

use std::path::{Component, Path, PathBuf};

/// The file, beside a container's `merged/`, that holds its image's owners.
pub const OWNERS_FILE: &str = "overlay-owners";

/// One entry the image gives to a user or group other than root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// Relative to the root filesystem: no leading `/`, no `..`.
    pub path: PathBuf,
    pub uid: u32,
    pub gid: u32,
}

/// `path` as a path relative to the rootfs, or `None` when it cannot name
/// something inside it (`..`, or nothing left). A leading `./` or `/` is dropped.
pub fn relative(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(p) => out.push(p),
            Component::CurDir | Component::RootDir => {}
            Component::ParentDir | Component::Prefix(_) => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// Resolves `rel` against `root` the way runc scopes a bind target: component by
/// component, so a symlink an image planted can never send a write OUT of
/// `root`. An absolute symlink restarts the walk at `root`; `..` stops at
/// `root`; an intermediate symlink is followed only INSIDE `root`; a component
/// that is not a symlink (including one that does not exist yet) is taken
/// literally. The returned path is therefore always within `root`, so the
/// caller may `create_dir_all`/`remove_file`/`remove_dir_all`/`mknod` on it — or
/// on a single leaf joined to it — without the join FOLLOWING a link out of
/// `root`, which is the symlink-escape that `std::fs`'s path-following
/// operations are blind to.
///
/// `None` only for a link chain too long to be anything but a loop.
///
/// This is the ONE primitive every scoped-path site in the engine shares.
/// `delonix_linux::safe_bind_target` (bind / `cp` / `COPY`) delegates to it, and
/// so do both OCI whiteout paths (`write_overlay_whiteout` on the per-layer
/// overlay, and the flat-bundle whiteout branch): the whole lesson of the
/// `cp`/`COPY` symlink-escape class is to spell this out once, so the next write
/// path cannot forget the guard the way the overlay whiteout did (DC-1/V1).
pub fn scoped_resolve(root: &Path, rel: &Path) -> Option<PathBuf> {
    use std::collections::VecDeque;
    use std::ffi::OsString;
    const MAX_LINKS: u32 = 40;
    let parts = |p: &Path| -> Vec<OsString> {
        p.components()
            .filter_map(|c| match c {
                Component::Normal(n) => Some(n.to_os_string()),
                Component::ParentDir => Some(OsString::from("..")),
                _ => None,
            })
            .collect()
    };
    let mut pending: VecDeque<OsString> = parts(rel).into();
    // The resolved path so far, relative to `root`: real components only.
    let mut resolved: Vec<OsString> = Vec::new();
    let mut links = 0u32;
    while let Some(c) = pending.pop_front() {
        if c == ".." {
            resolved.pop(); // never above `root`
            continue;
        }
        let mut here = root.to_path_buf();
        here.extend(&resolved);
        here.push(&c);
        let is_link = std::fs::symlink_metadata(&here).is_ok_and(|m| m.file_type().is_symlink());
        if !is_link {
            resolved.push(c);
            continue;
        }
        links += 1;
        if links > MAX_LINKS {
            return None;
        }
        let dest = std::fs::read_link(&here).ok()?;
        if dest.is_absolute() {
            resolved.clear();
        }
        for part in parts(&dest).into_iter().rev() {
            pending.push_front(part);
        }
    }
    let mut out = root.to_path_buf();
    out.extend(&resolved);
    Some(out)
}

/// On-disk form: one `uid gid path` record per entry, NUL-terminated. A path
/// may hold a newline or a space; it cannot hold a NUL.
pub fn encode(owners: &[Owner]) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    let mut out = Vec::new();
    for o in owners {
        out.extend_from_slice(format!("{} {} ", o.uid, o.gid).as_bytes());
        out.extend_from_slice(o.path.as_os_str().as_bytes());
        out.push(0);
    }
    out
}

/// Reads what [`encode`] wrote. A record that does not parse, or whose path is
/// not confined to the rootfs, is dropped: the index is advice about ownership,
/// and a damaged line must not become a path the init acts on.
pub fn decode(bytes: &[u8]) -> Vec<Owner> {
    use std::os::unix::ffi::OsStrExt;
    bytes
        .split(|b| *b == 0)
        .filter_map(|rec| {
            let mut parts = rec.splitn(3, |b| *b == b' ');
            let uid = std::str::from_utf8(parts.next()?).ok()?.parse().ok()?;
            let gid = std::str::from_utf8(parts.next()?).ok()?.parse().ok()?;
            let path = relative(Path::new(std::ffi::OsStr::from_bytes(parts.next()?)))?;
            Some(Owner { path, uid, gid })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(path: &str, uid: u32, gid: u32) -> Owner {
        Owner {
            path: path.into(),
            uid,
            gid,
        }
    }

    /// The on-disk form survives a path with a space and a newline, and a record
    /// that tries to leave the rootfs is dropped on the way in.
    #[test]
    fn the_index_round_trips_and_refuses_escapes() {
        let owners = vec![owner("a b/c\nd", 1000, 1000), owner("x", 0, 42)];
        assert_eq!(decode(&encode(&owners)), owners);
        let hostile = b"99 99 ../../home/user/.ssh\x0099 99 ok\x00garbage\x00";
        assert_eq!(decode(hostile), vec![owner("ok", 99, 99)]);
        assert!(decode(b"").is_empty());
        assert_eq!(
            relative(Path::new("/./etc/passwd")),
            Some("etc/passwd".into())
        );
    }
}
