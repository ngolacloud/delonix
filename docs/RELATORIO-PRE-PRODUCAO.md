# Pre-production report — `delonix` CLI

> **Re-run on 2026-08-10, on v0.47.0: 198 PASS · 0 FAIL · 0 SKIP.** The battery had not
> run since v0.3.0 and had nine failures — **eight of them because the test encoded
> a bug that had since been fixed**: it used `--subnet …/24`, which the engine ACCEPTED
> and silently discarded with the bridge driver until v0.48.0 gave it meaning. It's the same
> trap the compose `default_project_name` had already sprung, and the lesson is the
> same: when a fix makes an old test fail, the first hypothesis to test is that the test was
> pinning the wrong behavior. The ninth was a guard comparing the REPOSITORY when the
> next command used the TAG (`alpine:latest` in the store, `alpine:3.19` absent → the
> `grep alpine` passed and `describe` failed).
> Run in an isolated sandbox (its own `DELONIX_ROOT`/`DELONIX_NET_RUNTIME_DIR`),
> without touching this host's production containers. The rest of this document is the
> ORIGINAL report, from 2026-07-17, and stands as-is: it is a dated record.

**Date**: 2026-07-17 · **Binary**: `delonix 0.3.0` (local build, `target/debug`)
**Battery**: [`scripts/e2e.sh`](../scripts/e2e.sh) — 140 checks against the real CLI, with
real containers/networks/volumes. **Result: 139 PASS · 1 FAIL · 0 SKIP.**

`e2e.sh` covers: `--help` for every group **and every subcommand** (discovered by parsing
the help itself, so it cannot go stale on its own), every read command, a block of
**expected errors** (the CLI must REFUSE, not silently accept), volume/network/container
lifecycle, hot reconfig on both network paths, and `stack apply`/`describe`.

> **A rule that cost hours in other sessions and remains valid**: never test with the `delonix`
> on the `PATH`. Long-running processes run the binary they were BORN with, and an old binary in
> `~/.local/bin` sabotages tests of new code. `e2e.sh` uses the local build by default.

---

## 1. Bugs found and FIXED in this session

| # | Severity | Bug | Status |
|---|---|---|---|
| 1 | **CRITICAL** | **Every hot mount was failing.** `mount_live`/`unmount_live` gated the `setns(user)` on `container.userns`, but that field says whether the container **created** its own userns — rootless ingress containers **inherit** the holder's and end up with `userns=false` even though they are in a different userns. Without the setns, the following `unshare(NEWNS)` returned EPERM (code 124). This had never surfaced because these functions **had no caller at all**. | Fixed + validated with a real container |
| 2 | **HIGH** | **`stop && start` always failed.** `unpublish_ports` did a `return` on the slirp-per-container path, asserting that "slirp dies with the container's netns". That is false: slirp only exits once it NOTICES the netns has disappeared, and until then it holds the port. Measured: **0/3 startups** before, **5/5** after. | Fixed (`reap_slirp_for`, surgical and synchronous) |
| 3 | **HIGH** | **The regression tests for the 4 CRITICAL security findings were DEAD.** `cargo test -p delonix-runtime-bin` had not compiled for several commits (26 errors in `cmd/cluster.rs`: the specs gained fields and the tests did not keep up). AGENTS.md claimed these tests existed and ran. | Fixed — 61→72 tests running |
| 4 | **MEDIUM** | **`ssh @host` instead of `ssh delonix@host`.** `SshSpec` derived `Default`, so a `cloud.yaml` **with no `ssh:` block** produced `user: ""` (the `#[serde(default = "…")]` only kicks in when the block exists). | Fixed + 2 regression tests |
| 5 | LOW | `image ls` showed **"1 weeks ago"** for 7–13 days (new code from this session). | Fixed — ports docker's `HumanDuration` verbatim |

---

## 2. ⚠️ MAIN FINDING — published ports die on their own (**and it's not the runtime's fault**)

**This is blocker #1 for production, and it had been misdiagnosed for several sessions.**

### Symptom
A published port works (HTTP 200) and **~10–16s later stops serving**, with the container
still `Running` and no one having done `stop`/`rm`.

### What was documented (and is WRONG)
`AGENTS.md` said *"the two halves of `publish_port` (slirp_add_hostfwd + control_send) fail
SILENTLY"* and directed the search toward whoever called `unpublish_port`. **Both premises are
false** — that's why the investigation never closed.

### What was PROVEN, by measurement
1. **The DNAT half is always intact** — `nft list table ip dlxing` shows the rule on the
   `pre` chain long after `curl` already returns `000`. Only slirp's `hostfwd` disappears.
2. **`publish` WORKS** — `list_hostfwd` shows the entry and `curl` returns 200. It does not fail silently.
3. **No code in `delonix-runtime` removes it.** I instrumented `unpublish_port`,
   `slirp_remove_hostfwd`, **every** non-`list` command of `slirp_api` (which catches the
   `remove_hostfwd` that `reap_orphan_hostfwds` sends directly, bypassing the named functions)
   and `control_send`. **Zero occurrences**, across every reproduction.
4. **slirp does not restart** and the **holder does not restart** (same pid). The holder's
   `control_loop` is purely reactive — it has nothing periodic.
5. **A hostfwd added BY HAND through the api-socket, with no delonix involved at all, also
   disappears** (16.51s). So it is not our publish path.
6. **It is not a slirp4netns bug** — a clean-room `slirp4netns`, with **exactly the same flags**,
   against an `unshare -r -n` target, kept the hostfwd for the full 33s.

### Root cause (proven with SIGSTOP)

**It's `delonix-engine` — the private PaaS — reaping the runtime's ports.**

