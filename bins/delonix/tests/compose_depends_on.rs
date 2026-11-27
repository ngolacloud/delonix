//! `compose config` with a broken `depends_on` fails with a message, not a
//! panic. Delonix 0.2.0 crashed with `no entry found for key` (exit 101) on a
//! dependency that names no service. These run the real binary: the exit code
//! and the stderr a user sees are the contract. `compose config` only parses
//! and renders, it never reaches the engine; the state dirs point at the
//! test's own temp dir anyway.

use std::process::{Command, Output};

fn compose_config(yaml: &str) -> Output {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("compose.yaml");
    std::fs::write(&file, yaml).unwrap();
    Command::new(env!("CARGO_BIN_EXE_delonix"))
        .env("NO_COLOR", "1")
        .env("DELONIX_ROOT", dir.path().join("root"))
        .env("DELONIX_NET_RUNTIME_DIR", dir.path().join("net"))
        .args(["compose", "config", "-f"])
        .arg(&file)
        .output()
        .unwrap()
}

fn assert_refused(yaml: &str, want: &str) {
    let out = compose_config(yaml);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "must fail: {stderr}");
    assert_ne!(
        out.status.code(),
        Some(101),
        "a panic, not an error: {stderr}"
    );
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(stderr.contains(want), "expected {want:?} in: {stderr}");
}

#[test]
fn an_undeclared_dependency_is_an_error_naming_both_services() {
    // `db` sorts before `web`: the order that used to panic.
    assert_refused(
        "services:\n  web:\n    image: alpine\n    depends_on: [db]\n",
        "service 'web' depends_on undefined service 'db'",
    );
    assert_refused(
        "services:\n  web:\n    image: alpine\n    depends_on:\n      db:\n        condition: service_healthy\n",
        "service 'web' depends_on undefined service 'db'",
    );
}

#[test]
fn a_cycle_and_a_self_dependency_are_errors() {
    assert_refused(
        "services:\n  a:\n    image: alpine\n    depends_on: [b]\n  b:\n    image: alpine\n    depends_on: [a]\n",
        "depends_on cycle: a -> b -> a",
    );
    assert_refused(
        "services:\n  a:\n    image: alpine\n    depends_on: [a]\n",
        "service 'a' depends_on itself",
    );
}

#[test]
fn a_valid_dependency_still_renders() {
    let out = compose_config(
        "services:\n  web:\n    image: alpine\n    depends_on: [db]\n  db:\n    image: alpine\n",
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let (db, web) = (
        stdout.find("service db:").unwrap(),
        stdout.find("service web:").unwrap(),
    );
    assert!(db < web, "db starts before web: {stdout}");
}
