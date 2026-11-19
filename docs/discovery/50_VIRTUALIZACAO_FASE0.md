# Phase 0 — the ten fundamentals of virtualization, measured

Survey requested before any design: make the Delonix VM runtime a self-sustaining IaC
(without Terraform), integrable with Terraform via an SDK. This phase's question is a single one
— **what does this engine actually speak, out of the protocols and standards that the whole
virtualization platform shares?**

Measured on 2026-08-12 against `delonix 0.50.0` (commit `00968bd8`), on the development host, with
4 production containers running (`nginx`, `pgvector:pg15`, `kaeso-odoo:18`, `pgvector:pg16`) and the
holder UP (pin 71944, control 306593, bridge `delonix0`, refcount 1). **No measurement mutated
state**: every probe is `plan`/`ls`/`describe`/grep.

## The table

| # | Fundamental | State | Proof |
|---|---|---|---|
| 1 | **TCP/IP** | **Realized** | Live SDN: `kaeso-net` bridge `dlxnd2b6d4` 10.210.0.0/16; 4 containers Up, two with a published port (`8080->80`, `5433->5432`); internal DNS scoped to the namespace |
| 2 | **VLAN (802.1Q)** | **Does not exist** | `grep -rniE '"vlan"\|type vlan\|vlan_id\|vlanId'` → **0** hits outside `macvlan`/`ipvlan` (substring) |
| 3 | **LACP** | **Does not exist** | `lacp`, `bond`, `802.3ad` → **0** hits across all of `crates/` |
| 4 | **VXLAN** | **Realized — and reported as not realized** | `infra::set_vxlan`/`do_vxlan` creates the device (`id <vni>`, `dstport 4789`, `nolearning`), masters it onto the bridge and seeds the FDB; `network create --driver overlay` calls `realize_overlay`. **But see Finding 1** |
| 5 | **OVF/OVA** | **Does not exist** | `ovf`, `\.ova` → **0** hits. `vm convert` handles qcow2/raw/vmdk/vdi/vhdx/vhd — **disk** formats, not **packaging** format |
| 6 | **VirtIO** | **Realized, partially declarable** | libvirt XML emits `bus='virtio'`, `<model type='virtio'/>`, virtio-9p for volumes; CH likewise. Correct fail-closed refusal: `spec.volumes` on a CH says it needs libvirt (no virtio-fs) |
| 7 | **NFS** | **Exists; impossible to mount on this host** | `kind: Volume` with `nfs:`, `mount -t nfs` in `delonix-volume`, TrueNAS provisioning via API. **`CapEff: 0000000000000000`** and no `mount.nfs` → the mount fails here |
| 8 | **iSCSI** | **Does not exist** | `iscsi`, `iscsiadm`, `multipath` → **0** hits |
| 9 | **Fibre Channel** | **Does not exist** | `fibre`, `fcoe` → **0** hits |
| 10 | **NVMe over Fabrics** | **Does not exist** | `nvme-of`, `nvmeof`, `nvme connect` → **0**. The only `nvme` in the code is `/dev/nvme0n1` in an `--io-rate` test |

**Outside the list, but of the same family:** `macvlan`/`ipvlan` are **declarative only** — in the
`NetworkStore`, with a loud `Realized=False reason=DriverNotImplemented` warning. This is honest and
correct: their physical plane needs `CAP_NET_ADMIN` in the host's init-netns.

**Summary: of the ten, three realized (TCP/IP, VXLAN, VirtIO), one realized but unusable on this
host for lack of privilege (NFS), six nonexistent.**

## Finding 1 — `stack plan` lies about the overlay (MEDIUM)

The only one of the ten network fundamentals this engine implements is announced to the user as
not implemented.

Reproduced live, without mutating anything:

```
$ delonix stack plan -f ov.yaml          # kind: Network, driver: overlay, vni: 42
  +   Network/fase0-overlay-probe
        prerequisite Realized: driver 'overlay' has no physical plane yet —
        it stays in the registry but containers only attach to `bridge`
```