```
engines RUNNING   →  hostfwd created at t=0.00s  ·  DISAPPEARS at t=12.01s
engines FROZEN    →  hostfwd created at t=0.00s  ·  PERSISTS for all 30s
(SIGSTOP/SIGCONT, nothing killed; engines resumed normally)
```

Two factors adding up, both in `delonix-paas`:

1. **`crates/delonix-api/src/ui.rs:12937`** — `delonix_net::infra::reap_orphan_hostfwds(&live)`.
   `live` only contains the **PaaS's own** containers. Every port published by the runtime's CLI
   is, to the engine, an "orphan" to clean up. It is reaping state that is not its own.
2. **`crates/delonix-api/Cargo.toml:15`** — `delonix-net = { git = "…delonix-runtime", tag = "v0.1.0" }`.
   The PaaS is pinned to **tag v0.1.0**, i.e. the OLD version of the reaper (the fail-open bug:
   empty list ⇒ concludes nothing is in use ⇒ deletes everything). That's why removing the caller
   in `delonix-runtime` (commit `9bbbd11`) fixed nothing — the copy that runs is a different one.

This explains everything that didn't add up: why the DNAT survives (the reaper only touches
hostfwds), why the trace never fired (the remover is a different process, from a different
product), and why a previous session's "clean slate" didn't help (the engine never entered the
cleanup path).

### Recommendation
**This is not `delonix-runtime` work** — the fix lives in `delonix-paas` and **I did not make
it** (isolation rule: one worktree, one product). Suggestions, in order:

1. **`delonix-paas`**: the engine must not reap hostfwds it did not create. Either `live` starts
   including everything published on the shared ingress, or the reaper stops running at all (that
   was the decision taken in the runtime, in `9bbbd11`: reap **reactively and surgically**, only
   the port that fails, only when it fails).
2. **`delonix-paas`**: bump the `delonix-net` pin from `v0.1.0` to the current version.
3. **`delonix-runtime`** (defense in depth): `reap_orphan_hostfwds` is now **dead code** —
   zero callers. A public function that deletes shared state and fails OPEN on an empty list is a
   trap for any consumer. **Delete it**, or make it fail closed.

---

## 3. Open gaps and divergences (not fixed)

| # | Sev. | Finding | Note |
|---|---|---|---|
| 1 | MEDIUM | **`ssh: { port: 2222 }` is accepted and ignored** — `SshSpec.port` is parsed and never read (`SshTarget` doesn't even have the field). SSH always goes to 22. | Decide: wire it into `conn_args()` or drop it from the schema. It's the **only FAIL** I deliberately left red in e2e. |
| 2 | MEDIUM | **`vm rm <nonexistent>` returns 0** and prints the name, as if it had removed something. `container rm` errors; docker errors. | `delonix_vm::remove` is idempotent on purpose (cleans up remnants with no record, serves `cluster delete`). Changing the CLI without `--force` would break that path. **Your call.** |
| 3 | MEDIUM | **There is no way to reload the supervisors' binary.** A long-running process runs the code it was born with — in production there is no way to apply a fix without recreating containers. | A real operational problem, not a test one. |
| 4 | LOW | `Container` stores neither `finished_at` nor exit code. STATUS cannot say `Exited (0) 2 minutes ago` like docker — only `Exited (0)`. | I preferred showing less over fabricating a time from `created_unix`. |
| 5 | LOW | `vm describe <nonexistent>` errors with **"no such container"** — wrong noun, comes from the VM store. | |
| 6 | LOW | `image ls` SIZE = sum of the blobs in the CAS = **compressed size on disk**. Doesn't match `docker images` (which shows the decompressed rootfs). | Documented in the code; the numbers are real. |
| 7 | LOW | `ImageConfig` doesn't parse OCI's `ExposedPorts` → `image describe` doesn't show them. | Needs a change in `delonix-image`. |
| 8 | LOW | `image --vm pull` loses metadata (`Ubuntu`/`K8s` come back `<unknown>`). | Already known. |
| 9 | LOW | Orphan network named **`dlx-`** (empty suffix) in the store. | Looks like a truncated name from an old bug. |
| 10 | LOW | `cmd_rm` duplicates `remove_container`'s logic. | They will diverge the day one of them changes. |
| 11 | INFO | **`--net-connect`/`--net-rate` require `--net <network>`** — the veth and the shaping live in the holder's netns, which the `--net host/none` path doesn't have. | By design. Explicit error. |
| 12 | INFO | **`--publish-add` on a container created without `-p` AND without `--net <network>`** is impossible: slirp's api-socket is only opened when `run` carries ports. | By design. An error that teaches. |

The gaps already recorded in `AGENTS.md` and not re-tested here remain valid: ingress refcount
leak, orphan rootfs filling up disk, `image --vm pull` with no metadata.

---

## 4. What was properly validated

**Hot reconfig — with a real container, no restart, same PID start to finish:**

- `--publish-add` → HTTP **200** on the new port, live.
- `--publish-rm` → the port closes; the container's other ports stay intact.
- `--volume-add` → mounted live; a file written **inside** the container shows up in the
  host volume; `--volume-add …:ro` **refuses** writes and allows reads.
- `--volume-rm` → stops being a mountpoint inside the container.
- `--net-connect`/`--net-disconnect`/`--net-rate`/`--net-rate-clear` → all PASS through ingress.
- Every expected error **is refused** (duplicate port, duplicate target, network already
  connected, invalid rate, stopped container).

**Verdict**: this session's features are production-ready. What **blocks production is not in
this repo** — it's in `delonix-paas`'s reaper (section 2), and it's a one-line policy fix in a
product that is not this one.
