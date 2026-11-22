# Security assessment — `fa8c6297`

Not a full pentest. It is the threat model of the requested boundaries, cross-referenced with what was
**measured** in this session and what was merely **read** in the code.

## Boundaries and result

| Boundary | Measured | Result |
|---|---|---|
| Registry / pull | nonexistent digest refused; correct digest honoured; tag+digest in conflict writes the wrong label | PASS + G-033 |
| Archive extraction | `../`, absolute path and symlink→/tmp refused, nothing outside the root | PASS (G-035 residue) |
| Build context / Dockerfile | not re-tested (fixed and tested in earlier audits) | NOT TESTED |
| exec right after `run -d` (race against the host) | the E2E window and registration gates passed | PASS |
| Bind mounts / devices / GPU | covered by E2E; GPU without hardware | PARTIAL |
| Network — anti-spoof | **fails open** with nft failing (injection) | FAIL (G-008) |
| Network — namespace isolation | chaos PASS (containers and pods) | PASS |
| CRI / Docker API / mgmt socket | 0600 measured on the Docker API; SO_PEERCRED by reading; chmod after bind | PARTIAL (G-015) |
| MCP | stdio, no shell; `name` with a leading `-` not refused | PARTIAL (G-017) |
| Secrets `--secret-files` | a path with no writable `/run` delivers zero secrets with rc=0 (measured); a tmpfs failure would write to the rootfs (read) | FAIL (G-026 measured, G-007 read) |
| slirp api-socket per container | 0755 measured (not connectable by others); predictable path in /tmp | PARTIAL (G-016) |
| masked/readonly paths and rlimits hardening | refusal and honouring measured at both uids across the three mount slices (caller, `/proc` omission) and in the `rlimits`; ABSENT path ignored by decision | PASS (G-009) |
| Env var with no effect | `DELONIX_INTERNAL` defined, never read | G-014 |
| Supply chain CI | 0/20 actions pinned | FAIL (G-020) |
| VM providers | non-honourable option refused; no action outside delonix | PASS |
| PaaS × runtime (ports) | the PaaS reaper removes forwards with host address and without registration (read) | FAIL (G-036, G-037) |
| SELinux | host without SELinux | ENVIRONMENT BLOCKED |
| Rootful | not exercised | NOT TESTED |

## Open P0

No **confirmed** P0. P0 candidate pending reproduction: **G-007** (secrets on the on-disk rootfs
if the tmpfs fails) — today only read in the code, with no natural trigger found.

## Security P1

- **G-008** — source-IP isolation is the basis for namespace isolation and for `Dependency`; an
  `nft` that fails (full ruleset, incompatible binary/version, old holder) leaves the workload without
  anti-spoof and no one is warned.
- **G-036** — on a PaaS deploy with a shared root, containers published with an address lose the
  forward every 30 s (availability, not confidentiality).
