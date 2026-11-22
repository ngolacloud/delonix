# Release verdict — `fa8c6297` (v3.1.0 + 26)

## Verdict: **LAB READY**

With a documented exception, the **rootless CLI path for containers/stack on a single node** is at
**PILOT READY**: green quality baseline, E2E 729/1 (harness failure), chaos 29/0, fail-closed storage.

## §22 gates

| Gate | State |
|---|---|
| No open P0 | ✔ none confirmed (G-007 to be reproduced) |
| P1 resolved or with approved exception | ✘ 9 P1 open, 4 confirmed live |
| Rootless and rootful with current evidence | ✘ rootless only |
| Upgrade and rollback tested | ✘ NOT TESTED |
| Security audit completed | ✘ partial (this document) |
| Current CRI/OCI/Docker API/Compose matrices | ◐ CRI, Docker API and Compose measured; OCI driven, no upstream suite |
| Chaos and failure injection | ✔ chaos 29/0 + G-008 injection + storage |
| SBOM and provenance present | ✔ published; ✘ not verified in this session |
| Signed artefacts | ✔ minisign (not verified) |
| Documentation matches behaviour | ✘ G-025, CLAUDE.md out of date |
| Limitations published | ◐ partial |
| Results tied to the same commit | ✔ `fa8c6297` (origin/main already at `b19fddcb`) |

## By workload

| Workload | State |
|---|---|
| Rootless containers via CLI/`stack apply`, default network, local volumes | PILOT READY |
| `--secret-files` | BLOCKED until G-007/G-026 |
| Per-namespace multi-tenant isolation as a security boundary | BLOCKED until G-008 |
| Kubernetes via `delonix-cri` | DEVELOPMENT READY |
| Tooling via the Docker socket (Compose, Testcontainers, Dev Containers) | NOT READY |
| libvirt VMs | LAB READY |
| Cloud Hypervisor VMs | LAB READY |
| Proxmox / OpenStack VMs | NOT READY |
| Rootful / SELinux | NOT ASSESSED |

## Recommended next release

**3.1.1** (security/reliability patch): G-008, G-007/G-026, G-029, G-003, G-030, and confirm the
fixes already in `origin/main` (`4a4d029f` OOMKilled, `58fdc4e3` pause, Proxmox `a9c41cb9`).
Exit criterion: E2E 0 FAIL, chaos 0 FAIL in CI (not skipped), critest re-measured against the tag commit.

## Rollback

No change was applied to the product. For 3.1.1: reinstall the signed 3.1.0 binary; since
G-008 runs in the holder, the rollback requires `delonix net netns down/up` during a maintenance
window (containers are recovered on restart).
