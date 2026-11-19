# Analysed environment — phase 1 (inventory + baseline)

Measured on 2026-09-15. Everything here was read from a command executed in this session.

## Code

| Field | Value |
|---|---|
| Commit | `fa8c6297c19b7429e60644c5b950724254e6e9e2` (= `origin/main`) |
| git describe | `v3.1.0-26-gfa8c6297` — **26 commits after the v3.1.0 tag** |
| Tag v3.1.0 | `6680745557eb84bd005c93c3d0bdabe0e5e90db2` (published 2026-09-11T12:45Z) |
| Version in `Cargo.toml` | `3.1.0` (same as the tag — two different builds with the same number) |
| Worktree | `/tmp/wt-prodready`, branch `auditoria/prod-readiness-v3.1`, clean |
| Binary used in the live measurements | built from this tree (`commit: fa8c6297c`) |
| Binary installed on the host | `delonix 3.1.0`, `commit: 668074555` (= tag) — **not used** |

> Directory name: `v3.1.0` by request. The analysed commit **is not** the v3.1.0 tag;
> every result in this report refers to `fa8c6297`.

## Toolchain

| | |
|---|---|
| rustc / cargo | 1.96.0 (pinned in `rust-toolchain.toml`) |
| protoc | libprotoc 25.1 |
| cargo-deny | 0.20.2 |
| cargo-audit | **absent** — `NOT RUN` |
| cargo-llvm-cov | **absent** — `NOT RUN` |

## Host

| | |
|---|---|
| OS | Zorin OS 18.1 |
| Kernel | Linux 7.0.0-31-generic x86_64 |
| CPU / RAM / disk | 32 vCPU / 30 GiB (16 used) / root 87% full (117 GiB free) |
| cgroups | v2 (`cgroup2fs`) |
| cgroup of this session | `user@1000.service/app.slice/app-com.anthropic.Claude-*.scope` |
| LSM | `lockdown,capability,landlock,yama,apparmor,ima,evm` — AppArmor; **no SELinux** |
| userns | `unprivileged_userns_clone=1`, `apparmor_restrict_unprivileged_userns=0`, `max_user_namespaces=113359` |
| subuid/subgid | `walter:100000:65536` |
| KVM | `/dev/kvm` present, user in the `kvm` group |
| nftables | v1.0.9 |
| eBPF | `bpftool` present |
| slirp4netns / pasta / passt | 1.2.1 / present / present |
| runc / crun | 1.5.1 / **absent** |
| Docker / Podman | 29.8.0 (**daemon active**) / 4.9.3 |
| critest / crictl | v1.36.0 / v1.36.0 |
| kubectl / kind | v1.36.4 / absent |
| CNI plugins | `/usr/lib/cni` (bridge, host-local, loopback, firewall, …); `/opt/cni/bin` absent |
| cloud-hypervisor / virsh / qemu-img | v53.0 / libvirt 10.0.0 / present |
| WireGuard | `wg` present |

## Constraints that condition the following phases

1. **Host shared with production** (real user containers, Docker and Podman
   active). All live measurements use isolated `DELONIX_ROOT` **and** `DELONIX_NET_RUNTIME_DIR`.
   No `prune`, no respawn of the real holder, no `--force` over foreign state.
2. **No SELinux** — all SELinux validation stays `ENVIRONMENT BLOCKED` on this host.
3. **x86_64 only** — no aarch64 artefact exists to validate.
4. **Rootful** requires `sudo` over a production host; not exercised in this phase.
