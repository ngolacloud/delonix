# Sprint 0 — Audit and baseline of `delonix dashboard`

Date: 2026-10-25 · repo `delonix-runtime` v5.1.1 · ratatui 0.29 / crossterm 0.28.1
Branch: `feat/dash-metrics-correctness` (off `develop` @ e8708a02)
Process source of truth: `docs/dev/engineering-process.md` (develop-GitFlow).

## 1. Architecture map (current state)

The dashboard is **a single file** of 1933 lines:
`bins/delonix-runtime-bin/src/cmd/dash.rs`. There is no dedicated module/crate.

Layers, as they exist today:

| Responsibility | Where it lives | Note |
|---|---|---|
| Acquisition (counts, memory, io, net, storage) | `delonix-mgmt::dashstats::collect/_with_timeout/collect_container_net` | SINGLE collector shared with `/metrics` and `/v1/dash` |
| CPU/mem/io per container | `delonix_linux::container_usage` / `ContainerUsage` | `Option<u64>` **without reason** (`Why`) |
| Slice CPU | `delonix_linux::slice_cpu_usage_usec` / `slice_budget` | raw usec counter; `host_ncpu` read but discarded |
| Rich quality (PSI, throttle, OOM, `Why/Unavailable`) | `delonix-telemetry::workload` + `pressure.rs` | **only the `/metrics` server uses it; the dashboard does NOT** |
| Pure snapshot (data) | `DashData::collect_with` (dash.rs:232) | testable without a terminal |
| ANSI render (`--once`/`--json`) | `render_snapshot` (dash.rs:612) | pure over `DashData` |
| TUI (event loop, focus, draw) | `mod tui` (dash.rs:747-1764) | ratatui; rate state lives here |
| Per-resource operations | **absent from the dashboard** | see §3 of the plan |

Entrypoint: `run(scope, once, json)` (dash.rs:729) → `--json` | `--once`/non-tty | `tui::run_interactive`.

Cadence: 1s tick (counts+memory, cheap) + background thread at 15s
(`SLOW_REFRESH`) with a 120s ceiling (`COLLECT_TIMEOUT`) for network/storage (disk
walk measured at >1 min on a loaded host). CPU% is recomputed per **frame**
(200 ms poll) — see bug G3.

Strengths to preserve: single collector (TUI/JSON/Prometheus do not diverge in the
arithmetic); `Option` vs `-` vs `measuring…` (unavailable≠zero in the render);
`InFlight` circuit-breaker for hung I/O (NFS); `network_unmeasured_containers`
(host/none ≠ 0); `Why`/`Unavailable` model in `pressure.rs`/`workload.rs`.

## 2. Reproduced baseline (live, binary 5.1.1)

`delonix dashboard --once` (global scope), on this host:
- CONTAINERS 0/0 · VMs 1/1 · NETWORKS 7 · VOLUMES 27 · IMAGES 26 · SECRETS 17
- MEMORY 76 MiB / 53.07 GiB · TRAFFIC 0 B/0 B · STORAGE 26.93 GiB
- Table: 1 VM + 7 networks + volumes; **all non-container rows at `-`** in
  CPU/MEM/I/O/NET; the STATUS column shows `bridge` (networks) and `local` (volumes) —
  i.e. the **driver**, not the state.

Confirms, from the brief's list: the STATUS column mixes state/driver/id;
generic tables with inadequate columns and empty fields; long names/paths
(mountpoints) in INFO; `OK`/`no problems` with no diagnostic coverage;
`UP`=`-` for almost everything. (Live CPU%/mem not observable now: 0 containers
running — reserve proof for when there is load.)

Current tests: ~17 inline unit tests in dash.rs + ~14 in dashstats.rs; smoke
`scripts/e2e.sh` validates `dashboard --once --json`. The interactive TUI has no
automated test (needs a PTY).

## 3. Gap matrix

