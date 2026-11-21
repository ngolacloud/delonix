# Audit — NATIVE VM path (Cloud Hypervisor, no libvirt)

**Date:** 2026-10-24 · **Reference host:** see `evidence/reference-config.txt`
**Audited binary:** `delonix 5.1.1` (commit `f49d3570c`, +45 since v5.1.1) ·
**Repo `develop`:** `69c349c3`.
**Short verdict:** the native path **works end-to-end and is reproducible**
(harness: **28/28** assertions, `RC=0`, `evidence/harness-run-2026-10-24.txt`).
There is no libvirt on the native execution path. The initial "failures" I
chased were **expected rootless SDN topology**, not defects. There is **one real
correctness bug** (the `ssh <ip>` hint) and a **validation debt** (capabilities
honestly marked `partial` that the harness now proves).

---

## 1. How Delonix creates and controls a native VM (real components)

Demonstrated live: `vm create … --backend cloud-hypervisor` starts this
process (no libvirt, no qemu), confirmed by `ps`:

```
cloud-hypervisor --api-socket <root>/vms/<vm>.sock \
  --firmware /usr/local/share/delonix/CLOUDHV.fd \
  --disk path=<root>/vms/<vm>.qcow2,image_type=qcow2,backing_files=on \
  --disk path=<root>/vms/<vm>/seed.iso \
  --cpus boot=2 --memory size=2048M \
  --net tap=vt<hex>,mac=52:54:00:.. --serial socket=<root>/vms/<vm>.console --console off
```

| Responsibility | Component | Anchor |
|---|---|---|
| argv construction / boot | `boot_ch` | `providers/delonix-provider-cloud-hypervisor/src/lib.rs:1491-1592` |
| Boot: firmware **EDK2 `CLOUDHV.fd`** (not UEFI-libvirt) or direct kernel | `default_ch_firmware`, `DEFAULT_CH_FIRMWARES` | CH `:1317-1352` |
| VMM control: **hand-written HTTP/1.1 over the Unix api-socket** (no `ch-remote`, no log parsing) | `ch_api_call_with_body` | CH `:1398-1451` |
| pause/resume/stop/resize | `PUT /api/v1/vm.{pause,resume}`, `vmm.shutdown`, `vm.resize` | CH `:973-979,918-932,1012-1018,1072-1078` |
| Readiness = **VM state**, not "process alive" | `wait_vmm_ready` → `GET /api/v1/vm.info` waits for `"state":"Running"` | CH `:1725-1764` |
| Keeps pid+starttime (PID-reuse guard) + zombie | `vmm_left`, `vmm_to_signal`, `safe_to_signal` | CH `:130-190,801-804` |
| TAP created **inside the rootless infra netns** (`nsenter`) and attached to the bridge | `boot_on_network`→`do_vmtap` | CH `:709-749`; sdn `infra.rs:4417-4448` |
| Anti-spoof (nft table `dlxspoof`, MAC/lease binding) | `do_vmtap` | sdn `infra.rs:4163,4431-4446` |
| NoCloud cloud-init seed (user-data/meta-data/network-config → `cloud-localds`) | `generate_seed_iso` | adapters/delonix-vm `cloudinit.rs:145-219` |
| Persistence: JSON record, qcow2 overlay, pid, log, sockets | `JsonStore`/`vms_dir` | delonix-vm `lib.rs:91-103`; compute `vm.rs:333-353` |

**Processes alive while the VM runs:** only the `cloud-hypervisor` (one per VM) +
the netns holder (`delonix netns pin`) + `slirp4netns` shared by the SDN.
No VM daemon, no libvirtd on the path.

## 2. Backend contract and state

Two ports coexist (ADR-0044 migration in progress):
- **Legacy `trait VmBackend`** (wired today into selection/registration): rich
  lifecycle, optional *fail-closed* methods — `compute/src/vm_backend.rs:526-994`.
