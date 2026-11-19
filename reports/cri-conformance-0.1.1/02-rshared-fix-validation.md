# `rshared` (ADR-0093) — implemented and validated (2026-11-09)

Golden VM `delonix-vm-k8s:1.36` (libvirt, 4 vCPU/4G, ubuntu 24.04), release engine
`scp`'d as root, CNI bridge+host-local+portmap, `DELONIX_CRI_CAP_CEILING=all`.

## Spec in focus — PASSES

```
FOCUS="mount with 'rshared'"  → SUCCESS! -- 1 Passed | 0 Failed | 121 Skipped
```

`mount with 'rshared' should support propagation from host to container and vice
versa` passes: a `mount --bind /etc <point>` done INSIDE the container shows up on the
host (the container→host half that was missing). Also confirmed by hand, comparing
the container's and the host's `/proc/self/mountinfo`:

```
# source on the host
91 … /tmp/rs/src /tmp/rs/src … shared:1
# inside the container the volume is a PEER (shared:1, no master:)
… /tmp/rs/src /mnt/rs … shared:1
… /etc /mnt/rs/sub … shared:3        ← submount created in the container
# on the host, it appeared:
… /etc /tmp/rs/src/sub … shared:3    ← propagated container→host ✅
```

## Full suite

| Harness | Passed/103 | Notes |
|---|---|---|
| minimal (CNI only) | 90/103 | 9 AppArmor (securityfs not mounted) + seccomp-nil + OOM + streaming/network |
| + securityfs + `DELONIX_CRI_SECCOMP_NIL=unconfined` | **99/103** | closes AppArmor + seccomp-nil |

**`rshared` passes in both** (it is not among the failures). Of the 4 at 99/103, one is
flaky and three are environment/streaming — none is the engine nor `rshared`. Focused
re-run of the 4: `6 Passed | 3 Failed`:

- `should support network` — **passed on the re-run** (the 1st failure was a layer
  **unpack error**, `libgcrypt.so…tmp`, an extraction race on the `hostnet-nginx`
  image): FLAKY, non-deterministic;
- `OOMKilled` — report via cgroup2-memory (the baseline marked it flaky/to-be-confirmed);
- `portforward in host network` — network/hostNetwork;
- `exec with tty=true and stdin=true` — streaming (exit-code `-1`).

These are the same classes the baseline campaign resolved with the full harness to
reach 102/103 (with `rshared` as the only remaining **engine** failure). With
`rshared` now closed, the engine has no mount-propagation conformance gap;
the rest depend on the network+streaming+OOM environment/harness, not on this fix.

## Implementation

`crates/adapters/delonix-linux/src/lib.rs` (`setup_rootfs`, `detach_old_root`,
`enclosing_mount_point`, `per_mount_propagation`, `root_propagation`,
`rshared_allowed`), gate in `spawn` (`Error::BidirectionalNeedsPrivileged`,
DX-1117). See ADR-0093 for the measured mechanism and the reason for each step.
