# In-depth evaluation — delonix as the base of the NgolaCloud platform (2026-11-18)

Base: `origin/develop` @ `118a4132` (+309 commits since v0.1.0). Worktree
`fix/platform-blockers-w1`. Doctrine: only the verified effect/contract counts.

## Method
Read-only audit across 4 lenses (rootless · daemonless/state · OCI/CRI ·
security/networking), cross-checked with docker/podman/containerd/runc/k8s. Each finding with
`file:line` evidence. Test baseline measured beforehand: `make check` clean,
`make test` **3339 tests / 0 failures**, `version_gate` ok, `arch_fitness` 1 trivial
failure (baseline `cli_context_lines` −6).

## Platform angle
The control-plane does NOT link the runtime as a library (`delonix-seiva` is the Seiva
framework, not the runtime). delonix **is** the execution platform: it runs/orchestrates
workloads and commands nodes through the gRPC node-api + CRI. The risk concentrates on
**rootless multi-tenancy**, **daemonless recovery** and **CRI/OCI conformance**.

## 27 new findings (outside BUG-0007..0009, GAP-0018..0027, KRN/NAT)

### 🔴 Blockers (8) — confirmation status
| # | Finding | Evidence | Confirmation |
|---|---|---|---|
| R1 | subuid/subgid: host base fixed at 100000, ignores the real `start` from `/etc/subuid` | `delonix-linux/src/lib.rs:3160` (`"0 {euid} 1 1 {USERNS_UID_BASE} {range}"`), `:3238` (`subid_file_grants` discards `_start`) | ✅ code. Current host `walter:100000` = happy case; reproduction requires a 2nd tenant (tier-VM/sudo) |
| R2 | host netns shared by default (without `-p`) | `launch.rs:61` (`new_netns` only `net_none`/`slirp_ports`), `lib.rs:9003` (`CLONE_NEWNET` only if `new_netns`), test `lib.rs:16172` | ✅✅ **LIVE**: a container without `-p` read `127.0.0.1:45999` from the host; `ip addr` sees the host's wlp0s20f3/br-*/virbr0 |
| R3 | `default_root` uses `geteuid()` and not `in_initial_userns()` | `delonix-state/src/store.rs:319`, `delonix-oci/src/image.rs:166` (vs the fix already done in `lib.rs:3174-3177`) | ✅ code |
| D1 | `JsonStore<T>` without downgrade-guard (VMs/tunnels/gateways/object-storage) | `delonix-state/src/store.rs:730` (`load` without a gate) vs `:349` (`Store::decode` has a gate) | ✅ code |
| D2 | boot-persistence by bare name, without a namespace | `boot.rs:398` (`{UNIT_PREFIX}{c.name}`), `:415` (vm) | ✅ code (already separates container/VM by prefix, not by tenant) |
| O1 | multi-arch pull falls back to the 1st manifest; `architecture` records the requested one | `registry.rs:2232` (`.or_else(|| index.manifests().first())`), `:2434` (`architecture: requested_arch`) | ✅ code |
| O2 | `repo_digests` empty on pull by tag → kubelet `imageID` | `registry.rs:2414` (only `repo_tags`), `delonix-cri/src/lib.rs:426` (`partition_refs` requires `@`) | ✅ code |
| S1 | `workloadSecurity: baseline` does not reject `--network host` | `admission.rs:519-561` (covers `host_pid`/`host_ipc`, misses `host_network`) | ✅ code |

### 🟡 Medium (10)
O3 ARM variant ignored (`registry.rs:2229`) · O4 Docker-v2 manifest with OCI
mediaTypes (`registry.rs` `layer_media_type`) · O5 `load` does not read a classic
`docker save` (`load.rs:49`) · O6 `load` does not validate layers (`load.rs:96`) · O7 `load`/`save`
buffer in memory → OOM (`load.rs:44`, `save.rs:153`) · D3 PID-reuse `starttime`
without a boot epoch (`delonix-node/src/app/host.rs:119-137`) · D4 legacy records
trust a reused PID (`host.rs:135`) · R4 `/tmp/delonix-net-<uid>` predictable,
`set_permissions` ignored (`delonix-sdn/src/infra.rs:286-358`) · S2 `fwguard` only
IPv4 (v6 metadata SSRF) (`infra.rs:749`) · S3 silent SELinux transition
(`lib.rs:3764`) · S4 permissive v6 link-local anti-spoof (`infra.rs:4463`).

