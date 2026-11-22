//! Phase profiler for the **pre_spawn** region of `container run` — the parent's
//! work BEFORE `spawn()` (policy+preflights, image resolve, rootfs prep, record
//! build, network attach). The `delonix-linux` `spawn_profile` measures inside
//! `spawn`; this measures the span upstream of it, so `wall − spawn_total` stops
//! being a black box and ADR-0095's warm plane can be aimed at the phase that
//! actually costs.
//!
//! **Pure measurement — a strict no-op unless `DELONIX_PROFILE_SPAWN` is set**,
//! decided once from the env (an atomic), exactly like `spawn_profile`. With the
//! var a path, it appends one `{"role":"prespawn",...}` JSONL record per run,
//! which the existing `spawn_profile.sh` ignores (it greps `"role":"parent"`).

use std::cell::RefCell;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

static STATE: AtomicU8 = AtomicU8::new(0);

thread_local! {
    static MARKS: RefCell<Vec<(&'static str, Instant)>> = const { RefCell::new(Vec::new()) };
}

/// Whether profiling is on, decided once from `DELONIX_PROFILE_SPAWN`.
pub fn enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let on = std::env::var_os("DELONIX_PROFILE_SPAWN").is_some();
            STATE.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
    }
}

/// Start a fresh pre_spawn profile for one `cmd_run`.
pub fn begin() {
    if !enabled() {
        return;
    }
    MARKS.with(|m| {
        let mut v = m.borrow_mut();
        v.clear();
        v.push(("enter", Instant::now()));
    });
}

/// Stamp a pre_spawn phase boundary with a `'static` label.
pub fn mark(label: &'static str) {
    if !enabled() {
        return;
    }
    MARKS.with(|m| m.borrow_mut().push((label, Instant::now())));
}

/// Emit the pre_spawn profile (stderr, and JSONL if the env var names a path) and
/// reset. `container` names the run. Called right before `launch::start` hands
/// off to `spawn`, so the sum of these phases is the parent pre_spawn work.
pub fn flush(container: &str) {
    if !enabled() {
        return;
    }
    MARKS.with(|m| {
        let marks = m.borrow();
        if marks.len() < 2 {
            return;
        }
        let t0 = marks[0].1;
        let total_us = marks[marks.len() - 1].1.duration_since(t0).as_micros();
        let mut phases: Vec<(&'static str, u128)> = Vec::with_capacity(marks.len());
        for w in marks.windows(2) {
            phases.push((w[1].0, w[1].1.duration_since(w[0].1).as_micros()));
        }
        let human = phases
            .iter()
            .map(|(l, us)| format!("{l}={:.2}ms", *us as f64 / 1000.0))
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!(
            "delonix prespawn-profile [{container}] total={:.2}ms | {human}",
            total_us as f64 / 1000.0
        );
        let phases_json = phases
            .iter()
            .map(|(l, us)| format!("\"{l}\":{us}"))
            .collect::<Vec<_>>()
            .join(",");
        append_jsonl(&format!(
            "{{\"role\":\"prespawn\",\"container\":\"{container}\",\"total_us\":{total_us},\"phases_us\":{{{phases_json}}}}}"
        ));
    });
    MARKS.with(|m| m.borrow_mut().clear());
}

fn append_jsonl(record: &str) {
    if let Some(val) = std::env::var_os("DELONIX_PROFILE_SPAWN") {
        let s = val.to_string_lossy();
        if matches!(s.as_ref(), "1" | "true" | "yes" | "on" | "") {
            return;
        }
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(s.as_ref())
        {
            let _ = f.write_all(record.as_bytes());
            let _ = f.write_all(b"\n");
        }
    }
}
