# B1b — OCI runtime-spec validation suite, measured against Delonix itself

**Date:** 2026-11-20 · **Binary:** `delonix 0.1.2` (`oci-runtime`, ADR-0080) ·
**Suite:** `opencontainers/runtime-tools` validation suite
`v0.9.1-0.20261017090856-bd3b0bd00f44` · **Reference runtime:** `runc 1.5.3` ·
rootless, host kernel 7.0.0-43, cgroup v2.

This closes the open item left by [B1](B1-oci-runtime-baseline.md): *"Closing GAP-RT-4
(an `oci-runtime` shim) is what would allow running the upstream validation suite against
Delonix itself and giving a number comparable to runc/crun's."* GAP-RT-4 is now closed
(`delonix oci-runtime create/start/state/kill/delete`, ADR-0080), so for the first time the
upstream suite ran against Delonix — not only against the reference runtime.

## Method

The suite drives a runc-style CLI (`create --pid-file F --bundle D <id>`, `start`, `state`,
`kill`, `delete`). Delonix is measured through a thin translation wrapper
([`scripts/oci-runtime-rt4-wrapper.sh`](../../scripts/oci-runtime-rt4-wrapper.sh)) that maps
that contract onto `delonix oci-runtime …` (it strips `--pid-file`, which Delonix's `create`
does not take, and synthesises the file from `state` the way runc writes it on create;
it injects the per-subcommand `--root`). Both runtimes were run by the SAME harness
([`scripts/runtime-tools.sh`](../../scripts/runtime-tools.sh)) in the SAME mode (rootless),
so the numbers are directly comparable — as the harness header requires.

```sh
export PATH="$PATH:/usr/local/go/bin" GOFLAGS=-mod=mod
# reference
RUNTIME=runc ./scripts/runtime-tools.sh
# delonix, via the RT4 wrapper
DELONIX_BIN="$PWD/target/debug/delonix" \
  RUNTIME="$PWD/scripts/oci-runtime-rt4-wrapper.sh" ./scripts/runtime-tools.sh
```

## Result — parity with the reference runtime (rootless subset)

| Runtime | Suite files | Assertions `ok` | Assertions `not ok` | Per-file verdicts |
|---|---|---|---|---|
| `runc 1.5.3` (reference) | 58 | **15** | 26 | baseline |
| `delonix 0.1.2` (RT4) | 58 | **15** | 26 | **file-by-file identical to runc** |

The per-file verdict tables of the two runs are **byte-for-byte identical** once the
per-run temp path is normalised (see the two logs under
[`evidence/`](evidence/)). Delonix reproduces the reference runtime assertion-for-assertion
on everything the rootless environment can measure.

**What the 15 `ok` assertions are** (the privilege-free part of the spec):

- `validation/linux_ns_path` — 14 assertions: a runtime MUST honour a namespace `path` of the
  correct type and MUST error on a path of the wrong type (`/proc/self/ns/*`). Delonix matches.
- `validation/state` — 1 assertion: `state` MUST error when given no container ID. Delonix matches.

**Why the 26 `not ok` are not a Delonix gap.** They are the cases that require privilege —
creating cgroups, user/pid/net namespaces, mount propagation, seccomp, device nodes, sysctls,
rlimits, hooks. In a rootless shell they fail for `runc 1.5.3` **exactly as they fail for
Delonix** (identical failing-file set). This is a property of the *environment*, not of the
runtime — the same caveat `critest.sh` and the B1 baseline already carry. Measuring the full
green suite requires root and belongs to a VM run (future H3 `root` line), not this host.

## Conclusion

- Delonix's `oci-runtime` CLI now sits where runc/crun sit and is **measured by the upstream
  OCI runtime-spec validation suite**, reaching **parity with `runc 1.5.3`** on the complete
  rootless-measurable subset (15/15 of the assertions the environment can exercise).
- This is the behavioural counterpart to B1's structural result (B1 also noted the emitted
  `config.json` is now `ociVersion 1.3.0`, closing GAP-RT-1's version-string snag).
- **Scope / limit:** this is the *rootless* tier. A full-green run of the privileged cases
  (cgroups/namespaces/mounts/seccomp) is a separate root/VM measurement and is NOT claimed here.

## Reproduce

Re-run either command above; the summary line `== runtime-tools: N ok / M not ok ==` is the
figure, and `diff` of the two `prove` "Test Summary Report" sections (temp path normalised)
is empty. Raw TAP for both runs is preserved under
[`evidence/`](evidence/).
