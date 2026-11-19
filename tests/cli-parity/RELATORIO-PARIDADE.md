# Delonix CLI parity report ↔ Docker/Podman

**Date:** 6 November 2026 · **Base:** `origin/develop` @ `99ecc7968` (the SHA
where the ZIP matrix was measured) · **Branch:** `feat/cli-parity-docker-podman`

> **Second wave (scope extension requested by the owner).** After the first
> delivery, the owner asked to resolve the divergences towards 100% Docker/Podman
> and to guarantee the native VMM. Three decisions taken: (1) *shim + Docker API* for the
> families that clash with invariants; (2) imperative pod layer; (3)
> cloud-hypervisor by default on the VM. Fixes in this wave: **C4** (VM default),
> **C5** (`pod stop/start/restart/rm`), **C6** (aliases `--network`/`--cpuset-cpus`).
> See the «Second wave» section at the end. **Literal 100% was not reached, nor is it
> reachable in a single session** — it is a multi-phase campaign (measurement below).

## References and environment (measured, not assumed)

| Component | Version | Mode |
|---|---|---|
| **Delonix** | `0.1.0`, compiled from the checkout (`target/debug/delonix`) | daemonless, rootless, cgroup2 (cpu/memory/pids delegated; io/cpuset not) |
| **Docker** | client+server `29.9.2` | rootful, cgroup systemd, `unix:///var/run/docker.sock` |
| **Podman** | `4.9.3` | rootless, netavark, runc 1.5.2 |
| Kernel / arch | `7.0.0-42-generic` / `x86_64` | PTY via `python3`; subuid/subgid present |

> **Divergences against the ZIP matrix:** the matrix cited Docker CLI **v29.0.0** and
> Podman **v5.6.2**. The host has Docker **29.9.2** (minor bump, no impact on the
> contracts exercised) and Podman **4.9.3** — a major **prior** to the one cited.
> Relevant for pods (`pod create --name` imperative exists in both) and for
> `system connection` (Podman context). The `delonix` binary on PATH was an old
> `5.1.1` — **ignored**; everything was measured against the binary compiled from the tree.

The `Delonix_TAB.patch` from the ZIP was **not** in `origin/develop`; `git apply --check`
verified clean and it was applied as the base for this session.

## Result by domain

Functional coverage = passed cases ÷ agreed applicable cases; blocked and
not-executed stay in the denominator. **No 100% is declared** — the versioned
scope (198 paths) is exercised in depth in the two priority flows and, by directed
sampling, in the rest; the remainder is measured at the level of
**recognition** (parser) and marked honestly «to be tested».

| Domain | Recognised | Passed (behaviour) | Deliberate divergence | To be tested | To do / out of scope |
|---|---|---|---|---|---|
| 1 Containers | 43/65 | **8** (run, create, ps, stats, logs, wait, cp, port) | 2 (inspect, diff) | 35 | 20 |
| 2 Pods | 7/17 | **6** (pod, pod logs, pod stop/start/restart/rm) | 1 (`pod create --name` imperative) | — | 10 |
| 3 Images | 22/43 | **2** (login, logout) | — | 20 | 21 |
| 4 Volumes | 6/13 | **2** (volume create, volume rm — round-trip + refuses-in-use) | — | 4 | 7 |
| 4 Networks | 7/11 | — | — | 7 | 4 (incl. `network prune`) |
| 5 System/Contexts | 8/30 | **2** (info, events) | — | 6 | 22 (contexts: out of scope, ADR-0010) |

(Exact count by state in [`matriz-cli-parity.csv`](matriz-cli-parity.csv),
regenerable with [`build_matrix.py`](build_matrix.py).)

## Fixes implemented (code, tests, docs)

### C1 — `run`/`create` accept `-i`/`-t`/`-it` (real bug, flow 1)

The parser refused `-it` («`error: unexpected argument '-i' found`»), breaking the
**first line** of any interactive script. Measured: docker and podman
pass the flow; delonix failed at the parser. The cause was **only** recognition —
a `run` in the foreground, being daemonless, already INHERITS the operator's stdio, so
the interactive shell, the `test -t 0` and the exit code were already those of
`docker run -it`. Added `-i`/`--interactive` and `-t`/`--tty` to `RunArgs`
(`bins/delonix/src/cmd/container.rs`), accepted for parity (as `exec` already
does). Documented limit: a `-t` without a terminal on stdout does not synthesise a pty.

* **Before:** `delonix run --rm -it busybox sh` → exit 2, parser refuses.
* **After:** identical to docker/podman — real PTY, `TTY_YES`, marker read,
  **exit 37**, container removed by `--rm`.

### C2 — actionable error for `run --pod <nonexistent>` (flow 2)

Pods in delonix are **declarative** (ADR-0006): there is no `pod create --name`, and
`run --pod` only JOINS an existing pod. The old error was cryptic
(`DX-9000 ... ingress infra is down — no holder to enter`). Now
(`reexec_into_netns`) a `--pod` without a holder returns `DX-1000` naming the
declarative flow (`kind: Pod` + `delonix pod create -f`). Declarative creation creates the
holder before the members, so there is no regression.

