//! Unit tests for the wire protocol — pure parsing and an in-process round trip
//! over a socket pair, so no `delonix` binary is spawned.

use super::*;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::io::IntoRawFd;
use std::os::unix::net::UnixStream;

#[test]
fn ok_reply_parses_as_success() {
    let o = parse_outcome("OK");
    assert!(o.ok());
    assert_eq!(o.reason, None);
}

#[test]
fn err_reply_keeps_code_and_reason() {
    let o = parse_outcome("ERR 42 image not found: alpine");
    assert_eq!(o.code, 42);
    assert_eq!(o.reason.as_deref(), Some("image not found: alpine"));
    assert!(!o.ok());
}

#[test]
fn err_without_reason_still_fails_with_a_message() {
    let o = parse_outcome("ERR 7");
    assert_eq!(o.code, 7);
    assert!(o.reason.is_some());
}

#[test]
fn unknown_reply_is_a_failure_not_a_silent_success() {
    let o = parse_outcome("garbage");
    assert!(!o.ok());
    assert!(o.reason.unwrap().contains("unexpected"));
}

/// The full parked-runner protocol, driven in-process: a thread plays the
/// runner (`serve_parked`), the test plays the pool. Proves READY precedes the
/// spec and the outcome comes back shaped as the pool expects.
#[test]
fn serve_parked_round_trips_a_start() {
    let (pool_end, runner_end) = UnixStream::pair().unwrap();
    let runner_fd = runner_end.into_raw_fd();

    let warmed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let w2 = warmed.clone();
    let handle = std::thread::spawn(move || {
        serve_parked(
            runner_fd,
            move || w2.store(true, std::sync::atomic::Ordering::SeqCst),
            |spec| {
                assert_eq!(spec.to_str(), Some("/tmp/spec-xyz.json"));
                StartOutcome {
                    code: 0,
                    reason: None,
                }
            },
        )
    });

    let mut writer = pool_end.try_clone().unwrap();
    let mut reader = BufReader::new(pool_end);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "READY");
    assert!(warmed.load(std::sync::atomic::Ordering::SeqCst));

    writer.write_all(b"/tmp/spec-xyz.json\n").unwrap();
    writer.flush().unwrap();

    line.clear();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line.trim(), "OK");

    handle.join().unwrap().unwrap();
}

/// A failing start brings its reason back across the wire, newline-flattened.
#[test]
fn serve_parked_reports_failure_reason() {
    let (pool_end, runner_end) = UnixStream::pair().unwrap();
    let runner_fd = runner_end.into_raw_fd();
    let handle = std::thread::spawn(move || {
        serve_parked(
            runner_fd,
            || {},
            |_spec| StartOutcome {
                code: 125,
                reason: Some("policy refused\nthe source override".to_string()),
            },
        )
    });

    let mut writer = pool_end.try_clone().unwrap();
    let mut reader = BufReader::new(pool_end);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap(); // READY
    writer.write_all(b"/tmp/s.json\n").unwrap();
    writer.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    let o = parse_outcome(line.trim());
    assert_eq!(o.code, 125);
    assert_eq!(o.reason.as_deref(), Some("policy refused the source override"));

    handle.join().unwrap().unwrap();
}