State: **C**=Confirmed · **H**=Hypothesis · **GF**=Functional gap · **LP**=Provider limitation · **BA**=Blocked by environment.

### A. Metrics correctness (Sprint 1 focus)

| # | Requirement | Evidence | State | Cause | Impact | Fix | Acceptance test |
|---|---|---|---|---|---|---|---|
| G3 | Rate = Δcounter / Δreal interval | dash.rs:1083-1109,1166 | **C** | `cpu_prev` rewritten per frame (~200ms), counter changes per tick (1s): most frames `cur==u0`→0%, the frame after a tick divides Δ~1s by ~200ms → **~5× inflated** | fake CPU% (intermittent 0% + 5× spikes); misleads triage | anchor the previous sample to the **tick** (not the frame): only recompute when the counter changes; keep the last % between ticks | unit test of the calculation: 2 samples 1s apart → correct %; an intra-tick sample does not change it |
| G4 | Stable identity (ctx/provider/type/**id**) | dash.rs:1087 key `kind:name` | **C** | key by name, not `id` (already available in `ContainerRow`) | reused/duplicate name collides the sample → wrong Δ | key `cpu_prev` by stable `id` | test: two containers same name, distinct ids → independent %s |
| G5 | Explicit denominator for % | dashstats.rs:145 (`host_ncpu` discarded); `cpu.max` only in 4843-5769 | **C** | no CPU % uses a denominator; "% of 1 core" unrelated to the limit | the user does not know whether 180% is bad (2 cores? quota?) | label clearly "% of 1 core"; when `cpu.max`/`host_ncpu` is determinable, also expose % of the limit/host; never invent a denominator | test: with a known quota → %limit; without quota → only %core + label |
| G6 | Limit quality ≠ sentinel | slice_budget lib.rs:5797; DashSummary:43 | **C** | `memory_bytes_limit=0` collapses unlimited/unknown/unreadable | the memory gauge disappears without saying why | distinguish "no limit" from "unknown"; explicit text | test: limit=0 → label "no limit" vs read error |
| G2 | Metric state/quality visible | ContainerUsage lib.rs:10819 (no `Why`) | **C** | the dashboard uses a poor tier; it ignores `Why`/PSI/throttle/OOM | does not distinguish a real "0" from "not delegated/unreadable" | keep `-` for None (already ok); Sprint 1 minimum: do not regress; exposing PSI/OOM is Sprint 4 | test: None→`-`, Some(0)→`0 B` |
| G7 | Network as a rate (B/s), not just total | dash.rs:318,597 | **C** | only the accumulated total; CPU has a rate, network does not | "TRAFFIC" does not show throughput | derive the rx/tx rate at the tick (Δ/interval), label it; keep the total available | test: 2 samples → correct B/s; 1st sample → "collecting" |
| G8 | Timestamp/age in the snapshot | DashSummary:30-66 no `collected_at` | **C** | age only in the TUI (`slow_age`); `/v1/dash` does not carry it | a JSON consumer cannot tell fresh from stale | (Sprint 1 optional) do not regress the TUI; exposing age in JSON is a candidate | test: JSON has an age field |

### B. Semantics/UI (confirmed; fix in Sprints 2-4)

| # | Requirement | Evidence | State | Cause | Fix (sprint) |
|---|---|---|---|---|---|
| U1 | STATUS = runtime state, not driver | --once: networks `bridge`, volumes `local` | **C** | `Row.status` receives `n.driver`/`vol.driver` (dash.rs:456,476) | STATUS column state-only; driver/CIDR/mount in their own columns (S4) |
| U2 | Resource-specific tables | --once: CPU/MEM/IO/NET columns `-` for non-containers | **C** | single 9-col table for all types | columns per scope/subtab (S4) |
| U3 | `UP` only where applicable | `up="-"` in almost everything | **C** | only containers have `pid_starttime` | VMs: uptime from the backend; others: hide the column (S4) |
| U4 | `OK/no problems` with coverage | draw_problems:1619 | **C** | absence of problems = "OK" without saying how many checks | summary "N checks, M failures, K unavailable" (S4/S6) |
| U5 | `RUNNING CONTAINERS` sparkline only in the relevant scope | dash.rs:1669-1680 | **C** | fixed global sparklines in all scopes | contextual charts per scope (S4) |
| U6 | Truncated name/path accessible | TUI widths dash.rs:1560 | **H** | visual truncation without a detail panel | detail/inspector with the full value (S3) |
| U7 | Refresh ≠ freshness | age_suffix only in TRAFFIC/STORAGE | **C/partial** | counts/mem always "now"; no global badge | header with freshness per source (S4) |
| U8 | Connection/context state in the header | absent | **GF** | no persistent header | ctx/host/rootless/connection header (S4) |
| U9 | Esc does not quit the whole app | dash.rs:1187 Esc=quit | **C** | Esc mapped to quit | Esc=close modal/go back; `q` quits (S2) |

### C. Navigation / search / operations (Sprints 2-3)

| # | Requirement | State | Note |
|---|---|---|---|
| N1 | PgUp/PgDn, focus between panels, Enter→detail | **GF** | only ↑↓/Home/End/Tab today |
| N2 | Preserve selection by id after refresh/sort | **GF** | selection by index (dash.rs:1132) |
| N3 | Ctrl+K palette + parser `kind:… state:…` | **GF** | only `/` filters by name substring |
| O1 | Enter→detail (Overview/Metrics/Logs/…); start/stop/restart/exec/attach/remove/inspect | **GF** | **no operation today**; the services exist (see ops inventory) |
| O2 | Exec vs Attach distinct; PTY/resize/detach/restore | **GF/LP** | `exec_with` uses fd 0/1/2+global raw (collides w/ TUI); SIGWINCH only at startup (lib.rs:9512); `attach -i` not supported by the engine |
| O3 | VM console (libvirt virsh / CH socket) | **GF** | helpers exist (vm.rs:5530-5760) |
| O4 | Logs per CRI file (`parse_cri_log_line`) | **GF** | `cmd_logs` writes stdout/blocks; read the file directly |

## 4. Plan by sprints (dependencies)

- **S0 Audit** — this document. ✓
- **S1 Metrics and states** (this session): G3→G4→G5→G6; G7 if it fits. No
  artificial zeros; counter/reset/1st-sample/denominator tests. Depends on: nothing.
- **S2 Navigation+search**: focus model, selection by id (N2), Enter/Esc (U9),
  Ctrl+K+parser (N3). Depends on: light refactor of the TUI state (S1 touches the same
  loop — doing S1 first reduces the conflict).
- **S3 Details+operations**: O1-O4. Depends on S2 (Enter→detail) and on exposing
  `cmd_*` or calling `delonix_linux`/`delonix_vm` directly; suspend/resume the TUI
  around `exec_with`.
- **S4 Visual+charts**: Overview/Health, per-resource tables (U1-U3,U5),
  header (U8), 80x24/120x30/160x50 layout, high contrast. Depends on S1 (data).
- **S5 Robustness+integration**: 1k/10k load, resilience, secrets redaction,
  docs, benchmarks p95<100ms. Depends on everything.

## 5. Decisions and constraints

- Reuse `dashstats`/`delonix_linux`/`delonix_vm` — do not duplicate rules.
- No mandatory daemon, no mandatory Prometheus (daemonless-first).
- ratatui confined to `delonix-runtime-bin` (arch_fitness enforces; AGENTS.md:1128).
- Integration is by the owner's squash PR (hook refuses push/merge); gates: cli-surface
  (`cli_baseline.tsv`), docs_cli_gate, i18n `data/pt.po`, arch, clippy -D warnings.
  Sprint 1 does **not** add new leaves/flags → does not touch `cli_baseline.tsv`.