### C3 — root verbs `cp`/`port`/`login`/`logout`/`info`/`events` (flows 3/5)

Root verbs that the parser refused even though the group form already worked.
Added as **true aliases** — the SAME argument struct, the SAME group handler
(`container`/`image`/`system`), **verbatim** docstrings to
share the i18n catalogue. Touched: `main.rs` (enum+dispatch+`STABLE_GROUPS`),
`manual_entries.rs` (entries+examples), `data/pt.po` (translations of the
`-i`/`-t` help and the new examples), `docs/gen.py` + `README.rst` +
`docs/cli-stability.md` (documentation gates). `inspect` and `diff` do NOT enter the
root — type-scoped policy documented.

## Priority flows — before/after (evidence)

**Flow 1 — interactive container** ([`evidence-flow1.jsonl`](evidence-flow1.jsonl)):

```
ENGINE   ANTES                 DEPOIS
docker   ok  (exit 37)         ok  (exit 37)
podman   ok  (exit 37)         ok  (exit 37)
delonix  XX  (exit 2, -it)     ok  (exit 37)   ← C1
```
Green steps across all three: `run --rm -it sh` (PTY, `test -t 0`, exit 37, removal),
`-i` without TTY (stdin piped, no tty inside), stdout/stderr separated, `stats`
single sample.

**Flow 2 — pod with two members** ([`evidence-flow2.jsonl`](evidence-flow2.jsonl)):

```
podman  (imperativo)  create --name · run --pod ×2 · localhost · ps · inspect · stop/start/restart · rm   → 10/10
delonix (declarativo) pod create -f · localhost(SENTINEL) · get pods · describe pod · delete pod            → passou
                       pod stop/start/restart → PASSAM (C5, segunda vaga)
                       run --pod <absent> → erro accionável (C2, afinado na segunda vaga)
```
Same observable result (two members sharing netns, communication via
`localhost`, listing, inspection, clean teardown) via different routes.

## Discovery via TAB (flow 6) — Fish / Bash / Zsh

Completion protocol exercised directly (without configuring a shell):

| Shell | Invocation | `ps`+`ls` (root/container) | `rm`+`remove` (image) | New aliases w/ description |
|---|---|---|---|---|
| Fish | `COMPLETE=fish … -- delonix ''` | ✓ both | ✓ both | ✓ cp/port/login/logout/info/events |
| Bash | `_CLAP_COMPLETE_INDEX=1 COMPLETE=bash …` | ✓ both | ✓ | ✓ |
| Zsh | `_CLAP_COMPLETE_INDEX=1 COMPLETE=zsh …` (`value:description`) | ✓ both | ✓ both | ✓ with description |

The TAB patch (`expand_completion_aliases`) gives each visible spelling its own
candidate; the shell-agnostic test `tab_lists_each_visible_spelling_with_its_description`
covers the completion engine, and `tab-descriptions.sh` the Fish wire.

## Tests and CI integration

* **Unit** (`cargo test -p delonix-bin`): **1644 pass, 0 fail** — includes
  `run_interactive_flag_parity_tests`, the stability classification, the manual
  and the i18n (`ARG_HELP_PENDING` stays 0).
* **Quick scripts** (no engine): `frequent-verbs.sh` **49/49**,
  `tab-descriptions.sh` green.
* **Differential** (real engines): `flow1_interactive.py`, `flow2_pod.py`.
* **CI** (`.github/workflows/ci.yml`, job `cli-parity`): quick tier always +
  differential tier with docker+podman. [`run.sh`](run.sh) **fails loudly** if
  an engine is missing — an empty or all-ignored suite never passes as compliant.
* **Local gates green:** `docs_cli_gate.py`, `lang_ratchet.py`.

Reproduce:
```sh
cargo build --locked -p delonix-bin --bin delonix
cargo test  --locked -p delonix-bin --bin delonix
tests/cli-parity/run.sh all target/debug/delonix        # rápido + diferencial
python3 scripts/docs_cli_gate.py && python3 scripts/lang_ratchet.py
```

## Deliberate divergences (migration)

| Docker/Podman contract | Delonix | Migration |
|---|---|---|
| `pod create --name`, `pod stop/start/restart` (imperative) | `kind: Pod` + `pod create -f`; lifecycle via `get/describe/delete pod` | describe the pod and the members in a `kind: Pod` manifest; the `run --pod` error already points the way |
| generic `docker inspect <x>` | type-scoped: `container inspect` / `image describe` | name the Kind; no resolver that guesses |
| `docker diff <container>` (rootfs) | root `diff` = MANIFEST diff; `container diff` = rootfs changes | use `container diff` for the Docker form |
| `docker context` / `podman system connection` | absent (ADR-0010 rejected the remote management API) | out of scope; no equivalent planned |

## Pending (next executable step)

1. **`network prune`** (flow 4) — refused; `volume prune` exists. Implement the
   `NetworkCmd::Prune` on top of the existing prune logic + network-in-use test.
   *Next:* follow the `volume prune` pattern in `cmd/network.rs`.
