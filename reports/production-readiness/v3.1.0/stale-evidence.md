# Stale evidence matrix

Analysed commit: `fa8c6297` (3.1.0 + 26). Criterion: **CURRENT** = measured on 3.1.x;
**STALE** = measured on an earlier version; **UNDATED**; **DOCUMENTATION ONLY**.

## Conclusion

Almost no published number was measured on 3.x. CRI conformance, the benchmarks, the
E2E battery and the Docker/Compose comparisons are from v0.x (July–August 2026). **No
workflow runs critest, bench, coverage, the Docker API smoke or the compatibility
matrix.** The only work measured on 3.1.x is `docs/sovereignty-engine.md` and the
v3.1.0 notes.

## CI evidence on the commit itself (verified via `gh run view`)

| Workflow | Reported result | Reality |
|---|---|---|
| `ci.yml` on `fa8c6297` | success | jobs executed |
| `chaos.yml` on `fa8c6297` (run 34988281401) | **success** | `environment probe (userns)` success; **`chaos harness` = skipped** → the chaos harness **and the E2E battery did not run** |

## Claims contradicted by the current code

| Claim | Where | What the code says |
|---|---|---|
| Docker API 15 served / 7 refused; exec/logs/attach/stats/images-create "nowhere" | `../CLAUDE.md` | 14 served, **12 refused with a reason**, the 5 included (`dockerapi.rs:426-532`) |
| critest 77/103 (v0.42.2) | `../CLAUDE.md` | doc says 79/103 on v0.63.1 — **still 3 majors behind** |
| Compose swallows unknown keys | `../CLAUDE.md` | top/service allowlist refuses (`compose.rs:1576`); **but nested sub-keys remain ignored** |
| x86_64-only releases | `../CLAUDE.md` | **confirmed** (`release.yml`; v3.1.0 assets) |
| critest 65/103 | `paridade-docker-podman.md:216`, `tests/compat/cri-conformance.sh:6` | 79 |
| profiles/extends/configs/secrets/multi-file "to do" | `COMPARACAO-DOCKER-PODMAN.md:130` | implemented |
| `--pids-limit` missing | `COMPARACAO-DOCKER-PODMAN.md:169` | exists |
| `vm create` without `--disk-size` | `measured-comparison.md:126` | exists (`cmd/vm.rs:552`) |
| `cri-conformance.md` | header 24 failures vs "26 failures" (`:212`) vs "13 of 31" (`:177`) | **document contradicts itself** |
| `measured-comparison.md` | v0.63.1 recount (`:136`) vs "table is v0.53.0, recount to do" (`:183`) | **document contradicts itself** |

## Matrix (summary; detail per line in the raw inventory)

| Class | Evidence | Number | Measured on |
|---|---|---|---|
| STALE | CRI conformance | 79/103 (24 failures, 19 skip) | critest 1.36.0, **v0.63.1**, 2026-08-25 |
| STALE | run --rm latency Docker/Podman/Delonix | 299/277/80 ms (only the 4th on all three) | v0.63.1 |
| STALE | Latency (main table) | 208/268/91 ms | v0.53.0 |
| STALE | E2E battery (report) | 198/0/0 | v0.47.0 |
| STALE | E2E battery (script header) | **PASS 529 / FAIL 36 / SKIP 5**, runs 91/244 leaves (37%) | v3.0.0, 2026-09-09 |
| STALE | Docker API matrix / SDK smoke | 14/7; 14/14 | v0.42.2 |
| STALE | E2E / security audit | ~50k LOC, 9 crates | 2026-07-21 |
| STALE | Inventory `docs/runtime/current-state.md` | 10 crates, ~71k LOC | 2026-07-30 (today 15 crates, 156k LOC) |
| STALE | Code-quality reports | 138.5k LOC, scores | v1.0.0 |
| STALE | SBOM (roadmap) | 380 packages | v0.63.1 (Cargo.lock today: 412) |
| UNDATED | README / site — CRI 79/103 | 79/103 | no version |
| CURRENT | Sovereignty (0 network accesses at start) | — | 3.1.0, 2026-09-15 |
| CURRENT | 19 Kinds, 15 crates, 245 leaves | — | 3.1.0 |
| DOC ONLY | "multiple rounds of security review" | — | `SECURITY.md:59` |

## Re-measurements on the current commit (G6 of the NO-GO→GO plan)

| State | Evidence | New value | Commit / date |
|---|---|---|---|
| FRESH | Capability matrix (`docs/providers/capability-matrix.md`) | **181/271 (66.8%)**, engine 0.1.0 — was 180/270@5.1.1; `net.ipv6` moved to `supported` (ADR-0082 phase 4). Regenerated from the current binary; the `the_published_matrix_is_the_generated_one` test is GREEN again (drift closed) | feat/ipv6-phase1, 2026-11-02 |

**Still STALE, as they require a dedicated node/infra (not re-runnable in this box under foreign load):** latency (`bench.sh` refuses on a loaded node — by design), density (`bench_density.sh`, new — needs an idle node), CRI (`critest`, needs k8s/CRI), chaos+E2E (`chaos.sh`/`e2e.sh`, create VMs/kind). See G5/G6 of the plan.

## Regenerable evidence and who runs it

| Evidence | Script | CI? |
|---|---|---|
| CRI | `scripts/critest.sh`, `tests/compat/cri-conformance.sh` | **no** |
| Latency | `scripts/bench.sh` | **no** |
| Chaos + E2E | `scripts/chaos.sh`, `scripts/e2e.sh` | yes, **but skipped on this commit** |
| Coverage | `scripts/coverage.sh` | **no** |
| SBOM | `scripts/sbom.py` | yes, only on a tag |
| Docker API matrix | `delonix serve docker-api --matrix`, `delonix compatibility docker` | **no** (consistency tests yes) |
| Docker SDK smoke | `tests/compat/docker_api_smoke.py` | **no** |
