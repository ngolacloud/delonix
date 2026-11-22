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

#[test]
fn host_request_round_trips_as_a_bare_spec() {
    let req = StartRequest::host("/tmp/spec-1.json");
    assert_eq!(req.encode(), "/tmp/spec-1.json");
    assert_eq!(parse_request(&req.encode()), req);
    assert!(parse_request(&req.encode()).plane.is_none());
}

#[test]
fn isolated_request_carries_the_plane_across_the_wire() {
    let req = StartRequest::isolated(
        "/tmp/spec-2.json",
        ContainerPlane {
            id: "abc123".into(),
            netns: "abc123".into(),
            ip: "10.231.0.2".into(),
        },
    );
    assert_eq!(req.encode(), "/tmp/spec-2.json\tabc123\tabc123\t10.231.0.2");
    let back = parse_request(&req.encode());
    assert_eq!(back, req);
    let plane = back.plane.expect("isolated start keeps its plane");
    assert_eq!(plane.netns, "abc123");
    assert_eq!(plane.ip, "10.231.0.2");
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
            |req| {
                assert_eq!(req.spec.to_str(), Some("/tmp/spec-xyz.json"));
                assert!(req.plane.is_none(), "a bare spec is a host start");
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
            |_req| StartOutcome {
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
    assert_eq!(
        o.reason.as_deref(),
        Some("policy refused the source override")
    );

    handle.join().unwrap().unwrap();
}

// ---------------------------------------------------------------------------
// WarmPlane eviction policy — pure, no runner spawned.
// ---------------------------------------------------------------------------

#[test]
fn evictions_none_when_there_is_room() {
    // Two images of 2 parked each (4), cap 8, one more image of 2 fits.
    assert_eq!(evictions_for(&[2, 2], 2, 8), 0);
}

#[test]
fn evictions_drop_the_least_recently_used_first() {
    // MRU-first [2, 2, 2] = 6 parked, cap 6, adding 2 needs 2 freed → evict one
    // tail sub-pool (the LRU) of 2.
    assert_eq!(evictions_for(&[2, 2, 2], 2, 6), 1);
}

#[test]
fn evictions_can_clear_several_cold_pools() {
    // [1, 1, 1, 1] = 4, cap 4, adding 3 needs 3 freed → evict three LRU pools.
    assert_eq!(evictions_for(&[1, 1, 1, 1], 3, 4), 3);
}

#[test]
fn evictions_stop_once_the_incoming_pool_fits() {
    // [3, 1, 1] = 5 MRU-first, cap 4, adding 1: evict LRU(1) → 4, still +1 > 4,
    // evict next LRU(1) → 3, +1 = 4 ≤ 4, stop. Two evicted, the hot head kept.
    assert_eq!(evictions_for(&[3, 1, 1], 1, 4), 2);
}

#[test]
fn a_fresh_plane_is_empty_and_knows_its_pin() {
    let plane = WarmPlane::new("/bin/true", Vec::new(), Vec::new(), 4242, 2, 8);
    assert_eq!(plane.hot_images(), 0);
    assert_eq!(plane.parked(), 0);
    assert_eq!(plane.holder_pid(), 4242);
    assert!(!plane.has("alpine"));
}

#[test]
fn reconcile_holder_is_a_noop_on_a_matching_pin() {
    let mut plane = WarmPlane::new("/bin/true", Vec::new(), Vec::new(), 7, 2, 8);
    assert!(!plane.reconcile_holder(7, Vec::new()));
    assert_eq!(plane.holder_pid(), 7);
}

#[test]
fn reconcile_holder_adopts_a_restarted_pin() {
    let mut plane = WarmPlane::new("/bin/true", Vec::new(), Vec::new(), 7, 2, 8);
    assert!(plane.reconcile_holder(9, vec![OsString::from("nsenter")]));
    assert_eq!(plane.holder_pid(), 9);
    assert_eq!(plane.hot_images(), 0);
}

// ---------------------------------------------------------------------------
// Async refill bookkeeping — pure, no runner spawned. `ensure_pool` makes an
// EMPTY sub-pool (parks nothing), so these never launch the `bin`.
// ---------------------------------------------------------------------------

#[test]
fn ensure_pool_creates_empty_then_only_promotes() {
    let mut plane = WarmPlane::new("/bin/true", Vec::new(), Vec::new(), 7, 2, 8);
    // First time for an image: a NEW empty sub-pool, parking nothing yet.
    assert!(plane.ensure_pool("a"));
    assert_eq!(plane.hot_images(), 1);
    assert_eq!(plane.parked(), 0, "ensure_pool spawns no runner");
    assert!(plane.has("a"));
    // Idempotent: a second call for the same image only promotes it.
    assert!(!plane.ensure_pool("a"));
    assert_eq!(plane.hot_images(), 1);
    // A second image gets its own empty sub-pool.
    assert!(plane.ensure_pool("b"));
    assert_eq!(plane.hot_images(), 2);
    assert_eq!(plane.parked(), 0);
}

#[test]
fn refill_recipe_wants_a_runner_for_an_empty_pool_and_carries_the_pin() {
    let mut plane = WarmPlane::new("/bin/true", Vec::new(), Vec::new(), 4242, 2, 8);
    plane.ensure_pool("a");
    // The empty pool is below its target size → the refiller is asked for one,
    // and the recipe carries the pin so a deposit can detect a holder restart.
    let recipe = plane
        .refill_recipe("a")
        .expect("an empty pool wants a runner");
    assert_eq!(recipe.holder_pid(), 4242);
    // No sub-pool for an unknown image → nothing to refill.
    assert!(plane.refill_recipe("missing").is_none());
}

#[test]
fn take_runner_on_an_empty_pool_is_a_miss_but_keeps_the_pool() {
    let mut plane = WarmPlane::new("/bin/true", Vec::new(), Vec::new(), 7, 1, 4);
    plane.ensure_pool("a");
    // Momentarily empty (the refiller has not filled it yet): a take is a miss, so
    // the caller serves cold — but the sub-pool stays, for the refiller to fill.
    assert!(plane.take_runner("a").is_none());
    assert!(plane.has("a"));
    // An unknown image is also a miss and creates nothing.
    assert!(plane.take_runner("missing").is_none());
    assert_eq!(plane.hot_images(), 1);
}
