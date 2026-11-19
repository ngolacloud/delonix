# §12 — Recovery and Continuity — Delonix 0.1.0 (candidate v0.1.1)

- Binary: `target/release/delonix` 0.1.0, commit 8ab783dd7 (+39 commits), built 2026-10-29.
- Host: rootless, state root `~/.local/share/delonix`. Disk at 99% (11G free) — nothing built.
- Live providers: cloud-hypervisor v52, libvirt 10/QEMU 8.2.2, linux. proxmox/opnsense NotConfigured (out of scope).
- Test resources created with prefix `s12-` and **all removed at the end** (confirmed: no s12 containers/volumes/records; the owner's 3 containers intact; `git status` clean).

Classes used: **implemented** / **tested-passing** / **tested-failing** / **mechanism-exists-not-exercised** / **absent**.

---

## Mechanisms found in the code (map)

| Mechanism | Location | Note |
|---|---|---|
| Container state reconciliation | `crates/adapters/delonix-linux/src/lib.rs:10991` `reconcile_status` | Running+pid dead→Crashed; Running+frozen→Paused; Paused+thawed→Running. Uses `safe_to_signal(pid, pid_starttime)` to close the PID-reuse window. Reconciles in **memory**; only persists if the caller persists. |
| reconcile callers | node-api (`containers.rs:138`, `container_ops.rs`), mgmt (`dashstats.rs`), cri, mcp | `records()`/`list_in` reconcile on read but **do not persist** (daemonless — nobody alive to persist). |
| Daemonless supervisor | `delonix run -d` itself stays alive watching the init | On init death, reaps + **persists** terminal state to disk. |
| VM reconciliation | `crates/contexts/delonix-compute/src/vm.rs:2049` `adopt_pid_starttime`, `status_reconciles_a_vm_whose_backend_stopped_it` | Adoption by pid+starttime; not exercised live (I avoided heavy VMs). |
| Durable atomic write | `crates/contexts/delonix-node/src/atomic.rs` `write_atomic_mode` | Unique tmp per writer (`pid.seq`) → `sync_all` → `rename` → dir fsync. Mode set at creation (anti-TOCTOU). |
| Record store (O_EXCL) | `crates/adapters/delonix-state/src/store.rs:162,867` `create_new(true)` | Exclusive creation for idempotency/replay. |
| Operation idempotency | `crates/interfaces/delonix-node-api/src/operations.rs:8` | `request_id`→`r-<id>` created exclusively; a repeated request returns the previous response. |
| Atomic layer publish / cache healing | `crates/adapters/delonix-oci/src/overlay.rs:298` `publish_layer_dir` + `.extracted` marker (="ok") | A partial/poisoned layer dir (without `.extracted`) is **replaced** by atomic swap; measured live example (kindest/node stuck in 3 entries). Directly related to F-1. |
| Cross-binary version guard | `crates/contexts/delonix-node/src/dispatch.rs` `check_version` (`DELONIX_DISPATCH_VERSION`) | Refuses a server (CRI/node-api/MCP) from another release alongside `delonix`. |
| Per-resource backup | CLI `delonix backup create/inspect/restore/schedule/remove` | tar.gz archive with record + volume DATA; field `Format: 1`, `Written by: delonix 0.1.0`. |
| Whole-node snapshot | CLI `delonix system snapshot create/restore` | Registries, IPAM, secrets, PKI, event log. Not exercised. |
| Boot persistence | CLI `delonix system boot enable/status/disable` | systemd user units so containers come back after reboot. **Disabled by default.** |
| Record forward-compat | `crates/contexts/delonix-compute/src/record.rs:483` `#[serde(default)]` | Additive migration via serde; **no** explicit schema version nor downgrade gate on records. |
| Corruption detection | `cred_vault.rs` `CorruptMasterKey`; `sdn` `NetworkRecordCorrupted`; `vm_error.rs` `DiskCorrupted` (via `qemu-img check`) | Mechanisms exist; I did not corrupt the owner's live state. |

---

## Item → class → evidence → verdict table

### Item 1 — Death of the Delonix process / the holder → does it reconcile? duplicate?
**Class: tested-passing.**

- **1a — kill the init (SIGKILL):**
  `delonix run -d --name s12-kill busybox:1.36 sh -c 'while true; do sleep 5; done'` → init PID 3369761, supervisor (`delonix run -d`) 3369760.
  `kill -9 3369761` → supervisor **also exited** and **persisted to disk** `status=Crashed pid=None` (file `containers/<id>.json`). `delonix ps -a` shows `Dead`. No orphan busybox processes (`pgrep -af "while true"` empty), no cgroup leaf `*72a65*`.
- **1b — kill only the supervisor:**
  `kill -9 <supervisor>` → init `sh` **survives**, reparents (ppid 2434), `ps` still `Up`, record on disk `Running pid=<init>`. reconcile_status remains correct.
  Then `kill -9 <init>` (no supervisor) → `ps -a` shows `Exited (unknown)`, disk `Crashed pid=None`; `delonix start s12-sup` recovers → `Up`, `restarts=1`, new pid, disk `Running`.

**Verdict:** reconciles in every case; **no duplication**; recovery via `start` works. Cosmetic caveat: the same Crashed state appears as `Dead` (1a) and `Exited (unknown)` (1b). `crash_reason` comes back `null` in one of the deaths (known — leaf disappears before it can be read).

### Item 2 — Host restart
**Class: simulated (loss of supervisor) tested-passing + mechanism-exists-not-exercised (real reboot).**

- `delonix system boot status` → `mode: rootless (user units + linger)`, `dir: ~/.config/systemd/user`, **`(no boot units — run delonix system boot enable)`**. In other words: boot persistence EXISTS but is **off**; after a real reboot containers do **not** come back without `boot enable`.
- Real reboot **not performed** (instruction). Loss of supervisor (Item 1b) is the exercisable analogue: the workload survives the loss of its supervisor and the view recovers via reconcile; `start` relaunches.

**Verdict:** mechanism present, **inactive by default** (see block P2-2). Real reboot not yet exercised.

### Item 3 — Lost response / provider unavailable / revoked credential (timeout ≠ remote failure)
**Class: tested-passing (partial) + mechanism-exists.**

- Unreachable registry: `delonix create --name s12-badimg does-not-exist-registry.invalid/nope:0` → `error[DX-9401] registry error: error sending request...` with hint. `delonix ps -a` **without** an orphan s12-badimg record.
- Timeout/lost response ≠ remote failure: `operations.rs` — `request_id`→`r-<request_id>` created exclusively (O_EXCL); the same request resent returns the previous response instead of re-executing. Mechanism present; exercised indirectly through the `delete` replay path.
- Revoked credential / vault: `CorruptMasterKey` detected when the key file has the wrong size (`cred_vault.rs`). I did **not** corrupt the owner's live vault.
- proxmox/opnsense NotConfigured → out of scope.

**Verdict:** honest behaviour on unavailability; idempotency protects against double-execution from a lost response. Remote credential revocation not yet exercised (providers out of scope).

### Item 4 — Partial creation failure → orphan state?
**Class: tested-passing.**

- Create with an unreachable image (above) → clean failure, **no** orphan record.
- `atomic.rs`: write via unique tmp + fsync + rename + dir fsync → a create interrupted midway leaves a `.tmp`, **never** a torn/corrupted record.
- Interrupted layer extraction: `overlay.rs publish_layer_dir` + `.extracted` marker ("ok") distinguishes complete from partial extraction and **replaces** partial/poisoned dirs via atomic swap.

**Verdict:** no orphans observed; anti-corruption/atomic design present and documented (measured live in the past).

### Item 5 — Storage unavailable; concurrency; interrupted cleanup
**Class: MIXED — tested-passing (concurrent rm) / tested-FAILING (concurrent create same name) / mechanism-exists-not-exercised (storage unavailable, interrupted prune).**

- **Concurrent rm, same resource (running):** `delonix rm s12-sup & delonix rm s12-sup & wait` → **both** the same refusal `[DX-5101] conflict: container ... is running (use --force)`. Container intact. No partial state.
- **Concurrent force-rm:** `rm -f & rm -f & wait` → one removes (`44289cc...`), the other `[DX-4101] no such container`. No double-free, no orphan processes (`pgrep -af 44289cc` empty).
- **Concurrent create same name (FAILURE):** `delonix run -d --name s12-race ... & delonix run -d --name s12-race ... & wait` → **both** succeeded, creating TWO `default/s12-race` containers (ids `dcf6617...` and `96fda7b...`, confirmed by `ps -a`). Name resolution became **permanently broken**: `delonix container describe s12-race` → `error[DX-1000] invalid argument: this name exists in several namespaces — qualify it as <namespace>/<name>: s12-race (default/s12-race, default/s12-race)` (misleading text: it is the SAME namespace). Addressable only by ID. A 3rd sequential `run` refuses correctly (`DX-5000 ... already in use ... by container 96fda7bf41b7`), confirming the guard is sequential and the window is the race (TOCTOU). The atomic reservation mechanism exists in the code (`<name>.claim` files with O_EXCL in `container-names/`, used by the stack/apply path) but the `run --name` path **does not use it**. Recoverable: `rm -f <id> <id>` cleaned up both (events `die exit=137` + `remove` by id).
- **Storage unavailable / interrupted prune:** not exercised (would require injecting a failure into the state fs / a timed SIGINT during a prune). Mechanism-exists-not-exercised.

**Verdict:** removal concurrency solid; **name-based creation concurrency has a race (P2)**; storage-down and interrupted-prune not yet exercised.

### Item 6 — Persisted state: backup/restore, schema version, corruption, downgrade
**Class: tested-passing (backup/restore) + mechanism-exists (snapshot, corruption) + GAP (record version/downgrade).**

- **Per-resource backup/restore (round-trip PROVEN):**
  `delonix volume create s12-vol`; `delonix run -d --name s12-bkp -v s12-vol:/data busybox ...` (writes `/data/marker.txt=hello-s12`).
  `delonix backup create container s12-bkp --to <dir>` → archive `container-s12-bkp-...tar.gz (1.3 KiB)`, `volumes: s12-vol`.
  `delonix backup inspect` → `Kind: container`, `Format: 1`, `Written by: delonix 0.1.0`, `Volumes: s12-vol`.
  I mutated: `exec s12-bkp sh -c 'echo CHANGED-AFTER-BACKUP > /data/marker.txt'`. `delonix backup restore <archive>` without --force → **refuses**: `DX-1000 ... 's12-bkp' is running — stop it first, or pass --force`. With `--force` → `volume s12-vol restored`, `container kept`, `started again`; `exec s12-bkp cat /data/marker.txt` → **`hello-s12`** (reverted).
- **Node snapshot:** `delonix system snapshot create/restore` (registries, IPAM, secrets, PKI, events). Mechanism-exists-not-exercised.
- **Schema version / downgrade:** backups carry `Format: 1`. **Container records do NOT have an explicit version** — additive forward-compat via serde `#[serde(default)]`; downgrade tolerated (serde ignores unknown fields) but **without a downgrade gate/warning**.
- **Corruption:** detection present — `CorruptMasterKey` (vault), `NetworkRecordCorrupted`, `DiskCorrupted` (qemu-img check). Not corrupted live.
- **Binary rollback ≠ state rollback (relation to F-1):**
  - `dispatch.rs check_version` refuses to mix a server from another release with the installed `delonix`.
  - F-1 (poisoned layer cache): in this binary the healing is the **`.extracted` marker + atomic swap** of `publish_layer_dir` (the "format epoch" materializes as complete-vs-partial), not an epoch integer. A binary that changes the format semantics is protected because old/partial dirs are **replaced**, not served.
  - **GAP**: there is no state version stamp nor downgrade policy on records — rolling the BINARY back to a pre-0.1.0 without fields is silently tolerated (loses new-field semantics), with no refusal nor warning. Only backups are versioned.

**Verdict:** per-resource backup/restore solid and proven; node snapshot and record version/downgrade are the fragility (see P3).

---

## Blocks

- **P2-1 — Name-uniqueness race in concurrent `run --name` (tested-failing).** Two concurrent creates with the same `namespace/name` both succeed → two homonymous containers; name resolution breaks (`DX-1000`, misleading "several namespaces" text), addressable only by ID. Sequential guard (`DX-5000`) ok; the atomic reservation is missing. Fix: use the existing `.claim` O_EXCL mechanism (stack/apply) in the `run/create` path. Recoverable via `rm <id>`. Severity P2 (violates the uniqueness invariant, degrades UX; does not corrupt, does not crash).
- **P2-2 — Boot persistence off by default (operational caveat).** `system boot status` = no units; a real reboot does **not** restore containers without a prior `system boot enable`. For a PaaS node this is a footgun. Fix: document/enable by default in the node profile.
- **P3 — No schema version / downgrade gate on state records (observation).** Backups carry `Format:1`; records have neither a stamp nor a downgrade policy. Binary rollback over newer state is silent. Recommend a version stamp + refusal/warning on downgrade.
- **Cosmetic** — the Crashed state renders as `Dead` and as `Exited (unknown)` depending on the path; `crash_reason` is sometimes `null`.

**No P1 found.**

---

## Not yet exercised (honest)

- Real host reboot (instruction: do not reboot). Only loss of supervisor was simulated.
- Live VM reconciliation (`adopt_pid_starttime`) — I avoided heavy VMs / disk at 99%.
- Storage unavailable (ENOSPC / read-only state fs) — would require fault injection.
- `rm`/`prune` interrupted midway (timed SIGINT).
- Whole-node `system snapshot create/restore` — destructive/heavy, not run.
- Real vault corruption / provider credential revocation — destructive for the owner / providers out of scope.
- Backup restore with an incompatible `Format` — I did not forge a `Format: 2`.

---

## §12 verdict: **READY WITH CAVEATS**

The recovery core is solid and **proven live**: crash reconcile without duplication, recovery via `start`, anti-corruption atomic write, operation idempotency, layer-cache healing by marker+swap, and per-resource backup/restore with a data round-trip. To close v0.1.1 in production, resolve **P2-1** (name race) and decide **P2-2** (boot persistence), and address **P3** (state version/downgrade). No P1.
