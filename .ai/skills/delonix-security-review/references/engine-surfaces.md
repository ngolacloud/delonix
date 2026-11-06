# Engine surfaces — where privilege lives

Code anchors for the `delonix-security-review` skill. Read the actual symbol
before writing a finding; line numbers drift, symbol names are the stable
handle. Tiers refer to `.ai/agents/permission-tiers.md`.

## Container runtime — the most privileged crate
`crates/adapters/delonix-linux` (Tier D)
- `src/lib.rs` — `mount(...)`, `umount2(..., MNT_DETACH)`, `pivot_root`,
  `libc::unshare(CLONE_NEWUSER)`; the namespace/mount mechanism.
- `src/seccomp_profile.rs` — syscall allow-table (incl. `SYS_mount`).
- `src/capabilities.rs` — `KEPT_CAPS`, the single source of truth for retained
  capabilities; drops `CAP_MKNOD`. `--privileged` → `--cap-add ALL`.
- `src/cdi.rs` — device injection into containers.

## Networking / SDN / firewall
`crates/adapters/delonix-sdn` (Tier D)
- `src/pin_userns.rs` — `libc::unshare` + `libc::mount`.
- `src/cni.rs` — `libc::setns(fd, CLONE_NEWNET)`.
- `src/{lan,wg}.rs` — `ip netns add/del`, `ip link add … type wireguard`.
- `src/policy_nft.rs` — `nft` rule application. `src/bpf.rs` — `DELONIX_BPF_OBJECT`.
- `src/infra.rs` — note: a non-root is root INSIDE its own user+net namespace
  (rootless `CAP_NET_ADMIN` there) — this is the Tier-B rootless path.

## microVM / virtualization
`crates/adapters/delonix-vm` (Tier B rootless on `/dev/kvm`, Tier D for passthrough)
- Cloud-Hypervisor backend runs rootless inside the user namespace on `/dev/kvm`.
- `crates/providers/delonix-provider-cloud-hypervisor` — `/dev/kvm` api-socket.
- `crates/providers/delonix-provider-libvirt` — QEMU/virsh, hugepages, CPU
  pinning, **VFIO passthrough** (guest DMA to host hardware — "the `--privileged`
  of VMs and then some").

## Images / volumes / state
- `crates/adapters/delonix-oci` — `src/build.rs` rootless build (Tier B);
  `src/overlay.rs` overlay2 mount (`nix::mount`, Tier D).
- `crates/adapters/delonix-volume` — named volumes + bind mounts.
- `crates/adapters/delonix-state` — persisted state behind flock, atomic writes,
  the **encrypted secret vault**. Never write it directly.

## Admission & posture (the decision layer — pure, Tier A to read)
`crates/contexts/delonix-security` ("no privileges, no background work")
- `src/domain/admission.rs` — container + VM admission gate.
- `src/domain/policy.rs` — node policy: `denyPrivileged`, `denyHostNetwork`,
  `denyLatestTag`, `allowedRegistries`, `denyDevicePassthrough`,
  `denyLatestVmImage`, `allowedImageUrlHosts`.
- `src/domain/score.rs` — posture scoring. `src/domain/severity.rs` — severity.
- `src/domain/redact.rs` — PII/secret redaction (keeps secrets out of events).
- `src/app/event.rs` — the `events.jsonl` audit trail (best-effort, not
  tamper-proof; the hardened chain lives in delonix-paas).

## The one component designed to run as root
`bins/delonix-storage-helper` (Tier D)
- `src/main.rs` — the privileged side of storage pools; `create-pool`/
  `destroy-pool … --wipe-devices` are administrator-at-terminal ONLY and are
  NEVER served over the socket. Protocol: `crates/adapters/delonix-pool-helper`.

## Control surfaces (local sockets, SO_PEERCRED / calling-uid auth)
- `crates/interfaces/delonix-cri` (`cap_ceiling.rs` — `DELONIX_CRI_CAP_CEILING`),
  `delonix-mgmt`, `delonix-node-api`, `delonix-mcp` (`src/risk.rs` — the risk
  taxonomy; `src/audit.rs`). Sockets under `/run/delonix-*.sock`.

## Command-injection surface
- `bins/delonix/src/cmd/cluster.rs` — `ssh` driving a remote `bash -c` body
  built with `format!`. SECURITY.md lists `cluster apply`/`cluster kubeadm`
  command injection in scope. No agent-supplied string flows in.

## Fail-open flags to flag on sight
`DELONIX_ALLOW_UNENFORCED_ISOLATION`, `DELONIX_ALLOW_UNENFORCED_LIMITS`,
`DELONIX_ALLOW_LINK_LOCAL`, `DELONIX_ALLOW_HOLDER_INGRESS`,
`DELONIX_INSECURE_REGISTRIES`, `DELONIX_INSECURE_BESTEFFORT`,
`DELONIX_*_INSECURE_TLS`, `DELONIX_POLICY_LINT=0`, `DELONIX_CRI_CAP_CEILING*`,
`DELONIX_SSH_STRICT`, `DELONIX_NO_AUTO_DELEGATE`.