- **New `trait VmProvider: Provider`** (5 verbs: create/start/stop/destroy/
  observe) + named capabilities — `compute/src/vm_provider.rs:151-206`.

State is **not** a single `status` field: desired (record `Vm`/`VmConfig`),
observed (recomputed: `is_running`/`ip`/`observe` → `VmObservation`), and
in-progress (stages `CreateStage`/`DestroyStage`). IP confidence is its own
axis (`IpConfidence::{Observed,Predicted,Unknown}`). Readiness is **layered**
(VMM process → VM Running via `vm.info` → guest OS via `guest_info` →
guest service via `HotplugOutcome::Complete`). — `vm_provider.rs:170-192`,
`vm_backend.rs:400-498`.

**Backend selection with no silent fallback** (`compute/src/vm_registry.rs`):
explicit name → `make_backend`+`require_capabilities`; otherwise `auto_detect` by
*seed* order (cloud-hypervisor before libvirt). A record with an unresolvable
backend → error `UnregisteredBackendInRecord`, never a guess
(`:325-348`). The `vm default-backend` default on this host is `libvirt`, but the
native path is explicitly selectable and **does not fall back** to libvirt.

## 3. libvirt independence (native path)

- Crate `delonix-provider-cloud-hypervisor`: **no** libvirt/virsh dependency in
  `Cargo.toml`. Every mention of libvirt in `lib.rs` is a comment or error text
  that *suggests* `--backend libvirt` as an alternative — it never invokes it.
- When a feature is missing (e.g. virtio-9p volumes) the backend **refuses**
  with `Error::RequiresLibvirtBackend` (CH `:775-780`) instead of falling back to libvirt.
- Live: `ps`/`pgrep` confirm zero `qemu`/`libvirtd` for the native VM.
  (Environment note: this host *does* have libvirtd active for the owner's lab,
  but it does not participate in the CH path — independence is in the runtime path,
  not in the absence of the package. Validation on a host without libvirt installed
  is left for the queue.)

## 4. Reproduced failures — and what they actually are

