# Lab 5 — A real microVM

> **Identifier:** `lab-5` · **Level:** intermediate · **Journey:** first contact

## Concrete objective

Prove the **complete lifecycle** of a microVM with the same CLI as
containers: create from a local image, reach `Running` with an IP, take
a **system checkpoint** (memory + disk) with the VM **running**, roll
back with `restore`, and tear down with `vm destroy` **leaving no orphans**. The proof
is functional — it reads from `vm ls -o json`, not a loose exit code.

## Prerequisite knowledge

Linux terminal and a notion of VMs. Labs 1–4 (containers) help but are not
required.

## Tested version and environment

> **Historical baseline (5.1.x line):** the values below were measured on the old line and **have not yet been re-measured on the current 0.1.0 line**.

`delonix 5.1.1`, Linux with libvirt, rootless execution. Validated on 2026-10-24
(cycle `create → Running+IP → snapshot while running → restore → destroy`, positive
`EXIT=0`; failure modes "VM never booted", "no IP", and "snapshot/restore
failed" confirmed to yield `EXIT=1`).

## Minimum resources

A **local VM image** (e.g. `k8s-golden`, ~634 MiB) — the harness picks one
from `vm image ls` so as not to download GiBs. The per-VM overlay is around ~350 MiB.

## Required privileges and access

No `sudo` for this host's rootless libvirt path.

## Estimated duration

~10–20 seconds with the image cached (dominated by boot and DHCP).

## Documentation correction this lab brought

The old Lab 5 text had **two errors** that the live test exposed and that are
now fixed here:

- teardown is **`vm destroy <name>`**, not `delonix delete vms <name>` (the
  latter errors with `DX-4000 no such resource kind 'vms'`);
- a snapshot of a **stopped** VM does **not** fail — it is **disk-only** (supported). What
  `vm stop` *undefines* is the **running domain**; disk, record, and snapshots
  persist.

## Preparation files

No external files. The harness lives in this folder:

| File | Phase |
|---|---|
| `prepare.sh` | ensures binary + local VM image; destroys leftovers |
| `run.sh`     | `create` → waits for Running+IP → `snapshot create` (while running) → `restore` |
| `verify.sh`  | asserts the lifecycle and teardown with no orphans |
| `cleanup.sh` | idempotent `vm destroy` (verify already destroys it at step D) |
| `lab.sh`     | orchestrator `prepare → run → verify → cleanup` |

## How to run

```bash
# with delonix on the PATH (normal install):
bash docs/labs/lab-5/lab.sh

# pointing at a specific binary (e.g. a local build):
DELONIX=/path/to/delonix bash docs/labs/lab-5/lab.sh
```

> Note: `verify` destroys the VM at step D (it is the teardown assertion). To
> explore the VM (`vm console`/`vm ssh`), run only `prepare.sh` + `run.sh` and stop
> before `verify`.

## Steps (what `run.sh` does) and why

1. `vm create lab5vm --disk <image>` — a per-VM overlay from the local
   image (pinned to be reproducible and not download GiBs).
2. Waits for `Running` with IPv4 (cloud-init/DHCP takes time) — read from `vm ls -o json`.
3. `vm snapshot create lab5vm clean` **with the VM running** → checkpoint of
   **memory + disk**.
4. `vm snapshot restore lab5vm clean` — rolls back to the checkpoint.

## Checkpoints / success assertions (what `verify.sh` requires)

- **A** — the VM is **`Running`** with a valid **IPv4** (retry; distinguishes "never
  Running" from "Running without IP").
- **B** — the `clean` snapshot appears in `vm snapshot ls` (live checkpoint).
- **C** — after `restore`, the VM returns to `Running` with an IP.
- **D** — `vm destroy` removes everything and **leaves no orphans**: `describe vm` no longer
  resolves (`DX-4501`) and the VM drops out of `vm ls`. Fails loudly if anything is left
  behind.

## Cleanup

`cleanup.sh` runs `vm destroy` on the VM if it still exists (idempotent — `verify` already
destroys it at step D). It does **not** remove the local VM images (other labs
reuse them).

## Lesson learned

A microVM has the same lifecycle as a container, through the same CLI, with one
significant difference: a snapshot taken while the VM is running is a **system checkpoint**
(memory + disk), not just the disk. And the correct teardown — `vm destroy` — takes
disk, snapshots, and record in one go, leaving no orphans.

## Next lab

**Lab 6 — Your own VM image, with a VMfile** (build a custom-tailored
qcow2).