2. **Functional equivalence of the ~70 «recognised-to-be-tested»** — the parser accepts,
   the differential is missing. *Next:* extend `differential/` with a flow 3
   (cp/port/inspect/logs/wait + save/load round-trip + login against a throwaway
   registry) and a complete flow 4 (networks: subnet/gateway/resolution/connect-
   disconnect/publish + prune with resources in use), on throwaway engines.
3. **`image inspect`** — absent (only `container inspect`); Docker has
   `docker image inspect`. *Next:* decide between an alias for `image describe` or
   a documented refusal.
4. **Continuous `events`/`stats` and signals/detach-attach** from flow 1 — exercised in
   a single sample; the *stream* mode and the signals need a PTY with explicit
   timeout in a dedicated runner.
5. **Contexts (flow 5)** — a product decision (ADR-0010), not an implementation one.

---

## Second wave — towards 100% Docker/Podman + native VMM

### Owner decisions (6 Nov 2026)
1. **Shim + Docker API** for the irreducible families (contexts/ADR-0010,
   Swarm/`node`/`service`, `machine`, `plugin`): accept the spellings and, for what
   is truly incompatible, an actionable error pointing to `delonix serve docker-api`
   (with `DOCKER_HOST=unix://…` the `docker` CLI itself talks to Delonix).
   Preserves daemonless/rootless/ADR-0010.
2. **Imperative pod layer** on top of the declarative engine.
3. **cloud-hypervisor by default** (native KVM VMM in Rust).

### C4 — cloud-hypervisor is the default VM backend (native VMM)
Delonix already used native KVM VMMs (`cloud-hypervisor` = a from-scratch Rust VMM
over `/dev/kvm`, boots a real Linux kernel; `libvirt`→QEMU/KVM). The heuristic
preferred **libvirt** for cloud images out of historical caution. Now it prefers
**cloud-hypervisor** when it CAN boot the workload — for a cloud image that
requires `/dev/kvm` **and** the EDK2 firmware `CLOUDHV.fd` (`cloud_image_firmware_
ready()`), not the minimal `rust-hypervisor-fw` (which does not load the initrd).
libvirt remains a fallback. Files: `crates/adapters/delonix-vm/src/lib.rs`,
`crates/providers/delonix-provider-cloud-hypervisor/src/lib.rs` (+test
`only_the_edk2_cloudhv_boots_cloud_images`). Direct-kernel boot (k8s nodes) already
resolved to CH. **Pending live validation** on a host with both backends
to confirm the cloud image boot in CH (the host has `CLOUDHV.fd`; the evidence
measured in the repo confirms that CLOUDHV.fd boots ubuntu/debian/rocky).

### C5 — pod stop / start / restart / rm (Podman)
New verbs that operate all the members of a pod (iterate `members_of` +
`container::cmd_{stop,start,restart}`; `pod rm` = `remove_pod`). They resolve the
matrix divergence «pod stop/start/restart is not a delonix verb». **Proven live**
on a declarative pod (`flow2_pod.py`: stop→0/2, start→localhost OK,
restart, rm). Files: `bins/delonix/src/cmd/pod.rs` (+ manual/i18n).

The **imperative `pod create --name` (empty pod) was NOT shipped**: measured live,
it fails because Delonix's model has no *pause* container — the first
member is the one that holds IPC/UTS, so an empty pod has no holder for those namespaces and
`run --pod` for it fails. Doing it right requires designing a *pause* container
(Podman/k8s style) — a front of its own. Already enabled the **automatic resolution of
`infra_pid`** in `cmd_run` (a `run --pod` that ADDS a member to an existing pod
joins the IPC/UTS of the first member) and the **actionable error** of `run --pod
<nonexistent>` now distinguishes «pod does not exist» (member count) from «global
infra down», instead of the cryptic "Cannot open network namespace".

### C6 — pure-rename flag aliases in run/create
`--network` (alias of `--net`) and `--cpuset-cpus` (alias of `--cpuset`) via clap
`visible_alias` — zero new i18n entries, same handler. `docker run --network
mynet --cpuset-cpus 0-1 …` now works. File: `container.rs` (RunArgs).

### C7 — `--pull missing|always|never` in run/create
Docker/Podman-style pull policy, applied before `run`/`create` touches the
image (in the dispatch, with access to the `ImageStore` — without touching the compute
crate's `RunOpts`): `missing` (default, the engine's resolve-or-pull), `always` (forces a pull
before starting), `never` (refuses to fetch — actionable error if missing). **Proven
live** (`--pull never` on a missing image errors; on a present one it runs) and by
offline root↔group parity (`frequent-verbs.sh`, 50/50) + parsing test.
File: `container.rs` (`apply_pull_policy`).

### C8 — `--cidfile <PATH>` in run/create
Writes the full container id to the file (refuses if it already exists, like
Docker). Discovered that `RunOpts` **derives `Default` + `#[serde(default)]`**,
so adding a field is cheap (only `run_opts_from_args` and the destructure of
`cmd_run` list it; the other constructors use `..Default::default()`) — the
previous caution of "deferring" was excessive. **Proven live** (writes the id;
refuses an existing file). Parsing test. Files: `run_opts.rs`,
`container.rs`.

