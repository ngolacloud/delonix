//! Questions asked of the host and of processes: the clock, the user namespace,
//! whether a pid is still the process a record names, a fresh id. They came out of
//! `delonix-runtime-core` with the rest of the node's own concerns (ADR-0040 P3).

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch, `0` if the clock is before it.
///
/// The one definition: it was copied, identical, into ten modules across six
/// crates, each with its own `SystemTime` imports.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Are we in the INITIAL user namespace — i.e. is uid 0 here the host's root?
///
/// **`geteuid() == 0` does not answer this**, and the difference matters
/// everywhere the engine picks a privileged path. uid 0 inside a nested user
/// namespace buys nothing on the host: no write to the host's cgroup tree, no
/// `/run`, no privileged mount of the host's filesystems. Two independent
/// places in this workspace decided by `geteuid()` alone and both took the
/// ROOT path in exactly the environment where they had the least power.
///
/// The initial namespace is the only one whose `uid_map` is the identity map
/// over the whole range; anything else is nested. It is how podman answers the
/// same question. Unreadable `/proc` answers "initial" — the behaviour this
/// workspace had for years, so an unexpected environment keeps working exactly
/// as before instead of silently switching execution modes.
pub fn in_initial_userns() -> bool {
    let Ok(map) = std::fs::read_to_string("/proc/self/uid_map") else {
        return true;
    };
    initial_uid_map(&map)
}

/// Pure half of [`in_initial_userns`], so the parsing is tested without a
/// namespace to set up.
pub fn initial_uid_map(map: &str) -> bool {
    let mut lines = map.lines().filter(|l| !l.trim().is_empty());
    let Some(first) = lines.next() else {
        return true; // empty map: cannot tell, keep the historical answer
    };
    if lines.next().is_some() {
        return false; // more than one range is never the initial namespace
    }
    let f: Vec<&str> = first.split_whitespace().collect();
    f == ["0", "0", "4294967295"]
}

/// Is this process rootless — i.e. WITHOUT privilege over the host?
///
/// True when the euid is not 0, and ALSO when it is 0 inside a nested user
/// namespace. See [`in_initial_userns`].
pub fn is_rootless() -> bool {
    // SAFETY: geteuid has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        return true;
    }
    !in_initial_userns()
}

