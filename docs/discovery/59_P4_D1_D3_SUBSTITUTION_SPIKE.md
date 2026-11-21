# 59 — P4/D1-D4: the substitution spike (ADR-0044)

**Date:** 2026-09-18 · **Base:** `origin/main` (`6c063295`, v4.0.0 +9 commits) ·
**Verdict: GO.** The same `VmSpec` converges Cloud Hypervisor and libvirt end to end,
live, with zero branching by backend name outside the registry — the gate this ADR
requires before "Accepted" for D1-D4.

## The question

Spike #1 of ADR-0044: "Hand the *same* `VmSpec` to `delonix-provider-cloud-hypervisor`
and `delonix-provider-libvirt` and converge both — `delonix vm create`, `stop`, `start` on
each, from one manifest, with **zero** `#[cfg]`/name-branching outside the composition
root. Measure by `grep -rn 'backend.contains\|backend\.id() =='` returning zero outside
`bins/`, not by inspection."

## How it was measured

- **`crates/adapters/delonix-vm/src/provider_spike.rs`** (new, `pub mod`, marked SPIKE):
  `VmSpec`/`Extensions`/`CloudHypervisorExt`/`LibvirtExt` (D1/D2), `Provider`/`VmProvider`
  (D3), `LocalVmProvider` + `registry(id)` (D4). **Reuses `create_with`/`start`/
  `stop`/`status`/`remove` verbatim** — this spike's contract is the SHAPE (`VmSpec`+
  `Extensions` → `VmConfig`, one call per operation, zero name-branching outside the
  registry), not a second orchestration engine alongside the one that already exists,
  already tested, and that this repo already trusts.
- **`crates/adapters/delonix-vm/tests/provider_spike_live.rs`** (new, `#[ignore]`): the
  SAME `VmSpec` through `registry("cloud-hypervisor")`/`registry("libvirt")`,
  create→observe→stop→observe→start→observe→destroy, isolated root (`tempdir`), a shared
  64 MiB base disk from `qemu-img create`, `cloud_init: Some(false)` (no
  guest OS — this spike proves mechanics, not a real guest, the same choice already made
  for the Proxmox spike, `58_...md`).
- **The field classification was MEASURED, not copied from the D1 table** — `grep`/direct
  reading of `boot_ch`/`libvirt_domain_xml` instead of trusting the original ADR's table
  (see "Findings", below).

## What was measured

**libvirt: full convergence, through the `cargo test` test binary**:
```
=== libvirt ===
  created
  after-create: running=true ip=None confidence=Unknown
  after-stop: running=false
  after-start: running=true
  destroyed
```

