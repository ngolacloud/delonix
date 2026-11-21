//! Regression: the `/16` allocation in `infra::network_create` was a
//! read-then-write **with no lock**.
//!
//! Two parallel creations read the same set of already-used prefixes and chose
//! the SAME one. Measured before the fix: 10 concurrent creations gave 2 to 4
//! distinct prefixes; 20 gave 10. The bridges differ (the name derives from the
//! network's), so the networks LOOK separate — but the workloads draw addresses
//! from the same `/16`, and any rule indexed on an IP becomes ambiguous between
//! two networks the operator believes are isolated.
//!
//! Filesystem only: `network_create` touches neither netlink nor namespaces,
//! which makes the race testable without privileges. Threads (not processes)
//! are enough — `flock` is per *open file description*, and each `acquire()`
//! does its own `open`, so threads in the same process do exclude each other.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// `DELONIX_ROOT` is read from the PROCESS environment. Setting it per test
/// makes parallel tests fight over the same variable, so there is one root per
/// process and the variable is set once. This only names the path; the
/// directory itself exists while a [`RootLease`] does.
fn root() -> &'static std::path::PathBuf {
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let tmp = std::env::temp_dir();
        sweep_dead_roots(&tmp);
        let d = tmp.join(format!("{ROOT_PREFIX}{}", std::process::id()));
        std::env::set_var("DELONIX_ROOT", &d);
        d
    })
}

const ROOT_PREFIX: &str = "delonix-net-race-";

/// libtest has no "after all tests" hook, and it does not tell a test how many
/// others will run (filters, `--test-threads`), so "the last test removes the
/// root" cannot be decided by counting tests. It is decided by counting the
/// tests USING the root right now: the first lease creates it, the last one to
/// be dropped removes it — also when that test panics, because `Drop` runs on
/// unwind. If a later test starts after the count hit zero, it simply gets a
/// fresh root; no test here depends on another's networks. Measured on
/// 2026-09-28 before this: 2400 runs left ~2400 roots in the temp dir.
static LEASES: Mutex<usize> = Mutex::new(0);

struct RootLease(&'static std::path::Path);

fn lease() -> RootLease {
    let root = root();
    let mut n = LEASES.lock().unwrap_or_else(|e| e.into_inner());
    if *n == 0 {
        let _ = std::fs::remove_dir_all(root);
        std::fs::create_dir_all(root).unwrap();
    }
    *n += 1;
    RootLease(root)
}

impl std::ops::Deref for RootLease {
    type Target = std::path::Path;
    fn deref(&self) -> &std::path::Path {
        self.0
    }
}

impl Drop for RootLease {
    fn drop(&mut self) {
        let mut n = LEASES.lock().unwrap_or_else(|e| e.into_inner());
        *n -= 1;
        if *n == 0 {
            let _ = std::fs::remove_dir_all(self.0);
        }
    }
}

/// The lease does not run when the process is KILLED (Ctrl-C, a CI timeout,
/// SIGKILL), so each process also removes the roots of runs that died that
/// way: a `delonix-net-race-<pid>` whose pid is no longer alive has no owner
/// left to use it. A live pid is left alone even if it was reused by something
/// else — the cost of being wrong that way is one stale directory. Without
/// `/proc` every pid would look dead, so nothing is swept then.
fn sweep_dead_roots(tmp: &std::path::Path) {
    let proc = std::path::Path::new("/proc");
    if !proc.join("self").exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(tmp) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|n| n.strip_prefix(ROOT_PREFIX))
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != std::process::id() && !proc.join(pid.to_string()).exists() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// The three tests share the root and run in parallel. The ones that CREATE
/// networks may overlap each other — that is the race they measure — so they
/// take this lock shared; the one that INSPECTS the registry folder takes it
/// exclusive, to see it quiet. Without it, it caught the
/// `.<record>.json.<pid>.<n>.tmp` another test's `write_atomic` had open at
/// that instant (CI `test (arm64)`, 2026-09-28; 506/800 runs locally with the
/// CPU constrained).
static REGISTRY: RwLock<()> = RwLock::new(());

fn writing() -> RwLockReadGuard<'static, ()> {
    REGISTRY.read().unwrap_or_else(|e| e.into_inner())
}

fn quiet() -> RwLockWriteGuard<'static, ()> {
    REGISTRY.write().unwrap_or_else(|e| e.into_inner())
}