| Initial observation | Diagnosis | Verdict |
|---|---|---|
| `ping 10.200.254.151` from host → 100% loss | IP lives in the **rootless infra netns**; slirp4netns `--disable-host-loopback`; host has no route to 10.200/16 | **Expected** (topology), not a bug |
| `ip link show vt<hex>` on host → "does not exist" | The tap is **inside** the netns (`nsenter -t <holder> --user --net` shows it UP) | **Expected**, not a bug |
| Silent serial console | Serial socket is *live*; we connected after boot — no replay | **Expected**, not a bug |
| Suggested `ssh delonix@<ip>` fails (timeout) | **Unconditional hint** in `vm.rs:5487-5490`; in the rootless default the IP is not reachable from the host — only `delonix vm ssh` reaches it | **Real BUG** (see queue #1) |

The guest, reached by the correct path (`delonix vm ssh`), is **fully
operational**: cloud-init hostname+key applied, 2 vCPUs, ~2 GiB, writable
disk, reachable gateway, DNS/egress via slirp NAT.

## 5. Lifecycle — live result (harness, 28/28)

create+boot+cloud-init · guest exec · CPU/mem/disk/network/DNS · stop→start
(stable machine-id, disk persists) · restart (uptime resets, disk persists)
· pause→unpause · cold resize 2→3 vCPUs (**guest confirms `nproc=3`**) · snapshot
(stopped) created+listed+removed · concurrency (2 parallel creates → distinct IPs
**and** MACs) · **destroy with no orphans** (process, tap in netns, files,
record — all cleaned up). Evidence: `evidence/harness-run-2026-10-24.txt`.

## 6. Honest classification (implemented/validated/partial/not-supported)

See `MATRIZ-CAPACIDADES.md`. Summary: the capability catalogue
(`compute/src/capability.rs`) already **is** the mechanism asked for in the
briefing — 6 states, mandatory evidence referencing a test verified by a CLI
gate, a denominator that does not punish honest refusals. The problem is **not**
over-promising; it is **under**-promising: several capabilities are `partial`
only because "no named battery proves them" — and the harness now proves them.

## 6b. Fixes implemented in this session (patches ready to land)

Both prepared in *detached* worktrees (without consuming the WIP cap), compiled
and verified; the `gitflow start`+commit is left for when there is a slot (see §7).

- **`fix-vm-ssh-hint.patch`** (base `main` @ab4751ac) — `print_vm_next_steps`
  now leads with `delonix vm ssh <name>` and keeps `ssh <user>@<ip>` as a
  labelled fallback; pt-AO translation included. `cargo check -p
  delonix-runtime-bin` green.
- **`feat-vm-capability-graduation.patch`** (base `develop` @69c349c3) — graduates
  6 CH capabilities from `partial`→`supported` (`vm.destroy/restart/pause/resume/
  resize.cold` via new `check "CH: …"` checks in `scripts/e2e.sh`;
  `vm.cloud-init` via `live:scripts/vm-native-ch-lifecycle.sh`, the harness
  made tracked). Matrix regenerated. **Validated:**
  - both gate tests pass: `every_supported_capability_cites_evidence_that_exists`
    and `the_published_matrix_is_the_generated_one` (`cargo test -p delonix-runtime-bin`).
  - the VMM-level checks run LIVE with the new binary: **14/14**.
  - new discovery: vm-compute **12 supported / 6 partial** (was 6/12).

## 7. Prioritised fix queue (root causes)

1. **[BUG] misleading `ssh <ip>` hint on the rootless native path.**
   `bins/delonix-runtime-bin/src/cmd/vm.rs:5482-5493`: lead with
   `delonix vm ssh <name>` (works in all topologies, as the *appliance* branch
   already does in `:5499-5506`) and demote `ssh <user>@<ip>` to a secondary
   line labelled "host-reachable networks". Fix on **`main`** (bug), propagates via GitFlow.
2. **[VALIDATION] graduate `partial→supported` capabilities** with `evidence:`
   pointing at this harness, after integrating it as a named battery — targets:
   `vm.destroy`, `vm.restart`, `vm.pause`, `vm.resume`, `vm.resize.cold`,
   `vm.cloud-init` (guest login proof). Touches the CH provider's `match classify`;
   requires the gate that verifies the evidence reference (see `capability.rs:21-24`).
   Feat on **`develop`**.
3. **[CI] integrate `harness-ch-lifecycle.sh` as a battery** gated on
   `/dev/kvm`+`cloud-hypervisor`+golden image; clean *skip* when absent,
   failure when present and an assertion breaks (§13.9/§16).
4. **[ENVIRONMENT] re-run the harness on a host WITHOUT libvirt installed** to
   close the total-independence proof (§3, §16).

## 8. Acceptance criteria (§16) — status

| Criterion | Status |
|---|---|
| Works without libvirt in the reference environment | ✔ (runtime path; host without the package still missing — queue #4) |
| Advertised capabilities with real tests | ◑ harness proves the core; graduation (#2) and CI (#3) still missing |
| No hidden fallbacks | ✔ (`vm_registry.rs:325-348`) |
| Guest boot + readiness proven | ✔ (harness A/A2) |
| Network and persistence | ✔ (harness A2/B/C) |
| Idempotent lifecycle where defined | ✔ (idempotent start; stop/start/restart) |
| Concurrency does not duplicate resources | ✔ (distinct IPs+MACs+taps) |
| Restarts/failures with recovery | ◑ reboot ✔; crash-recovery (pid+starttime guard exists) still to be exercised |
| Deletion with no orphans | ✔ (harness H) |
| Persistent volumes not deleted | ✔ (destroy removes only the VM's overlay/ephemeral) |
| Security/permissions | ◑ /dev/kvm by group, structured argv (no shell-injection), socket 0600; dedicated audit still to do |
| Logs diagnose failures | ✔ (`<vm>.log` + tail on errors) |
| CI reproduces | ✗ pending (#3) |
