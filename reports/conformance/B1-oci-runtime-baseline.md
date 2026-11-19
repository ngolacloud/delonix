# B1 — OCI Runtime Spec Baseline (Track B of the conformance plan)

**Date:** 2026-10-24 · **Binary:** `delonix 5.1.1 @ ee615f3b` · **Validator:** `oci-runtime-tool`
`v0.9.1-0.20261017-bd3b0bd` (from `@master`; `@latest`/v0.9.0 does not compile under Go 1.25) ·
**runc:** 1.5.2 · rootless, host kernel 7.0, cgroup v2.

## Method

1. `delonix image export alpine <bundle>` → `config.json` + `rootfs/`.
2. `oci-runtime-tool validate --path <bundle>` (structural + semantic validation against the runtime-spec).
3. `runc --root <tmp> run` of the bundle, as-is, rootless (interop proof).

Reproducible; no upstream suite was installed on the host (as `oci-conformance.md` already noted).

## Result

| Check | Result |
|---|---|
| `config.json` structure vs runtime-spec | **CONFORMANT** — `Bundle validation succeeded` (with the version string at 1.3.0) |
| `runc run` of the exported bundle, rootless, as-is | **FAILS** — `rootless container requires user namespaces` |

`config.json` emitted: `ociVersion 1.0.2`, `process.user {0,0}`, namespaces
`pid/network/ipc/uts/mount/cgroup` (**no user**), `capabilities.bounding` = 6 (reduced set),
**no `uidMappings`/`gidMappings`**, **no `seccomp` section**.

## Gaps found (entering Track B)

| ID | Gap | Impact | Effort |
|---|---|---|---|
| **GAP-RT-1** | emitted `ociVersion` is **1.0.2** (current spec 1.2/1.3); tools that pin the current version reject the bundle | modern OCI tools do not validate the bundle without a manual bump | low — stamp the current version + newly required fields |
| **GAP-RT-2** | export **does not emit `uidMappings`/user-ns** — bundle is root-only; rootless `runc run` fails | the bundle is not portable to a rootless runtime (runc/crun) without editing | medium — `image export --rootless` that injects the subuid mapping |
| **GAP-RT-3** | export **does not carry `seccomp`** — Delonix's profile does not travel to the bundle; a `runc` of the bundle runs without seccomp | loss of hardening when leaving the engine | medium — serialize the profile into the config |
| **GAP-RT-4** | Delonix **does not expose a runc-style CLI** (`create/start/state/kill/delete <bundle>`) — it is a producer of bundles + its own init, not a drop-in runtime | cannot be measured by the `runtime-tools` *validation suite* nor replace runc/crun directly | high — a `delonix oci-runtime` shim over the init |

## B1 conclusion

- **Positive:** the shape of the bundle Delonix produces is **structurally conformant** to the runtime-spec
  (the validator only tripped on the version string).
- The **behavioural conformance of Delonix's own runtime** remains covered by E2E/chaos,
  **not** by `runtime-tools` — because the latter requires a runc-style CLI that Delonix does not have (GAP-RT-4).
  Closing GAP-RT-4 (an `oci-runtime` shim) is what would allow running the upstream *validation suite*
  against Delonix itself and giving a number comparable to runc/crun's.
- Next in Track B: **B2 (mount propagation `rshared`/`rslave`)**, which also closes 3 CRI failures.

## Repro

```bash
export PATH=$PATH:/usr/local/go/bin
go install github.com/opencontainers/runtime-tools/cmd/oci-runtime-tool@master
delonix image export alpine /tmp/b && (cd /tmp/b && ~/go/bin/oci-runtime-tool validate --path .)
cd /tmp/b && runc --root /tmp/rr run delonix-oci   # rootless → "requires user namespaces"
```
