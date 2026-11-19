# PERF_KERNEL_BACKLOG.md — 0.1.1 (@596663c6)

**Top-priority axis (owner's decision, 2026-11-17):** make delonix
ever more a tool **native to the Linux kernel**, be strict about
performance and make better use of Rust for the problems Rust solves
very well (memory safety, zero-cost abstractions, controlled ABI/FFI).

Inventory of what is already native (strong): complete namespaces (`delonix-linux`,
direct syscalls, "mini-runc as a library"), the **new mount API** `fsopen`/
`fsmount`/`move_mount` via `rustix` (`lib.rs:2146-2270`, ADR-0037), seccomp via
`seccompiler` (`seccomp_profile.rs`), cgroup v2 + freezer + PSI through the direct fs
(`workload_view.rs:83`, `pressure.rs:296`), raw v3 `capset` capabilities
(`lib.rs:862`). Base crates: `nix`/`libc`/`rustix`/`seccompiler`.

The 5 highest-return levers + hygiene. Own numbering `KRN-NNNN`.

## State (2026-11-17) — KRN-0001 and KRN-0004 with SLICE 1 DONE (not committed)

| Item | Slice 1 delivered | Proof | Test | Gate |
|------|------------------|-------|-------|------|
| **KRN-0004** ✅ | Best-effort Landlock FS sandbox on init (`delonix-linux/src/lib.rs` `apply_landlock`), opt-in via `--read-only`; covers RO rootfs + tmpfs/`--tmpfs`/rw binds. dep `landlock=0.4` | `run --read-only --tmpfs /scratch`: rw tmpfs writes, RO rootfs denies; EACCES distinct from EROFS proves enforcement | 250+ delonix-linux tests green; regression in `scripts/e2e.sh` | `arch_fitness` green (baseline `library_prints` 111→113, 2 security eprintln) |
| **KRN-0005** ✅ (s1) | Native KVM probe via `kvm-ioctls=0.25` (rust-vmm): `kvm_usable()` in `delonix-provider-cloud-hypervisor` does `Kvm::new()`+`get_api_version()==12` instead of `Path::new("/dev/kvm").exists()` (which lied when the device was present but there was no permission). Does NOT touch boot (the VMM stays external) | host with /dev/kvm: `provider ls`/`describe` Ok; strace `openat(/dev/kvm)+ioctl(KVM_GET_API_VERSION)=12` | 44 provider tests (probe agrees with a real open) | `arch_fitness` + `cargo check --workspace` green |
| **KRN-0003** ✅ | io_uring on host-side I/O: `copy_file_uring` (new `delonix-oci/src/uring.rs`, synchronous `io-uring=0.7` crate) with a `std::fs::copy` fallback; applied in `delonix cp`. HOST-side only (the container seccomp keeps io_uring_* denied) | `cp` roundtrip host→container→host of 1.5 MiB with identical sha256; strace confirms 4 io_uring syscalls in ROOTLESS (fast path exercised, not the fallback) | 198 delonix-oci tests (byte-identical + mode + empty) | `arch_fitness` + `cargo check --workspace` green |
| **KRN-0006** ✅ | `cap_num` via the `caps=0.5` crate (manual ~45-entry ABI table removed); `cap_name` tied to the crate by 2 tests (round-trip + direct agreement with `caps::all()`) | n/a (refactor); guaranteed by tests | 252 delonix-linux tests green | `arch_fitness` + `cargo check --workspace` green |
| **KRN-0002** ✅ | pidfd on signal sending: `send_signal` (`delonix-linux/src/lib.rs`) opens `pidfd_open` before the identity check and uses `pidfd_send_signal` (fallback `kill` < 5.3) — closes the PID-reuse TOCTOU window of `kill`/`stop`/docker-api | `kill`/`stop` kill via pidfd (kernel 7.0); regression test: pidfd gives ESRCH after reap | 251 delonix-linux tests green | `arch_fitness` + `cargo check --workspace` green |
| **KRN-0001** ✅ | **F1** `link_exists`→`RTM_GETLINK` (new `delonix-sdn/src/netlink.rs`, sync→async tokio `block_on` bridge; kills the stdout-sniffing bug). **F2** `link_set_up`→`RTM_NEWLINK` (`LinkUnspec::new_with_index().up()`). **F3** `addr_add`→`RTM_NEWADDR` (`address().add()`). **All HOST-SIDE call-sites migrated** (10 in infra.rs: link exists/up + addr add). deps `rtnetlink=0.24`+`tokio`+`futures-util` | F1 `lo`→true. F2/F3 e2e networking: bridge+veth `UP,LOWER_UP`, **gateway reachable** (native addr_add), ingress HTTP 200 on port, DNS, c2c | 349 delonix-sdn tests green | `arch_fitness` + `cargo check --workspace` green |

**Slice 4 (COMPLETE, except dynamic sync)**: `with_netns` (setns CLONE_NEWNET on a throwaway thread, the `cni::set_netns_sysctls` pattern) runs netlink INSIDE the container netns. New native ops: `route_add_default` (RTM_NEWROUTE v4+v6) and `addr_add_flags` (IFA_F_NODAD). `do_attach` and `do_attach_extra` fully migrated (v4+v6: addr/link/route). **Proven in rootless**: container with v4 (`UP,LOWER_UP`) + v6 ULA nodad + default v4 and v6 routes, connectivity + ingress HTTP 200. **Zero `ip netns exec` in interface setup.** All that remains in the CLI is `sync_workload_routes` (query+diff+mutate of routes with parsing — a slice of its own in the route family).

Remaining: KRN-0001 `sync_workload_routes` (route family: typed show/add/del). KRN-0004 slice 2 (its own opt-in). KRN-0002/0003/0005/0006 not yet started.

---

## P1 (performance / hot path)

### KRN-0001 — Native netlink (`rtnetlink`) instead of `ip`/`tc`/`bridge`/`nft` via CLI
- **Current behaviour:** the whole SDN datapath does `Command::new("ip"/"tc"/"nft")`: `crates/adapters/delonix-sdn/src/infra.rs:4821,4921,5291,5583`, `cni.rs:703-773`, `wg.rs:75`, slirp in `infra.rs:2522`. Dozens of fork+exec+text-parse per container/VM start.
- **Opportunity:** replace with `rtnetlink`/`netlink-packet` (and evaluate `nftables`/`rustables` for the firewall). Gains: less latency on the hot path (no fork+exec+parse), **structured errors** instead of text parsing, and removal of the dependency on iproute2/nftables being installed on the host — closer to "native to the kernel".
- **Acceptance criteria:** creation of a container's netns+veth+bridge+addr without invoking `ip` (measure the reduction in syscalls/latency at startup vs baseline); documented fallback. Before/after benchmark in `tests/`.
- **Dependencies:** none. **Complexity:** XL. **Tier:** B.

### KRN-0003 — io_uring on heavy I/O (CRI logs, CAS copies, overlay copy-up)
- **Current behaviour:** ABSENT out of caution (`crates/adapters/delonix-linux/src/lib.rs:719` — "io_uring is LEFT OUT, as in Docker"); async = `tokio` epoll (`delonix-cri/src/lib.rs:500`). But the runtime moves a lot of bytes: log streaming (`delonix-cri/src/streaming.rs`), extraction/copy of OCI layers, overlay copy-up.
- **Opportunity:** an `io-uring`/`tokio-uring` backend **on the host side of the runtime** (never inside the container seccomp — keep `io_uring_*` denied there). The biggest pure performance gain in I/O, fewer syscalls and context switches.
- **Acceptance criteria:** the log I/O and layer-copy path with an opt-in io_uring backend, with a throughput/latency benchmark vs epoll; io_uring stays denied in the container seccomp profile.
- **Dependencies:** none. **Complexity:** XL. **Tier:** B.

## P1 (robustness / security)

### KRN-0002 — pidfd as primary supervision of containers/VMMs (PARTIAL today)
- **Current behaviour:** `pidfd` only in `exec` (`crates/interfaces/delonix-cri/src/child_handle.rs:62,94`, ADR-0027). The core still signals by PID and acknowledges the gap (`delonix-linux/src/lib.rs:3401` "to fix with a pidfd"). The `clone3` `pidfd` field already exists in the struct but is not consumed (`lib.rs:11900`).
- **Opportunity:** `clone3(CLONE_PIDFD)` + `pidfd_open`/`pidfd_send_signal` throughout the lifecycle (`launch.rs` supervisor, watchers). Closes the PID-reuse window in supervision/signalling — correctness and robustness.
- **Acceptance criteria:** supervisor and watchers signal by pidfd; a test that provokes PID recycling and confirms the signal reaches the right process.
- **Dependencies:** connects to GAP-0026 (ensure-running). **Complexity:** L. **Tier:** B.

### KRN-0004 — Landlock as an unprivileged FS sandbox (ABSENT)
- **Current behaviour:** FS confinement only via namespaces+seccomp+caps+AppArmor; `landlock_create_ruleset` is never called (only listed in `seccomp_profile.rs:148-150`).
- **Opportunity:** Landlock is *unprivileged* by design — it fits the **rootless-first** model. A ruleset on the container `init` (and in the runtime's own helpers/daemons) gives FS defence-in-depth without requiring capabilities. A low-cost security gain.
- **Acceptance criteria:** a Landlock ruleset applied to the container init (opt-in by policy); a test proving that an access outside the ruleset is denied even without AppArmor. Evaluate the `landlock` crate.
- **Dependencies:** none. **Complexity:** M. **Tier:** B.

## P2 (platform investment)

### KRN-0005 — Native KVM/virtio in Rust (rust-vmm) on the microVM path
- **Current behaviour:** `/dev/kvm` only probed (`crates/providers/delonix-provider-cloud-hypervisor/src/lib.rs:256`); the whole VMM is external (cloud-hypervisor via api-socket; libvirt via `virsh`). No `kvm-ioctls`/`vhost`/`virtio-bindings`.
- **Opportunity:** integrate the rust-vmm ecosystem (which Cloud Hypervisor itself uses) for microVM boot without depending on the external binary or on api-socket parsing — more control, less process surface, better use of Rust in the virtualisation subsystem.
- **Acceptance criteria:** a minimal microVM boot path via `kvm-ioctls` (a PoC measured vs external cloud-hypervisor). Decision by ADR given the size.
- **Dependencies:** none. **Complexity:** XL. **Tier:** D (needs `/dev/kvm`/VM).

#### Execution prompt (future session, Tier D — on a host with `/dev/kvm`)

```
/escopo

KRN-0005 fatia 1 — adoptar o ecossistema rust-vmm (kvm-ioctls) no delonix,
começando pela peça MAIS ISOLADA e de menor risco, sem tocar no caminho de boot
do cloud-hypervisor. É Tier D: precisa de `/dev/kvm` (host bare-metal com KVM ou
uma VM com nested virt). Dep nova `kvm-ioctls=0.25` (ADR-0040: versão no
workspace; membro `{ workspace = true }`).

Contexto: hoje o VMM é 100% externo (cloud-hypervisor por api-socket, atrás do
port VmBackend de delonix-compute) e o KVM só é SONDADO por existência de ficheiro
(`delonix-provider-cloud-hypervisor/src/lib.rs:256` -> `Path::new("/dev/kvm").exists()`).

FATIA 1 recomendada (a mais segura): substituir a sonda de existência por uma
sonda NATIVA via kvm-ioctls — `Kvm::new()` + `get_api_version()` (== 12) +
`check_extension` de umas poucas capabilities-chave (USER_MEMORY, etc.). Isto:
(a) adopta o rust-vmm de forma mínima e provável; (b) melhora o diagnóstico
(hoje `/dev/kvm` pode existir mas o processo não ter permissão / nested-virt estar
off — a sonda de existência mente); (c) NÃO toca no caminho de boot (zero risco de
regressão de VMs reais). Fallback: se `Kvm::new()` falhar (sem permissão, sem
device), reportar NoKvm com a razão do errno, como hoje mas mais preciso.

FATIA 2+ (fora deste escopo, ADR obrigatório): PoC de boot de microVM mínima
(Kvm + VmFd + VcpuFd + vm-memory + linux-loader a correr um kernel trivial),
medido vs cloud-hypervisor; depois virtio-blk/net via vhost. É XL — só avançar
com ADR que nomeie o consumidor concreto.

Done when:
- A sonda de capacidade do provider cloud-hypervisor usa kvm-ioctls (Kvm::new +
  api_version + check_extension), com fallback de erro preciso; compila e
  `cargo test -p delonix-provider-cloud-hypervisor` passa.
- Prova ao vivo num host com /dev/kvm: `delonix provider ls`/`describe
  cloud-hypervisor` reporta KVM OK via o caminho nativo (e NoKvm+razão quando
  /dev/kvm é removido/sem permissão). Teste de regressão da sonda.
- arch_fitness + cargo check --workspace verdes.

Fora de escopo: substituir o boot do cloud-hypervisor; virtio/vhost; libvirt.
```

### KRN-0006 — Crate hygiene: `caps` and `aya`/`libbpf-rs` (less `unsafe`/fragile ABI)
- **Current behaviour:** v3 `capset` written by hand (`delonix-linux/src/lib.rs:862-899`); eBPF device-cgroup writes a 128-byte `bpf_attr` with manual offsets (`lib.rs:5372-5408`); tc/clsact datapath via a C+clang object (`delonix-sdn/src/bpf.rs`, `build.rs:51`). No `aya`/`libbpf-rs`.
- **Opportunity:** adopt `caps` (replace the raw capset) and `aya`/`libbpf-rs` (eliminate the manual `bpf_attr` offset computation). It brings no new perf, but removes `unsafe`/fragile ABI — aligned with "making better use of Rust".
- **Acceptance criteria:** capset and device-cgroup/eBPF switch to ecosystem crates, with no manual offsets; existing tests stay green.
- **Dependencies:** none. **Complexity:** M. **Tier:** B (eBPF device-cgroup needs rootful validation — partly D).

---

## Additional nativisation opportunities (survey 2026-11-17, KRN-0007..0014)

A sweep of `Command::new`/`run`/`capture` for external tools across the whole
engine. Lists what STILL depends on an external binary + text parsing and could
be a syscall/netlink/native crate. Out of scope (external by nature):
`git`, `systemctl`/`busctl` (systemd/dbus), `ssh`/`scp`, `kubectl`/`helm`,
`qemu-img`/`virt-*` (libguestfs/qcow2), `sudo`, `nvidia-smi`.

### KRN-0007 — Firewall/NAT via NFTABLES netlink (replaces the `nft` CLI)
- **Current behaviour:** ~64 `nft` invocations (of which 57 in `crates/adapters/delonix-sdn/src/infra.rs`, + `policy_nft.rs:4`) build rules by text and read state by parsing. It is the largest block of CLI+text that remains and it sits on the network hot path.
- **Opportunity:** native NFTABLES netlink (crate `rustables`/`nftables`) — typed rules, structured errors, no fork+parse, no dependency on the `nft` binary.
- **Acceptance criteria:** one rule family (e.g. a container's publish/DNAT chain) built via netlink; port publishing + isolation proven live; incremental fallback/migration. **ADR** given the size.
- **Dependencies:** none. **Complexity:** XL. **Tier:** B.

### KRN-0008 — The rest of `ip` via rtnetlink (netns mgmt, rule, route show)
- **Current behaviour:** ~31 `ip` remaining after the link/addr/route migration (`delonix-sdn/src/infra.rs:19`, `lan.rs:4`, `cni.rs:4`, `service_lb.rs:2`): create/delete netns, `ip rule`, `ip route show` (the `sync_workload_routes` with parsing — infra.rs:~5652), `bridge fdb`.
- **Opportunity:** complete the already-integrated `rtnetlink` (KRN-0001) for typed route show/add/del + rule + netns, removing the last bit of network text parsing.
- **Acceptance criteria:** `sync_workload_routes` and netns management without `ip netns exec`/`ip route show`; e2e network tests intact.
- **Dependencies:** on KRN-0001. **Complexity:** L. **Tier:** B.

### KRN-0009 — `setns`/`unshare` via syscall (replaces the `nsenter`/`unshare` CLI)
- **Current behaviour:** `nsenter` ×9 + `unshare` ×10 as BINARIES (18 in `delonix-sdn/src/infra.rs`), to enter/create namespaces.
- **Opportunity:** direct `setns(2)`/`unshare(2)` — the `with_netns` throwaway-thread pattern (`delonix-sdn/src/netlink.rs`, proven this round) generalises to these call-sites.
- **Acceptance criteria:** the host-side `nsenter`/`unshare` call-sites switch to a syscall; container/VM start and e2e networking intact.
- **Dependencies:** none (pattern proven). **Complexity:** M. **Tier:** B.

### KRN-0010 — eBPF datapath via `aya` (replaces `tc` + a C/clang object)
- **Current behaviour:** `delonix-sdn/src/bpf.rs` + `build.rs:51` compile an eBPF object in **C with clang**, loaded via `tc`.
- **Opportunity:** rewrite the datapath in `aya` (eBPF in pure Rust, no C toolchain), loaded via native netlink/bpf syscall.
- **Acceptance criteria:** the observability/datapath eBPF program switches to `aya`; per-container metrics identical; no clang in the build.
- **Dependencies:** none. **Complexity:** L. **Tier:** B (partly D).

### KRN-0011 — device-cgroup without manual `bpf_attr` (via `aya`/`libbpf-rs`)
- **Current behaviour:** `delonix-linux/src/lib.rs:5460` writes a 128-byte `bpf_attr` (PROG_LOAD) with offsets computed by hand (`bpf_insn` at `:5427`) — fragile to ABI changes.
- **Opportunity:** `aya`/`libbpf-rs` eliminate the manual offset computation of the eBPF device-cgroup.
- **Acceptance criteria:** device-cgroup loaded via an ecosystem crate; device policy identical; tests green.
- **Dependencies:** couples to KRN-0010 (same crate choice). **Complexity:** M. **Tier:** D (device-cgroup needs rootful validation).

### KRN-0012 — Writing capabilities via the `caps` crate (raw capset)
- **Current behaviour:** v3 `capset` written via raw syscall in `delonix-linux/src/lib.rs:930` (`switch_user_keeping_caps`) and `:1024` (`drop_capabilities`). KRN-0006 migrated only the READ (`cap_num`).
- **Opportunity:** `caps::set`/`drop` instead of `SYS_capset` + a manual struct.
- **Acceptance criteria:** confinement stays EXACT (KEEPCAPS, bounding drop, setgid/setuid order), proven by CRI capabilities conformance. **CAUTION: security-critical path.**
- **Dependencies:** none. **Complexity:** M, high risk. **Tier:** D (needs rootful-VM to prove confinement).

### KRN-0013 — WireGuard via netlink (replaces the `wg` CLI)
- **Current behaviour:** `wg` ×2 (`delonix-sdn/src/wg.rs`) configures WireGuard interfaces via the CLI.
- **Opportunity:** native genetlink (crate `wireguard-control`/`defguard_wireguard_rs`).
- **Acceptance criteria:** creation/config of a WireGuard peer via netlink; tunnel proven.
- **Dependencies:** none. **Complexity:** M. **Tier:** B.

### KRN-0014 — Minor nativisable items (grouped)
- `bridge` fdb/vlan → rtnetlink; `ss` → `sock_diag` netlink; `losetup` → `LOOP_CONFIGURE` ioctl; `getent` → `getpwnam`/`getgrnam` (libc). Each S, low value in isolation, but removes dependencies on external binaries.
- **Complexity:** S each. **Tier:** B.

> Note: native microVM boot (KVM) is **slice 2 of KRN-0005** (prompt already above), not a new item.

---

## PROTOCOL nativisation in Rust (NAT-0001..0005) — not kernel-native

A category distinct from the KRNs: here there is no syscall that replaces the binary — the
gain is speaking the PROTOCOL in a Rust crate (typed, no fork, no text parsing)
instead of `fork+exec+parse` of a CLI. Only worth it where there is fragile parsing or
a binary dependency to remove. Prefix `NAT-` so as not to confuse with the KRNs.

### NAT-0001 — systemd/D-Bus via `zbus` (replaces `busctl`/`systemctl`) ✅ best candidate
- **Current behaviour:** `busctl` (`delonix-linux/src/lib.rs:11760`) and `systemctl` (`bins/delonix/src/cmd/vm.rs:2521`, boot.rs units) — parsed text, fragile; a fork per operation.
- **Opportunity:** the `zbus` crate (pure-Rust D-Bus) against `org.freedesktop.systemd1` (StartUnit/EnableUnitFiles/etc.). systemd stays external (we do not control it), but the conversation becomes typed.
- **Gain:** HIGH (`busctl` returns text; D-Bus is typed). **Acceptance:** one unit operation (enable/start of boot-persistence) via zbus, proven live; no `busctl` parse.
- **Complexity:** M. **Tier:** B.

### NAT-0002 — k8s client via `kube` (kube-rs) (replaces `kubectl`)
- **Current behaviour:** `kubectl` (`bins/delonix/src/cmd/k8s_recipes.rs:104`, apply/generate manifests) — depends on the binary and its version; output parsing.
- **Opportunity:** `kube` (kube-rs) — typed apply/get/watch against the API-server.
- **Gain:** HIGH if the k8s interaction is programmatic (types, structured errors, no binary). **Acceptance:** an apply/get via kube-rs equivalent to the current `kubectl`, proven against a cluster.
- **Complexity:** L. **Tier:** B (needs a cluster for e2e).

### NAT-0003 — GPUs via `nvml-wrapper` (replaces `nvidia-smi`)
- **Current behaviour:** `nvidia-smi` (`delonix-linux/src/resource_advice.rs:571`) parsed for GPU count/memory/utilisation.
- **Opportunity:** the `nvml-wrapper` crate (NVIDIA Management Library) — typed query. Behind a feature-flag (it pulls in `libnvidia-ml`, proprietary, NVIDIA hosts only).
- **Gain:** MODERATE-HIGH (removes fragile parsing). **Acceptance:** GPU count/usage via NVML == what `nvidia-smi` reported; a build without NVIDIA still compiles (feature off).
- **Complexity:** M. **Tier:** B (needs an NVIDIA host for e2e).

### NAT-0004 — git via `gix`/`git2` (replaces the `git` binary in gitops)
- **Current behaviour:** the `git` CLI in gitops (source pull, ADR-0021) and at build-time (`delonix-node-api/build.rs:58` — this one is only `git describe`, do NOT migrate).
- **Opportunity:** `gix` (pure Rust, no C) or `git2` (libgit2) for clone/fetch/checkout of gitops, without depending on the `git` binary.
- **Gain:** MODERATE — mostly for the control-plane's DISTROLESS IMAGES (no `git`). **Acceptance:** a GitOps reconcile (clone+checkout of a ref) without the `git` binary; the build-time `git describe` stays as is.
- **Complexity:** L. **Tier:** B.

### NAT-0005 — SSH via `russh` (replaces `ssh`/`scp`) — only with fine control
- **Current behaviour:** `ssh` (`delonix-provider-proxmox/src/lib.rs:9322`) and `scp` (`bins/delonix/src/cmd/remote.rs:182`) — cluster/remote bootstrap via the CLI.
- **Opportunity:** `russh` (pure-Rust SSH2) — a programmatic session (exec, SFTP), progress streaming, multiplexing.
- **Gain:** MODERATE (the `ssh` CLI is very robust; SSH is a large surface — auth, known_hosts). Only worth it when fine channel control is needed. **Acceptance:** an equivalent remote exec + transfer via russh; known_hosts/auth preserved.
- **Complexity:** L, medium risk. **Tier:** B.

> KEEP EXTERNAL (no return / high risk): `qemu-img`/`virt-*` (qcow2/libguestfs — complex format, qemu is the reference), `helm` (Go templating), `sudo` (an escalation mechanism — the "native" path would be our own setuid helpers like `delonix-storage-helper`, a privilege re-architecture, not a crate).

---

## Suggested sequence within the axis
KRN-0001 (netlink) and KRN-0003 (io_uring) first — the biggest performance gain on the
hot path. KRN-0002 (pidfd) and KRN-0004 (landlock) next — low-cost robustness and
security. KRN-0005 (native KVM) is the biggest investment (ADR).
KRN-0006 can run in parallel as continuous hygiene.