**Cloud Hypervisor: blocked in the test binary for a structural reason, not of this
port** — `no VM network provider is registered in this process`. `delonix_vm::
set_network(...)` is only called by the real `main.rs`; a `cargo test` binary never runs
`main`. Temporarily fixed by registering the port in the test (needed
`delonix-sdn` as a dev-dependency — reverted afterward, see "What stayed", below) and
measured again: **a SECOND blocker, this one genuine and with no cheap fix**: the
`netns pin`'s re-exec uses `std::env::current_exe()`, which in a test binary resolves to
the TEST binary ITSELF, not to `delonix`. Without a `DELONIX_BIN`-style override for
this path (one exists for OTHER self-execs, `delonix_node::dispatch::cli_bin`, but not
for the SDN's pin/holder), CH cannot complete its lifecycle from a `cargo test`.

**The CH proof was completed through the REAL `delonix` binary**, via a temporary hidden
verb (`DLX_P4SPIKE=1 delonix version`, reverted — see "What stayed"):
```
$ DLX_P4SPIKE=1 delonix version
P4SPIKE: created
P4SPIKE: after-create running=true ip=Some("10.200.254.164") confidence=Predicted
P4SPIKE: after-stop running=false
P4SPIKE: after-start running=true
P4SPIKE: destroyed
```

**Confirmed clean afterward**: no orphan lease in IPAM, no leftover VM, no leftover `nft`
rule, production containers untouched (verified with `delonix network ipam ls`/
`delonix vm ls`/`nft list ruleset`/`delonix container ls` against the real host).

**The ADR's own measurement** (`grep -rn 'backend.contains\|backend\.id() =='
--include=*.rs crates/`) remains at **1 hit, inside a comment** — the same as
before this spike. The new module introduces no name-branching outside
`registry(id)`, the only function where a string is compared.

## Findings — D1 had two fields misclassified

Measured by direct reading of `boot_ch` (lines 1907-2177) and `libvirt_domain_xml`
(2590-2957), not from the ADR's table:

1. **`devices` (VFIO passthrough) IS shared by CH and libvirt** — D1's original table said
   "not universal even between the two local backends — libvirt-only in practice
   today". False: `boot_ch` iterates `cfg.devices` for `--device path=…` (line 1968) and
   `libvirt_domain_xml` iterates the SAME field for `<hostdev>` (line 2922) — the same
   `Vec<String>` of sysfs paths, identical format. It stays duplicated in
   `CloudHypervisorExt`/`LibvirtExt` (not promoted to `VmSpec`, because Proxmox refuses it
   entirely — it's not universal to ALL providers, only to the two local ones) — a
   divergence documented in the module itself, with the honest route (a shared `LocalExt`
   tier for CH+libvirt, absent for remote providers) named and not built, out of scope for
   this spike.
2. **`serial_capture` was in D1's table, but misclassified — lumped into the list of
   fields "refused by Proxmox" (destined for `Extensions`), as if it were libvirt-specific.**
   It is universal. `boot_ch`
   (lines 1980/1986) and `libvirt_domain_xml` (line 2578) read it identically: "capture
   the console to a file instead of exposing it interactively" is not a notion
   specific to either one. It belongs in `VmSpec`.

## Operational findings — two real blockers no reading would ever reveal

3. **`network: "default"` is not the CLI's default value — it's `"ingress"`.** This
   spike's first attempt used `"default"` (by analogy with `namespace: "default"`) and
   got `no such ingress network 'default' does not exist`. `cmd/vm.rs:519` shows the
   real `default_value`: `"ingress"`. An easy mistake to repeat for anyone designing
   against `VmSpec` without running anything.
4. **The `netns pin`'s re-exec has no override for `current_exe()`, unlike
   OTHER self-execs in this engine.** `delonix_node::dispatch::CLI_BIN_ENV`
   (`DELONIX_BIN`) exists precisely for "a server that re-runs the CLI" — but
   `delonix-sdn::infra::start_control` (and the other two `current_exe()` calls in the
   same file) don't consult it. This is DIRECTLY relevant to spike #2 (the
   `delonix-launcher`/`delonix-netns-holder` binary pair, P4d): if the holder becomes its
   own separate binary, the question "which binary re-executes itself" stops having an
   implicit answer (`current_exe()` == `delonix`) and needs the same
   `DELONIX_BIN`-style override `delonix_node::dispatch` has already resolved for another
   path. **A finding that carries over to spike #2**, not fixed here.

## What stayed

**Stayed in the repository** (not "spike, don't merge" this time — see why): the entire
`provider_spike.rs` module (types + `spec_to_config` + `registry` + 5 pure tests) and
the live test `tests/provider_spike_live.rs` (the libvirt half is genuinely green in CI;
the CH half is SKIP with the exact reason documented, never an opaque error). Unlike the
D5/D6 spikes (a throwaway exploration), this code duplicates NOTHING — it's a thin shell
over the already-tested `create_with`/`stop`/`start`/`status`/`remove`, introduces no
risk to the existing public surface, and is directly the real start of what P4b has to
build. Keeping this is work saved, not debt.

**Reverted, not kept**: the `delonix-sdn` dependency as a `[dev-dependencies]` of
`delonix-vm` (it reintroduced exactly the dependency this port exists to remove —
ADR-0040 P3 — without resolving the `current_exe()` blocker in any way); the hidden
verb `DLX_P4SPIKE` in `main.rs` + `cmd::vm::p4_substitution_spike_ch()` (a one-off
investigation, not a capability to expose). The report above cites its textual output as
proof; the code that produced it doesn't need to survive this session.

## Proven vs not validated

**Proven**: full convergence of CH+libvirt through the SAME `VmSpec`, live, in both
directions of the lifecycle (stop→start); zero name-branching outside `registry`; the two
corrections to D1 (`devices`, `serial_capture`); the real value of the `network` default;
the `current_exe()` blocker in the test binary, and its lack of an override.

**Not validated**: `Extensions` with real content (this spike used `Extensions::default()`
on both sides, deliberately, to isolate the convergence of the universal fields —
`spec_to_config`'s pure tests cover the mapping of `CloudHypervisorExt`/`LibvirtExt`,
but not a VM actually booting with them); a real guest (installed OS, real cloud-init);
Proxmox (out of scope, P4c); withdrawing `delonix-vm` as a crate name and splitting it
into `delonix-provider-cloud-hypervisor`/`delonix-provider-libvirt` (D2.3/D2.5) — this
spike proves the SHAPE works, it does not replace the real work of moving files and
crates that P4b still has to do.
