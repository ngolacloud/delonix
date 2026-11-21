# OCI conformance — measured against `fa8c6297`

Isolated state (`DELONIX_ROOT=/tmp/dxo`), real registry `registry-1.docker.io`, 2026-09-15.
**No upstream suite was run** (the OCI runtime/image suites are not installed on the
host) — the results below are directed tests, not certified conformance.

## OCI Image / distribution

| Requirement | Test | Result | State |
|---|---|---|---|
| Pull by tag | `image pull alpine:3.20` | rc=0, `bf8527eb54c3` | PASS |
| Pull by correct digest | `alpine@sha256:d9e853e8…` (index digest read from the registry) | rc=0, same content | PASS |
| Nonexistent digest | `alpine@sha256:000…0` | rc=4, nothing installed | PASS |
| Tag + digest in conflict | `alpine:3.19@<3.20 digest>` | rc=0, **writes `alpine:3.19` with 3.20 content, no warning** | FAIL (G-033) |
| Platform selection | `platform selected: linux/amd64` on all pulls | multi-arch index resolved | PASS |
| Nonexistent repo | `library/does-not-exist-audit-xyz:1` | rc=1 (registry 401) | PARTIAL — class 4 expected |
| Malformed reference | `alpine::bad`, `Alpine/UPPER:1`, `alpine@sha256:abc` | sent to the network; rc=4/4/1 | PARTIAL (G-034) |
| Manifest vs pinned digest verification | code reading of `verify_manifest_digest` (audit #3) | present | NOT RE-TESTED (no tampered registry) |
| `image load` current docker format | archive `blobs/sha256/*` + `manifest.json` | loaded | PASS |
| `image load` legacy format | `layer.tar` + `manifest.json` | rc=74 `I/O error: No such file or directory` | PARTIAL (G-039) |
| Path traversal in the layer (`../`, absolute, symlink→/tmp) | `container run` of the malicious image | **refused** «trying to unpack outside of destination path»; nothing written outside the root | PASS |
| Cleanup after refused extraction | 4 attempts | 4 orphaned `layers/.*.tmp`; `image prune` does not remove them | PARTIAL (G-035) |
| Signed images (`image verify`/`--verify`) | — | — | NOT TESTED |
| OCI bundle import/export | — | — | NOT TESTED |
| Archive/decompression bomb | — | — | NOT TESTED |

## OCI Runtime (code reading, see `architecture-inventory.md §4`)

| Requirement | State |
|---|---|
| create/start/kill/delete lifecycle | PASS via E2E (729 checks) |
| namespaces / capabilities / seccomp / no_new_privs | PASS via E2E + chaos |
| masked/readonly paths | PASS — a mount failure on a path that EXISTS refuses the container and names it; absent path ignored by decision (G-009) |
| mount propagation | PARTIAL — critest fails on the environment, not exercised in the engine |
| rro | UNSUPPORTED (G-010) |
| hooks | UNSUPPORTED (G-019) |
| rlimits | PASS — unknown name refused with the list of accepted ones; `setrlimit` error propagated (G-009) |
| exit status | PASS (E2E: wait, 137, real codes) |