/// Formats a unix instant as LOCAL date/time "YYYY-MM-DD HH:MM:SS".
/// Uses `localtime_r` (honors /etc/localtime|TZ); on failure, returns the raw value.
pub fn fmt_local_ts(unix: u64) -> String {
    let t = unix as libc::time_t;
    // SAFETY: `libc::tm` is a plain C struct of integers (and one pointer, which may be null);
    // all-zero bytes is a valid value.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: `t` is valid; `localtime_r` writes into `tm` (our buffer, of the
    // right size) and returns NULL only on error — handled below.
    if unsafe { libc::localtime_r(&t, &mut tm).is_null() } {
        return unix.to_string();
    }
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

/// A microVM (Cloud Hypervisor) — the unit of `kind: VM`. SIBLING model of the
/// [`Container`]: a VM has no rootfs/cgroup/seccomp/init-pid, so it does not make
/// sense to overload the `Container`. Persisted via [`store::JsonStore`]
/// (one JSON per name, under `$DELONIX_ROOT/vms`).
/// `true` if process `pid` still exists (signal 0 = only tests liveness).
pub fn is_alive(pid: i32) -> bool {
    // SAFETY: signal 0 sends nothing — it only tests that we may signal `pid`.
    pid > 0 && unsafe { libc::kill(pid, 0) } == 0
}

/// The host's `MemAvailable` in MiB (from `/proc/meminfo`) — memory that can be
/// given to new processes without swapping. `None` if unreadable.
pub fn mem_available_mib() -> Option<u64> {
    parse_mem_available_mib(&std::fs::read_to_string("/proc/meminfo").ok()?)
}

/// `MemAvailable` in MiB out of the text of `/proc/meminfo`. Pure.
fn parse_mem_available_mib(meminfo: &str) -> Option<u64> {
    let kib: u64 = meminfo
        .lines()
        .find_map(|l| l.strip_prefix("MemAvailable:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some(kib / 1024)
}

/// The process `starttime` (field 22 of `/proc/<pid>/stat`, jiffies since
/// boot). Unique and stable for the process's lifetime — we use it to detect
/// PID reuse.
pub fn proc_starttime(pid: i32) -> Option<u64> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The comm (field 2) may contain spaces/parentheses — cut up to the last ')'.
    let rest = &s[s.rfind(')')? + 1..];
    rest.split_whitespace().nth(19).and_then(|f| f.parse().ok()) // field 22 = the 20th after the comm
}

/// Clock ticks per second (`_SC_CLK_TCK`), the unit of `/proc/<pid>/stat`
/// field 22. `0` only when `sysconf` fails, which callers read as "cannot
/// compute" rather than a bogus zero.
fn clock_ticks_per_sec() -> u64 {
    // SAFETY: `sysconf` reads a system constant — no preconditions, touches no
    // memory of ours.
    let v = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    u64::try_from(v).unwrap_or(0)
}

/// The host's boot instant as Unix seconds — the `btime` field of `/proc/stat`.
/// `None` if `/proc/stat` is unreadable or carries no `btime`.
///
/// Unlike field 22 of `/proc/<pid>/stat` (jiffies SINCE boot), this is an
/// absolute wall-clock anchor: it changes on every boot. It is what turns a
/// boot-local process starttime into a boot-INVARIANT identity (see
/// [`abs_starttime`]).
pub fn host_boot_time() -> Option<u64> {
    parse_btime(&std::fs::read_to_string("/proc/stat").ok()?)
}

/// Pure half of [`host_boot_time`]: the `btime <secs>` line of `/proc/stat`.
fn parse_btime(proc_stat: &str) -> Option<u64> {
    proc_stat
        .lines()
        .find_map(|l| l.strip_prefix("btime ")?.trim().parse().ok())
}

/// The host's per-boot UUID, from `/proc/sys/kernel/random/boot_id` — the kernel
/// regenerates it on every boot. An alternative boot anchor to [`host_boot_time`]
/// that does not depend on the clock being set: two records written in different
/// boots carry different boot ids even if the wall clock never moved. `None` if
/// the file is unreadable or empty.
pub fn host_boot_id() -> Option<String> {
    let s = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    let id = s.trim();
    (!id.is_empty()).then(|| id.to_string())
}

/// Boot-INVARIANT start instant of a process: the host boot time plus field 22
/// (jiffies since boot) converted to seconds. Pure, so the reboot reasoning is
/// tested without `/proc`.
///
/// **Why this and not raw field 22**: field 22 is jiffies SINCE boot, so it
/// resets to a small number on every boot. A pid the kernel recycles AFTER a
/// reboot can start at the very same jiffy offset its predecessor did, and raw
/// field 22 then collides — a dead record would match an unrelated host process
/// and we would signal it (finding D3). Folding in `btime` makes the value an
/// absolute Unix instant: the same jiffy collision maps to two DIFFERENT instants
/// (the boots are seconds-to-days apart), so the identity no longer matches.
pub fn abs_starttime(boot_time: u64, starttime_jiffies: u64, hz: u64) -> Option<u64> {
    (hz > 0).then(|| boot_time.saturating_add(starttime_jiffies / hz))
}

/// [`abs_starttime`] for a live pid: reads its field-22 starttime and the host
/// boot time. `None` if either is unreadable or the clock rate is unknown — the
/// callers must read that as "cannot prove identity" and fail safe.
///
/// This is the reboot-robust identity the record format should persist to CLOSE
/// D3 end-to-end; `safe_to_signal` still compares the raw field the record
/// carries today (see its doc for why the switch is a cross-crate contract
/// change, not a local one).
pub fn proc_abs_starttime(pid: i32) -> Option<u64> {
    abs_starttime(host_boot_time()?, proc_starttime(pid)?, clock_ticks_per_sec())
}

/// `true` if it is safe to send a signal to `pid` on behalf of this
/// container/VM: the PID is alive AND is still the SAME process we recorded.
/// Guards against PID reuse — we must never signal a process belonging to the
/// host.
///
/// The identity is the recorded `starttime` (field 22 of `/proc/<pid>/stat`).
/// Within one boot it is unique and stable for the process's lifetime, so the
/// equality below is exact. Across a REBOOT it is weaker — field 22 is jiffies
/// SINCE boot and resets, so a recycled pid can collide (finding D3). The
/// boot-invariant remedy is [`abs_starttime`]/[`proc_abs_starttime`]; using it
/// is a contract change, because the record must PERSIST the absolute instant
/// and the same field is read raw for `docker ps` uptime and usage sampling in
/// other crates — flipping it here alone would make every existing record
/// mismatch and break the normal path. So this still compares the raw value the
/// record carries, correct for the overwhelmingly common same-boot case.
///
/// A record with NO recorded starttime (`None`) cannot prove identity by the
/// starttime alone. `None` is NOT only a legacy artefact: the current engine also
/// leaves it `None` when `proc_starttime` misses at registration (a transient
/// `/proc` read race in THIS boot), and there the pid is still ours — so refusing
/// to signal every `None` would break the normal path. The reboot hazard of D4 (a
/// `None` record that OUTLIVES a reboot names a pid the kernel has reused) is
/// handled where the record's creation instant is known — see
/// `Container::is_live`/`Vm::is_live`, which compare `created_unix` against the
/// host boot time ([`host_boot_time`]). Here a live `None` pid stays signalable,
/// correct for the same-boot case.
pub fn safe_to_signal(pid: i32, starttime: Option<u64>) -> bool {
    if !is_alive(pid) {
        return false;
    }
    match starttime {
        Some(want) => proc_starttime(pid) == Some(want),
        None => true,
    }
}

/// Generates a container id: 16 hexadecimal characters.
pub fn generate_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let pid = std::process::id() as u64;
    let mixed = nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ pid.rotate_left(32);
    format!("{mixed:016x}")
}

/// Path of the `delonix` binary to relaunch to delegate operations (exec, run,
/// API mutations…). Prefers the executable itself, but is **robust to
/// binary replacement** while the server runs (install/upgrade): in that
/// case `/proc/self/exe` is marked `" (deleted)"` and `current_exe()` returns
/// a nonexistent path — which made spawns fail with `os error 2`. Tries,
/// in order: current exe if it exists → path without the `(deleted)` suffix → `delonix`
/// on the `PATH` → the plain name.
pub fn self_bin() -> std::path::PathBuf {
    use std::path::{Path, PathBuf};
    if let Ok(p) = std::env::current_exe() {
        if p.exists() {
            return p;
        }
        let s = p.to_string_lossy();
        if let Some(real) = s.strip_suffix(" (deleted)") {
            let pb = PathBuf::from(real);
            if pb.exists() {
                return pb;
            }
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let cand = Path::new(dir).join("delonix");
            if cand.exists() {
                return cand;
            }
        }
    }
    PathBuf::from("delonix")
}

/// The local host's capacity to run workloads: CPU, memory, swap and the disk
/// that holds the VM images. For a provider whose VMs run on THIS machine
/// (Cloud Hypervisor, libvirt `qemu:///system`), the "node" of `host.capacity`
// `HostCapacity` is the pure value this module's `capacity()` produces; it lives
// in the context's domain (ADR-0040 A4). Re-exported so `delonix_node::host::HostCapacity`
// keeps resolving.
pub use crate::domain::HostCapacity;

/// Reads [`HostCapacity`] for this host, using `disk_path` for the filesystem
/// figures (the provider passes the directory its VM images live in).
pub fn capacity(disk_path: &std::path::Path) -> HostCapacity {
    let mut cap = HostCapacity::default();
    match std::fs::read_to_string("/proc/cpuinfo") {
        Ok(text) => {
            let (cpus, model) = parse_cpuinfo(&text);
            if cpus > 0 {
                cap.cpus = cpus;
            } else {
                cap.unmeasured.push("cpus");
            }
            match model {
                Some(m) => cap.cpu_model = m,
                None => cap.unmeasured.push("cpu_model"),
            }
        }
        Err(_) => cap.unmeasured.extend(["cpus", "cpu_model"]),
    }
    match std::fs::read_to_string("/proc/meminfo") {
        Ok(text) => match parse_meminfo_bytes(&text) {
            Some(m) => {
                cap.mem_total_bytes = m.mem_total;
                cap.mem_used_bytes = m.mem_total.saturating_sub(m.mem_available);
                cap.swap_total_bytes = m.swap_total;
                cap.swap_used_bytes = m.swap_total.saturating_sub(m.swap_free);
            }
            None => cap
                .unmeasured
                .extend(["mem_total", "mem_used", "swap_total", "swap_used"]),
        },
        Err(_) => cap
            .unmeasured
            .extend(["mem_total", "mem_used", "swap_total", "swap_used"]),
    }
    match statvfs_bytes(disk_path) {
        Some((total, used)) => {
            cap.disk_total_bytes = total;
            cap.disk_used_bytes = used;
        }
        None => cap.unmeasured.extend(["disk_total", "disk_used"]),
    }
    cap
}

/// Logical CPU count (`processor` lines) and the first `model name`, out of the
/// text of `/proc/cpuinfo`. Pure.
fn parse_cpuinfo(text: &str) -> (u64, Option<String>) {
    let cpus = text
        .lines()
        .filter(|l| l.split_once(':').map(|(k, _)| k.trim() == "processor") == Some(true))
        .count() as u64;
    let model = text
        .lines()
        .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim() == "model name"))
        .map(|(_, v)| v.trim().to_string())
        .filter(|m| !m.is_empty());
    (cpus, model)
}

