# e2e round `delonix 0.1.3` — distroless-by-default + `vm --distro` + surface profile

> Binary under test: `delonix 0.1.2 (+33 commits)` in the worktree
> `feat/distroless-default-vm-distro`. This round validates the
> **distroless-by-default** and **`vm create --distro`** differentiators and measures the **entire CLI
> surface** (time per leaf) and the **concurrency/high
> availability** behaviour. Tree source: `scripts/cli-tree.sh` (**366 leaves / 413
> commands / 78 groups**).

## 1. What changed in this round (vs 0.1.2)

| Area | Change | Proof |
|---|---|---|
| Distroless (containers) | Generic scaffold and `go/rust/python/node` templates now have a distroless RUNTIME image (multi-stage); registry configurable via `DELONIX_DISTROLESS_REGISTRY` (default `gcr.io/distroless`) | §3 |
| Pull fallback | A recognised and absent (not-found) distroless image falls back to the equivalent conventional base, instead of failing | §3 |
| `vm create --distro` | New semantic selector `ubuntu\|debian\|rocky\|fedora`; checks that the VM image repository is reachable; cross-check with `--disk` | §3 |
| i18n | 7 new strings in `data/pt.po` (EN source) | `--l18n=pt vm create --help` |

## 2. Surface coverage

| Axis | Value | Source |
|---|---|---|
| Leaves discovered (real parser) | **366** / 413 commands / 78 groups | `cli-tree.sh --count` |
| Leaves with measured time (`--help`, RUNS=20) | **366 (100%)** | `cli_timing_profile.sh` → `timing-results.jsonl` |
| Leaves exercised under concurrency (safe read-only) | 7 active (1 correct SKIP) | `cli_concurrency_probe.sh` |
| Behaviour regression (pass/fail per leaf) | baseline **0.1.2** (1735 PASS / 22 triaged FAIL / 35 SKIP) — this round's changes are additive (new flag, templates) and covered by 50 scaffold tests + 4 distroless tests | `reports/e2e/0.1.2/` |

The distroless conversions were proven **running** (not just compiling): see §3.

## 3. Proof of the differentiators (observed behaviour)

**Distroless-by-default**
- `delonix init -t go` → `FROM gcr.io/distroless/static-debian12:nonroot`, `USER 65532:65532`, `ENTRYPOINT ["/usr/local/bin/server"]`, `HEALTHCHECK CMD ["/usr/local/bin/server","healthcheck"]` (exec-form, no shell).
- Generic (no `--image`) → multi-stage `busybox:1.37-musl AS tools` + `FROM gcr.io/distroless/base-debian12:nonroot`; **build+run+`curl` → `hello-distroless`** (rc=0).
- Real **go** template → build (golang-alpine→distroless/static) + `run -d -p` + `curl /api/v1/health/live` rc=0 + `exec … server healthcheck` rc=0.
- Configurable registry: `DELONIX_DISTROLESS_REGISTRY=cgr.dev/chainguard delonix init` → `FROM cgr.dev/chainguard/base-debian12:nonroot`.

**Pull fallback**
- `FROM gcr.io/distroless/static-debian12:<nonexistent-tag>` → stderr `distroless image … not found; falling back to alpine:3.24` → pull alpine → build OK (`fbtest:dev`, 8.3 MiB). Only triggers on **not-found** (`DX_NOT_FOUND`), never on a network/auth error; only for recognised distroless refs. Node is excluded on purpose (the `node` path diverges between distroless and `node:*-slim`).

**`vm create --distro`**
- `vm create --help` shows `--distro <ubuntu|debian|rocky|fedora>` (en and pt-AO).
- `vm create --distro debian` with the repo unreachable → clear error: *«VM image repository 'ghcr.io/ngolacloud/delonix-vm-base' is not reachable: …»*.

## 4. Performance — time per leaf (`--help`, RUNS=20)

**parse+startup** latency per leaf (includes binary `fork`/`exec` — the real cost of the CLI surface). Base startup: `--version` mean **9.874ms** (p95 11.212), `--help` mean **9.886ms** (p95 12.253).

