# 63 — The plan for the Proxmox LXC provider (ADR-0058)

**Date:** 2026-09-27 · **Base:** `origin/main` (`d3d6f394`, v4.4.0 + commits) · **Lab:**
cluster `lab` (`pve` 192.168.122.91 + `pve2` 192.168.122.55, PVE 9.2.2).

**What this is:** the plan to apply every fix and improvement the ADR-0058 spike measured.
Each slice has an exit gate and a live proof. **It is not authorization to build**:
decisions D1 through D4 below come first, and only Slice 0 proceeds without them, because it fixes
a defect that already exists on the QEMU path.

## The question

ADR-0058 decides that a Proxmox LXC container does **not** serve `kind: Container` (no `exec`,
no logs, no exit code, and outside the engine's dataplane), and that, if it enters, it enters as its own
**system container** resource, with semantics close to a VM's. This plan's question is in what
order that resource is built so that each measured trap closes in code and is held by a test before the
next step.

## The measured traps, and where each one closes

| # | Trap (ADR-0058) | Closes in | Gate |
|---|---|---|---|
| T1 | No `exec`, no logs, no exit code in the API | Slice 3 | `unsupported-by-provider` capabilities with the reason; the verbs refuse by name |
| T2 | `entrypoint` and `env` silently overwritten on create from OCI | Slice 3 | create without them, `PUT …/config`, reread, compare; divergence is an error |
| T3 | A start with failed DHCP ends in `WARNINGS: 1`, and the engine reads it as failure | **Slice 0** (#546) and 0b | third task verdict, with the `WARN:` lines read from the log; in SDN, each node's child tasks |
| T4 | The node's pull only accepts tags, loses the digest, does not accept credentials, and rewrites the name | Slices 1 and 2 | the engine pulls and verifies the image and sends it via `upload`; `oci-registry-pull` is not used |
| T5 | The node refuses an archive with Docker v2 media types ("Unsupported CPU architecture") | Slice 1 | the uploaded archive carries OCI media types; test with a real Docker v2 manifest |
| T6 | Each container is a full copy of the image | Slice 3 | stated in the capability detail; nothing to fix, only to not hide |
| T7 | The engine's dataplane (SDN, isolation, DNS, `publish`) does not apply | Slice 3 | resource `net.*` capabilities declared for what they are: the node's bridge and VLAN |

## Owner decisions, before any code for Slices 1 through 6

**Decided on 2026-09-28** by the owner: D1 accepted (ADR-0058 becomes Accepted); D2 — the
need is **parity with Proxmox**: covering in the engine the LXC the operator already uses on the node,
so it can be managed through the same declarative API; D3 — `kind: SystemContainer` in
`compute.delonix.io/v1alpha1`, with no new CLI group in v1; D4 fulfilled (#541 merged).

- **D1 — Accept ADR-0058.** Its first two decisions (it is not `Container`; it is its own
  resource) are what this plan executes.
- **D2 — Name the need.** ADR-0049 D5 builds slices against a need, not
  against a route count. Without a written need (who uses it, for what), the plan stops
  after Slice 0.
- **D3 — The name.** Recommendation: `kind: SystemContainer` in the `compute.delonix.io/v1alpha1`
  group, and **no new CLI group in v1**: the generic verbs `get`/`describe`/`delete` and
  `apply -f` already serve any Kind (B1 of the CLI restructuring). A dedicated group only enters if
  day 2 asks for it. The name cannot be `LxcContainer`: it names the provider, and the engine's
  boundary forbids that.
- **D4 — Merge #541 first.** The `upload_import` from ADR-0057 already implements the `upload`
  route with the sha256 in the body. Slice 2 generalizes it; it does not write a second one.

## Slice 0 — The task has three outcomes (already applies, QEMU included)

**Why first:** it is a defect in a path that already exists, not in LXC. `task_verdict`
(`crates/providers/delonix-proxmox/src/lib.rs`) reads anything that is not `"OK"` as a failure. A
`vzstart` with failed DHCP ends with `exitstatus: "WARNINGS: 1"` and the container running. For a
VM, that same third state would be a `TaskFailed` over an effect that happened, and the next
operation (a resend) would duplicate the action.

- `task_verdict` now returns `Ok`, `Warnings(n)` or `Failed(reason)`. `WARNINGS: <n>` with
  a numeric `n` is the only format accepted as a warning; any other text is still a failure.
- With `Warnings`, the client reads `GET /nodes/{node}/tasks/{upid}/log` and keeps the `WARN:` lines.
  The route was already being called (`task_error_line`, `task_log`), so the inventory does not change.
- The task ledger gains the `ok_with_warnings { warnings }` state. The call returns success and
  the warnings bubble up to the caller (`tracing::warn!` in the crate, printed by the CLI). A warning is never
  silently dropped, and is never counted as a failure.
- **Gate:** unit test of `task_verdict` with the three formats; a new scenario in
  `tests/failure_injection.rs` (`WARNINGS: 1` task + log with a `WARN:` line), reading the ledger
  and not just the `Ok`; and the same reading against the lab's task history, to say whether
  any real QEMU task already ended in `WARNINGS` (measured, not assumed).
- **Does not need D1 through D4.**
- **Done in #546** (2026-09-27): the injection scenario fails with the fix reverted. The
  lab history measurement found 51 tasks in `WARNINGS`, and what it showed is in Slice 0b.

## Slice 0b — An SDN apply waits for each node's reload

**Why:** of the 51 tasks in `WARNINGS` in the lab history, 50 are `srvreload
networking`, the per-node network reload a `PUT /cluster/sdn` triggers. The warnings include
`missing 'source /etc/network/interfaces.d/sdn' directive` (24) and `reloading frr configuration
failed` (5). The task the engine waits for, `reloadnetworkall`, ended in `OK` all 61 times: PVE
launches each node's reload in the background and does not track it (`PVE/API2/Network/SDN.pm`, with
a `FIXME` from upstream itself). An apply's `OK` means "requests sent", and that is how the
applies in #493 and #497 were accepted and never carried out.

- Before `PUT /cluster/sdn`, `apply_sdn` records the `srvreload networking` tasks that already
  exist per online node; after the parent, each node's child is the first one that was not in the
  list, and it is awaited with Slice 0's three verdicts. The parent's timestamp does not work for
  finding it: in the lab history the children arrive up to 37 s after it and the previous apply's up
  to 5 s before. A node without that task within a deadline is an error with the node's
  name, never a success.
- The children's warnings bubble up to the caller with the node's name. A failed child makes the apply
  fail, even with the parent in `OK`.
- **Gate:** injection scenario with the parent in `OK` and a child in `WARNINGS`, and another with a
  failed child; live, an apply on the lab with the `source` directive removed from a node
  must return that node's warning.
- **Waits for #542**, which is touching the SDN apply. Does not need D1 through D4.
- **Done** (2026-09-27): the three injection scenarios pass and the failed-child one fails with
  the wait reverted; live, with the directive removed from `pve2`, the apply returned the warning with
  `node=pve2`, and none without it. A child really failing on the node was only via injection.

## Slice 1 — The archive the node accepts

- `delonix-oci` gains writing an archive with **OCI media types**. Converting from Docker v2
  to OCI only changes the `mediaType` fields (manifest, config and `tar+gzip` layers); the config and
  layer bytes stay the same, and the manifest's digest changes. The archive records both
  digests: the registry one (the image's identity) and the converted manifest's one (the identity
  of the uploaded file).
- Today's `delonix image save` stays as is: it is read by `docker load` and `ctr import`, and
  there is no measured reason to change its output. The OCI write is a separate function, with one caller
  (Slice 2), until someone asks for an `image save --oci`.
- A layer without a simple OCI equivalent (zstd, foreign) is **refused by name**
  until it is measured what the node does with it.
- **Gate:** test with a real captured Docker v2 manifest (`alpine:3.20`'s) requiring the
  OCI media types and byte-identical blobs; and, live, the archive the engine produces being
  accepted by the node (`Detected OCI archive`), with the Docker v2 control being refused in the same
  run.

- **Closed on 2026-09-28.** `delonix_oci::write_oci_media_archive` (with `build_oci_manifest`
  and the pure `to_oci_manifest` conversion) returns both digests (`OciArchive`): the image id,
  which conversion does not change, and the converted manifest's. The store does not keep the registry
  manifest's digest; it is Slice 2, which pulls the image, that knows it. A zstd or
  foreign layer is refused with DX-1409 naming the media type and the layer. `image save` kept
  Docker v2 (a test requires it). Live proof on `pve` (PVE 9.2.2), in the same run and through the API
  (`pvesh create /nodes/pve/lxc`): the engine's archive for `alpine:3.20` was accepted
  ("Detected container architecture: amd64", `vzcreate` `OK`); the `image save` Docker v2
  control was refused with "Unsupported CPU architecture".

## Slice 2 — The generic upload, and an image sent once

- #541's `upload_import` becomes `upload(storage, content, …)`, with `content` being `import`
  or `vztmpl`. The lost response is resolved as there: listing the storage's content.
- The filename is derived from the converted manifest's digest (`dlx-<digest>.tar`, only with the
  characters the node does not rewrite — measured: `upload` kept the name exactly as sent). An
  image already present in storage is not sent again, as in ADR-0057.
- The engine pulls the image with its own client: digest verified, and credentials from
  `delonix image login`. This closes T4 entirely: pinning by digest and private registries start to
  work, because the node no longer pulls.
- **Gate:** live case of uploading with the correct checksum (`imgcopy` task `OK`) and **with a
  wrong checksum**. In the spike this request never got to create a task (`http=000`, cause not
  isolated); the slice only closes when that path is measured and the refusal is visible.

- **Closed on 2026-09-28.** `upload_import` became `Client::upload(storage, UploadContent, …)`
  (`import` or `vztmpl`), and listing became `list_content(storage, UploadContent)`.
  `Client::stage_template(storage, archive, manifest_digest)` refuses before sending a byte:
  - a digest that is not sha256;
  - a storage without `vztmpl` (DX-6513);
  - a storage without space (DX-6514).

  It names the file `dlx-<hex>.tar` by the converted manifest's digest and does not resend an archive
  the node already has. A lost response is resolved by listing the storage.
- **The gate, measured on 2026-09-28 on `pve`** (PVE 9.2.2): the spike's `http=000` does not recur.
  With the wrong checksum the node answers 200 with a UPID, and the `imgcopy` task ends in error
  ("checksum mismatch: got '…' != expect '…'"), without saving any file. With the right one it ends
  in `OK` and saves the file with the name sent. The live case
  `a_container_archive_is_staged_as_vztmpl_and_a_wrong_checksum_is_refused` runs both
  things; it ran twice in a row (upload, then cache).
- **What is left for Slice 3**: wiring the engine's pull (`pull_from_registry_with_creds`, which already
  verifies a pin's digest and uses `image login`'s credentials) to
  `write_oci_media_archive` and to `stage_template`. The composition lives in the provider, because
  `delonix-proxmox` does not depend on `delonix-oci`. Only with that wiring does T4 close end to
  end.

## Slice 3 — The port and the provider

- **The port** lives in `delonix-compute`, in a new file, in the ADR-0044 shape (spec, handle,
  observation, `capabilities()`). It does not touch `vm_backend.rs`, because P4b.3 is touching that
  spot.
- **The provider** lives in a new module, `delonix-proxmox/src/lxc.rs`, like `vm_firewall.rs` and
  `sdn.rs`. `lib.rs` has 7,755 lines and is the conflict point of every Proxmox PR.
- **Create:** `ostemplate` from the upload, `unprivileged: 1` always. An `unprivileged: 0`
  request is refused by name: privilege boundaries on the remote node are a separate decision, with
  its own spike (guardrail 5).
- **Configure (T2):** create without `entrypoint`/`env`, apply them with `PUT …/config`, reread
  the config and compare field by field. A divergence is an error, and the created container is destroyed,
  so a running resource with the wrong configuration is never left behind.
- **Start (T3):** the task goes through Slice 0's verdict. With networking requested, the result is
  judged by `GET …/interfaces`: without IPv4 after startup, the resource gets the
  `NetworkReady=False` condition with the reason read from the log, rather than "running, all good".
- **Stop and destroy:** `shutdown` with a deadline and then `stop`; `DELETE` with `purge` and
  `destroy-unreferenced-disks`. All through the existing task ledger.
- **Capabilities:** new `system-container.*` rows in the catalog, which bumps to `1.1.0` (new
  entries bump the minor). `exec`, logs and exit code become `unsupported-by-provider` with the
  measured reason (T1). The full copy goes in the detail (T6). Networking declares the node's bridge
  and VLAN (T7). The `linux`, `libvirt` and `cloud-hypervisor` providers answer the new rows, because
  the catalog's wildcard-free `match` forces it.
- **Gate:** a case in `tests/live.rs` that walks create → configure → start → observe →
  stop → destroy with tracing on, and reads the ledger at the end; injection scenarios for T2
  (config reread differently) and T3 (start with warnings); the ADR-0050 evidence gate green.

- **Closed on 2026-09-28.**
  - **Port.** `delonix_compute::system_container`: `SystemContainerSpec` (local OCI archive +
    manifest digest, entrypoint, env, resources, networking, `unprivileged`),
    `SystemContainerHandle { locator }`, `SystemContainerObservation { running, network }` with
    `NetworkState::{NotRequested, Ready, NotReady{reason}, Unknown}`. `start` and `observe`
    receive the spec, because it is what says whether a DHCP network was requested.
  - **Provider.** `delonix-proxmox/src/lxc.rs`: the client's `lxc_*` methods, routed through the
    task ledger (`vzcreate`, `vzstart`, `vzshutdown`, `vzstop`, `vzdestroy`), and the
    `ProxmoxSystemContainerProvider`. `task_inner` now returns the `WARN:` lines
    (`task_collecting_warnings`), for startup to judge the network with them.
  - **Refusals by name (DX-1540):** `unprivileged: false`, an entrypoint argument with
    spaces, a variable name outside `[A-Za-z0-9_]`, an invalid hostname. A config reread
    differently from what was requested destroys the container and ends in an error naming the field.
  - **Catalog 1.2.0.** The `system-containers` domain and eight `system-container.*` entries.
    All providers answer.
- **Measured on `pve` before writing**, via the API:
  - `env` is a NUL-separated list and `entrypoint` a single line, and `PUT …/config` answers
    `null`;
  - `vzstart` without a DHCP server on `vmbr0` ends in `WARNINGS: 1` after about 2 min,
    with the container running and only IPv6 link-local on `eth0`;
  - a shutdown with a deadline against an init that does `sleep` ends in "container did not stop";
  - delete with `purge` and `destroy-unreferenced-disks` takes the volume with it.
- **Gate.**
  - Live case `a_system_container_runs_its_lifecycle_through_the_node`: create, configure,
    start (`NotReady` with the node's warning), observe, stop and destroy. Passed in 134 s, with the
    tracing on and the ledger read at the end.
  - Injection T2 (config reread differently → destroyed) and T3 (start with warnings → `NotReady`);
    each one fails with the fix reverted.
  - ADR-0050 evidence gate green. Regenerated route matrix: the 9 lifecycle `…/lxc` routes become
    `supported+tested` (170 of 675).
- **What is left for Slice 4**: pulling the image (`pull_from_registry_with_creds`, with the digest
  verified) and writing it with `write_oci_media_archive` before calling the port. The `-bin` crate is the
  only one that knows both sides.

## Slice 4 — The Kind

Follows the 12-step list in `docs/dev/adding-a-kind.md`: the `KindFacts` line, the spec with
`JsonSchema`, `desired`/`actual`/`converge`, `hot_fields`, `presence`, the generic verbs, the
`drift`, the regenerated schema and `pt.po`.

- **Hot fields** (converge without recreating): `memory`, `swap`, `cores`. Before declaring them
  hot, it was measured whether the node applies them to a running container's cgroup; whatever does not apply
  stays cold.
- **Cold fields:** `image`, `entrypoint`, `env`, `rootfs`. Changing them plans a `Replace`, refused
  without `--replace`, as with the other Kinds. A "restarts to apply" class is ADR-0012
  (proposed) and does not enter here.
- **Gate:** `stack apply` followed by `stack plan --detailed-exitcode` with code 0; a
  `PUT …/config` done by hand on the node gives code 2 in `plan` and shows up in `delonix drift`;
  `delete` leaves the node with no container and no volume.

- **Closed on 2026-09-28.** `kind: SystemContainer` (`compute.delonix.io/v1alpha1`, `sc`,
  group `systemContainers`), in `bins/delonix-runtime-bin/src/cmd/system_container.rs`. The module
  is the composition root: it reads the same Proxmox configuration VMs use and builds the
  `ProxmoxSystemContainerProvider`.
  - **The pull is the engine's:** `resolve_or_pull`, with the digest verified and the
    `image login` credentials, then `write_oci_media_archive` for a per-digest cache.
  - **The record keeps the locator and what was declared.** `actual()` reads from the node the memory, the
    swap, the cores, the entrypoint and the env, through new port methods (`configuration`,
    `resize`).
  - **Entrypoint and env only enter the comparison when they are declared.** If they are not, the node
    has the image's, and comparing them to "nothing" would give drift in every plan.
- **Hot fields, measured before declaring them**, on a container running on `pve`: `memory` and
  `swap` reach the cgroup immediately, `cores` reaches the `cpuset` after a few seconds, and nothing is left
  pending. `image`, `entrypoint`, `env`, `rootfs` and `network` are cold.
- **Gate, via the CLI, against `pve`, with an isolated root:**
  - `stack apply` creates and starts it (18 s); `stack plan --detailed-exitcode` gives 0.
  - A `PUT …/config memory=384` done by hand makes `plan` give 2 (`memory: 384 → 256`), and
    `delonix drift` shows it with code 2.
  - `apply` converges hot and `plan` goes back to 0.
  - The manifest's `memory` at 512M converges hot, with the container running and the cgroup at
    536870912.
  - A different `rootfs` plans a `Replace`, which is refused without `--replace`, without touching anything.
    With `--replace` the container is destroyed and recreated with 2G.
  - `delete systemcontainers` leaves the node with no container and no volume, and the record empty.
  - The refusal without a configured provider exits with code 69, translated.

## Slice 5 — Day 2, only on request

Each item enters when requested, with its own live case: snapshots and rollback (measured in the
spike on `local-lvm`), per-container firewall (the `…/lxc/{vmid}/firewall/*` routes, through the
ADR-0052 path), backup with `vzdump`, clone and template, resizing the disk, and migration
between nodes (LXC migrates with a restart, not live; to measure before promising it).

- **Owner decision (2026-09-28), revises D3.** All six operations enter, one PR per operation.
  What fits in a manifest goes into the Kind (growing `rootfs`, the firewall via `kind:
  NetworkPolicy` with `scope: systemcontainer`); the one-off one goes into a new group, `delonix
  systemcontainer` (snapshot, clone, migrate), and into `backup create systemcontainer`.
- **Snapshots and rollback: done.** `delonix systemcontainer snapshot create|ls|rm|restore`, the
  same four verbs as `vm snapshot`; port `snapshot`/`snapshots`/`delete_snapshot`/`restore`; the
  four `…/lxc/{vmid}/snapshot*` routes become `supported+tested` (13 LXC routes); catalog 1.3.0
  (`system-container.snapshot`). Measured on `pve`: a container snapshot is volume-only (the
  `vmstate` field is commented out in the node's schema); rollback stops a running container and leaves it stopped
  without `start=1`, so `restore` sends `start=1` when it was running. **And it caught a design
  defect**: after a rollback with `start=1` the task ends before the node starts answering
  for the container again — `status/current` hung ~40 s (curl) and pveproxy answered the client with HTTP 596.
  `restore` now waits for `running` within the tasks' time budget, and a failed read counts as "not yet".
  Via the CLI: a `restore` that undoes a change made by `stack
  apply` shows up in the next `plan` as drift (2), which is the honest outcome.

- **Growing `rootfs`: done.** The Kind's `rootfs` field converges hot when it grows and
  plans a `Replace` when it shrinks: a new reconciler rule, the "grow only" fields
  (`grow_only_fields`/`is_hot_change`), which `apply` also uses, so plan and apply never
  disagree. `actual` reads the size on the NODE (`size=` from `rootfs`), so a disk grown by
  hand is drift. Port `grow_rootfs`, route `PUT …/lxc/{vmid}/resize` (worker `resize`, confirmed
  by the UPID) becomes `supported+tested` (14 LXC routes); capability `system-container.resize`. Measured on
  `pve`: a RUNNING container grows from 1 to 2 GiB and keeps running; shrinking is refused
  before any request (DX-1540). Via the CLI: growing via manifest converges (plan 0 afterward);
  a manual `pct resize` to 3G shows up in `drift` and in the plan; requesting less than the node has plans
  `-/+` and `apply` refuses without `--replace`, without touching anything.

- **Backup: done.** `backup create|schedule systemcontainer <name>` archives to the node's storage
  (`--to <storage>`, `.` is the default: `DELONIX_PROXMOX_BACKUP_STORAGE`, otherwise `local`), and
  `backup ls --kind systemcontainer`, `inspect`, `restore` and `remove` recognise a node archive
  by name (`<storage>:backup/vzdump-lxc-<vmid>-…`) and go to the node instead of reading a local file.
  Port `backup`/`backups`/`delete_backup`/`restore_backup`, with `vzdump` (mode `snapshot`, or
  `stop` with `--stop`) and `POST …/lxc` with `restore=1 force=1` (worker `vzrestore`, read from the
  `PVE/API2/LXC.pm`). An archive of ANOTHER container is refused before any request; restoring
  a running container requires `--force`, stops it, restores and starts it again.
  Capability `system-container.backup`, `partial` for the reason measured next. **Measured on `pve`,
  and it is the finding**: a restore done by an API token ends in `WARNINGS` and **drops the raw
  `lxc.*` keys** the node wrote from the image (`lxc.init.cwd`, `lxc.signal.halt`) — only
  `root@pam` may write them (`restricted` in `PVE/LXC/Create.pm`). Without `lxc.signal.halt`, a
  `shutdown` sends SIGPWR instead of SIGTERM. The provider reads them before and after and `restore`
  names each one that was lost, instead of leaving it in a task log. **Along the way**: the timer for
  `backup schedule` was running `backup <kind> <name>`, the form before the split into verbs, which
  the CLI refuses — every scheduled backup was failing; it now becomes `backup create`, and there is a test
  that passes the argv through the real parser.

- **Clone: done.** `delonix systemcontainer clone <name> <new> [--from-snapshot <s>]` makes a
  full copy on the node (`POST …/lxc/{vmid}/clone`, worker `vzclone`), records it with the image and the
  declared fields of the source, without a stack (a manifest naming it adopts it), and starts it if
  the source was running. Read in `PVE/API2/LXC.pm`: a full clone of a RUNNING container is only
  accepted from a snapshot, so the provider takes a temporary one and always removes it (measured:
  none is left on the source); the node gives the copy a new MAC. Capability `system-container.clone`.
  **The template is left out, on purpose**: a template does not start, and a container managed by a
  manifest that can no longer run is a resource the Kind cannot converge. It enters if a named
  need shows up (linked clones from a base image on the node).
  **Two defects only the re-run caught**: (1) `destroy` treated ANY `GET …/config` error as "already
  gone" — measured: a request that failed before reaching the node made `destroy` answer success
  without destroying anything, and the container stayed on the node. Only the node's "does not exist"
  counts now as absence (`lxc_exists`), and a transport error propagates; (2)
  `list_images` reads `content=images`, and a container's volume is `rootdir` — the "no volume left"
  assertions in the five live LXC cases were empty. They now read `rootdir`
  (`list_ct_volumes`), and the five passed this way (572 s).

- **Firewall: done.** `kind: NetworkPolicy` with `scope: systemcontainer` puts the policy in the
  container's firewall on the node, following the same rules as `scope: vm` (ADR-0052): ownership by the
  `delonix-managed:<n>` comment, the default policy written last, and the DATACENTER `enable`
  refused when off (DX-6508), never turned on by the engine. The provider turns on the container's `enable`
  and `firewall=1` on `net0`; a container without `net0` is refused (DX-1540) before
  writing any rules. The code is a SINGLE one for both scopes (`GuestFirewall`, with one QEMU
  implementation and one LXC), so the VM and the container never disagree on order or
  ownership. The target is checked against the SystemContainers declared and recorded in `stack validate`/`apply`.
  Capability `system-container.firewall`, `partial`: no packet crossed the container in a
  test. **Measured via the CLI on `pve`**: apply → plan with no changes; a rule deleted by hand on the node
  came back as `~` with the rule missing, the apply converged and the plan went back clean; delete left no volume.
  Firewall writes answer inline (no UPID), so they leave no record in the task
  ledger.

- **Migration: done.** `delonix systemcontainer move <name> --node <n> [--restart]
  [--with-local-disks] [--target-storage <s>]` moves the container to another cluster node and only
  after the node proves it (`/cluster/resources` lists it on the target, the config is read there) changes
  the record's locator; the next `stack plan` stays clean. Capability
  `system-container.move`, code DX-5517 for refusals. **Measured on the lab (`pve` → `pve2`) before
  writing**, and it is what drove the design:
  - a STOPPED container migrates offline and the node copies a whole volume the target does not share
    (4 GiB of a thin volume via `dd`, 1m16s) — hence the refusal without `--with-local-disks`, as in the
    `vm move`; the LXC `GET …/migrate`, unlike the VM one, says nothing about local volumes,
    so the provider reads them from `rootfs`/`mpN` and compares against the `/storage`'s `shared`;
  - a RUNNING container is refused by the node without `online`/`restart`; with `online=1` the API accepts and
    the task fails ("lxc live migration is currently not implemented");
  - the node's own `restart=1` shuts down with `--nokill` and **aborts at the deadline when the init
    ignores SIGTERM** (a `sleep`), leaving the container running at the origin — and `pvesh` exits with 0.
    So the provider does not use it: it stops the container itself (shutdown, then stop), migrates
    offline and starts it at the target. A migration that fails with the container still at the origin starts it
    there again.
  Live case `a_system_container_moves_to_another_node_offline_and_by_restart` (212 s): the four
  refusals leave the container where and as it was, the restart route and the offline return leave no
  volume on either node.
  **A finding to validate via the CLI**: a session that died after the node had moved the container and
  before the record changed left the locator pointing at the old node. With nothing else, the
  `configuration` read "does not exist" and the Kind planned a `Create` — a second container. The
  provider now resolves each operation by `located`: if the container is not on the recorded
  node, it asks `/cluster/resources` where it is and follows it (the ADR-0053 D3 rule for
  VMs). Proved with the real leftover (plan `=`, move refused as "already on node", moves again) and
  in the live case, with the old handle reading the container running at the target. The record itself is not
  fixed by the read: only the next `move` updates it.
  **Shared storage, measured later via the CLI** (the lab's `nfs-lab` with `rootdir` added
  only for this measurement, and reset to `images` afterward): a rootfs on `nfs-lab` is accepted without
  `--with-local-disks`; `vzmigrate`'s log says "volume … is on shared storage 'nfs-lab'" and the
  task ends in 16 s, with no copy. The whole `move --restart` took 102 s — most of it is the
  `shutdown` deadline the init's `sleep` ignores, before the `stop`.

## Slice 6 — Close-out

- The matrix regenerated with the tracing: the routes used become `supported+tested`, and the rest
  stay `unsupported-by-design`, citing ADR-0058 instead of ADR-0049 D4.
- ADR-0058 becomes `Accepted` with the date and each slice's PRs.
- A section in `AGENTS.md`, and checks in `scripts/e2e.sh` for the refusals that need no
  node (an `exec` on a system container, `unprivileged: 0`, an image with zstd layers).

- **Closed on 2026-09-28.** The matrix already had the 9 lifecycle LXC routes in
  `supported+tested` (Slice 3, trace of the live case); the reason for the remaining 53 now names
  ADR-0058 and `kind: SystemContainer` instead of ADR-0049 D4. ADR-0058 gained the
  "Implementation" section with each slice's PRs and what was measured and what is still to measure. In
  `scripts/e2e.sh`, a section of 7 no-node checks. **It caught a Slice 4 defect**: `unprivileged: false`
  was "unknown field — ignored", and with a node `apply` created an unprivileged container with
  code 0. It became DX-1540, before the provider resolves. `exec` on a system container has
  no verb (there is nothing to refuse via the CLI); the check reads the
  `system-container.exec` `unsupported-by-provider` line from `provider describe`. The zstd layer stays in the unit
  test `a_zstd_or_foreign_layer_is_refused_by_name`: through the Kind it can only be reached with a node. The lab
  was left clean: the `dlxs2` token revoked and the test `dlx-*` archives removed from `local`.

## Routes this plan calls

About 13 of the 62 LXC routes: listing, creating, destroying, `config` (GET and PUT),
`status/current`, `start`, `stop`, `shutdown` and `interfaces`, plus `tasks/{upid}/log` and the storage's
`upload` and `content` (shared with ADR-0057). The remaining 49 only enter via Slice 5, each with its
own named need.

## Out of the plan, on purpose

- Serving `kind: Container` from LXC (decision 1 of ADR-0058).
- Emulating `exec` or logs through a websocket console (`termproxy`/`vncproxy`): it gives no exit code
  nor separates the streams.
- Entering the node via SSH and using `pct exec`: it falls outside the API, and ADR-0049 D3 and ADR-0010 keep it
  out of the engine.
- Privileged containers, `nesting`, and the node's `oci-registry-pull`.

## Risks and dependencies

- **Conflicts:** the open Proxmox PRs (#541 and #542) touch the same five spots (`codes.rs`,
  `pt.po`, `error.rs`, `lib.rs`, `tests/live.rs`). The slices go one at a time, with a rebase
  before each push.
- **P4b.3** is touching the `VmBackend` port. The new port lives in its own file.
- **Lab:** on `vmbr0` the host's anti-spoof filter drops the frames of a nested
  guest, so a DHCP request inside a container never gets answered there. The network cases use
  `vmbr1` (NAT inside `pve`) with a fixed IP. After a host restart, the lab's two VMs end up
  powered off, and the cluster only has quorum with both up.
- **The `http=000` cause** in the upload with the wrong checksum was not isolated. It is Slice 2's
  first task, not a detail.