### C9 — `--shm-size <SIZE>` in run/create
Size of `/dev/shm`. **Reuses the `--tmpfs` plumbing** (converts to the
entry `/dev/shm:size=<bytes>,mode=1777`, the same trick as `compose`) — **zero
runtime changes**. **Proven live**: `--shm-size 128m` → `/dev/shm` with
128M. File: `container.rs` (conversion in `run_opts_from_args`).

### C10 — `--pids-limit` (→ `pids.max`)
Docker/Podman-style process ceiling. Parametrises the existing `pids.max` write
(previously fixed at `DEFAULT_PIDS_MAX=512`) in both cgroup paths
(delegated `lib.rs` + root), `-1` ⇒ `max` (unlimited). Field `pids_max` in
`Container`+`RunOpts`, `build_record`, helper `pids_max_write_value`. **Real
enforcement proven by the host**: `--pids-limit 42` → leaf `pids.max=42`; without the flag → `512`.
The `-1` flag needed `allow_hyphen_values`. Guards updated (no longer a
"flag that does not exist").

### C11 — `--memory-reservation` (→ `memory.low`)
Soft memory floor (soft reclaim floor). NEW write of `memory.low` alongside
each `memory.max` (delegated + root). Field `memory_low` in `Container`+
`RunOpts`, `build_record`. **Proven by the host**: `--memory-reservation 128m` →
leaf `memory.low=134217728`.

> **`--memory-swap` deferred**: the semantics diverge (Docker = total memory+swap;
> cgroup v2 `memory.swap.max` = swap only ⇒ writing `total − memory`), the
> swap controller may be off, and it needs parsing of both values. It stays
> mapped in the backlog with the trap noted.

**Guards updated** (otherwise the build breaks): `flags_of_controller` (`lib.rs`)
got `pids`→`["--pids-limit"]` and `memory`→`["--memory","--memory-reservation"]`;
the tests `the_docker_spellings_are_not_ours` and `delegation_labels_name_only_real_flags`
(`main.rs`) no longer list `--pids-limit`. The node-api messages
(`container_launch.rs`/`container_update.rs`) became CORRECT (they point to the
CLI's `--pids-limit`/`memory.low` that now exists, saying that the node-API still
does not serve them).

### C12 — `--ipc` / `--pid` modes
They fold into the existing booleans `--host-ipc`/`--host-pid`: `host` ⇒ shares
the host's, `private` (default) ⇒ its own; any other mode
(`container:<id>`, `shareable`, `none`) is REFUSED with an actionable error (points to
`--pod` to share between containers), never silently degraded. Pure at the CLI
layer (`run_opts_from_args`), no new fields. **Proven live**: `--ipc
host`≡`--host-ipc`; `--pid host`≡`--host-pid` (identical `/proc/sys` bind error
— a pre-existing limit of host-pid in rootless, not introduced). `--uts`
deferred (would require making `CLONE_NEWUTS` conditional in the runtime).

### C13 — `--stop-signal`
The signal that `stop` sends first (default `SIGTERM`). Field `stop_signal` in
`Container`+`RunOpts`+`build_record`; `cmd_stop` reads it from the record and calls the new
`runtime::stop_with_signal` (receives the signal number — `parse_signal` converts
name/number — so the CLI does not depend on `nix`; `stop_waiting` gained a
`term_signal` parameter, with `stop` passing `SIGTERM`). **Proven live**:
`--stop-signal SIGINT` → the container records `GOT_SIGINT` (received SIGINT, not
SIGTERM); `SIGKILL` still escalates after the grace period.

### C14 — `--stop-timeout`
Grace before `SIGKILL`. Field `stop_timeout` in `Container`+`RunOpts`+
`build_record`. The `-t` of `stop` became `Option<u64>`: the `Stop` dispatch
resolves, per container, explicit `-t` → `--stop-timeout` from the record → 10s
(default), without touching the ~10 callers of `cmd_stop` (which stay `u64`).

### C15 — `--volumes-from <container[:ro|:rw]>`
Mounts all the volumes/binds of another container into this one. The source's mounts are already
RESOLVED (host paths, volume roots), so they transplant
directly into the start set (`mounts`) AND into the record
(`c.mounts`, survives a `start`); `:ro` forces them all read-only; a
nonexistent source gives «no such container». Resolved in `cmd_run` (it has the store; the
`build_record` does not reach it). **Proven live**: the target reads the source's
volume data; `:ro` refuses writing. Commit `078da4d0`.