struct MemInfo {
    mem_total: u64,
    mem_available: u64,
    swap_total: u64,
    swap_free: u64,
}

/// The four byte counts out of the text of `/proc/meminfo` (its values are kB).
/// `None` when `MemTotal`/`MemAvailable` are absent — a meminfo without them is
/// not a measurement. Swap lines absent means a host with no swap: a real `0`.
/// Pure.
fn parse_meminfo_bytes(text: &str) -> Option<MemInfo> {
    let kib = |key: &str| -> Option<u64> {
        text.lines()
            .find_map(|l| l.strip_prefix(key))?
            .split_whitespace()
            .next()?
            .parse::<u64>()
            .ok()
    };
    Some(MemInfo {
        mem_total: kib("MemTotal:")?.saturating_mul(1024),
        mem_available: kib("MemAvailable:")?.saturating_mul(1024),
        swap_total: kib("SwapTotal:").unwrap_or(0).saturating_mul(1024),
        swap_free: kib("SwapFree:").unwrap_or(0).saturating_mul(1024),
    })
}

/// `(total, used)` bytes of the filesystem holding `path`, via `statvfs(3)`.
/// `None` when the path cannot be stat'd (absent store, unreadable) — surfaced
/// as unmeasured, never as zero bytes.
fn statvfs_bytes(path: &std::path::Path) -> Option<(u64, u64)> {
    let c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: `statvfs` fills a plain C struct of integers; all-zero is valid.
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid NUL-terminated path; `st` is our out-param buffer.
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let frsize = st.f_frsize as u64;
    let total = (st.f_blocks as u64).saturating_mul(frsize);
    let used = (st.f_blocks.saturating_sub(st.f_bfree) as u64).saturating_mul(frsize);
    (total > 0).then_some((total, used))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The reader on the REAL host: a machine always has at least one CPU, some
    /// total RAM and a filesystem under the state root — and "in use" never
    /// exceeds "total" for any of the three. The parallel of Proxmox's
    /// `the_node_reports_its_cpu_memory_and_disk_capacity`, but of THIS host.
    #[test]
    fn host_capacity_reads_the_local_host() {
        let cap = super::capacity(Path::new("/"));
        assert!(cap.cpus >= 1, "the host has at least one CPU: {cap:?}");
        assert!(
            cap.mem_total_bytes > 0,
            "the host reports total RAM: {cap:?}"
        );
        assert!(
            cap.mem_used_bytes <= cap.mem_total_bytes,
            "RAM in use never exceeds total: {cap:?}"
        );
        assert!(
            cap.swap_used_bytes <= cap.swap_total_bytes,
            "swap in use never exceeds total (both 0 without swap): {cap:?}"
        );
        assert!(
            cap.disk_total_bytes > 0,
            "the host reports the VM-image disk: {cap:?}"
        );
        assert!(
            cap.disk_used_bytes <= cap.disk_total_bytes,
            "disk in use never exceeds total: {cap:?}"
        );
    }

    /// `/proc/cpuinfo` parsing: one count per `processor` line, the first
    /// `model name`, both pure.
    #[test]
    fn cpuinfo_counts_processors_and_names_the_model() {
        let text = "processor\t: 0\nmodel name\t: Fake CPU @ 3GHz\nprocessor\t: 1\nmodel name\t: Fake CPU @ 3GHz\n";
        let (cpus, model) = super::parse_cpuinfo(text);
        assert_eq!(cpus, 2);
        assert_eq!(model.as_deref(), Some("Fake CPU @ 3GHz"));
        // No processor lines and no model: zero and None (caller marks unmeasured).
        assert_eq!(super::parse_cpuinfo("bogus\n"), (0, None));
    }

    /// `/proc/meminfo` kB → bytes; absent swap is a real zero, absent MemTotal
    /// is `None` (not a measurement).
    #[test]
    fn meminfo_converts_kb_to_bytes_and_tolerates_missing_swap() {
        let text = "MemTotal:       32000 kB\nMemFree: 1000 kB\nMemAvailable:   20000 kB\n";
        let m = super::parse_meminfo_bytes(text).expect("MemTotal+MemAvailable present");
        assert_eq!(m.mem_total, 32_000 * 1024);
        assert_eq!(m.mem_available, 20_000 * 1024);
        assert_eq!(m.swap_total, 0, "no SwapTotal line is a real zero");
        assert!(super::parse_meminfo_bytes("MemFree: 1 kB\n").is_none());
    }

    /// `MemAvailable` in kB becomes MiB; a text without the line is `None`,
    /// which the VM admission reads as «cannot tell» and does not block on.
    #[test]
    fn mem_available_is_read_from_meminfo() {
        let text = "MemTotal:       32000000 kB\nMemFree:  1000 kB\nMemAvailable:   20480000 kB\n";
        assert_eq!(super::parse_mem_available_mib(text), Some(20000));
        assert_eq!(super::parse_mem_available_mib("MemTotal: 1 kB\n"), None);
        assert_eq!(
            super::parse_mem_available_mib("MemAvailable: lots kB\n"),
            None
        );
    }

    /// `btime` is pulled from its `/proc/stat` line, among the others.
    #[test]
    fn btime_is_read_from_proc_stat() {
        let text = "cpu  1 2 3\nbtime 1700000000\nprocesses 42\n";
        assert_eq!(super::parse_btime(text), Some(1_700_000_000));
        assert_eq!(super::parse_btime("cpu 1 2 3\n"), None);
    }

    /// The boot-robust identity (finding D3). Folding `btime` into field-22
    /// jiffies turns a boot-LOCAL starttime into an absolute instant. The
    /// regression this pins is the PID-reuse-across-reboot collision: two boots
    /// whose recycled pid starts at the SAME jiffy offset — identical raw field
    /// 22 — map to DIFFERENT absolute instants, so the identity stops matching.
    #[test]
    fn absolute_starttime_survives_a_reboot_pid_collision() {
        let hz = 100;
        let jiffies = 450; // the SAME raw starttime in both boots — the collision
        let boot_a = super::abs_starttime(1_000_000, jiffies, hz);
        let boot_b = super::abs_starttime(2_000_000, jiffies, hz);
        assert_eq!(boot_a, Some(1_000_004));
        assert_eq!(boot_b, Some(2_000_004));
        // Raw field 22 alone would call these the same process; the absolute
        // instant does not — which is the whole point of folding in `btime`.
        assert_ne!(boot_a, boot_b);
        // Same boot, same jiffies: a stable identity across repeated reads.
        assert_eq!(boot_a, super::abs_starttime(1_000_000, jiffies, hz));
        // hz == 0 (sysconf failed) is "cannot compute", never a bogus zero.
        assert_eq!(super::abs_starttime(1_000_000, jiffies, 0), None);
    }

    /// `safe_to_signal` on the REAL host: our own pid with its starttime is ours,
    /// a wrong starttime is a recycled pid, and a `None` starttime on a LIVE pid
    /// stays signalable (it also covers a same-boot `/proc` read miss, not only
    /// legacy records). The reboot hazard of a `None` record is caught by
    /// `is_live`'s `created_unix` vs boot-time guard (D4), tested there.
    #[test]
    fn safe_to_signal_matches_identity_and_trusts_a_live_none_in_this_boot() {
        let me = std::process::id() as i32;
        let real = super::proc_starttime(me).expect("own starttime is readable");
        assert!(super::safe_to_signal(me, Some(real)), "the same process");
        assert!(
            !super::safe_to_signal(me, Some(real.wrapping_add(1))),
            "a different starttime is a recycled pid"
        );
        assert!(
            super::safe_to_signal(me, None),
            "a live pid without a recorded starttime stays signalable in this boot"
        );
        // i32::MAX is above every kernel's pid_max: dead, never signalled.
        assert!(!super::safe_to_signal(i32::MAX, None));
        assert!(!super::safe_to_signal(i32::MAX, Some(1)));
    }

    /// The `/proc`-reading boot anchors answer on the real host: a boot time in
    /// the past, a non-empty boot id, and our own absolute starttime between boot
    /// and now.
    #[test]
    fn boot_anchors_read_from_the_host() {
        let boot = super::host_boot_time().expect("the host has a btime");
        assert!(boot > 0 && boot <= super::now_unix(), "boot is in the past");
        let id = super::host_boot_id().expect("the host has a boot_id");
        assert!(!id.is_empty());
        let me = std::process::id() as i32;
        let abs = super::proc_abs_starttime(me).expect("own absolute starttime");
        assert!(abs >= boot, "a process starts at or after boot");
        assert!(abs <= super::now_unix() + 1, "and not in the future");
    }
}
