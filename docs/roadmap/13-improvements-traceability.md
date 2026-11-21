# Programme of 13 improvements — traceability matrix

> **What this document is.** The single record of the state of M01–M13, with
> the **measured baseline** for each one. A cell that states a number says
> where it was obtained; a cell with no measurement says `to be measured` —
> never an estimate.
>
> **What it is not.** A statement of intent. A `DONE` here requires all four
> things at once: code, tests, documentation and reproducible evidence.
>
> **Why it exists.** This repo's `docs/AUDITORIA-E2E.md` spent weeks claiming
> 27 problems resolved as live debt, for not having been updated as fixes came
> in. A table that does not track the code lies in both directions. This one
> has to be updated in the SAME commit that changes the state it describes.

| Field | Value |
|---|---|
| Baseline measured on | **2026-08-25** |
| Against | `origin/main` `b4653002b`, version `0.63.1` |
| Where | development host, Linux 7.0, rootless, cgroup v2 |
| Binary | `cargo build -p delonix-runtime-bin` from the commit itself |

## Factual baseline of the engine (measured today, not quoted)

| Measure | Value | How obtained |
|---|---|---|
| Crates in the workspace | **13** | `ls crates/` + `Cargo.toml` per directory |
| Top-level commands | **28** | traversal of `--help` from the binary |
| Invocable CLI leaves | **229** | same traversal, counting the ones with no subcommands |
| Tests in the workspace | **1207** | `#[test]` + `#[tokio::test]` in `crates/` |
| Checks in the E2E battery | **270** | `scripts/e2e.sh` |
| Chaos scenarios | **8** | `scripts/chaos.sh` |
| Routes in the Docker Engine API | **15 served, 11 refused with a reason** (updated 2026-09-22 — `POST /images/create` moved off the list; the rest of this table's baseline is still from 2026-08-25) | `API_MATRIX` / `API_UNIMPLEMENTED` in `cmd/dockerapi.rs` |
| Kinds in the manifest | **20** (updated 2026-09-22 — `kind: RuntimePolicy`, M04; the rest of this table's baseline is still from 2026-08-25) | `cmd/kinds.rs` table |
| ADRs | **18** (13 Accepted, 2 Proposed, 1 Rejected, 2 with no parseable status) | `docs/adr/` |
| CI jobs | **7** (`fmt`, `lang`, `clippy`, `test`, `deny`, `docs` + chaos on the side) | `.github/workflows/ci.yml` |

> **Correction to this baseline (2026-08-25).** The first version of this line
> said «21 routes served», counted by route literals in the file. It was
> wrong: 21 is the sum of served plus **refused** — `API_UNIMPLEMENTED` also
> holds path literals. The correct number is 14 served. This is recorded
> rather than quietly fixed, because it was exactly the error this programme
> exists to catch, committed by the very document that measures it.

> **The surface is not the coverage.** The 229 leaves have had their `--help`
> verified; the number the battery actually **executes** was not re-measured
> in this pass, and the fraction quoted in `AGENTS.md` is from another date.
> It stays `to be measured` in M02/M11 until it is recounted with that day's
> `scripts/e2e.sh` — quoting a total that went up would make coverage look
> better without a single new leaf exercised.

## Matrix

States: `NOT_STARTED`, `IN_PROGRESS`, `PARTIAL`, `BLOCKED`, `DONE`.

| ID | Improvement | State | Measured baseline | Missing deliverables | Tests | Dep. | Risk | PR | DoD |
|---|---|---|---|---|---|---|---|---|---|
| **M01** | Positioning and architecture | `IN_PROGRESS` | `ARCHITECTURE.md` has C4 1–3 and mini-ADRs. **Forbidden-dependency gate (PaaS boundary) DONE (2026-09-16)**: `consumer_mentions()` in `scripts/arch_fitness.py` refuses any mention of a consumer (`delonix-paas`, `ngolacloud`, `delonixctl`, …) in `crates/`, `bins/`, `proto/`, `Cargo.toml`/`Makefile` — comments included — and is already wired into the `main()` of the gate, not opt-in. **Missing**: `capability discovery` (an ADR-0040 D3 design item — **the ADR was ACCEPTED on 2026-10-06**, so what is missing is the work, not the decision); the per-API stability policy is still only partial (`cli-stability.md` does not cover the management API nor the `mcp` with a formal contract). | capability discovery (ADR-0040 is already accepted — 2026-10-06); close the per-API stability policy | `tests/architecture.rs` — 3 gates, green; regression verified in both directions | — | low | — | 4 of 5 |
| **M02** | Compatibility and migration | `IN_PROGRESS` | CRI **79/103** (`critest` v1.36.0, engine **v0.63.1**, re-measured 2026-08-25). Docker API: **15 served, 11 refused with a reason** (2026-09-22: `POST /images/create` no longer missing — see below). Native `compose`. **Baseline correction (2026-09-23)**: `compatibility` EXISTED before this line was written (`cmd/compatibility.rs`, `docker` branch) — the same drift already fixed in M01/M05/M12. **`compatibility compose` DONE**: a three-state matrix against the specification's key list, MEASURED one by one against `docker compose config` (v29.8.1, 2026-09-23) — 28 served, 12 refused with a reason, 49 missing, out of 89; top-level 7/1 out of 8. **`migrate assess` DONE**: reads a real compose file and classifies every key USE, with the engine's equivalent command when one exists. `compatibility cri`/`oci` are left out with a written reason (the CRI number is from a hand-written doc; OCI has no conformance here at all). | `compatibility cri` (needs `critest` to produce an artefact the binary can read, not a hand-written doc); OCI conformance (no work on it in the repo at all) | `tests/compat/` + 3 Docker matrix gates + 4 `parse_registry_auth` tests + 4 Compose matrix gates + 6 `cmd::migrate` tests + 8 E2E checks (one cross-checking the 88 keys against the real `compose config`) | M01 | medium | #123, #124, #460, #471 | 3 of 4 |
| **M03** | Production build | `PARTIAL` | `build` has `--secret`, `--platform`, `--no-cache`, `--build-arg`, per-instruction cache (rootless), multi-stage. **Automatic SBOM per built image DONE (2026-09-22)**: every `delonix build` that commits generates `<root>/images/<id>.sbom.spdx.json` (SPDX 2.3, best-effort — never fails the build), served by `delonix image sbom <ref>` (instant cache hit for whatever this engine built; computed and stored on the fly for a pulled/older image). **`RUN --mount=type=cache` DONE (2026-09-22)**: a persistent, read-write directory (`<root>/build-cache/mounts/<sha256(id)>`, `id` defaulting to `target`) that survives between builds — the same mechanism as `type=secret` (`mount_live`, invisible to the final layer), generalized. Validated LIVE: two separate builds wrote to the SAME cache mount and the second one saw what the first had written; the content never reached the final image (confirmed by `container run`); `sharing=`/`ro` refused instead of silently swallowed. Along the way it fixed a pre-existing LANG-01 debt (this `--mount` parser's error messages had been in Portuguese all along — never revisited). **Without** `--ssh`, registry-shared `type=cache` (local to this node only), per-image signed provenance, measured comparison with BuildKit. | `type=ssh` mounts; build cache shared across nodes (registry); signed provenance per built image (distinct from M04's SLSA, which covers the engine's own RELEASE, not every image it builds); measured comparison with BuildKit | `crates/adapters/delonix-scanner` (4 `sbom_spdx_*`/`spdx_safe_*` tests) + `crates/adapters/delonix-oci::build` (7 `run_mount_*` tests) + `cmd::build::cache_mount_dir_*` + `crates/adapters/delonix-image/benches` exists | M04 | medium | #466, #468 | 3 of 4 |
| **M04** | Verifiable security | `IN_PROGRESS` | Signed releases (minisign) + SLSA provenance + SPDX SBOM of 380 packages inside the signature. `cargo-deny` in CI. `policy.json`: NODE ceiling enforced in `cmd_run` — denies `--privileged`, `--net host`, `:latest` and off-list registries, fail-closed with all reasons at once. **Fuzz job in CI (2026-09-22)**: `fuzz/` (its own crate, cargo-fuzz), two targets — `parse_dockerfile` and `parse_reference` —, 60s smoke per PR; already found a real DoS in a doubling `ARG` chain (fixed in #463). **`kind: RuntimePolicy` DONE (2026-09-22)**: `stack apply` raises/tightens the ceiling declaratively (converges hot, `presence: Registry`), but never lowers it — `teardown: false` in the table, `stack destroy`/`--prune` never touch it, and `delonix policy unset` is the only verb that removes it, with interactive confirmation or `--force` and a printed summary of what was removed. Two documents of the Kind in the same manifest are refused before touching anything (it is a node singleton — always the same `policy.json`, whatever the `metadata.name`). Read via `get runtimepolicies`/`describe runtimepolicy`. **Baseline correction**: the "advisory process" was never genuinely missing — `SECURITY.md` has existed since 2026-07-27, BEFORE this 2026-08-25 baseline; this line was wrong from the day it was written, not a code regression. What is missing is narrower: a written process for the CVE FEED that `image scan --update` consumes. | CVE feed maintenance process (`image scan --update`) | 7 gates + 6 E2E checks + 2 fuzz targets + Kind unit tests (`cmd::policy::tests`) | — | medium | #125, #126, #461, #462 | 3 of 4 |
| **M05** | Desired State and GitOps | `IN_PROGRESS` | `stack` serves **11** verbs: `init apply destroy prune plan ls describe wait validate history rollback`. `plan` does not change state and has `--detailed-exitcode`; 3-way diff with no state file; **persisted revisions** (ADR-0019). **`diff <kind> <name>` DONE (#200, 2026-08-31)** — the three faces (manifest, last-applied, observed) side by side for ONE named resource, its own top-level verb, not `plan` under another name. **`drift` DONE (2026-09-23)**: its own top-level verb, with no named target and **with the manifest OPTIONAL** — compares each resource's `delonix.io/last-applied` stamp against what is observed, which is the question `plan` cannot answer once someone has edited the file. Kinds only enumerable from the document are NAMED in the output, never omitted; a resource with no stamp is a third state counted separately. **`reconcile` still missing**, continuous, which brings up the daemon question. `apply` remains fail-fast with no rollback, by design, and no longer leaves invisible orphans. | optional rate-limited reconciler (brings up the daemon question: its own ADR) | chaos `stack_converge` + `stack_partial_apply`; **19 E2E checks** for `history`+`rollback`, one of them deleting `stacks/` and another requiring the full cycle; 8 `cmd::drift` tests (one of them reading the `stack.rs` wiring) + 4 `drift` E2E checks | M01 | medium | #120, #121, #122, #200, #470 | 6 of 6 |
| **M06** | Fleet management | `BLOCKED` | `node`/`cordon`/`drain` **do not exist** (0 occurrences). **ADR-0010 REJECTED the remote management API** (2026-08-10) and `AGENTS.md` states that `delonix node add` is blocked. | **successor ADR** naming the concrete consumer — see the note below | — | ADR | — | — | blocked |
| **M07** | OTel observability | `PARTIAL` | OpenTelemetry **0.32** and `prometheus-client` in the tree (`delonix-runtime-core`); `/metrics` on `delonix-cri` and `delonix-mgmt`; `system events`; `dash --json`. **Without** `observe`/`trace`/`diagnose` as commands; correlation and a sanitized bundle still to do. | `observe`/`diagnose` commands; versioned event schemas; measured overhead | — | M01 | medium | — | 2 of 4 |
| **M08** | SLOs and health | `NOT_STARTED` | `slo` has **0 occurrences** in the whole CLI. `--health-*` exists on `run` and compose probes. No SLI/SLO, no error budget, no reason codes. | everything | — | M07 | medium | — | 0 of 5 |
| **M09** | MicroVMs and isolation | `PARTIAL` | 3 backends (`cloud-hypervisor`, `libvirt`, `proxmox`) behind `VmBackend` **with a registry**; ADR-0006 fixed `type: microvm`; snapshots on both local backends. **Without** `isolation: {container,microvm,vm,auto}` (0 occurrences of `isolation` in `cmd/workload.rs`). | an `isolation` axis with an explainable `auto`; Firecracker/Kata evaluated; SEV-SNP/TDX capability model | 28 `vm` leaves; CH section in `e2e.sh` | M01 | medium | — | 2 of 5 |
| **M10** | Plugin SDK and providers | `PARTIAL` | **One** provider trait in the whole workspace: `VmBackend` (`delonix-vm`), with a registry and `BackendRegistration`. Real providers: Proxmox (ADR-0008), TrueNAS (ADR-0009). **Without** versioned interfaces for network/storage/secrets/tunnel/registry, no contract tests. | versioned SDK; shared contract tests; plugin isolation and signing | `crates/providers/delonix-proxmox/tests/live.rs`, `delonix-truenas/tests/live.rs` | M01 | medium | — | 1 of 5 |
| **M11** | Proven performance | `IN_PROGRESS` | **Recount DONE on 2026-08-25** with `scripts/bench.sh`: docker **299 ms**, podman **277 ms**, delonix **80 ms** on default (median of 10, dispersion published). The harness refuses to run on a loaded bench — and refused on the first attempt, with load 55-72. **Regression threshold DONE (2026-09-23)**: `scripts/bench_gate.py` compares against the MACHINE's baseline (`scripts/bench_baseline.json`, one entry per bench) and has THREE outcomes — 0 within tolerance, 1 regression, **3 refuses to judge** (unpublishable run, no baseline for the machine, too scattered a line, or the anchors degraded as much as the engine). It is the 2026-08-10 lesson turned into code: docker and podman are anchors, and a shared degradation belongs to the bench, not the engine. `bench.sh --json` used to be accepted and ignored and now actually works; `time_n` now checks the exit status (a tool that was installed but unusable used to land in the table as the fastest); and every line publishes the DISPERSION (max/min), with a 3× ceiling to judge and 1.5× to record — measured once, a run at load 6.44 gave docker 434→5407 ms. This machine's baseline: docker 224 ms, podman 279 ms, delonix 87 ms. In CI it stays `skipped` on a hosted runner (userns blocked, like chaos), and the job says so. | measure 4b/4c on another bench to attribute the difference (still without a cause) | `bench.sh` + 2 E2E checks + 18 `scripts/test_bench_gate.py` tests, now run by the `script-tests` job (which did not exist: no `scripts/test_*.py` was running in CI) | M01 | medium | #125, #126 | 3 of 4 |
| **M12** | Developer Experience | `PARTIAL` | `completion`, `explain`, `man`, `syntax`, `init` with **11 templates**, `install.sh` with signature verification, classed exit codes (v0.49.0). **`doctor` DONE** (baseline was wrong — it already existed): `system doctor [--strict]` checks real host prerequisites (`br_netfilter`+`bridge-nf-call-iptables`, cgroup2 delegation, conntrack/ARP tables, ephemeral port range, `subuid`, minimum publishable port, presence of `slirp4netns`/`nft`/`ip`), read-only, `--strict` for a CI/provisioning gate. **Stable error IDs DONE** (baseline also wrong): the `DX-CDNN` dictionary (ADR-0043, *Accepted* 2026-09-17) with **198 codes** registered in `delonix-model::codes`, `delonix explain DX-NNNN`/`explain codes --json`. **Without** hot reload/file sync (a `docker compose watch`/skaffold-style flow for the local loop); local→CI→k8s golden paths still unmeasured end to end. | hot reload/file sync for the local development loop; local→CI→k8s golden paths measured | 329 E2E checks (incl. `system doctor` and `explain`/`explain codes`) | M01 | low | — | 3 of 4 |
| **M13** | Maturity and certification | `IN_PROGRESS` | **`system features`**: 13 capabilities with a level from E to the evidence supporting it, with gates refusing a line with no evidence, blocking `certified` (the matrix defining it does not exist) and requiring maturity ordering. `cli-stability.md` + signed releases with SBOM and provenance. **`release verify` DONE (2026-09-22)**: `scripts/release_verify.py` reads `system features -o json` (the SAME table, never a copy) and, for every `stable`+ capability, extracts the file references from its evidence and confirms they still exist in the tree — fails before a tag is cut, not after a reader finds out. `--report` generates the `conformance report` in Markdown, timestamped to the commit, to travel alongside a release's SBOM/provenance. New CI job (`release-verify`). **Without** per-level release gates (e.g. a `production-ready` level also requiring that chaos scenarios have run) — only the rotting-evidence check is done. | per-level release gates (chaos/e2e mandatory per level) | 5 gates + 5 E2E checks + 10 unit tests (`scripts/test_release_verify.py`) | all | medium | #126 | 4 of 5 |

## M06 — why it is `BLOCKED` and not `NOT_STARTED`

The difference is not one of degree. `NOT_STARTED` says «nobody got there
yet»; here someone did, measured, and **decided against it**: **ADR-0010 is
`Rejected`** (2026-08-10), with the reason written — of the consumers it
listed, the evidence pointed to the fleet control-plane, and that is
`delonix-paas`; *remoteness* with no identity, authorization and audit is not
*remoteness* worth having. `AGENTS.md` repeats this and adds the reopening
condition: **a successor ADR naming the concrete consumer**, never a command.

Two consequences, and neither is negotiable within this programme:

1. **Implementing `node add`/`cordon`/`drain` now would contradict an
   accepted decision of the repository**, and the house rule is that an ADR
   is superseded by another ADR — not revoked by a line in another document
   nor by a feature request. It is also what the programme's own rule 9
   mandates («preserve contracts») and what the guard at the PaaS boundary
   requires (the engine has no notion of tenancy).
2. **The M06 work that does NOT depend on the decision can proceed** and is
   not blocked: node identity, mTLS and the capability model already have
   ADR-0003 in `Proposed`. What is held back is the command surface and the
   controller.

The unblocking is a piece of M01 (boundaries), not of M06: write the
successor ADR naming the consumer, or formally record that M06 is out of
scope for this engine. Until either happens, M06 remains `BLOCKED` — and
will **not** count as `DONE` by default.

## M05 — what Phase B closed, and what remains

**Closed:** an `apply` dying midway left, ownerless, everything earlier
layers had created — the stamp was only written at the end. Measured against
`b465300`: the following `stack plan` did not mention the resource at all,
and `stack destroy` took the stamped siblings and left it behind. In a tool
whose `destroy` promises «remove everything this stack owns», that is a
manifest-driven resource leak. Gate: the `stack_partial_apply` chaos
scenario, which runs the **full cycle** — each isolated command returns the
same with and without the defect; what tells them apart is whether `destroy`
reaches the orphan.

**Not closed, and this is a design decision, not debt:** `apply` remains
fail-fast with **no rollback**. Undoing midway is worse than stopping, and the
code itself says so in three places. What is missing for a real way back is
not transactional rollback — it is `history` + `rollback`, i.e. **persisted
revisions**, which is M05's next slice and is new surface (where they live,
what a revision stores, what is reversible and what is not).

**A reasoning correction recorded in the code.** The first written
justification for excluding `Update`s from the save stamp was wrong — it
said the change would read as «already applied» and would be lost. Reading
`diff_fields` showed otherwise: for a field the manifest still declares, the
desired value is compared against the machine and `last-applied` is never
consulted. The real reason is different and comes later — `last-applied` is
what authorizes **reverting** a field that has been dropped from the
manifest, so stamping it without having applied it makes the engine claim
authorship of a value it did not set.

## M05 — revisions (ADR-0019)

A stack now records **one revision per apply**, in
`<root>/stacks/<stack>/revisions/`: the rendered manifest plus a header
(number, timestamp, path, whether it succeeded, plan counts). Failed applies
are recorded **and marked** — after an incident the question is what the
machine was TOLD to do, not what it managed to do.

**The property that sets this apart from a `terraform.tfstate`, and is the
sole reason the ADR was accepted:** a `.tfstate` is the source of truth about
what exists, so when it drifts the tool acts on a lie. Here nothing is read
to decide what exists — ownership and the 3-way diff still come from the
stamp on the resource itself. **Deleting `<root>/stacks/` and `plan`/`apply`/
`prune`/`destroy` still work**, and that is not a claim: it is three checks
of the E2E battery that run right after an `rm -rf`.

Retention of 20, pruned by the writer (there is no daemon), and
**best-effort**: a revision that fails to write never fails an apply that
otherwise succeeded.

## M05 — `rollback` (the slice after revisions)

A rollback **IS an apply**: it replays the revision's manifest through
`apply_docs`, the same path a normal apply follows — not a second path that
would diverge from it. It gets its own revision, marked with which one it
replicated, otherwise the history would show two identical entries without
saying the second was a deliberate step back.

**What it cannot undo is printed BEFORE running**, and counted from this
invocation's own plan rather than being a generic warning: resources created
after the target stay (they only leave with `--prune`), a recreated resource
comes back **empty** (the registry keeps the manifest, never the bytes), and
a cold field still needs `--replace`. A FAILED revision is refused as a
target.

**The gate's own verification found a defect in the gate itself.** The
refusal check measured the exit code, and with the refusal disabled it still
passed — replaying a manifest that does not apply also fails, so `rc` does
not distinguish «refused up front» from «tried and blew up midway», which is
the whole feature. It now requires the phrase only the refusal produces.
Without the reversal pass, there was a green gate guarding nothing.

## M02 — the Docker matrix's third state

The published matrix had **two** states: served, and refused with a reason.
Missing was what this repo writes as a rule and was not following — the
**missing** state. A reader of the two lists cannot tell «not implemented»
from «nobody thought of this», and the difference decides whether they wait
or go elsewhere.

Measured: `POST /images/create` (the pull almost every tool does FIRST) and
`GET /containers/{id}/stats` did not appear in any list. Two network routes
that `kind` also calls were missing too, because the generic `/networks`
does not match `/networks/create` or `/networks/{id}`.

`API_UPSTREAM_USED` closes it with one source per line — the capture of the
52 real-`docker` invocations during a `kind create cluster`, transcribed in
`AGENTS.md`, and the compose sequence this layer was built against. A line
with no capture of its own (`stats`) **says it has none**. A gate requires
every route in that list to be classified; it failed with 4 when it was
written.

What the table now shows, and did not before: of the **21** routes real
tools call, **8 are refused** — including pull. That is what separates «has
a Docker API» from «Testcontainers runs against this».

## M02 — `POST /images/create` is no longer missing (2026-09-22)

The gap, closed: `pull_from_registry_with_creds_full` already existed
(per-layer progress callback, used by the CLI); what was missing was for
`serve docker-api` to translate that callback into the chunked-JSON format a
Docker client knows how to read (`{"status":...,"progressDetail":{...},
"id":...}` per line) and to return the response as a STREAM instead of
waiting for the whole pull to finish before writing a byte. `handle`
(`cmd/dockerapi.rs`) gained a second response body type
(`BoxBody<Bytes, Infallible>`, alongside the usual `Full<Bytes>`) for this
route only — the rest still return a single chunk, no shape regression.

**Validated with a REAL client, not just `curl` against the raw stream**:
`docker pull alpine:3.20` and `docker pull nginx:alpine` (8 layers, one
26 MB) through `DOCKER_HOST=unix://<socket>` pointed at this server, `docker
version` 29.8.1 — both finished successfully and `docker images` showed the
images afterward. A nonexistent-reference error is reported INSIDE the
stream with HTTP 200 (`{"error":...}`), the same contract as real Docker —
the client decides failure from the content of the last line, not from the
status code. A missing `fromImage` still returns `400` before the stream
starts (there is nothing to stream yet).

The `id` of each progress line is synthetic (`layer-<position>`, not the
layer's real digest) — the engine's callback does not carry the digest, only
the position, and a real client (tested) only uses the field to draw a
progress bar per line, never to compare it against anything. Private
registry credentials (`X-Registry-Auth`, base64 JSON
`{username,password}`) are accepted; the `identitytoken`-only form has
nowhere to go (this engine's vault only stores username/password pairs) and
falls through to `None` (tries anonymous / `delonix image login`).

## Execution order

The programme's own order, with one justified change: M01 delivers the
**gates** first, and only then the prose. An architecture document with no
gate was exactly what left 3 crates out of the C4 without anyone noticing.

1. **Phase A** — M01 + this matrix.
2. **Phase B** — M04 and the reliability P0s (the `apply` with no rollback
   is the biggest).
3. **Phase C** — M02, M03 and the M13 foundation.
4. **Phase D** — M05, M07, M08.
5. **Phase E** — M09, M10.
6. **Phase F** — M06, **if and when** the successor ADR exists.
7. **Phase G** — M11, M12 and closing M13.

## Changelog

| Date | Change |
|---|---|
| 2026-08-25 | Initial baseline measured against `b4653002b`/v0.63.1. M01 moves to `IN_PROGRESS`: `tests/architecture.rs` installed, 3 crates restored in the C4, count corrected in two documents. |
| 2026-08-25 | M13: `system features` — maturity derived from evidence. M04: `policy.json`, the ceiling the node enforces at the admission point. M11: recount DONE (the host's load dropped) — the 2026-08-13 ratio survived, and two harness method errors were caught while doing it. |
| 2026-08-25 | M04: SPDX 2.3 SBOM generated from `Cargo.lock` and SLSA provenance in releases, both inside the signature. Risk drops from high to medium. |
| 2026-08-25 | M11: `scripts/bench.sh` — the lesson from the discarded battery becomes code, and the harness refuses to measure on a contaminated bench. The recount was BLOCKED for lack of an idle machine, with the reason measured. |
| 2026-08-25 | M02 moves to `IN_PROGRESS`: the Docker matrix gains the third state (the «missing» one) and a gate enforcing it. Baseline corrected — it was 14 served, not 21. |
| 2026-08-25 | `stack rollback`: revisions now have a way back. M05 ends up with 11 verbs and continuous `reconcile` as the only serious item still missing. |
| 2026-08-25 | Phase D (brought forward). `stack history` + ADR-0019: persisted revisions, with the property «deleting `stacks/` breaks nothing» fixed by a gate. M05 rises to 5 of 6. |
| 2026-08-25 | Phase B. M05 moves to `IN_PROGRESS`: closed the resource leak of a partial `apply` (`salvage_ownership`), with the `stack_partial_apply` chaos scenario fixing it. M05's risk drops from **high** to medium — what remains (`history`/`rollback`) is missing surface, not data loss. |
| 2026-09-22 | M02: `POST /images/create` implemented (chunked-JSON streaming, delegating to the existing pull) and moved from `API_UNIMPLEMENTED` to `API_MATRIX`. Validated with a real `docker` CLI (29.8.1) — `docker pull` of two images through the socket, including one with 8 layers/26 MB. M02 rises to 3 of 4. |
| 2026-09-22 | M04, proof of the value of the freshly merged fuzz job: on the first real CI run, `parse_dockerfile` found an `out-of-memory` — an `ARG B=${A}${A}` chain doubles the stored value on every line (exponential expansion bomb), reachable from a `Dockerfile`/`Delonixfile` in a cloned repository, before any container exists to contain it. Fixed with a 1 MiB ceiling in `substitute_vars` (`delonix-oci::build`), shared by both callers (`ARG` in parsing, `ENV` in `delonix-runtime-bin::cmd::build`). Regression tests on both sides; confirmed live with the fuzzer itself (2.8M executions/45s with no crash, after the fix). |
| 2026-09-22 | M03: automatic SPDX 2.3 SBOM per built image. `delonix_scanner::sbom_spdx` reuses the already-existing `extract_sbom` (apk/dpkg, no mounting or running the image); `delonix build` writes it best-effort right after a successful commit, and `delonix image sbom <ref>` serves it (instant cache hit, or computes-and-stores for a pulled image). Validated LIVE against the release binary: `delonix build` of an `alpine:3.20` + a real `apk add curl` produced an SBOM with 25 packages (including `curl`), and `image sbom` returned the file byte-for-byte identical to what the build had already written (proof of the cache hit, not recomputation). M03 rises to 2 of 4. |
| 2026-09-22 | M12: baseline corrected — `doctor` (`system doctor [--strict]`, 8 host prerequisite checkpoints, one of them iterating over 3 tools) and the error ID dictionary (`DX-CDNN`, ADR-0043 *Accepted* 2026-09-17, 198 codes, `delonix explain`) already existed, and the matrix still said «0 occurrences»/«no error IDs» — the same class of drift already fixed in M01/M02/M05. Confirmed by `grep`/code reading and by the already-existing E2E coverage (`scripts/e2e.sh`, `system doctor` and `explain`/`explain codes` sections). What remains missing in M12 is only genuinely-unbuilt `deliverables`: hot reload/file sync for the local loop, and local→CI→k8s golden paths measured end to end. DoD kept at 3 of 4 — not enough confidence in the original counting methodology to invent a new numerator. |
| 2026-09-22 | M03: `RUN --mount=type=cache,target=<path>[,id=<name>]` — a persistent, read-write directory, surviving between builds, generalizing the already-existing `type=secret` mechanism (`mount_live`, invisible to the final image). Directory at `<root>/build-cache/mounts/<sha256(id)>` — hashed, never the raw string (an omitted `id` defaults to `target`, an arbitrary path controlled by the Dockerfile). `sharing=`/`ro` refused instead of silently accepted-and-ignored. Fixed, along the way, a pre-existing LANG-01 debt: this `--mount` parser's error messages had been in Portuguese all along, never revisited. Validated LIVE against the release binary: two separate `delonix build`s (different tags) wrote to the SAME cache mount and the second build saw what the first had written; the cache mount's content never appeared in the final committed image (confirmed by `container run` inside the resulting image); `type=ssh`/`sharing=locked`/bare `ro` refused with clear English messages. M03 rises to 3 of 4 — only `type=ssh`, cache shared across nodes via registry, per-image signed provenance, and the measured comparison with BuildKit remain. |
| 2026-09-22 | Two baseline corrections, found while auditing where else the matrix might be lying: M04 "advisory process" was never genuinely missing — `SECURITY.md` has existed since 2026-07-27, BEFORE this document's 2026-08-25 baseline; the line was wrong from the day it was written, not a code regression — what is missing is narrower, a written process for the CVE feed that `image scan --update` consumes. M05: `delonix diff <kind> <name>` (#200, 2026-08-31) is already its own top-level verb, with the three faces (manifest/last-applied/observed) for a named resource — not `plan` under another name, as the matrix still said; only `drift` (with no named target) remains to be done. M05 rises to 6 of 6. |
| 2026-09-22 | M04: `kind: RuntimePolicy` — the node's admission ceiling (`policy.json`) gains a declarative form. `stack apply` raises/tightens it (converges hot, every field is hot); never lowers it (`teardown: false`, `stack destroy`/`--prune` explicitly refuse to touch it). `delonix policy unset` is the only verb that removes it — asks for confirmation (or `--force`) and always prints what was there before deleting. Kinds in the manifest: 19 → 20. Validated live (`DELONIX_ROOT` isolated): `container run --privileged` ACTUALLY refused by a manifest-applied policy; reapplying with a changed field converges in place (`stack plan` shows `~`, not `-/+`); `stack apply --prune` and `stack destroy` leave `policy.json` untouched; two documents of the Kind in the same manifest are refused before any write. |
| 2026-09-23 | M11: performance regression gate. The non-obvious part is the third outcome: the gate REFUSES to judge when it cannot attribute the difference — and docker/podman are anchors precisely for that. Three harness defects fixed along the way: `--json` accepted-and-ignored, `time_n` not checking the exit status, and no dispersion measure at all (a run at load 6.44 gave docker 434→5407 ms and the median absorbed it). And one dead gate: no `scripts/test_*.py` was running in CI — `test_release_verify.py` was born the day before with ten tests that executed nothing. |
| 2026-09-23 | M05: `delonix drift` — the missing verb. Distinct from `plan` by CONSTRUCTION, not by spelling: it drops the manifest from the comparison and keeps only the last apply's stamp against what is observed, which is the only way the question keeps holding after someone edits the file. The manifest is optional (a node with no repository still answers) and the six Kinds only enumerable from the document are named in the output. A resource with no stamp is a third state, not drift. The `doc_scoped_matches_the_stack_wiring` gate — which reads `stack.rs` instead of trusting a comment — caught my own mistake on the first run: `network_access_rule` is passed `docs` and ignores them, so it did not belong on the list. Validated live with drift triggered by `container update`. |
| 2026-09-23 | M02: `compatibility compose` + `migrate assess`. The third state needed a denominator, and the denominator is the specification's key list — measured one by one against `docker compose config` (docker v29.8.1) instead of written from memory. One battery check cross-checks the 88 classified keys against what `compose config` actually does with each: all agree. Two findings: the `ENGINE_HAS_IT_SERVICE` table I was about to duplicate ALREADY EXISTED (my copy classified as `missing` what the engine refuses with its own message), and it was INCOMPLETE — `sysctls`/`ulimits`/`gpus`/`userns_mode` have a flag on `container run` and fell into the generic error; the test that checked two lines by hand now walks all twelve. |
| 2026-09-22 | M13: `scripts/release_verify.py` — the `system features` table stops being unverified prose. Validates both sides (accepts when the evidence is real, refuses when it is not — sabotaged on purpose with a nonexistent reference and reverted afterward, against the real binary). `--report` generates the conformance report. New `release-verify` CI job. M13 rises to 4 of 5. |