/// Creates `n` networks in parallel and returns the resulting `NetDef`s.
/// The caller holds a [`RootLease`] for as long as it reads the results.
fn create_in_parallel(name_prefix: &str, n: usize) -> Vec<delonix_sdn::infra::NetDef> {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(n));
    let mut hs = Vec::with_capacity(n);
    for i in 0..n {
        let name = format!("{name_prefix}-{i}");
        let b = barrier.clone();
        hs.push(std::thread::spawn(move || {
            // All threads release at the same time — without this, starting in
            // single file hides the race the test exists to catch.
            b.wait();
            delonix_sdn::infra::network_create(&name)
        }));
    }
    hs.into_iter()
        .map(|h| {
            h.join()
                .expect("thread panicked")
                .expect("network_create failed")
        })
        .collect()
}

#[test]
fn concurrent_creations_do_not_share_the_same_16() {
    let _g = writing();
    let _root = lease();
    let defs = create_in_parallel("corrida", 16);

    let prefixes: HashSet<&str> = defs.iter().map(|d| d.prefix.as_str()).collect();
    assert_eq!(
        prefixes.len(),
        defs.len(),
        "two distinct networks ended up on the same /16 — prefixes: {:?}",
        defs.iter()
            .map(|d| (&d.name, &d.prefix))
            .collect::<Vec<_>>()
    );

    // And what landed on disk must agree with what was returned: one write
    // clobbering another gives unique prefixes in memory and duplicates on disk.
    let on_disk: Vec<_> = delonix_sdn::infra::network_list()
        .into_iter()
        .filter(|d| d.name.starts_with("corrida-"))
        .collect();
    assert_eq!(on_disk.len(), defs.len(), "networks lost on disk");
    let disk_prefixes: HashSet<String> = on_disk.iter().map(|d| d.prefix.clone()).collect();
    assert_eq!(
        disk_prefixes.len(),
        defs.len(),
        "duplicate prefixes on disk: {on_disk:?}"
    );
}

/// Honesty NOTE: this passes on the OLD code too (6/6 runs). With the registry
/// empty, all threads read the same `used`, pick the same prefix and write
/// IDENTICAL `NetDef`s — one write clobbering another is invisible when the
/// content matches. It stays as a guard of the invariant (and of the recheck
/// inside the lock), not as proof of the fix: that is
/// `concurrent_creations_do_not_share_the_same_16`.
#[test]
fn the_same_name_in_parallel_converges_to_one_network() {
    let _g = writing();
    let _root = lease();
    const N: usize = 12;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(N));
    let hs: Vec<_> = (0..N)
        .map(|_| {
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                delonix_sdn::infra::network_create("mesmo-nome")
            })
        })
        .collect();
    let defs: Vec<_> = hs
        .into_iter()
        .map(|h| h.join().unwrap().expect("network_create failed"))
        .collect();

    // The recheck INSIDE the lock is what guarantees this: without it, whoever
    // arrived later rewrote the winner's `NetDef` with a different prefix,
    // changing the bridge under whatever was already attached to it.
    let prefixes: HashSet<&str> = defs.iter().map(|d| d.prefix.as_str()).collect();
    assert_eq!(
        prefixes.len(),
        1,
        "the same name gave different networks: {prefixes:?}"
    );
    let bridges: HashSet<&str> = defs.iter().map(|d| d.bridge.as_str()).collect();
    assert_eq!(
        bridges.len(),
        1,
        "the same name gave different bridges: {bridges:?}"
    );

    assert_eq!(
        delonix_sdn::infra::network_list()
            .iter()
            .filter(|d| d.name == "mesmo-nome")
            .count(),
        1
    );
}

#[test]
fn the_lock_does_not_land_in_the_networks_registry() {
    // The lock lives ALONGSIDE the registry. If it fell inside it, `network_list`
    // would have to skip it by accident (by failing to parse) rather than by
    // design.
    //
    // The assertion stays STRICT — anything that is not `.json` fails, temps
    // included — and it is the quiet folder that makes it deterministic, not an
    // exemption for `*.tmp`. With the writes finished, a temp still there is no
    // longer in flight: it is junk `write_atomic` left behind, and that must
    // break this test too.
    let _g = quiet();
    let root = lease();
    let _ = create_in_parallel("vizinha", 2);
    let dir = root.join("ingress").join("networks");
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        assert!(
            n.ends_with(".json"),
            "unexpected file in the networks registry: {n}"
        );
    }
    assert!(root.join("ingress").join("networks.lock").exists());
}
