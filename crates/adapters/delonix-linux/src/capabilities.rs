//! Linux capabilities: the name↔number table, the default kept set, and the
//! resolution of `--cap-drop`/`--cap-add` into a mask.
//!
//! Public because it is the SINGLE source of truth for the whole workspace: the
//! CRI's capability ceiling (`delonix-cri`'s `cap_ceiling`) has to reason about
//! exactly the set this engine would grant, and a second name↔number table on
//! that side would drift the day a capability is added here — the same
//! generator-and-reader-share-the-format discipline as `fw_rule_tail` on the
//! network side.

/// The capabilities the container MAY keep. This is Docker/moby's default set
/// (`oci/defaults.go`) minus `CAP_MKNOD` — without a device cgroup, dropping MKNOD
/// is how we prevent access to host disks. Everything else is dropped.
///
/// `CAP_NET_BROADCAST` is NOT in Docker's default and used to be kept here, making
/// the set a SUPERSET of the model it claims to follow (S5). Removed to match
/// Docker/Podman exactly; a workload that needs it asks with `--cap-add`.
pub const KEPT_CAPS: &[u8] = &[
    0,  // CHOWN
    1,  // DAC_OVERRIDE
    3,  // FOWNER
    4,  // FSETID
    5,  // KILL
    6,  // SETGID
    7,  // SETUID
    8,  // SETPCAP
    10, // NET_BIND_SERVICE
    13, // NET_RAW
    18, // SYS_CHROOT
    29, // AUDIT_WRITE
    31, // SETFCAP
];

/// Capability number from the name (`CAP_NET_ADMIN` or `NET_ADMIN`).
///
/// KRN-0006: delegates to the `caps` crate — the ecosystem's source of truth for
/// the capability name↔number ABI of the kernel — instead of a hand-maintained
/// match that drifts when the kernel adds a capability. The inverse [`cap_name`]
/// stays a static table (the crate exposes no `&'static str` name); the
/// round-trip test keeps it in agreement with the crate.
pub fn cap_num(name: &str) -> Option<u8> {
    let n = name.trim().to_ascii_uppercase();
    // The crate's `FromStr` wants the `CAP_` prefix; we accept either form.
    let qualified = if n.starts_with("CAP_") {
        n
    } else {
        format!("CAP_{n}")
    };
    qualified
        .parse::<caps::Capability>()
        .ok()
        .map(|c| c.index())
}

/// Mask with ALL capabilities supported by the kernel (`--privileged`).
/// Reads `/proc/sys/kernel/cap_last_cap` so as not to pass invalid bits to `capset`
/// (which would give EINVAL). Conservative fallback: CAP_CHECKPOINT_RESTORE (40).
pub fn all_caps_mask() -> u64 {
    let last: u32 = std::fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(40);
    let last = last.min(63);
    if last >= 63 {
        u64::MAX
    } else {
        (1u64 << (last + 1)) - 1
    }
}

/// Computes the mask of capabilities to keep: starts at [`KEPT_CAPS`], applies
/// `--cap-drop` (`ALL` → none) and then `--cap-add`.
pub fn resolve_cap_keep(cap_drop: &[String], cap_add: &[String]) -> u64 {
    let mut keep: u64 = if cap_drop.iter().any(|c| c.eq_ignore_ascii_case("all")) {
        0
    } else {
        let mut m = 0u64;
        for &c in KEPT_CAPS {
            m |= 1u64 << c;
        }
        for c in cap_drop {
            if let Some(n) = cap_num(c) {
                m &= !(1u64 << n);
            }
        }
        m
    };
    // `--cap-add ALL` (docker) / the CRI translation of `privileged` → keeps ALL
    // capabilities. Without this branch, `cap_num("ALL")` returned `None` and `ALL` was
    // silently ignored — a "privileged" container via CRI ended up without
    // CAP_SYS_ADMIN (e.g. `sethostname` gave EPERM even though CRI requested it).
    if cap_add.iter().any(|c| c.eq_ignore_ascii_case("all")) {
        return all_caps_mask();
    }
    for c in cap_add {
        if let Some(n) = cap_num(c) {
            keep |= 1u64 << n;
        }
    }
    keep
}

