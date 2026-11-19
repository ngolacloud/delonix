//! The functions `Net` took with it, restored on the rootless path.
//!
//! Tests what can be tested without a holder: the VALIDATIONS and the report.
//! What needs a netns (the attach itself, rule removal) is left to live
//! validation — a test that needs a holder stops running in CI and becomes
//! decoration.

use std::io::Write;

/// `import_iptables` does not touch the host: it reads, counts and reports.
#[test]
fn import_iptables_counts_without_applying() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("save.txt");
    let mut fh = std::fs::File::create(&f).unwrap();
    writeln!(
        fh,
        "*filter\n:INPUT ACCEPT [0:0]\n:FORWARD DROP [0:0]\n-A INPUT -i lo -j ACCEPT\nCOMMIT"
    )
    .unwrap();

    let r = delonix_sdn::infra::import_iptables(&f).expect("should parse");
    assert!(r.contains("1 table"), "table count: {r}");
    assert!(r.contains("2 chain"), "chain count: {r}");
    assert!(r.contains("1 rule"), "rule count: {r}");
}

/// A file that does not exist gives a NAMED error — not a report of zeros.
///
/// The distinction matters: "there are no rules at all" and "I could not read
/// the file" lead whoever is migrating to opposite decisions.
#[test]
fn import_iptables_rejects_a_missing_file() {
    let e = delonix_sdn::infra::import_iptables(std::path::Path::new("/nao/existe/save.txt"))
        .expect_err("should fail");
    let t = e.to_string();
    assert!(
        t.contains("save.txt"),
        "the error should name the file: {t}"
    );
}

/// The IP of an attach with a chosen address is validated against the network.
///
/// An address outside the prefix is not an exotic request, it is a mistake:
/// applying it would give an unreachable container with a successful attach —
/// the worst of both worlds, because nothing flags the problem.
#[test]
fn attach_on_ip_rejects_an_address_outside_the_network() {
    // The only test in this binary that reads `DELONIX_ROOT`, so a per-test
    // dir is safe even though the variable is process-wide — re-checked
    // 2026-10-07 against `import_iptables_*`, which touch no state root.
    let d = tempfile::tempdir().unwrap();
    std::env::set_var("DELONIX_ROOT", d.path());

    let net = delonix_sdn::infra::network_create("rede-teste").expect("create network");
    let outside = "10.99.0.5";
    assert!(
        !outside.starts_with(&net.prefix),
        "the test needs an IP outside {}",
        net.prefix
    );

    let e = delonix_sdn::infra::attach_container_on_ip("abc123", "rede-teste", outside, "")
        .expect_err("should reject an IP from another network");
    let t = e.to_string();
    assert!(t.contains(outside), "the error should name the IP: {t}");
    assert!(t.contains("rede-teste"), "and the network: {t}");

    // A removal that fails FAILS the test: `TempDir`'s own `Drop` ignores the
    // error, and a cleanup that reports nothing is how a stray `.tmpXXXXXX`
    // comes to sit in `TMPDIR` (`scripts/tmp_roots_gate.py`).
    //
    // The pin is deliberately NOT removed afterwards — `scripts/arch_fitness.py`
    // ratchets the number of env writes, and a second one here would be new
    // debt for no gain: what made a dead root dangerous was production code
    // RESOLVING it to create state, and since 2026-10-07 the one path that did
    // (`NetworkStore`'s default-octet cache) uses the store's own root
    // (`delonix_sdn::testenv::TempRoot` records the whole failure).
    d.close().expect("temp root removed");
}