The claim is false on both halves. `cmd/network.rs:572` says the opposite in its own comment
("Unlike macvlan/ipvlan, the overlay IS realizable without host privilege — it lives entirely in
the holder netns") and calls `realize_overlay`, which brings up a bridge + VXLAN uplink + WireGuard.
`AGENTS.md` records the live validation. The one at fault is `cmd/conditions.rs:186-192`, which puts
`overlay` in the same `match` arm as `macvlan`/`ipvlan`.

**Scope:** `conditions_for` feeds three consumers in `cmd/stack.rs` — the plan (line 404,
printed at 588), `stack wait` (667) and `print_missing_conditions` (828). **It doesn't block**:
`wait` warns once and never waits on a failed prerequisite, by explicit design. The damage is to
trust, not to availability — but it is exactly the "dishonest report" class that this repo
pursues, and it falls on the most advanced networking feature it has.

**Fix:** take `"overlay"` out of the arm, giving it its own condition — realized, with the measured
caveat that inter-node forwarding was never proven with a real 2nd node. A test that fails with the
fix reverted.

## Finding 2 — `kind: Vm` accepts 36 fields and converges 5 (HIGH)

This is the central obstacle to the "self-sustaining IaC" goal, and it isn't a feature gap: it's an
accept-and-silently-drop path.

```
VM_SPEC_FIELDS      → 36 distinct fields accepted by the manifest
RECONCILED_VM_FIELDS → ["disk", "vcpus", "memory", "network", "backend"]
```

Reproduced live against an existing VM (`lab`, Stopped, libvirt), with **six** declared properties
the VM doesn't have — `cpuTopology: 2×4×2`, `tpm: true`, `vnc: true`, `machine: q35`,
`bootOrder`, an `extraDisks` pointing at a nonexistent file and an `extraNics` pointing at a
nonexistent bridge:

```
$ delonix stack plan -f vmdrift.yaml
  +~  Vm/lab  — exists and belongs to no stack — will be taken over
Summary: 1 to adopt
```

Zero changes proposed. An `apply` would report success and the VM would keep its old shape.

**Control that makes this conclusive** — the absence of a warning could mean "field not recognized
by the plan." It doesn't:

```
$ delonix stack plan -f vmbogus.yaml      # same, with one made-up field
WARNING: Vm 'lab': unknown field 'campoQueNaoExiste' in spec — ignored (check the spelling)
```

`warn_unknown_fields` **does run** in the plan. So the six fields above are recognized, parsed,
and **dropped by the reconciler** — the worst of the three possible outcomes, because the user has
every reason to believe they were applied.

**Root cause, and it's one already catalogued in this repo.** The `Vm` registry persists 10 fields
(`disk`, `overlay`, `vcpus`, `memory`, `network`, `tap`, `mac`, `restart_policy`, `namespace`,
`backend`, `devices`); the `VmConfig` that creation consumes has ~30. The 21 missing ones —
`kernel`/`initrd`/`firmware`/`cmdline`/`seed`/`hugepages`/`cpu_affinity`/`bridge`/`volumes`/`vnc`/
`static_ip`/`machine`/`cpu_model`/`cpu_topology`/`tpm`/`video`/`boot_order`/`extra_disks`/
`extra_nics`/`libvirt_xml_overlay`/`libvirt_xml` — **exist only during `vm create` and die with
it**. The reconciler can't compare what the registry doesn't store, and `vm start`/`restart` already
documents this loss in its own `--help`.

It's the **fifth occurrence** of the trap: *state needed to REBUILD the resource must be
persisted, not just used at creation* (previously: unpersisted `-v`, `-p` on a custom network, extra
networks, `Container.pod`).

Note what the list of 21 contains: CPU topology, TPM, boot order, extra disks and NICs, static IP,
machine type. **This is exactly the vocabulary of a virtualization platform.** Delonix today
converges what a container needs; what a VM needs is left out.

## What this answers about the goal

**Without Terraform, today, it doesn't work** — and the reason isn't a missing `terraform apply`.
It's that the declarative VM object neither retains nor compares what a VM is. A `stack apply` that
promises convergence and ignores 25 of 30 fields isn't replaceable by Terraform because it's worse:
it reports success.

**The right order of work is, therefore, the opposite of intuition.** No new protocol (VLAN,
iSCSI, OVA) is worth anything before the Kind retains and compares what it already accepts —
otherwise more vocabulary gets added to the same list of silently dropped fields.

## Proposed order

1. **Finding 1** — cheap, isolated, pure. One `match` arm and a test.
2. **Finding 2, part A: persist.** Extend the `Vm` registry to the fields `VmConfig` already
   accepts (`#[serde(default)]`, old registries stay valid — the precedent is `namespace` from
   v0.40.0, with a dedicated regression test). Without this, the rest is impossible.
3. **Finding 2, part B: compare.** Extend `RECONCILED_VM_FIELDS`, with the test this repo already
   requires per Kind — an unchanged manifest must give ZERO diffs. Cold fields (that force a
   recreate) must enter the fail-closed `-/+` path, not converge in-place.
4. **Only then** decide which of the six missing fundamentals go into the manifest — with the
   criterion Phase 1 of the prompt sets: where each one's realization lives, and what is
   `delonix-deploy`'s substrate rather than this repo's work.
5. **SDK/Terraform last**, and the piece that unblocks it is `--format json` on listings: a
   provider that parses tables whose width changes with content and whose text changes with
   `--l18n=pt` is born broken.

## Execution status

**Finding 1 — CLOSED** (`6fd74e9`). `overlay` came out of the `macvlan`/`ipvlan` arm; the real
prerequisite was declared (an encrypted overlay needs `wg`), probed by the SAME `wg::available()`
the realizer uses. The old test walked all three drivers expecting `DriverNotImplemented` — it
fixated the defect — and was replaced by two that fail with the fix reverted (verified).

**Finding 2, part A — CLOSED** (`43b3a85`). The `Vm` registry now stores the boot shape
(`VmBootSpec`, in `core`, with the four helper types moved there and re-exported by
`delonix-vm`). Entire block `skip_serializing_if`: a VM with no advanced options doesn't grow a
byte, and an old registry has no such key. Absent means **unknown**, not "had none."

The guard that prevents recurrence: `boot_spec_of` exhaustively destructures `VmConfig` and
`config_from` builds it without `..Default::default()` — a new field breaks the build in both
places. Verified by adding a test field. It was precisely `..Default::default()` that let 21
fields silently turn into defaults on every restart.

Validated live: the three registries on this host written by the previous binary still read fine,
`vm ls`/`describe` intact, and none gained the key just by being read.

## Finding 2, part B — "compare the safe ones" today has an EMPTY set

The choice was to compare fields that don't force a recreate. When implementing it, the set turned
out empty, for a structural reason:

- `Action::Update` is defined as "converges without recreating **and without changing the PID**."
  This engine does no hotplug: none of these fields apply to a running VM.
- `Action::Replace` destroys and recreates — and recreating throws away the overlay, i.e.
  everything the guest wrote. That's why it's refused without `--replace`.
- `create_with` returns early when the VM is already running (`return Ok(ex.clone())`, idempotent),
  so an `apply` over a live VM **does nothing** even now that the shape is persisted.

Extending `RECONCILED_VM_FIELDS` under this model would turn every new field into a destructive
`Replace` — exactly what the choice meant to avoid. In other words: comparing more fields today
means comparing toward data loss.

**What part A unlocked, and didn't exist before**: a third class between the two. Changing
`tpm`/`machine`/`cpuTopology`/`vnc`/`bootOrder`/`extraDisks`/`extraNics`/`video`/`cpuModel` is
applicable via **stop + start** — the PID changes, but the overlay survives, and now that the
shape is in the registry the next boot actually uses it. It isn't `Update` (the PID changes) nor
`Replace` (no data loss): it's a restart.

Adding that class touches `enum Action`, which is part of the `-o json` payload — a contract
ADR-0005 exists to keep stable. **That's why part B stops here and moves to an ADR**, following
the discipline this repo applies to structural boundaries. The questions to close in that ADR: can
an `apply` restart a VM unasked (is it a service interruption, not data loss)? If not, what's the
gate — a `--reboot` alongside `--replace`? And what does `--detailed-exitcode` do with a change the
user hasn't yet authorized.

## The three missing measurements — done (2026-08-12, with authorization)

**E2E battery: 264 PASS, 0 FAIL, 0 SKIP.** `scripts/e2e.sh` is safe on a production host by
construction — a unique per-PID prefix on everything it creates, its own manifest, and `stack
destroy` only takes what the stack owns. A baseline was taken before and compared after: containers,
networks, VMs and holder PIDs **identical**.

**VXLAN proven live.** The "realized" classification stopped resting on reading code alone. A
disposable overlay network (`--vni 4242`, two peers in TEST-NET-3) produced, inside the holder's
netns:

```
dlxvx1092: ... master dlxn98b86f58
    vxlan id 4242 ... dstport 4789 nolearning
bridge fdb: 00:00:00:00:00:00 dst 203.0.113.7 self permanent
            00:00:00:00:00:00 dst 203.0.113.9 self permanent
bridge:     dlxn98b86f58  UP  10.211.0.1/16
```

A real device, mastered onto the network's bridge, FDB seeded with both peers, and the bridge with
an address — that's what a container attaches to. **One naming correction along the way**: the
device is `dlxvx{vni:04x}`, hexadecimal — decimal 4242 is `dlxvx1092`. My first probe looked for the
decimal name, found nothing, and for a moment it seemed the overlay wasn't coming up.

**Bug found by doing this proof** (`2253b9e`): `network rm` removed the registry and the bridge and
left the **VXLAN uplink orphaned** — UP, with the peers' FDB, no master. A leak on every
create/rm cycle, that only died with the holder. Fixed and proven in both directions.

**Real `stack apply`.** I didn't use the user's `lab` VM: an apply on a `kind: Vm` starts a
stopped VM, and starting someone else's machine isn't the same as stamping it. I created my own VM
via a real apply, and the second apply on it — already existing — printed:

```
vm/fase0-apply-probe: ensured
Conditions (attention — missing prerequisites):
  Vm 'fase0-apply-probe': Converged=False (FieldsNotCompared) — declared but NOT
  applied to an existing VM: cpuTopology, machine, tpm, vnc — ...
```

Before these fixes the same command said `ensured` and nothing else. Everything removed afterward
(`stack destroy`), final state identical to the baseline, including the on-disk VM registries.
