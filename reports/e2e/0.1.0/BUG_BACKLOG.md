# BUG_BACKLOG.md — 0.1.0 (@ 1898466b)

Grouped by severity. Evidence in `report.md`; CRI baselines in
`reports/cri-conformance-0.1.1/`. All reproduced on 2026-11-08.

## P1

### BUG-0001 — `make build`/`make install` do not produce the `delonix-cri` binary
- **Component:** `Makefile:44` (`PACKAGES`), `install`/`build` target
- **Owner:** unassigned
- **Impact:** a clean install from the tree fails (`error: target/release/delonix-cri is missing` → `Error 1`). Any user running `make install` cannot install the full set; the CRI is left out.
- **Reproduction:** `make install` on a clean `develop` tree. `PACKAGES` lists the library crate `delonix-cri` (no `[[bin]]`) instead of `delonix-cri-bin` (`bins/delonix-cri-bin/Cargo.toml`, which produces the `delonix-cri` bin). ADR-0086 moved the bin; `scripts/critest.sh` has already been fixed, the `Makefile` has not.
- **Acceptance criteria:** `make install` on a clean tree installs the 5 binaries with no manual steps; `make build` produces `target/release/delonix-cri`.
- **Related tests:** none (see M-TEST-03: add gate).
- **Dependencies:** none. **Complexity:** XS.

### BUG-0003 — AppArmor is not enforced (4 CRI specs fail on rootful-VM)
- **Component:** `crates/adapters/delonix-linux/src/lib.rs:3370` (`apply_apparmor`, called at `:4583` run / `:9803` exec); `crates/adapters/delonix-linux/src/run_host.rs:58-131` (`ensure_apparmor`, loads the profile via `apparmor_parser`); `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:1664-1705` (security-context parse: new field `apparmor`/`SecurityProfile` + `apparmor_profile` fallback), validation `:625`, reapply `:2049`.
- **Owner:** unassigned
- **Impact:** the runtime does not enforce the AppArmor profile requested by the pod — writes the profile should block go through. A security-isolation failure; blocks CRI conformance and use as a k8s node runtime where profiles are required.
- **Reproduction:** critest rootful-VM, specs "AppArmor runtime should support apparmor field" (blocking writes; permissive deprecated) and "should prefer new apparmor field" (blocking writes; work with profile). They fail with real root (`apparmor_linux.go:252`) → this is not a "profile not loaded because rootless" case.
- **Acceptance criteria:** the 4 AppArmor specs pass on rootful-VM; a profile that blocks writes is effectively enforced on the container (write denied with EACCES).
- **Related tests:** critest `-ginkgo.focus=AppArmor`.
- **Dependencies:** investigate whether `ensure_apparmor` loads the profile the suite installs and whether `apply_apparmor` (aa_change_onexec) runs on the rootful path. **Complexity:** M.

### BUG-0004 — HostNetwork and HostIpc not supported (4 CRI specs)
- **Component:** `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:38-43` (fields `host_network`/`host_ipc`), `:86-92` (`ns_mode` → `NamespaceMode::Node`), `:1093-1098`; `crates/adapters/delonix-linux/src/launch_spec.rs:43` (`host_ipc` carried through).
- **Owner:** unassigned
- **Impact:** pods that request host netns or host ipc do not get the sharing — this breaks networking/monitoring DaemonSets and any host-network workload (the mode in which k8s runs many add-ons).
- **Reproduction:** critest rootful-VM, "Security Context NamespaceOption" HostNetwork true/false and HostIpc true/false. The flag is **recorded and propagated**, but Explore found no mechanism to *join* the host IPC namespace in `delonix-linux` (no `setns` IPC) — the likely root of the gap.
- **Acceptance criteria:** the 4 specs pass on rootful-VM; a container with `hostIpc`/`hostNetwork` effectively shares the host namespace.
- **Related tests:** critest `-ginkgo.focus=NamespaceOption`.
- **Dependencies:** enables GAP-0001 (PodPID) and GAP-0003 (port-forward host net). **Complexity:** L.

## P2

### BUG-0005 — Mount Propagation `rshared` does not work (host↔container)
- **Component:** `crates/adapters/delonix-linux/src/lib.rs:2504-2514` (root propagation decision: `MS_PRIVATE` by default, changes when some mount `wants_propagation()`), `:1505-1508`/`:1715-1718` (`mount()` with `MS_SLAVE`/`MS_SHARED`); `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:3357-3361` (`PropagationBidirectional` → `rshared`).
- **Owner:** unassigned
- **Impact:** mounts created on the host after startup do not propagate bidirectionally to the container (and vice versa). Affects CSI/mount-propagation in k8s.
- **Reproduction:** critest rootful-VM, "Container Mount Propagation … `rshared` should support propagation from host to container and vice versa". (`rprivate`/`rslave`/readonly already pass.)
- **Acceptance criteria:** the `rshared` spec passes; a mount made on the host after startup appears in the container and vice versa.
- **Related tests:** critest `-ginkgo.focus=Mount Propagation`.
- **Dependencies:** none. **Complexity:** M.

### BUG-0006 — OOMKilled not reported (exitCode 137 / reason)
- **Component:** `crates/adapters/delonix-linux/src/lib.rs:146` (`died_of_oom`, reads `memory.events.local`), `:111`/`:126` (`parse_oom_kill`/`oom_kill_count`), tag `:9000`/`:9132-9144`; `crates/foundation/delonix-model/src/records.rs:227-230` (`exit_code()` → 137); `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:1022` (`exit_reason` → `"OOMKilled"`).
- **Owner:** unassigned
- **Impact:** a container killed by OOM is not reported as `OOMKilled`/137 — operators and k8s lose the memory-restart signal.
- **Reproduction:** critest rootful-VM, "Container OOM … should terminate with exitCode 137 and reason OOMKilled". The infra exists end-to-end; it fails even with real root.
- **Acceptance criteria:** the spec passes on rootful-VM; a container that exceeds its memory limit reports exit 137 + reason OOMKilled.
- **Related tests:** critest `-ginkgo.focus=OOM`.
- **Dependencies:** confirm whether `memory.max` is applied and `memory.events.local` is read on the container cgroup inside the VM. **Complexity:** M.

## Outside this file
- The `same_behaviour "stats all"` failure in the `fast` tier is **NOT a product bug**
  (identical handler, verified by `diff`). It is a harness defect → **M-TEST-02**
  in `BACKLOG.md`. The deliberate seccomp `nil`=unconfined divergence stays as a
  conformance-only flag, not a bug.