### 🟢 Low / hygiene (9)
O8 `Cas::verify` reads everything into memory (`cas.rs:141`) · O9 CRI `inodes_used=0` + double
counting (`cri/lib.rs:377`) · O10 `remove_image` swallows errors (`cri/lib.rs:351`) ·
O11 resolves by ambiguous prefix (`image.rs:296`) · D5 `--rm` orphan watcher
(`container.rs:4391`) · D6 double reconcile (`container.rs:4334`) · R5 slirp without
`--enable-sandbox/seccomp` (`infra.rs:2507`) · S5 `CAP_NET_BROADCAST` in the default
(`capabilities.rs:24`) · arch_fitness baseline.

## Owner decisions (2026-11-18)
- **Full campaign** until everything is green (gate: `make ci` + rootless e2e + daemonless
  crash/reboot).
- **Align divergent defaults with Docker/Podman** (R2 isolated netns by default;
  R5 slirp sandbox; S5 CAP_NET_BROADCAST; GAP-0023 restartPolicy) with an ADR + a migration
  note for each default change.

## Waves
F0 (this one) → W1 blockers → W2 medium → W3 low → W4 kernel nativisation
(KRN/NAT) + audit of uncovered areas (VM/storage).

## W4 audit — microVM + storage (5 new findings, V1–V5)

Areas the initial 4 lenses did not cover in depth. Numbering `V`.

| # | Finding | Evidence | Status |
|---|---|---|---|
| V1 | Quota-hard volume (loopback): a failed post-reboot remount silently serves an EMPTY `_data` (`Ok`) → the DB re-initialises the datadir (loss/fork of the tenant dataset) | `delonix-volume/src/lib.rs:619` (`let _ = run("mount"...)`) | ✅ **FIXED** — a failed remount is a hard error; rootless not affected (`img.exists()` false) |
| V2 | `policy.json` only applied in the CLI; the gRPC node-API and CRI/kubelet do not consult it → bypass of `deny_latest`/`allowed_registries`/`deny_device_passthrough` | `delonix-node-api/src/vms.rs:166`, `delonix-cri/.../lifecycle.rs:1658` (a comment admits the gap) | ☐ HIGH — architectural (move `policy::enforce` from the `-bin` into the engine); next slice |
| V3 | VFIO passthrough without a per-device allowlist; control is global all-or-nothing | `cloud-hypervisor/src/lib.rs:2128`, `libvirt/src/lib.rs:2125`, `admission.rs:474` | ✅ **FIXED** — `allowedPassthroughDevices` (an exception to `denyDevicePassthrough`); `evaluate` refuses only the BDFs outside the list, matching via `normalize_pci_bdf` (bare BDF or sysfs path, case-insensitive); fail-closed for non-BDF |
| V4 | libvirt silently discards malformed `spec.devices` (non-BDF) (no `else` branch) | `libvirt/src/lib.rs:2125-2142` | ✅ **FIXED** — `validate_passthrough_devices` at the top of `boot` refuses any non-BDF before any effect; same parser as `<hostdev>`. (CH passes verbatim → fails loudly in the VMM, without the silent defect) |
| V5 | `libvirt_xml_overlay`: raw XML injected verbatim, with no admission gate | `libvirt/src/lib.rs:2160` | ✅ **FIXED** — `denyLibvirtXmlOverlay` + `Violation::LibvirtXmlOverlay`; the `Request::with_libvirt_xml_overlay` flag wired into manifest apply (the only surface with raw XML). Live proof of the EFFECT out of scope (needs root + spare PCI) |

Solid areas confirmed: storage-pool allowlist/ownership, volume-name validation,
`dir_usage` (rootless quota), CH `stop`/cgroup-leaf/`move_namespace`,
crash-consistent clone/flatten, libvirt namespace isolation.

## Kernel nativisation (W4) — slices done
`ss` → `/proc/net/*` (vm.rs) · `losetup` → sysfs find + `LOOP_SET_CAPACITY` ioctl
(delonix-volume, fallback for old kernels) · KRN-0001 F5 `sync_workload_routes` →
typed netlink (SHOW/DEL/REPLACE). Remaining backlog: KRN-0007 (nft netlink, XL/ADR),
KRN-0005 slice 2 (microVM KVM boot, Tier-D), NAT-0001..0005.