/// Canonical name (without the `CAP_` prefix) of a capability number — the
/// inverse of [`cap_num`]. Used to render a mask back into the `--cap-add`
/// arguments the engine accepts, and to name capabilities in policy errors.
///
/// `None` for a number this build does not know (a kernel newer than the table):
/// deliberately not a synthetic `CAP_38`-style string, because such a name would
/// travel into an argv that [`cap_num`] would then silently drop.
pub fn cap_name(n: u8) -> Option<&'static str> {
    Some(match n {
        0 => "CHOWN",
        1 => "DAC_OVERRIDE",
        2 => "DAC_READ_SEARCH",
        3 => "FOWNER",
        4 => "FSETID",
        5 => "KILL",
        6 => "SETGID",
        7 => "SETUID",
        8 => "SETPCAP",
        9 => "LINUX_IMMUTABLE",
        10 => "NET_BIND_SERVICE",
        11 => "NET_BROADCAST",
        12 => "NET_ADMIN",
        13 => "NET_RAW",
        14 => "IPC_LOCK",
        15 => "IPC_OWNER",
        16 => "SYS_MODULE",
        17 => "SYS_RAWIO",
        18 => "SYS_CHROOT",
        19 => "SYS_PTRACE",
        20 => "SYS_PACCT",
        21 => "SYS_ADMIN",
        22 => "SYS_BOOT",
        23 => "SYS_NICE",
        24 => "SYS_RESOURCE",
        25 => "SYS_TIME",
        26 => "SYS_TTY_CONFIG",
        27 => "MKNOD",
        28 => "LEASE",
        29 => "AUDIT_WRITE",
        30 => "AUDIT_CONTROL",
        31 => "SETFCAP",
        32 => "MAC_OVERRIDE",
        33 => "MAC_ADMIN",
        34 => "SYSLOG",
        35 => "WAKE_ALARM",
        36 => "BLOCK_SUSPEND",
        37 => "AUDIT_READ",
        38 => "PERFMON",
        39 => "BPF",
        40 => "CHECKPOINT_RESTORE",
        _ => return None,
    })
}

/// Mask of the default kept set ([`KEPT_CAPS`]) — what a container gets when the
/// caller asks for nothing.
pub fn default_kept_mask() -> u64 {
    KEPT_CAPS.iter().fold(0u64, |m, &c| m | (1u64 << c))
}

/// Bitmask of the named capabilities (`CAP_NET_RAW` or `NET_RAW`). Names are
/// expected to have been validated upstream (an unknown name is a `create`-time
/// refusal in `oci-runtime`); any that still fail to resolve here are skipped
/// rather than panicking, exactly as [`resolve_cap_keep`] skips unknown deltas.
pub fn mask_from_names(names: &[String]) -> u64 {
    names
        .iter()
        .filter_map(|n| cap_num(n))
        .fold(0u64, |m, n| m | (1u64 << n))
}

/// The five OCI capability sets as kernel bitmasks. The OCI/runtime-spec model
/// keeps these DISTINCT — bounding, effective, permitted, inheritable and
/// ambient — and a conformant runtime must install each one exactly (the
/// `runtime-tools` suite reads all five back from `/proc/self/status`). The
/// engine's older model carried a SINGLE kept mask; [`CapSets::from_keep`] is
/// that model expressed in these terms, so the docker/CRI path keeps
/// byte-for-byte the confinement it always had.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapSets {
    /// Ceiling: a cap outside this set cannot be reacquired via setuid/exec.
    pub bounding: u64,
    /// The caps active for permission checks right now.
    pub effective: u64,
    /// The caps the thread may make effective.
    pub permitted: u64,
    /// The caps a child may inherit across an `execve`.
    pub inheritable: u64,
    /// The caps that SURVIVE an `execve` into a non-privileged program — raised
    /// with `PR_CAP_AMBIENT_RAISE`, which requires each one to be in both
    /// `permitted` and `inheritable` first.
    pub ambient: u64,
}

