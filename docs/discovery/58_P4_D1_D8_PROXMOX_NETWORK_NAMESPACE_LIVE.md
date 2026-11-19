# 58 — P4/D1,D8: `network`/`namespace` on Proxmox, confirmed live (ADR-0044)

**Date:** 2026-09-18 · **Baseline:** `origin/main` (`afbe4bfb`, v4.0.0 +8 commits) · **Target:**
`pve92`, a libvirt VM already existing on this host (Proxmox VE 9.2.2-6-pve, created in
an earlier ADR-0008 session), 192.168.122.220 · **Verdict: D1's finding was
CORRECT for `network`, and WRONG for `namespace` — ADR-0044 itself measured its
Context against an incomplete grep.**

## The question

Item 5 of the spikes ADR-0044 requires asks exactly this: run
`delonix vm create --backend proxmox --network <sdn-net> --namespace teamA` against the
real `proxmox-ve:9.2` and observe what actually happens — the ADR's Context had
already flagged its own static reading as "should be confirmed live before anyone
treats it as more than a strong signal".

## How it was measured

- `pve92` started (it was `Stopped`), API confirmed with `root@pam`/`delonix-admin`
  (the known credentials for this repo's Proxmox appliances — `answer-pve.toml`).
  The node is called `pve`, a single `vmbr0` bridge connected to `eth0` (which in turn
  sits on the HOST's `virbr0`/192.168.122.0/24, the same libvirt NAT — there is no
  bridging at all between this and delonix's rootless SDN, which lives inside this
  host's holder netns).
- Delonix's own network for the spike (`p4spike-net`, `10.253.0.0/16`) — never a
  production network already in use.
- `delonix vm create p4d5-proxmox-test --backend proxmox --disk local-lvm:2 --network
  p4spike-net --namespace teamA`.

## What was measured

1. **`--namespace teamA` was REFUSED, before any API call**:
   ```
   error invalid argument: namespace 'teamA' is not enforceable on the 'proxmox' backend:
   its VMs live on the host's libvirt bridge, outside the Delonix SDN, so nothing here can
   isolate them. Use `--backend cloud-hypervisor` (its VMs share the containers' SDN), or
   drop `--namespace`
   ```
   `git blame` (`crates/adapters/delonix-vm/src/lib.rs`, `vm_namespace_supported`) places
   this guard at **`c1ed34ec8`, 2026-08-05** — more than a month before ADR-0044 was
   written. `vm_namespace_supported` returns `true` only for `"cloud-hypervisor"`,
   so the refusal applies to libvirt AND to Proxmox, generically, from
   `delonix-vm::create_with` — it isn't `delonix-proxmox` code, which is why the ADR
   Context's `grep` (restricted to `crates/providers/delonix-proxmox/src/lib.rs`) never
   reached it. **The "neither read nor refused" finding was right about where it
   looked and wrong in its practical conclusion** — the gap the ADR worried about was
   already closed, just one level above where the search looked.
2. **With no `--namespace`, `--network p4spike-net` was ACCEPTED and IGNORED** — confirmed not
   by reading code, but by the actual HTTP form the client sends (instrumented with a
   temporary `eprintln!` in `post_form`, reverted after this measurement):
   ```
   post_form /nodes/pve/qemu form=[("vmid","101"),("name","p4d5-proxmox-test3"),
   ("memory","1024"),("cores","1"),("ostype","l26"),("scsihw","virtio-scsi-single"),
   ("scsi0","local-lvm:2"),("net0","virtio,bridge=vmbr0"),("agent","1"),
   ("ipconfig0","ip=dhcp"),("ide2","local-lvm:cloudinit")]
   ```
   `net0` is `bridge=vmbr0` — the node's DEFAULT, never `p4spike-net` nor the SDN's
   bridge (`dlxn…`). `delonix vm describe p4d5-proxmox-test3` shows `Network: p4spike-net`
   in the LOCAL registry — the registry **lies about what the VM actually is**, because
   `net0_arg` (`crates/providers/delonix-proxmox/src/lib.rs`) only reads `cfg.bridge`, never
   `cfg.network`. Confirmed: `grep -n "cfg\.network\b" crates/providers/delonix-proxmox`
   still returns zero hits on this tree, exactly as the ADR's Context had already read.
3. **Side finding, unrelated to D1/D8**: the first two attempts (before
   this one) failed with `400 {"errors":{"ipconfig0":"type check ('string') failed - got
   ARRAY"}}` — the same text as a bug ALREADY fixed in the code (the comment in
   `create_form` documents it and there's a test, `the_create_sends_each_key_once`). Reproduced
   twice, and then, after the `pve92` VM had more time to settle (`pmxcfs`/
   `pveproxy` fully ready), the SAME call, with the SAME binary, with no code
   change whatsoever, succeeded — and the instrumented form confirms
   `ipconfig0` appears **exactly once**. It isn't the already-documented bug
   recurring; it's a startup race of the nested Proxmox node itself, unrelated to this
   ADR. Logged here so whoever repeats this spike doesn't waste time chasing it: waiting
   for the API to respond isn't the same as waiting for the node to be ready to validate
   SCHEMAS — the two mature at different moments.

## Interpretation, for D1 and D8

- **D1's field-classification table stays correct as it is**: `network`/`namespace` are
  universal by intent (every provider attaches to SOME network and SOME
  isolation domain), and today Proxmox honours neither — one silently, the other
  with an explicit refusal that already exists, just in a different place than the
  Context had mapped.
- **The D8 "network"/"namespace" gap in practice has TWO halves with different
  weight**: the `namespace` one is already closed (fail-closed, with a message that names
  the alternative) — D8 just needs to **move that same refusal inside
  `refuse_unsupported`** once `VmSpec`/`Extensions` exist, not invent it. The
  `network` one stays open and is the one requiring a real decision: silently
  accepting "network: p4spike-net" on a VM that ends up on `vmbr0` is the "accepted and
  ignored" pattern this repo has already fixed three times for other flags — D8 must
  either (a) make `delonix-provider-proxmox` explicitly refuse `network` when it isn't
  `default` (the cheapest route, and the one the topology measured here recommends: there
  is no bridging between delonix's SDN and a remote Proxmox's network, and forging one is
  not this ADR's job), or (b) build a new bridging mechanism (the remote equivalent of
  the already-existing EXPERIMENTAL `vm bridge` for CH on libvirt) — which is a separate
  ADR, not a D8 line item.
- **The topology by itself already answers the reachability question item 5 of the
  spike asked**, with no need to boot a full guest OS: `pve92`'s `vmbr0` is
  bridged to ITS OWN `eth0`, which sits on the HOST's `virbr0`/192.168.122.0/24 — the
  SAME network `pve92` itself lives on, and structurally disconnected from the
  rootless SDN (`10.2xx.0.0/16`, inside the holder netns). A VM created this way would
  NEVER be "isolated nor open" relative to a delonix container — it would simply be on
  another network, with no path at all between the two, with or without `--namespace`.
  Booting a real guest would only confirm that same absence of a path, already decided
  by the wiring.

## Proven vs not validated

**Proven**: the two behaviours measured above (`namespace` refusal, `network`
accepted-and-ignored), with the real HTTP form; the origin and date of the `namespace`
guard via `git blame`; `pve92`'s network topology via the Proxmox API itself
(`/nodes/pve/network`); that the `ipconfig0`/ARRAY error is transient and not a code
defect (reproduced failing, then passing, same binary, same command).

**Not validated**: actual reachability of a real guest on this network (not
necessary — the topology already answers it); the behaviour of a `network`/`namespace`
explicitly REJECTED (D8 doesn't implement it yet — today only `namespace` refuses,
`network` doesn't).

## Cleanup

`p4d5-proxmox-test*` removed (`delonix delete vm`), `p4spike-net` removed
(`delonix network rm`), `pve92` returned to `Stopped` — nothing was left standing beyond
what was already on this host before this spike. The debug `eprintln!` in `post_form`
(`crates/providers/delonix-proxmox/src/lib.rs`) was reverted in the same commit as this
report — it isn't part of the final code.
