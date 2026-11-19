# RT-4 Phase 1 — implementation map (`delonix oci-runtime`)

> Concretizes ADR-0080 at the code level, so implementation in the build window is mechanical.
> Based on the 2026-10-24 investigation of `delonix-linux`'s init.

## The discovery that makes this tractable

`delonix_linux::create_with(store: &Store, container: &mut Container, rootfs: &str, spec: &RunSpec)`
(`lib.rs:6989`) is the **reusable launch entry point**: it starts the init from a
`Container` + a **rootfs path** + `RunSpec` — **not** from an image. And `ContainerInitSpec`
(`lib.rs:3876`) consumes `rootfs: &str` + process/filesystem/security/namespaces/io — it is not coupled
to an image. An OCI bundle **already has** `rootfs/` extracted + `config.json`, so:

- **No image store or unpack is needed** — the bundle's rootfs is used directly.
- The real work is **translating `config.json` (oci_spec::runtime::Spec) → `Container` + `RunSpec`**.

## Structure (files to touch)

| File | What |
|---|---|
| `bins/delonix-runtime-bin/src/cmd/oci_runtime.rs` (new) | the subcommand, the 5 handlers, the state model, the Spec→Container translator |
| `bins/delonix-runtime-bin/src/main.rs` | register `OciRuntime(OciRuntimeArgs)` in the top-level enum + dispatch; `--l18n`/pt.po for the help text |
| `bins/delonix-runtime-bin/data/pt.po` | 3 help texts (the new clap flag hits ARG_HELP_PENDING) |

## CLI (clap)

```
delonix oci-runtime [--root <state-dir>] create <id> --bundle <dir>
delonix oci-runtime [--root <state-dir>] start  <id>
delonix oci-runtime [--root <state-dir>] state  <id>
delonix oci-runtime [--root <state-dir>] kill   <id> [signal]
delonix oci-runtime [--root <state-dir>] delete <id> [--force]
```
`--root` default: `$XDG_RUNTIME_DIR/delonix-runtime` (rootless) / `/run/delonix-runtime` (root).

## Bundle-keyed state (in `--root`, NOT in the image store)

```rust
#[derive(Serialize, Deserialize)]
struct OciState { oci_version: String, id: String, status: String, pid: i32, bundle: String }
// status: "creating"|"created"|"running"|"stopped" (the runtime-spec's enum)
```
File `--root/<id>/state.json`. Pure and unit-testable (serde round-trip + the shape `state` prints).

## `config.json` → `Container` + `RunSpec` translator (the core)

Read `oci_spec::runtime::Spec` from `<bundle>/config.json`. Map:

| OCI `config.json` | delonix |
|---|---|
| `process.args` | `Container.cmd` |
| `process.env` | `Container.config.env` |
| `process.cwd` | `Container.config.working_dir` |
| `process.user.{uid,gid}` | `RunOpts.user`/run_uid/run_gid |
| `root.path` (relative to the bundle) | `rootfs = <bundle>/<root.path>` |
| `mounts[]` | `Container` mounts (reuse the volume's `Mount` type) |
| `linux.namespaces` (user/net/…) | `RunSpec.new_netns` + `Container.userns` + mappings |
| `linux.resources.{memory,cpu,pids}` | `Container` limits (the delegated controllers) |
| `linux.seccomp` | `seccomp_profile::parse` (ALREADY consumes OCI seccomp JSON) |
| `hostname` | `Container.hostname` |

**Refuse-by-name (guardrail 6, never ignore):** `hooks` (no OCI-hook protocol), `annotations`
that imply behavior, `linux.resources` with no delegated controller (as `system resources`
already does), AppArmor on rootless. Each refusal names the field + the reason.

## Handlers

- **create**: parse config.json → `Container` + save `OciState{status:"created",pid:0}` in `--root`;
  do NOT start it. (The bundle's rootfs is already ready.)
- **start**: read the state → `create_with(&store_in_root, &mut container, &rootfs, &RunSpec{detach:true,…})`;
  record `pid`/`status:"running"`.
- **state**: read the state, `reconcile_status`, print the OCI JSON (`{ociVersion,id,status,pid,bundle}`).
- **kill**: `runtime::send_signal(&store, id, sig)`.
- **delete**: teardown (netns/cgroup/mounts via `cmd_rm`'s path) + remove `--root/<id>`.

## Verification (in the build window)

1. `cargo build --release -p delonix-runtime-bin` (incremental; disk ≥12G).
2. Unit: `OciState` round-trip + the translator on a minimal `config.json` (fail-without/pass-with).
3. Live: `delonix image export --rootless alpine /tmp/b` (already have it) → `delonix oci-runtime create c1 --bundle /tmp/b` → `start c1` → `state c1` (running) → `kill c1` → `delete c1`; zero orphans.
4. Suite: `RUNTIME=<wrapper for delonix oci-runtime> scripts/runtime-tools.sh` → baseline vs runc.

## Phase 1 gate

A `runtime-tools: N ok / M not ok` number against `delonix oci-runtime`, published alongside runc's
(the reference). Phase 2 closes the highest-value `not ok`s, one per slice, with a test.