impl CapSets {
    /// The single-mask model: `bounding = effective = permitted = keep`, with
    /// inheritable and ambient empty. This is EXACTLY what the engine applied
    /// before it learned the five-set model, so every caller that has no
    /// distinct sets (docker `run`, the CRI, `exec`) keeps the same confinement.
    pub fn from_keep(keep: u64) -> Self {
        Self {
            bounding: keep,
            effective: keep,
            permitted: keep,
            inheritable: 0,
            ambient: 0,
        }
    }
}

/// The names of every capability set in `mask`, in ascending capability number
/// (stable order, so the argv this feeds is reproducible). Bits without a known
/// name are skipped — see [`cap_name`].
pub fn names_from_mask(mask: u64) -> Vec<&'static str> {
    (0u8..64)
        .filter(|c| (mask >> c) & 1 == 1)
        .filter_map(cap_name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cap_name` and `cap_num` are two hand-written tables that MUST agree: the
    /// ceiling renders a mask with one and the engine parses it back with the
    /// other, so a single mismatched entry would silently drop a capability the
    /// operator explicitly allowed.
    #[test]
    fn cap_name_and_cap_num_are_inverses() {
        for n in 0u8..=40 {
            let name = cap_name(n).unwrap_or_else(|| panic!("cap {n} has no name"));
            assert_eq!(cap_num(name), Some(n), "round-trip failed for {name}");
            assert_eq!(
                cap_num(&format!("CAP_{name}")),
                Some(n),
                "CAP_ prefix should be accepted for {name}"
            );
        }
        assert_eq!(cap_name(41), None, "41 is not in the table yet");
    }

    #[test]
    fn names_from_mask_is_ordered_and_ignores_unnamed_bits() {
        let mask = (1u64 << 21) | (1u64 << 0) | (1u64 << 63);
        assert_eq!(names_from_mask(mask), vec!["CHOWN", "SYS_ADMIN"]);
    }

    #[test]
    fn default_kept_mask_matches_kept_caps() {
        let m = default_kept_mask();
        for &c in KEPT_CAPS {
            assert_eq!((m >> c) & 1, 1);
        }
        assert_eq!((m >> 21) & 1, 0, "SYS_ADMIN is never in the default");
    }

    /// A mask rendered into names and fed back as `--cap-drop ALL --cap-add <…>`
    /// has to resolve to the SAME mask — this is the round-trip the CRI ceiling
    /// depends on to clamp without reimplementing the resolution.
    #[test]
    fn mask_survives_round_trip_through_drop_all_plus_add() {
        let want = default_kept_mask() | (1u64 << 12); // + NET_ADMIN
        let add: Vec<String> = names_from_mask(want)
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(resolve_cap_keep(&["ALL".to_string()], &add), want);
    }

    /// KRN-0006: `cap_num` now delegates to the `caps` crate; the remaining
    /// hand-written `cap_name` table must match the crate name-for-name, so the
    /// two sources of truth cannot drift. (Also pins that the crate still maps the
    /// 0..=40 range this engine relies on.)
    #[test]
    fn cap_name_agrees_with_the_caps_crate() {
        for n in 0u8..=40 {
            let ours = format!("CAP_{}", cap_name(n).expect("name in the table"));
            let theirs = caps::all()
                .into_iter()
                .find(|c| c.index() == n)
                .map(|c| c.to_string())
                .expect("the caps crate knows this capability");
            assert_eq!(ours, theirs, "cap {n} diverges from the caps crate");
        }
    }
}
