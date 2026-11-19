# 07 — IaC / manifests (compose)

SUT: `delonix 5.1.1` @ `bca34f6c3`, rootless. Project: `relc-stack` (web=nginx-unprivileged, cache=redis-alpine).

| # | Case (§9) | Command | Observed | State |
|---|---|---|---|---|
| 1 | validate correct manifest | `compose config` | resolves project, normalizes services, network `relc-stack_default`, exit 0 | **PASSED** |
| 2 | reject invalid field | `compose config` with `porz:` | `error[DX-1000] 'porz:' is not understood and was refused rather than ignored` + list of supported keys, exit 1 | **PASSED** (fail-closed) |
| 4 | apply | `compose up -d` | network created, 2 services `Running`; `web` responds `HTTP 200` on 18090 | **PASSED** |
| 5 | confirm real state | `compose ps` + curl | both Running; HTTP 200 | **PASSED** |
| 6/7 | reapply without changes / no recreation | `compose up -d` 2nd time | "already exists, nothing to do"; **web id unchanged** (`40f5505569ec0bb0`) | **PASSED** |
| 10/11 | introduce drift + reconcile | manual stop of `cache` → `compose up -d` | **`cache` stays `Stopped`** despite `up` saying "Creates/starts every service" | **FAILED** → DEF-003 |
| 12 | remove with ownership | `compose down` | removes exactly the project's 2 containers + the network (by labels); port 18090 freed | **PASSED** |

## Main result

- **Exemplary fail-closed validation:** unknown fields are **refused** (not ignored), with the full
  list of accepted keys — exactly the behaviour §9 requires.
- **Real idempotency:** reapplying does not recreate (same id), not merely "does not fail".
- **Ownership on removal:** `down` removes only what `up` created (derived from labels); `kaeso-*` and pre-existing networks intact.
- **RECONCILIATION DEFECT (DEF-003, P2):** `compose up` reconciles **existence** but **not running state** —
  a stopped/crashed service (drift) does not return to `Running`, contradicting its own help ("Creates/starts every service")
  and docker-compose (the declared compatibility target). Workaround: `delonix start <svc>`.

## Coverage and limits

- Tested the **compose path** (the primary single-node IaC surface). The
  `stack apply/plan` with `kind:` documents and `delonix-model` (drift with mutable fields
  vs replacement, external resource adoption, stale plan, interruption during
  application — §9 steps 3,8,9,13-17) **was not exercised live** in this round →
  **NOT EXECUTED** (medium release risk: full declarative reconciliation remains unproven; see DEF-003 which already hints at partial reconciliation).

## Cleanup

Project `relc-stack` fully removed; port freed; temporary directory deleted; `kaeso-*` preserved.