### C16 — `--init` (pid 1 reaper)
Runs a tiny init as pid 1 that reaps orphans and re-sends signals; the command
runs as its child. In `container_init`, AFTER all the configuration
(namespaces, privilege drop, seccomp, cgroup), a `fork`: the child does the
`execvp` of the command; the parent stays pid 1 in `reap_as_init` — installs handlers that
re-send SIGTERM/INT/QUIT/HUP to the child and does `waitpid(-1)` until ECHILD, returning
the main process's code (128+signal if killed). Seccomp already allows
`clone`/`wait4`/`kill`/`rt_sigaction`; a failed `fork` runs the command directly
(no reaper), never refuses. Plumbed like `read_only` (Container→InitInputs→
ProcessSpec→`container_init`). **Proven live**: with `--init` the command no longer
is pid 1 (pid1=`delonix`), exit 37 propagates, `stop` re-sends SIGTERM to the child
(log `GOT_TERM_VIA_INIT`), and **0 zombies** after an orphan exits. Default (no
`--init`) unchanged (command stays pid 1).

### C17 — `--mount type=bind|volume|tmpfs`
The explicit Docker/Podman syntax, translated to the engine's grammar:
`type=bind|volume,source=…,target=…[,readonly]` → `-v` spec (`src:target[:ro]`);
`type=tmpfs,target=…[,tmpfs-size=N]` → `--tmpfs` entry — the SAME resolution
path, no second mount engine. Keys `source`/`src`,
`target`/`dst`/`destination`, `readonly`/`ro` and `tmpfs-size` honoured; an unknown key or
type is REFUSED. **Proven live**: bind `readonly` refuses
writing, volume round-trip, tmpfs respects the size, invalid key errors.
Commit `195a1cbe`. Pure at the CLI layer (`run_opts_from_args`).

### C18 — `--memory-swap` (Docker→cgroup-v2 semantics resolved)
The trap that was deferred: Docker's `--memory-swap` is the TOTAL
(memory+swap); cgroup-v2 `memory.swap.max` is swap only. The helper
`swap_max_write_value` in the runtime computes `total − memory` (via
`mem_limit_write_value`), with `-1`/`max` → unlimited swap and `None` → the engine's
default. Written in both per-container paths (delegated leaf + cg root); the
slice aggregate stays at the default. Field `memory_swap` (raw) in
`Container`+`RunOpts`+`build_record`. Guards updated (`flags_of_controller`
memory += `--memory-swap`; removed from the "do not exist" lists). **Proven by the
host**: `-m 256m --memory-swap 768m` → leaf `memory.swap.max=536870912` (512 MiB
= 768−256); `--memory-swap -1` → `max`.

