# Lab 6 — Your own VM image, with a VMfile

> **Identifier:** `lab-6` · **Level:** advanced · **Journey:** first contact

## Concrete objective

Build a **custom-tailored qcow2 image** from a `VMfile` — just as you build a
container image — and boot a VM **from it**:
`vm image init` (scaffold) → `vm image build` (bakes the base with libguestfs) →
`vm image ls` → `vm create --disk`. The proof is functional — it reads from
`vm image ls -o json` and `vm ls -o json`, not a loose exit code.

## Prerequisite knowledge

Lab 5 (microVM: lifecycle) and some notion of container images (a `Dockerfile`
is the parallel of the `VMfile`).

## Tested version and environment

> **Historical baseline (5.1.x line):** the values below were measured on the old line and **have not yet been re-measured on the current 0.1.0 line**.

`delonix 5.1.1`, Linux with libguestfs (`virt-customize`) and libvirt, rootless
execution. Validated on 2026-10-24 (cycle `init → build → create → Running+IP →
destroy`, positive `EXIT=0`; failure modes "image did not build", "VM did not
boot from the image", and "orphan after destroy" confirmed to yield `EXIT=1`).

## Minimum resources

**libguestfs-tools** on the host (`virt-customize`). The build downloads the
base `FROM ubuntu:24.04` (a few hundred MiB) and produces a sparse qcow2; the
per-VM overlay is small. Expect **minutes** on the first run.

## Required privileges and access

No `sudo` for this host's rootless path. The build uses libguestfs in
user mode.

## Estimated duration

~3–10 minutes on the first run (dominated by the base download and
`virt-customize`); booting the VM takes seconds.

## Documentation correction / finding from this lab

The `vm image init` scaffold (and its "Next:" hint) prints
`delonix vm create dev --disk-image my-base:1.0`, but the actual flag for
`vm create` is **`--disk`** (`--disk-image` does not exist). The harness uses `--disk`,
which is correct; the scaffold is yet to be fixed in the binary (out of scope for this
lab, which only touches the labs).

## Preparation files

No external files. The harness generates the build context (scaffold `VMfile`) and
a lab-only SSH key. Everything lives in this folder:

| File | Phase |
|---|---|
| `prepare.sh` | ensures binary + libguestfs + lab key; `vm image init`; points `SSHKEY` at the lab key; destroys leftovers |
| `run.sh`     | shows the `VMfile` → `vm image build` → `vm image ls` → `vm create --disk` → waits for Running+IP |
| `verify.sh`  | asserts: image in the store, VM boots from the image, teardown with no orphans, image survives destroy |
| `cleanup.sh` | `vm destroy` + `vm image rm` + removes context and key (idempotent) |
| `lab.sh`     | orchestrator `prepare → run → verify → cleanup` |

## How to run

```bash
# with delonix on the PATH (normal install):
bash docs/labs/lab-6/lab.sh

# pointing at a specific binary (e.g. a local build):
DELONIX=/path/to/delonix bash docs/labs/lab-6/lab.sh
```

> Note: `verify` destroys the VM at step C (it is the teardown assertion), but
> the **built image stays in the store**. Run `./lab.sh --no-clean` to keep
> it and boot another VM from it.

## Steps (what `run.sh` does) and why

1. Shows the scaffold `VMfile` — `FROM ubuntu:24.04`, `SIZE 20G`, offline `RUN`s,
   `USER`/`SSHKEY`, `VCPUS`/`MEMORY`. Builds **as is**.
2. `vm image build -t my-base:1.0 .` — bakes the base with libguestfs. The scaffold's
   `RUN`s are **offline**; only the `FROM` base is downloaded. (A `RUN` with
   `apt-get install` needs `--network`.)
3. `vm image ls` — your image has entered the VM image store.
4. `vm create lab6vm --disk my-base:1.0` — a per-VM overlay from **your**
   image.
5. Waits for `Running` with IPv4 (cloud-init/DHCP takes time) — read from `vm ls -o json`.

## Checkpoints / success assertions (what `verify.sh` requires)

- **A** — the `my-base:1.0` image appears in `vm image ls -o json` (the build
  produced the disk).
- **B** — the VM boots `Running` with IPv4 **from your image** (retry;
  distinguishes "never Running" from "Running without IP").
- **C** — `vm destroy` removes the VM and **leaves no orphans** (`describe vm` no longer
  resolves), and the **image survives** the destroy (the overlay was per-VM).

## Cleanup

`cleanup.sh` runs `vm destroy` on the VM (if it exists), `vm image rm` on the
built image, and removes the build context and the lab key. It does **not**
touch the VM images of other labs.

## Lesson learned

Building a VM image is like building a container image: a `VMfile` with a
`FROM`, a `SIZE`, `RUN`s and a `USER`, and a `build` that bakes it. The image is a
complete bootable disk that lives in the store and serves as a base for as many VMs as you want
— each `vm create --disk` from it takes its own overlay.

## Next lab

**Lab 7 — Kubernetes without Docker** (a local cluster whose node runtime is
Delonix).