- **global mean of the 366 leaves: 9.12 ms** · median 9.065 ms · min 7.785 ms · max 10.715 ms.
- Very tight distribution (all leaves 7.8–10.7 ms mean): there is no parse-outlier leaf; the cost is dominated by binary startup, not by the command.
- Occasional p99 18–26 ms on isolated leaves (`pod`, `vm`, `container`, `network`) = scheduling jitter, not leaf work.

Full table per group in [`PERFORMANCE.md`](PERFORMANCE.md). Top (slowest by mean-of-means):

| group | leaves | mean-of-means (ms) | p95 max (ms) | p99 max (ms) |
|---|--:|--:|--:|--:|
| `run` | 1 | 10.53 | 12.14 | 12.26 |
| `pull` | 1 | 10.27 | 11.73 | 11.75 |
| `completion` | 2 | 10.1 | 12.35 | 18.4 |
| `version` | 1 | 10.07 | 11.63 | 24.35 |
| `logout` | 1 | 10.05 | 11.96 | 13.34 |
| `migrate` | 1 | 10.02 | 11.32 | 11.69 |
| `rm` | 1 | 9.97 | 18.23 | 24.87 |
| `stop` | 1 | 9.93 | 12.35 | 12.53 |
| `hosts` | 1 | 9.85 | 11.37 | 11.83 |
| `systemcontainer` | 6 | 9.84 | 14.53 | 17.75 |
| `create` | 1 | 9.78 | 11.29 | 12.77 |
| `bucket` | 3 | 9.69 | 12.52 | 16.93 |


## 5. Concurrency / high availability

`cli_concurrency_probe.sh`: **32 parallel `delonix` processes**, 20s, over safe read-only leaves (`version`, `info`, `ps`, `image ls`, `vm ls`, `system info`, `events`).

| Metric | Value |
|---|--:|
| Invocations | **8736** |
| Success (rc=0) | **8736 (100.00%)** |
| Errors | 0 (0.00%) |
| State corruption | **none** (`state_corruption_suspected=False`) |
| Throughput | **435.58 inv/s** |
| Latency p50 / p95 / p99 / max | 51.657 / 108.59 / 362.212 / 736.948 ms |

**Reading:** under 32× contention, the daemonless read-only surface keeps **0 errors and 0 state corruption** — the `flock`/atomic state and the opportunistic startup GC hold up under concurrent access. The tail (p99 362.212 ms, max 736.948 ms) is the expected contention of 32 `fork`/`exec` + per-process state scan; there is no failure, only tail latency.

## 6. Where to improve performance (actionable)

1. **Binary startup dominates** (~8–10 ms per invocation, 45 MB). Targets: (a) the warm plane (ADR-0095) already attacks `pre_spawn` on the container/CRI path; (b) for the pure CLI surface, the latency is the cost of `fork`/`exec`+clap parse — an optional long-lived daemon for the high-frequency path would reduce this, but breaks the daemonless invariant (keep as is).
2. **Per-process opportunistic GC** (`gc_orphan_cgroup_leaves_once`) adds work to every command that opens the store — under 32× concurrency it is part of the p99 tail. Consider making it *best-effort with amortisation* (1×/N processes or per TTL) to lower the tail without losing post-crash recovery.
3. **parse p99 on `pod`/`network`/`vm`** (18–26 ms) — scheduling jitter, not work; acceptable. Re-evaluate only if a CLI latency SLO requires it.

## 7. Artifacts

- [`timing-results.jsonl`](timing-results.jsonl) — 368 lines (366 leaves + 2 startup), one per leaf with min/p50/p95/p99/max/mean.
- [`concurrency-results.json`](concurrency-results.json) — aggregates from the concurrency probe.
- [`PERFORMANCE.md`](PERFORMANCE.md) — full table per group.
- [`COVERAGE_MATRIX.md`](COVERAGE_MATRIX.md) — coverage per axis/group.

> New (reusable) harnesses: `scripts/cli_timing_profile.sh`, `scripts/cli_concurrency_probe.sh`.