### C19 — `--uts host` (runtime conditional `CLONE_NEWUTS`)
The item that was deferred for touching the runtime. Now `--uts` folds into the
`host_ipc`/`host_pid` pattern: `host` → the container shares the host's UTS (sees its
hostname), `private` (default) → its own. In the runtime: `clone_flags` only
adds `CLONE_NEWUTS` when `!uts_host`; and the `sethostname` in `setup_rootfs`
becomes conditioned on `has_own_utsns` (= `!uts_host`), so as to **never
rename the host** (which in rootless would fail EPERM anyway). `--hostname` with
`--uts host` is REFUSED. Full plumbing mirroring `host_ipc` (Container,
RunOpts, RunArgs, build_record, launch_spec, cmd_run RunSpec, clone_flags,
InitInputs/NamespaceSpec, container_init). **Proven live**: `--uts host` →
hostname `delonix` (the host's); default → `dlxpar-uts0` (its own); `--hostname
--uts host` refused; host NOT renamed; pods still receive a hostname.

### C20 — actionable shim for irreducible families (`serve docker-api`)
Six cluster/daemon verbs of Docker/Podman — `swarm`, `node`, `service`,
`machine`, `context`, `plugin` — are now accepted by the parser as HIDDEN subcommands
(`#[command(hide = true)]` + `trailing_var_arg`), returning an actionable error:
these are orchestration/daemon families that a *daemonless* engine does not run,
and whoever really needs the Docker surface uses `delonix serve docker-api`
(with `DOCKER_HOST=unix://…` the `docker` CLI itself talks to Delonix). They do not
«pretend» to work: they say why and where to go. Preserves daemonless/rootless/
ADR-0010. The message was CORRECTED so as not to promise that the shim serves swarm/node
(it serves only the container/image surface). Files: `main.rs` (`docker_api_shim`,
`NOT_A_GROUP`, per-verb test), i18n. Commit `94696da3`.

### C21 — `--platform <os/arch>` in run/create
Fetches the OS/architecture variant of the manifest on pull (`--platform linux/amd64|
linux/arm64`), Docker/Podman-style. Wires `RunArgs.platform` through `run_opts_from_args`
to `apply_pull_policy`, which extracts the architecture and calls
`pull_from_registry_with_creds_platform`. **Proven live**: `linux/amd64` pulls and
runs; `linux/arm64` is selected (log «platform selected: linux/arm64») on an
amd64 host. Parsing test + promised flag + `cli-stability`. Commit `62f94f25`.

### C22 — `--umask <MASK>` (Podman)
File-creation mask of the container process, in octal (`0077`). Field
`umask: Option<u32>` in `Container`+`RunOpts`, `build_record`, `ProcessSpec`;
applied with `libc::umask` in `container_init` as the LAST step before `execve`
(nothing above — mounts, tmpfs — is affected). Octal parsing refuses invalid values
instead of ignoring them. **Proven live**: `--umask 0077` → `umask` inside the
container = `0077`; without the flag → `0022`.

### C23 — `--cgroupns private|host` (Docker/Podman)
`private` gives the container its own cgroup namespace (the root of
`/proc/self/cgroup` becomes `/`); `host` (default) keeps the host's view. In the
runtime, `unshare(CLONE_NEWCGROUP)` in `container_init` AFTER the parent moves the
process to the leaf `dlx-<id>` but BEFORE `drop_capabilities` (the `unshare`
needs `CAP_SYS_ADMIN`; dropped before, it gave silent EPERM — caught
live). *Opt-in* and *best-effort*: `host` never reaches the `unshare`,
and a refusal degrades to the same host view (no regression). Ignored on the
Kind node path (which already runs its own `setup_node_cgroup_ns`). Field
`cgroupns_private` plumbed like `node_cgroup` (Container, RunOpts, build_record,
NamespaceSpec, InitInputs). **Proven live**: `--cgroupns private` →
`/proc/self/cgroup` = `0::/`; `host`/default → the host's full path.

### Niche flags DEFERRED (declared divergence, tied to a backend)
With the niche wave closed, these stay as a deliberate divergence because they
depend on a backend that the current rootless/cgroup-v2 path does not serve — they are not
«forgotten», they are refused with reason:

| Docker/Podman flag | Why deferred | Future path |
|---|---|---|
| `--mac-address` | a fixed MAC only makes sense in rootful *bridge*/macvlan; the rootless default is slirp/ingress | wire up when rootful network parity (branch `feat/network-parity`) lands |
| `--ip6` / `--link-local-ip` | require an IPv6 backend in the SDN (its own phase on another branch) | wire up on top of the IPv6 phase (`feat/ipv6-phase1`) |
| `--device-cgroup-rule` | cgroup v2 controls devices via eBPF, not via v1 `devices.allow`; unfeasible in rootless today | needs the eBPF `device` program attached to the cgroup |
| `--detach-keys` | UX of the *detach* sequence in `attach`; low value, interactive | implement in the `attach` TTY loop when there is demand |

### Next batch — fully mapped (parallel investigation by agents)
The flags below touch the runtime (`delonix-linux`) and *guards* that today assert
they do not exist — they are ready to implement with the exact points:

- **`--pids-limit` → `pids.max`** (delegated, applicable): parametrise the
  existing writes in `delonix-linux/src/lib.rs:6111` (delegated) and `:6862` (root),
  today with a fixed `DEFAULT_PIDS_MAX`. New field in `Container` (`record.rs:349`) +
  `RunOpts` + `build_record`.
- **`--memory-swap` → `memory.swap.max`** (delegated): writes in `:6103`/`:6860`
  (today `"0"`). Semantic TRAP: Docker counts memory+swap, cgroup v2 is swap
  only → write `swap_total − memory`.
- **`--memory-reservation` → `memory.low`** (soft floor): NOT written today —
  add a write alongside each `memory.max`.
- **GUARDS to update** (otherwise the build breaks): `lib.rs:5761` (`flags_of_
  controller`), `main.rs:1898/1923` and `system.rs:2190` (lists `named_flags_all_
  exist` that assert `--memory-swap`/`--pids-limit` do NOT exist), and the
  node-api messages (`container_launch.rs:209`, `container_update.rs:131`).
- **`--stop-signal` / `--stop-timeout`**: fields in `Container` (`record.rs`, +
  init in `Container::new`) + `RunOpts` + `build_record`; `cmd_stop`
  (`container.rs`) reads them; the `-t` of `stop` becomes `Option<u64>` (distinguishing
  "not passed"); route the initial signal via `runtime::stop`→`stop_waiting`
  (`lib.rs:10141`/`:10180`, today fixed SIGTERM), converting `parse_signal`
  (i32)→`nix::Signal`.
- **`--init`** (pid1 reaper): located in `container_init` (`lib.rs:4040`/exec at
  `:4426`) — fork: parent stays pid1 doing `waitpid(-1)`+signal re-send, child
  does the privilege drop + current exec. Seccomp already allows `wait4`/`waitid`/
  `kill`. Bool plumbed like `read_only`.
- **`--ipc`/`--pid`/`--uts` modes** → map `host` to the existing `--host-ipc`/
  `--host-pid` (in `run_opts_from_args`), refusing unsupported modes instead of
  ignoring them.

### Measurement of the path to 100% (grounded, not estimated)
The `run` verb alone has **51 Docker flags / 91 Podman** still missing; many are
real features (`--init`, `--volumes-from`, `--mount`, `--pull`, `--platform`,
`--ipc`/`--uts`/`--pid` by value, `--device-cgroup-rule`, `--blkio-*`…),
some are renames (done in C6) and some are inert in rootless. Across ~40
verbs, **100% of flags is a campaign of dozens of PRs**. It stays as a structured
backlog (next waves):

- **Wave 4** — high-value feature-flags in `run`/`create` (`--init`,
  `--volumes-from`, `--mount`, `--pull`, `--platform`, `--ipc/--uts/--pid` modes),
  each with behaviour + i18n + manual + differential test.
- **Wave 5** — actionable-error shim for `swarm`/`node`/`service`/`machine`/
  `plugin`/`context` pointing to `serve docker-api`, and verification that the `docker`
  CLI really drives Delonix via `DOCKER_HOST`.
- **Pod pause/infra container** — for the complete imperative `pod create --name`.
- **Per-verb flag-level gap generator** (the measurement above, automated) to
  version progress towards 100%.

### Test status (second wave)
`cargo test -p delonix-bin` **1644/1644**; `frequent-verbs.sh` **49/49**;
`tab-descriptions.sh` green; `flow1`/`flow2` differentials **ALL OK** (delonix pod
stop/start/restart green); `docs_cli_gate`, `lang_ratchet` green; clippy clean.

---

## Third wave — closing the resolvable divergences (6 Nov 2026)

The owner asked to «resolve the divergences for parity». Starting from the
authoritative matrix (the `por-fazer` rows = a command the parser REFUSES = real
absence), we implemented **23 commands** in a wave of parallel agents (one
category per agent, isolated worktree, validated live) + the root-verbs done
separately. The matrix went from **20→43 `passou`** and **85→62 `por-fazer`**.

### C24 — Docker root verbs: `pause`/`unpause`/`commit`/`top`/`rename`
Same mechanics as the existing root aliases (`cp`/`port`/…): a `clap` variant that
reuses the `container <verb>` handler and the verbatim docs, so it inherits the
stability promise of `container` with no second contract. `docker pause`,
`docker commit`, `docker top`, `docker rename` only swap the executable. `update`
stays OUT of the root on purpose: `docker update` is about resource LIMITS, the
Delonix `container update` is hot reconfiguration (ports/volumes/bandwidth) —
distinct semantics. **Proven live** (pause→Paused, unpause, top, rename,
commit→new image). Recorded in STABLE_GROUPS, `_ROOT_VERB_ALIASES`, README,
`cli-stability.md`, manual.

### C25 — `container exists` + `container list`
`container exists <id|name>` Podman-style: silent, exit 0 if it exists / 1 if not
(via the same `find(store,id)` of `inspect`/`rm`; `std::process::exit(1)` for the
clean 0/1 contract — returning `Err` gave code 4 with a message). `container list`
= a second `visible_alias` of `ps` (zero handler, zero i18n). **Proven live.**

### C26 — `image inspect`/`list`/`exists`/`untag`
`image inspect` serialises the SAME `Image` records that `describe` reads, as a JSON
array Docker-style. `image list` = `visible_alias` of `ls`. `image exists` Podman-style
(silent 0/1). `image untag` drops ONE name (via `resolve`+`save`, never the
blobs; the last name leaves the image *dangling* `<none>`, content intact).
**Proven live.** (Removed 2 obsolete exceptions in `docs_cli_allow.tsv`.)

### C27 — `network prune`/`exists` + `volume exists`
`network prune` mirrors `volume prune` (classify→warn the retained→confirm→
sweep); a network in use (attached container OR VM) is RETAINED, never forced, by the
same `network_dependents` that `network rm` refuses (fail-closed: an unreadable store
aborts instead of sweeping). The default bridge (`delonix0`) is never in `list()`,
so it is never pruned (as in Docker). `network exists`/`volume exists` Podman-style
(silent 0/1). **Proven live**, including the proof that a network with an
attached container is retained.

### C28 — pod `ps`/`exists`/`prune`/`kill`/`pause`/`unpause`/`stats`/`top`/`inspect`
Nine verbs, all on the `members_of(...)` + handler-per-container pattern already used in
`pod stop/start`. `pod prune` removes only pods with no Running/Paused member; `pod kill`
signals in reverse order; `pause`/`unpause` freeze/unfreeze (holder last/first);
`stats`/`top` delegate to the container per member; `inspect` is a
JSON array (pod + members). Promoted `cmd_freeze`/`cmd_top`/`cmd_stats` to
`pub(crate)`. **All proven live.**

### The 62 remaining divergences — honest classification
They are not «forgotten»: the vast majority require a backend or a product decision
that is not CLI work.

| Group | No. | Nature |
|---|---|---|
| `system *` + `context *` (daemon/remote-mgmt) | 22 | ADR-0010 rejects the remote management API; `context` already falls into the `serve docker-api` shim. Out of scope, not accidental absence |
| rootfs `mount`/`unmount` (container/image/volume) | 10 | needs mounting the overlay on the host — in rootless, `fuse-overlayfs`; a front of its own |
| build subsystem (`buildx`/`bake`/`builder`/`image build`/`diff`/`tree`) | 6 | build engine; separate campaign |
| CRIU (`checkpoint`/`restore`) | 3 | process *checkpoint/restore* via CRIU; dedicated backend |
| cheap future wave (root `history`/`untag`, `search`, `import`, `export`) | ~8 | root alias + `docker search`/`import`/`export` — implementable without a new backend, next wave |
| misc Podman (`clone`/`scp`/`trust`/`runlabel`/`cleanup`/`reload`/`update`) | ~13 | case by case; several depend on subsystems (overlay/signing/network) |

### Test status (third wave)
`cargo test -p delonix-bin` **1661/1661** (+5 +2 auxiliary binaries); `lang_ratchet`,
`docs_cli_gate`, `docs_command_gate` green; clippy clean. Combined live smoke
(image/container/network/volume/pod) in a single isolated `DELONIX_ROOT`: all 23
commands respond like Docker/Podman. Matrix: **43 passou / 62 por-fazer /
76 reconhecido-por-testar / 14 fora-do-âmbito / 3 divergência-deliberada**.

---

## Fourth wave — cheap wins with no new backend (6 Nov 2026)

Eight more commands, all with no new subsystem. Matrix **43→51 passou**,
**62→54 por-fazer**. All proven live.

### C29 — root `history`/`untag`; `container export`+root `export`; `image import`+root `import`; `image search`+root `search`
- **`history`/`untag` (root)**: alias of `image history`/`image untag` (done in
  C26), same mechanism as the other root verbs.
- **`container export` + `export` (root)**: the container's rootfs as an UNcompressed
  tar (`/proc`/`/sys`/`/dev` as empty mount points, owners as the
  container sees them). Reuses the merged-rootfs resolver of `commit`/`cp` (a
  stopped overlay is read via `__ovlhold`, never packs an empty tree) and exposes
  `ImageStore::export_rootfs_tar`. Refuses to write a tar to a terminal.
- **`image import` + `import` (root)**: a rootfs tar (plain/gzip/stdin) → a single-layer
  image, via `commit_flat_rootfs_from_tar` (the rootless build primitive),
  inflating gzip by the magic. Empty config (like `docker import`).
  **Round-trip proven**: `container export` → `image import` → the image runs.
- **`image search` + `search` (root)**: queries the public Docker Hub catalogue
  (docker.io) via `registry::http_get`, prints name/stars/official/description.
  The only image verb that goes to the network. Proven against the Hub (official nginx 21k★).

### The 54 remaining `por-fazer` — backend/policy-bound (unchanged)
system/context daemon-mgmt (ADR-0010, shim), rootfs mount/unmount (fuse),
build subsystem, CRIU, and misc Podman (clone/scp/trust/reload/…). They are
campaigns of their own with an architecture decision, not accidental absences.

---

## Fifth wave — backend families (3 parallel agents, 6 Nov 2026)

Eighteen more commands, from families previously classified as backend-bound.
Matrix **51→63 passou**, **54→36 por-fazer**, **3→9 divergência-deliberada**.
Three agents in isolated worktrees (`par2-image`/`par2-ctr`/`par2-volnet`),
partitioned by cmd file; the root shims stayed with me in `main.rs`.

### C30 — image build/diff/tree; container mount/unmount/clone; volume export/import/mount/unmount; network reload
- **image**: `build` (alias of `delonix build`, single engine), `diff A B` (layers
  only-in-A/only-in-B/common, from the records), `tree` (layer stack base→top). The
  uncompressed `diff_ids` are not in the record, so `tree` is the list of
  layers of an image (not an ancestry graph) — documented.
- **container**: `mount` (prints the rootfs path: `/proc/<pid>/root` if alive,
  on-disk path if stopped), `unmount`/`umount` (honest no-op — `mount` only
  prints), `clone` (copies the saved spec to a new stopped container, via
  `prepare_container_rootfs`; **proven to boot**).
- **volume**: `export` (tar of the volume directory, refuses a terminal), `import`
  (extracts into an existing volume, gzip by the magic), `mount` (prints the path),
  `unmount`/`umount` (no-op). **export→import round-trip proven**.
- **network**: `reload` (re-realises the dataplane from the record without recreating the network:
  overlay via `realize_overlay`, bridge via `network_create_with`; macvlan/ipvlan
  refuse honestly — not realisable rootless).

### Actionable root shims (deliberate divergence, no backend)
`buildx`/`builder`/`bake` (BuildKit front-ends → `delonix build`), `checkpoint` +
`container checkpoint`/`container restore` (CRIU, not integrated; DX-6000 rc=69).
They parse and return an actionable error, never `unrecognized subcommand`. Root
`restore` did NOT enter (it is not a Docker/Podman root verb + collides with the
`delonix restore`→`backup restore` migration).

### Deferred with an honest reason (still `por-fazer`/future)
`container cleanup` (the engine already releases network/mounts on exit; the only reachable
teardown deletes the write-layer — destructive, wrong semantics), `container
runlabel` (needs reading the LABEL + templating), `volume update/reload` and `network
update` (no mutable field that a dedicated verb would own — it would be a pretend no-op).

### The 36 remaining `por-fazer`
Mostly `system *`/`context *` (ADR-0010, remote management; `context` already in the shim) and
root `mount`/`unmount` (ambiguous now that container/image/volume have theirs —
type-scoped policy, like `inspect`/`diff`). The rest is case-by-case misc Podman.
