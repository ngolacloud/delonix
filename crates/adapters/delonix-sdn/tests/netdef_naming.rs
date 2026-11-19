//! Regression: the record for an ingress network was named with `sanitize(name)`,
//! which **truncates to 12 characters**.
//!
//! That limit exists for DEVICE names (the kernel has `IFNAMSIZ`); in a file
//! name it is pure loss. `producao-alpha` and `producao-alpine` shared the file
//! `producao-alp.json` — and what a record stores is the BRIDGE, so the second
//! network's workloads ended up on the first network's bridge, while the two
//! still showed up as separate in `network ls`.
//!
//! Filesystem only: `network_create` touches neither netlink nor namespaces.

use std::sync::{Mutex, OnceLock};

/// Names the root and sets `DELONIX_ROOT` once per process; the directory
/// itself exists while a [`RootLease`] does.
fn root() -> &'static std::path::PathBuf {
    static ROOT: OnceLock<std::path::PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        // Sweep what PREVIOUS runs left behind: the folder is named by the pid
        // and nothing removed it at the end, so every `cargo test` left one
        // behind — 23 were measured in `/tmp` on 2026-08-15, and they were what
        // made a chaos check that counted `/tmp/delonix-net-*` USELESS (it
        // passed with the fix reverted, because the glob caught these
        // leftovers).
        //
        // Only pids that are DEAD get removed. It is the same ownership
        // principle this series applied to the slirp reaper and to
        // `kill_pidfile`: a destructive sweep proves the target is not in use,
        // and two parallel runs (several sessions work on this clone) do not
        // destroy each other.
        //
        // The sweep only covers runs that were KILLED; a run that ends normally
        // removes its own root through the last [`RootLease`].
        sweep_dead_runs();
        let d = std::env::temp_dir().join(format!("delonix-net-naming-{}", std::process::id()));
        std::env::set_var("DELONIX_ROOT", &d);
        d
    })
}

/// The same lease as `network_alloc_race.rs` (#567): libtest has no "after all
/// tests" hook, so the root is removed when the last test USING it drops its
/// lease — also on panic, because `Drop` runs on unwind. A test that starts
/// after the count hit zero gets a fresh root; none of these tests reads
/// another's networks. Before this, every run left its root behind for the
/// sweep above to find on the NEXT run, so the last run's root never went.
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

impl Drop for RootLease {
    fn drop(&mut self) {
        let mut n = LEASES.lock().unwrap_or_else(|e| e.into_inner());
        *n -= 1;
        if *n == 0 {
            let _ = std::fs::remove_dir_all(self.0);
        }
    }
}

