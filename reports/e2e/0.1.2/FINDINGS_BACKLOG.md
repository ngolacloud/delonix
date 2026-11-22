# FINDINGS_BACKLOG.md — 0.1.2 (@b72edeed)

**New** findings from this CLI regression round. Numbering continues from 0.1.1
(last GAP-0027). Reproduced live, rootless host, 2026-11-18. Doctrine: only the
verified effect counts. **0 binary regressions**; what follows is 1 consistency
GAP, 1 harness robustness finding, and battery staleness debt.

---

## GAP-0028 — VM with the same name in another namespace: coexists (not a conflict)

> **RESOLVED 2026-11-18 (`119221e7`) — reclassified as battery staleness.**
> In-depth investigation: the engine indexes the VM registry by namespace
> (`vms_root` → `vms/<ns>/<nome>.json`, port of the ADR-0086 catalogue) and exposes `vm
> move-namespace` (#982/#999) — **VMs are namespaced objects by current design**.
> Proven live: `apply` in teamA and teamB both `ensured`, distinct registries
> `vms/teamA/nsvm.json` + `vms/teamB/nsvm.json`, each ns seeing its own. The conflict(5)
> was the OLD rule (ADR-0069, single-node registry `vms/<nome>.json`), superseded
> when VMs became namespaced; it is the e2e test that lagged behind. Fix: the
> section now asserts namespaced coexistence (not conflict). **Pending on the
> owner:** reconcile the VM clause of ADR-0069 (doc drift).

- **Behaviour (OBSOLETE — historical record of the finding):** `stack apply` of a `VirtualMachine` with the same
  name in another namespace **is accepted** (`rc=0`, `vm/<nome>: ensured`), instead of
  being refused as a conflict.
- **Expected:** parity with fix #824 (`3de5db5c`, "a repeated container name
  in another namespace is a conflict, not 'already exists'") — which 0.1.2 applied to
  **containers** but **not** to `VirtualMachine`. The `e2e.sh` battery already expects
  `exit 5` ("the same name in another namespace is a conflict (5)") and observes `rc=0`.
- **Why it matters:** model inconsistency between container and VM in the same
  engine; two VM objects with the same name in different namespaces can collide
  on resources named by bare name (cf. `boot.rs:398/:415`, names by prefix without
  tenant — linked to D2 of PLATFORM_EVAL 0.1.1).
- **Reproduction:** `stack apply` of the same `VirtualMachine` in `teamA` and then
  `teamB` → the second is `ensured`, not refused.
- **Acceptance:** apply the same cross-namespace name-conflict
  normalisation/check to VMs; the `e2e.sh` "the same name in another namespace is a conflict"
  becomes PASS. Test in `delonix-compute`/`stack`.
- **Dependencies:** reuses the #824 path. **Complexity:** S. **Tier:** B (rootless).

---

## H-1 (harness finding) — `e2e.sh`/VM sensitive to the length of `DELONIX_ROOT`

> **RESOLVED 2026-11-18 (`119221e7`).** `ch_usable()` now requires `ch_sockfits`
> (budget `len(DELONIX_ROOT)+50 <= 107`); when the root is too deep, the 11
> CH sections turn to **SKIP** with a reason in `ch_missing` instead of 6+ spurious FAILs.
> Proven: short root (21B) runs, long root (121B) SKIP. The binary already guarded
> (`SUN_PATH_MAX=107`); this only closes the measurement trap in the harness.

- **Observed:** with a long `DELONIX_ROOT` (the session's scratch path), **the whole**
  cloud-hypervisor section of the battery fails with `error[DX-1506] … socket path …
  is 143 bytes, a UNIX socket path is limited to 107 bytes` — 6 spurious FAILs on the
  1st run. The binary **diagnoses correctly**; the problem is the harness, which
  does not guard against the AF_UNIX limit of 107 bytes when choosing the isolation root.
- **Expected:** the VMM socket path (`<DELONIX_ROOT>/vms/<nome>.sock`)
  should be robust to long roots (hash/shorten the name, or socket under a short
  `XDG_RUNTIME_DIR`), **or** the battery should refuse/warn when the chosen root
  plus the longest VM name exceeds 107 bytes — instead of producing
  FAILs that read as a regression.
- **Impact:** measurement trap — it contaminated the 1st run; any CI/lab runner
  with a deep root (e.g. agent workspace paths) inherits it.
- **Reproduction:** `DELONIX_ROOT=<long path> delonix vm create x --backend
  cloud-hypervisor --disk d.qcow2` → DX-1506; with a short root → `✓ VM started`.
- **Acceptance:** the VMM socket survives a long `DELONIX_ROOT` (or the battery
  shortens/warns). Documented in the E2E skill (preflight). **Tier:** B.

---

## Staleness debt of the `e2e.sh` battery (correct binary, aged assertion)

These are not engine defects — the battery is the one carrying expectations from before
0.1.2. Each one fails the battery with no finding behind it; they should be reconciled
so the gate goes green again in an appropriate lab.

| Battery check | Observed (0.1.2) | Stale expectation | Action |
|---|---|---|---|
| `vm resize --disk-size 0 recusa (1)` / `CH: --disk-size 0 recusa (1)` | clap refuses with `rc=2` with a useful message | `rc=1` (domain refusal) | update expectation to `2` (refusal in the parser is correct) |
| `delete de um NetworkGateway recusado termina` / `… apaga-se sem provider` | `DX-4000 no such kind 'networkgateways'` (`rc=4`) | existing kind | the 0.1.2 catalogue has `Gateway` + `Router.networkGateways` (field); the test uses a non-existent kind — fix the kind or remove/xfail |
| `métricas: GET /metrics devolve OpenMetrics…` | `# TYPE delonix_build info` + `delonix_build_info{version="0.1.2"} 1` (OpenMetrics `info`) | grep for `^# TYPE delonix_build_info ` | adjust the grep to the line `# TYPE delonix_build info` (the `info` type is more correct; links to the 0.1.2 telemetry fix) |
| `cluster kube generate: … is 4` | `DX-1000` (`rc=1`) for "no such pod or container" | `rc=4` (not-found) | decide: map not-found→`4`, or fix the expectation to `1` |
| `cluster network profiles: a row per profile` | "no support level is reported" | a row per profile | reconcile the output format |
| `system monitor --no-stream: … counting the running containers` | `1 containers, — active connections (conntrack)` | exact "1 containers" | grep tolerates the conntrack suffix (`—` under rootless, measured-not-available) |

### Environment gaps (BLOCKED, not debt) — need a rootful VM
CRI default-caps, landlock+tmpfs, privileged `/sys` mounts (on-top/rshared),
`net netns firewall/publish` (nft-no-netns), slirp-flaky, `vm --require` (libvirt
capability probe). All would measure in a rootful lab + CNI; the host is rootless
without sudo. → `evidence.jsonl` `lab-ephemeral` profile.

---

## Carried over from 0.1.1 (confirmed still open in `b72edeed`, partly already closed in `9ec12592`)
- **GAP-0019** (`vm ls` only `-A`) and **GAP-0027** (build-cache without GC): observed
  open in this round; **the owner closed them** in `9ec12592`
  (`fix/cli-ls-parity-and-buildcache-prune`) during the campaign.
- GAP-0018/0020..0026 and KRN/NAT: see `reports/e2e/0.1.1/` — out of scope for this
  regression round (no targeted re-verification).