/// Removes the `delonix-net-naming-<pid>` folders whose process no longer exists.
///
/// `/proc/<pid>` is the proof; without it nothing is touched. Best-effort
/// end to end — a failure here can never fail a test that is not about cleanup.
fn sweep_dead_runs() {
    let tmp = std::env::temp_dir();
    let Ok(rd) = std::fs::read_dir(&tmp) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(pid) = name.strip_prefix("delonix-net-naming-") else {
            continue;
        };
        if pid.parse::<u32>().is_ok() && !std::path::Path::new(&format!("/proc/{pid}")).exists() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

fn networks_dir() -> std::path::PathBuf {
    root().join("ingress").join("networks")
}

#[test]
fn names_sharing_the_same_12_chars_do_not_share_a_record() {
    let _root = lease();
    let a = delonix_sdn::infra::network_create("producao-alpha").expect("alpha");
    let b = delonix_sdn::infra::network_create("producao-alpine").expect("alpine");

    assert_ne!(a.prefix, b.prefix, "two networks on the same /16");
    assert_ne!(a.bridge, b.bridge, "two networks on the same bridge");

    // And each name resolves to ITS OWN record — before, the second
    // `network_get` returned the first, with the wrong bridge.
    let ga = delonix_sdn::infra::network_get("producao-alpha").expect("get alpha");
    let gb = delonix_sdn::infra::network_get("producao-alpine").expect("get alpine");
    assert_eq!(
        (ga.name.as_str(), ga.bridge.as_str()),
        ("producao-alpha", a.bridge.as_str())
    );
    assert_eq!(
        (gb.name.as_str(), gb.bridge.as_str()),
        ("producao-alpine", b.bridge.as_str())
    );

    let listed: Vec<String> = delonix_sdn::infra::network_list()
        .into_iter()
        .map(|d| d.name)
        .filter(|n| n.starts_with("producao-"))
        .collect();
    assert_eq!(listed.len(), 2, "`ls` should see both: {listed:?}");
}

#[test]
fn removing_one_does_not_destroy_a_similarly_named_neighbor() {
    let _root = lease();
    let keep = delonix_sdn::infra::network_create("contabilidade-a").expect("a");
    delonix_sdn::infra::network_create("contabilidade-b").expect("b");

    delonix_sdn::infra::network_remove("contabilidade-b");

    // The second network's `rm` sent `netdel` to the FIRST network's bridge and
    // deleted its record — the network nobody asked to remove was left with no
    // bridge and no file, and the operator only found out when its workloads
    // went silent.
    let survivor = delonix_sdn::infra::network_get("contabilidade-a")
        .expect("the network that was not removed disappeared");
    assert_eq!(survivor.bridge, keep.bridge);
    assert_eq!(survivor.prefix, keep.prefix);
    assert!(delonix_sdn::infra::network_get("contabilidade-b").is_none());
}

#[test]
fn removing_a_nonexistent_name_touches_nothing() {
    let _root = lease();
    let before = delonix_sdn::infra::network_create("logistica-primaria").expect("create");
    // Shares the first 12 characters with the one above, and was never created.
    delonix_sdn::infra::network_remove("logistica-primaria-2");
    let after = delonix_sdn::infra::network_get("logistica-primaria").expect("still there");
    assert_eq!(after.bridge, before.bridge);
}

#[test]
fn legacy_record_is_still_read_and_the_write_migrates_it() {
    let _root = lease();
    // A record written by the OLD formula (truncated to 12), as it sits today on
    // the disk of anyone already running this. Losing it would mean losing the
    // bridge and the /16 of a network possibly with workloads attached.
    std::fs::create_dir_all(networks_dir()).unwrap();
    let legacy = networks_dir().join("arquivo-anti.json");
    std::fs::write(
        &legacy,
        r#"{"name":"arquivo-antigo","bridge":"dlxnaaaabbbb","prefix":"10.249"}"#,
    )
    .unwrap();

    let read = delonix_sdn::infra::network_get("arquivo-antigo").expect("the legacy record should be read");
    assert_eq!(read.bridge, "dlxnaaaabbbb");
    assert_eq!(read.prefix, "10.249");

    // One network, not two, even mid-migration.
    assert_eq!(
        delonix_sdn::infra::network_list()
            .iter()
            .filter(|d| d.name == "arquivo-antigo")
            .count(),
        1
    );

    // The next write migrates it: `network_create` is idempotent by name, so it
    // returns the existing record without changing its prefix.
    let again = delonix_sdn::infra::network_create("arquivo-antigo").expect("idempotent");
    assert_eq!(
        again.prefix, "10.249",
        "idempotency must not renumber the network"
    );

    // The legacy file only disappears once something rewrites it — force it
    // through a path that does write (declaring the gateway).
    delonix_sdn::infra::network_create_with_gateway("arquivo-antigo", "10.249", Some("10.249.0.9"))
        .expect("declare gateway");
    assert!(!legacy.exists(), "the legacy record was left behind");
    assert_eq!(
        delonix_sdn::infra::network_get("arquivo-antigo")
            .unwrap()
            .bridge,
        "dlxnaaaabbbb",
        "the migration must not invent a new bridge"
    );
}

/// NOTE: this one passes on the old code too, for a different reason (the file
/// name is not what the old formula looked for). It guards the NEW formula —
/// that the `name` field check keeps existing — and is not proof of the fix;
/// that is the four above.
#[test]
fn a_record_from_another_network_is_not_accepted_as_this_one() {
    let _root = lease();
    // Defense in depth: even if two names fall into the same file, the record's
    // `name` field is the authority. The result is "does not exist" — a
    // refusal — and never the wrong network.
    std::fs::create_dir_all(networks_dir()).unwrap();
    std::fs::write(
        networks_dir().join("intruso-0000ffff.json"),
        r#"{"name":"outra-qualquer","bridge":"dlxn00000000","prefix":"10.248"}"#,
    )
    .unwrap();
    assert!(delonix_sdn::infra::network_get("intruso").is_none());
}
