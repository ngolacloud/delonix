# Leaf inventory — the seven surfaces

> **What this document is.** The denominator of the proof work: every
> invokable unit of every public surface, with the proof class it has **today**
> and the measured source of that classification. It is not a plan — the
> per-surface plan lives in
> [`plano-folhas.md`](plano-folhas.md), and the missing infrastructure in
> [`lab-requisitos.md`](lab-requisitos.md).
>
> **What it is not.** A reading of `--help`. Every "run" below comes from a
> `results.jsonl` from a real run, from a route trace, or from a cited test
> name. Where there is no measurement, it says **not measured** — never
> "probably".

| Field | Value |
|---|---|
| Measured | **2026-10-20** |
| Commit | `2663c846` |
| Engine | `delonix 5.1.1` (`commit: 2663c846f (+49 commits since v5.1.1)`) |
| Host | `delonix`, kernel 7.0.0-38-generic, Intel i9-13900H, 20 threads |
| Binaries | the six from `target/release`. The engine reports `2663c846f`; the sidecars were built minutes before, from a tree that differs **only in `scripts/`** — not one line of Rust between the two states, so they are the same binary in behaviour |

## The classes

| class | meaning | what is missing |
|---|---|---|
| **A** | run and asserted in a real environment | only the Phase 2 case matrix |
| **B** | exists, implemented, but no run exercises it | making it runnable |
| **C** | run only with mocks or only in one encoding | taking it to real |
| **D** | declared but `UNIMPLEMENTED`, or with a known defect | red test + fix, or an ADR to remove/defer |
| **E** | needs infrastructure the lab does not have | see `lab-requisitos.md` |

## Counts — surface × class

<!-- folhas:counts:begin — verified against docs/testing/folhas.tsv by scripts/folhas_gate.py -->

| surface | leaves | A | B | C | D | E | denominator source |
|---|---:|---:|---:|---:|---:|---:|---|
| cli | **371** | 272 | 88 | 0 | 0 | 11 | `scripts/cli_baseline.tsv` (gate `rc=0`) |
| node-api | **69** | 18 | 7 | 15 | 29 | 0 | `proto/delonix/node/v1/*.proto` (`rpc <Name>`) |
| mgmt-api | **32** | 1 | 6 | 19 | 6 | 0 | `delonix-mgmt/src/lib.rs` (`.route("…"`) |
| mcp | **15** | 0 | 5 | 10 | 0 | 0 | `delonix-mcp/src/lib.rs` (`name = "<tool>"`) |
| cri | **34** | 24 | 3 | 5 | 2 | 0 | `delonix-cri/proto/api.proto` (`rpc <Name>`) |
| kinds | **165** | 40 | 90 | 8 | 23 | 4 | `kinds.rs` `FACTS` — 33 kinds × 5 verbs |
| **total** | **683** | **355** | **199** | **57** | **60** | **12** | — |

<!-- folhas:counts:end -->

> **These numbers are those of `folhas.tsv`, and `scripts/folhas_gate.py` fails
> if this table stops matching them.** They were written by hand once and
> diverged at the first review — the CLI said 7 `B` and 12 `E` where the
> measurement says 9 and 10 —, which is precisely the defect of a line whose
> state lives in two files with nobody comparing them.

Three denominator notes, because the count above is the **gate**'s and there
are legitimate different readings:

- **Node API: 59 RPCs, not 116.** The scope counts gRPC and REST as separate
  leaves, and by that rule it would be 59 + 57 mapped. The gate counts the
  **RPC**, once, because that is the unit the `proto` defines and the one
  `contract_gate.py` already guards. The gRPC encoding gap does not disappear
  because of this — it is recorded as **E1** work in `plano-folhas.md`, and is
  worth 57 leaves.
- **Mgmt API: 32 paths, 40 operations.** The `.route()` gives the path; some
  carry more than one method (`/v1/volumes` serves `GET` and `POST`). 40 is
  the per-operation count, and it is the one ADR-0042 and `55_API_UNICA.md`
  publish.
- **Providers fall outside the gate.** ~430 operations across the nine
  crates, of which ~196 have live proof. Their denominator is a published
  HTTP schema or a `virsh help` of a named version, and it already has its
  own instrument (ADR-0049); a second, worse extractor would produce a second,
  conflicting number. See §7.

---

## 1. CLI — 300 leaves

`scripts/cli-tree.sh --count` → **316 commands · 273 leaves · 40 groups**;
restructuring classes 169 `~`, 85 `=`, 17 `→`, 2 `!`. `--gate` **rc=0**.

`scripts/cli_exec_ratchet.py` **rc=0** → **263 of 273 invoked under assertion —
96.3%**. Trace provenance: 2026-10-20, commit `2663c846`, isolated root,
`PASS=1340 FAIL=0 SKIP=16`.

> **Update of 2026-10-22 (merge of `origin/main` `4b860b51`).** The CLI
> grew to **280 leaves** (`gitops reconcile/resume/status/suspend`,
> `network vip-pool clear/set/show`), and the committed trace became the one
> from `main`: 2026-10-21, `6dc31582`, `PASS=1537 FAIL=0 SKIP=15`, **268 of 280**.
> The seven new leaves are in that trace and are **A**; `vm pause`/`vm unpause`
> too (the `main` battery had a libvirt VM running) and moved out of **E**.
> The four this branch promoted — `net l4guard set/clear` and `system boot
> enable/disable` — went back to **B**: the checks that drive them are in
> `scripts/e2e.sh`, but the `6dc31582` trace predates them, and the trace is
> only re-recorded from a real run. They go up to **A** on the next recording.
> **Done the same day**: the trace was re-recorded from the battery run on
> the merge (`fa34383b`, `PASS=1577 FAIL=0 SKIP=14`), **272 of 280**, and the
> four are at **A**. The text below
> describes the state of 2026-10-20 and stays as it was.
>
> **Update of 2026-10-22 (Docker/Podman root verbs, #930/#933).** The
> CLI grew to **291 leaves**: the 10 root shortcuts
> `run/create/ps/start/stop/restart/rm/exec/logs/pull` and the canonical
> `container create`. The 11 are at **B**, not **A**: the committed trace
> (`fa34383b`) predates them and the battery only exercises the canonical
> form `container <verb>`/`image pull` (those are **A**), never the root
> shortcut. They go to **A** when a check drives the root form and the trace
> is re-recorded from a real run.
>
> **Update of 2026-10-22 (Docker/Podman table completed).** The CLI
> grew to **300 leaves**: the 9 root shortcuts that were missing —
> `kill/attach/stats` (container) and `images/rmi/tag/save/load/push` (image).
> Same reason as the previous ones: they are at **B**, share the struct and
> the handler of the canonical form `container <verb>`/`image <verb>` (those
> are **A**, or **E** in the case of `image push`), and the battery exercises
> the canonical form, not the shortcut. They go to **A** when a check drives
> the root form and the trace is re-recorded.

The 10 that no run executes, and what **each one** needs — measured, not
assumed. Each group's siblings already run, and that is what makes them
cheap: `net netns` (10/10), `net l4guard` (3/3) and `system boot` (3/3) have
been **complete** since 2026-10-20, `vm image import/convert/describe/ls/rm/init`
and **19 of 21** `cluster` leaves are already run. The remaining 10 are
**all** class `E` — none is a coverage gap, all are waiting on
infrastructure.

| leaf | needs | class |
|---|---|---|
| `vm pause` | running VM + bootable image in the isolated store | **E** (only the image) |
| `vm unpause` | same | **E** (only the image) |
| `vm image build` | `virt-customize` (present) + download of the cloud image | **E** (network) |
| `cluster create` | `kindest/node` image; `kind`+`kubeadm` present, runs the nodes **on the engine itself** (no Docker) | **E** (image) |
| `cluster kubeadm` | golden image + real VMs | **E** |
| `image push` | OCI registry | **E** |
| `image sign` | OCI registry (the signature is published alongside the image) | **E** |
| `vm image pull` | OCI registry | **E** |
| `vm image push` | OCI registry | **E** |
| `vm image ls-remote` | OCI registry (tag listing) | **E** |

**Five of these leaves were a missing package, and no longer are.** The
battery **already had** checks for `net netns exec/detach/firewall/publish/unpublish`,
in `scripts/e2e.sh`, since `5c62db13` (2026-10-11), and are among the best
there are here — they judge **on the wire**, with a token that A answers and
B consumes, because "a listener on a published port accepts the connection
whether or not anyone is answering behind it". They counted as zero because
the whole block skipped on a `command -v socat` and `socat` was not on the
host, not because they did not exist: the block was already there when the
`e5be0903` trace (2026-10-16) was recorded.

Measured on 2026-10-20: `socat` **is** at `/usr/bin/socat`, the block runs,
and the five leaves moved to **A** without a line of code — they converted
by installing a package, exactly as `lab-requisitos.md` predicted as the
cheapest item on the whole list.

**None of the 10 is a stub.** `bins/delonix-runtime-bin/src/` has no
`unimplemented!` nor a `todo!` — so none is class `D`. And `vm pause`/`vm
unpause` are real in all **three** backends: `delonix-provider-cloud-hypervisor`
(`:847`/`:851`), `delonix-provider-libvirt` (`:3254`/`:3268`) and `delonix-proxmox`
(`:6488`/`:6497`); the trait's default fails closed on purpose
(`vm_backend.rs:570` — "never as a silent no-op").

**`image sign` does not need `cosign`**: it has its own ECDSA-P256, with the
key generated at `<DELONIX_ROOT>/keys/image-signing.key`. Only signature
*interoperability* would require it.

---

## 2. Node API — 59 RPCs, in both encodings

Contract `proto/delonix/node/v1/{node,infra,compute,operations,common}.proto`.
The REST routes are **generated**, not hand-written
(`delonix-node-api/build.rs:283-356` → `$OUT_DIR/rest_routes.rs`), and
`scripts/contract_gate.py:rpc_mappings()` requires `google.api.http` on every
non-client-streaming RPC and **forbids it** on the two bidirectional streams.

| | n |
|---|---:|
| RPCs | **59** across 9 services |
| mapped to REST | 57 (2 with no mapping on purpose: `Exec`, `Console`) |
| server-streaming that answer 501 in REST | 3 (`WatchEvents`, `WatchOperation`, `Logs`) |
| implemented | **30** |
| **not implemented** | **29** |
| with ≥1 test invoking them | 35 |
| **run in a real run** | **16** (15 positive + `StartContainer` only as 501/405) |
| **never invoked by any test** | **24** |

The 29 not implemented are **entire** services: `ContainerService` (12),
`PodService` (4), `ImageService` (7), `StackService` (3) — with no handler and
not even registered in `service.rs:router()` (:519-549) —, plus
`ConnectContainer`, `DisconnectContainer` and `Console`, which are registered
and return `service.rs:unserved`. **Class D, and it is the biggest open
decision in this inventory** (see "Decisions I need from you").

**Structural gap (class C, entire surface):** the battery exercises **only
the REST encoding**. Every call is `curl --unix-socket`; there is no
`grpcurl` nor a gRPC client in `scripts/` or `tests/`. gRPC is only exercised
by `cargo test` component tests. So the 57 mapped RPCs have no gRPC leg in a
real run.

Implemented but with the success path **untested in any layer** (they only
go through `T2`, where every assertion is a `NOT_FOUND` refusal):
`ResumeVirtualMachine`, `ListSnapshots`, `RestoreSnapshot`, `DeleteSnapshot`.
And the **entire `VirtualMachineService`** (12 implemented RPCs) never enters
the real battery.

Partial (narrower class D): `CreateNetwork` refuses overlay topology;
`CreateVolume`/`DeleteVolume` refuse share/remote/pool/`destroy_remote`;
`WatchEvents` refuses a namespace filter; `CancelOperation` is implemented
but **by design never cancels** (only `NOT_FOUND` or `FAILED_PRECONDITION`
DX-5002).

`Idempotency-Key` **is** honoured here (`service.rs:595`) and genuinely
measured by `scripts/e2e_node_network_ops.py` (`scenario_replay`).

---

## 3. Mgmt API — 40 operations

Router and handlers in a single file: `delonix-mgmt/src/lib.rs:router`
(:188-278), 32 flat `.route()`s, no `nest`, no `merge`, no middleware. Starts
via **`delonix serve api`** (not `serve mgmt`), which `execve`s its sibling.

| | n |
|---|---:|
| paths / operations | 32 / **40** |
| implemented | 33 |
| withdrawn (501 `DX-6302`) | **7** |
| `Idempotency-Key` | **0 of 40** — the string does not exist in the crate |
| with ≥1 test | 33 |
| **run over real HTTP** | **1 of 40** (`GET /v1/dash`) |
| with no test at all | **7** |

**Structural gap (class C, entire surface):** `test_state()` (:1463) sets
`bin: PathBuf::from("/bin/false")` (:1470). Every component test covers
**only pre-exec validation and refusals** — **no route that shells out has a
tested success path anywhere**.

The 7 with no test at all, in order of risk:
`POST /v1/containers/{id}/exec` (**the engine's highest-privilege route**),
`GET /v1/containers/{id}/logs`, `GET /v1/images/scan`, `GET /v1/images/sbom`,
`DELETE /v1/networks/{name}`, `GET /v1/net/dhcp6/{net}/{mac}`,
`POST /v1/vms/{name}/action`.

**Security:** no TLS and no TCP — HTTP over `AF_UNIX`, socket at `0600` and
`SO_PEERCRED` on every accept (`:167-169`); a mismatched pair is silently
dropped, so there is no HTTP-level 401/403. No tokens and **no
per-route authorization**: any process with the server's euid has the whole
surface, including `exec`.

**Scope note that changes the priority:** ADR-0042, steps D-E/F (:131-132,
:148), plans to migrate every mgmt path to the node contract and **delete
`delonix-mgmt`**. Investing 40 leaves of proof in a surface marked for
removal is your decision, not mine.

---

## 4. MCP — 15 tools

All in `crates/interfaces/delonix-mcp/src/lib.rs` (router `#[tool_router]`,
:297-768). **stdio-only** transport — `delonix-mcp-bin/src/main.rs:28-33`
refuses any other ("ADR-0025 defers loopback HTTP").

| | n |
|---|---:|
| tools | **15** |
| implemented | **15** (none is a stub) |
| **invoked by the real protocol** | **0** |
| with a test that invokes the handler | **1** (`audit.query`) |
| with no test at all | **5** |

**`tools/call` does not exist anywhere in the repository.** The only
protocol test (`scripts/e2e.sh:6974`) sends `initialize`,
`notifications/initialized` and `tools/list`, and asserts that `serverInfo`
exists and that `"logs.query"` is in the name list. **No tool body has ever
run through the protocol.** Class B/C for the entire surface.

With no test at all: `runtime.info`, `runtime.health`, `resources.get`,
`metrics.query`, `network.inspect`.

**resources** sub-surface, equally untouched: `read_delonix_uri`
(:789-817) and `resources/list` are not touched by any test;
`delonix://schema/{kind}` is an explicit `NOT_IMPLEMENTED` (:839-846).

**No ratchet guards the tool list** — a tool can enter or leave without any
gate noticing (`cli_baseline.tsv` only has the CLI's 3 leaves
`mcp capabilities|doctor|serve`).

Two findings to confirm: `task.get` and `task.cancel` **never call
`self.log(...)`** (:723-746), against ADR-0025 §8's promise of one audit
line per call — and the test comment
`audit_query_logs_its_own_call` claims that `audit.query` "was the only one
of fourteen" without `self.log`, which is no longer true. And ADR-0025 says
`events.list`/`security.explain` "return a clear not-implemented error":
they do not exist, a repo-wide grep returns zero.

---

## 5. CRI — 34 RPCs

| | n |
|---|---:|
| RPCs (29 Runtime + 5 Image) | **34** |
| complete | 22 |
| partial | 10 |
| `UNIMPLEMENTED` | 2 (`CheckpointContainer`, `GetContainerEvents`) |
| with no handler | **0** — the generated trait is exhaustive |
| exercised by `critest` | 24 |
| **with no coverage from any source** | **5** |

With no coverage at all: `PodSandboxStats`, `ListPodSandboxStats`,
`CheckpointContainer`, `GetContainerEvents`, `ImageFsInfo`.

**Two recorded baselines, and they contradict each other** — the repo itself
already recorded it as gap **G-025 / STALE EVIDENCE**:

| | published | with raw logs |
|---|---|---|
| file | `docs/cri-conformance.md` | `reports/production-readiness/v3.1.0/cri-conformance.md` |
| engine | **v0.63.1** | commit `fa8c6297` |
| date | 2026-08-25 | 2026-09-15 |
| result | **79 Passed · 24 Failed · 19 Skipped** | **82 Passed · 21 Failed · 19 Skipped** |

The 79/24 is what `README.rst`, `docs/cri.html` and `docs/gen.py` publish.
**Neither one is from the current version (5.1.1)**, and between the two
`ListMetricDescriptors`/`ListPodSandboxMetrics` went from stub to real
(commit `343e0447`). Of the 21 named failures: host environment 10, rootless
limitation 3, **Delonix defect 3**, missing feature 2, deliberate divergence
1, **unknown 2**.

> **RESOLVED (re-measurement of 2026-10-24).** critest v1.36.0 was re-run on
> v5.1.1 (`main @ ab4751ac`, the line that ships today as 0.1.0): **84 Passed ·
> 20 Failed · 18 Skipped** — surpasses both baselines above. `docs/cri-conformance.md`
> and `docs/gen.py` were updated to this number; open question #3 below is
> now closed. Report: `reports/cri-conformance-2026-10-24/00-audit-report.md`.

**`critest` is not run by any automation**: it does not appear in the
`Makefile` nor in any workflow. It is a manual measurement.

**Contract drift:** 7 RPCs of critest v1.36.0's `cri-api` do not exist in the
repo's proto (`StreamContainers`, `StreamContainerStats`, `StreamPodSandboxes`,
`StreamPodSandboxStats`, `StreamPodSandboxMetrics`, `UpdatePodSandboxResources`,
`ImageService/StreamImages`). No document acknowledges this.

---

## 6. Kinds — 26 kinds × 5 verbs = 130 cells

> **Update of 2026-10-22.** `GitOpsSource` (ADR-0021) entered the catalog
> after this measurement: `apply`, `plan` and `delete` are **A** (the
> `gitops:` section of `scripts/e2e.sh`), `diff` and `drift` are **B** —
> reachable (`actual()` carries `last_applied`) but no run names them. The
> rest of this section is the 2026-10-20 measurement, with 25 kinds.

Measured with `delonix api-resources` against the tree's binary.

| form | kinds |
|---|---|
| `primary` | **19** — RuntimePolicy, Secret, Network, NetworkRoute, StoragePool, Volume, Image, App, VirtualMachine, SystemContainer, Pod, Service, NetworkPolicy, NetworkAccessRule, NetworkGateway, IPPool, HTTPRoute, Gateway, KubernetesCluster |
| `sunset` | 2 — NetworkZone → Network, Container → Pod |
| `compat` | 1 — Ingress → HTTPRoute |
| `sugar` | 2 — Workload → Container/Pod/VirtualMachine, Dependency → NetworkPolicy |
| `aggregate` | 1 — Stack |

Published groups: `core`, `compute`, `networking`, `gateway`, `storage`,
`artifact`, `infrastructure`, `security` — **eight**, not the seven that
`AGENTS.md` lists (`security.delonix.io/v1alpha1 RuntimePolicy` exists and is
not in the "Engine identity and boundary" section's list).

The source of truth is `const FACTS: &[KindFacts]` in
`crates/contexts/delonix-stack/src/kinds.rs` (:298-714), re-exported by
`cmd/mod.rs:37`; `api-resources` derives from it (`cmd/resource.rs:113`) and
there is no second list — the test
`every_registry_kind_is_listed_and_resolvable_by_its_listed_name` enforces it.

`in_stack: true` on 21 kinds, all `converges: true`; **15** have
`teardown: true`. `DELETE_ROUTES` = 16 (`verbs.rs:80`); `destroy_one` = 15
(`stack.rs:2383`). `provider` blocks in **5** kinds (VirtualMachine, Network,
SystemContainer, NetworkGateway, Gateway) plus Volume's per-vendor equivalent
(`provision.truenas`).

**No kind is untested**: CI (`ci.yml:476-490`) runs
`stack apply --dry-run` + `stack validate` over every `examples/*.yaml`, and
`examples/core/stack.yaml` loads the 23 groups. The gap is one of *real run*.

### The 9 kinds whose `apply` no real run exercises

**App, Service, Ingress, IPPool, Gateway, Workload, Dependency,
KubernetesCluster, NetworkZone.** Partial: **SystemContainer** (only refusals,
e2e.sh §5879) and **NetworkGateway** (ADR-0071 says so in writing: "Not
proven live: the lab OPNsense VM does not boot"). Conditional: **VirtualMachine**
(e2e.sh:3278, skipped without a hypervisor).

`scripts/e2e-all-templates.sh` **exercises no kind at all** — it drives
`init`/`build`/`container run`/`container rm`, the imperative surface.

### Verbs actually exercised

**`Container` is the only kind with all five** (`apply`, `diff`, `plan`,
`delete`, `drift`). The only *named* subjects of `diff`/`drift` in the whole
battery are `diff containers` (e2e.sh:6724-6726) and the Container's drift
cycle (§2476-2523) — inspecting the 18 `drift` and 2 `diff` of `e2e.sh`, none
names another kind. So `diff`/`drift` of `Network` and `Volume` is **co-occurrence
in the same section, not measurement**: it remains **undetermined**.

Nine kinds have **none** of the five verbs exercised: RuntimePolicy,
Service, Ingress, IPPool, Gateway, App, Workload, Dependency and
KubernetesCluster.

### Defects found in the Kinds

1. **`Pod.spec.shareProcessNamespace` is read by nobody and documented as
   working.** `delonix-compute/src/pod.rs:149-150`, accepted in
   `POD_SPEC_FIELDS:308`. The field's own comment says "Honored by
   `kind: Pod` (see `cmd::pod`)", but `cmd/pod.rs:18` says "Shared **PID** …
   lands next" and `delonix-linux/src/lib.rs:6290` agrees it is not
   implemented. It is the **only** parsed-but-ignored field that is both
   unread **and** documented as working — the others are deliberate and
   labelled. Matches `critest` failure #15 (`Security Context PodPID`).
2. **`FACTS` declares the apply order for itself and `run_layers_inner`
   disagrees in two lines.** `kinds.rs:269` says "The order of the rows below
   is the order of the apply", and `teardown_order` (`stack.rs:2429`) derives
   removal by inverting it. But `run_layers_inner` (`stack.rs:2237`) runs
   **IPPool before** NetworkPolicy/NetworkAccessRule/NetworkGateway, and the
   table places it after the three; and **Ingress has no layer of its own**.
   The dependencies the comments cite still hold, so the practical impact is
   **undetermined** — but the two lists disagree, which is exactly the class
   of defect `kinds.rs` exists to eliminate, and **nothing tests it**.
3. **Two generic `delete`s with no written reason.** `NetworkAccessRule` and
   `HTTPRoute` are missing from `DELETE_ROUTES` **and** from `no_verb_reason`
   (`verbs.rs:226` returns `""`), so they answer "not wired yet". The guard
   `a_kind_never_both_routes_and_claims_it_cannot` (`verbs.rs:602`) only does
   `eprintln!` — **never asserts** —, so this debt cannot fail CI.
   `NetworkAccessRule` also has no `get` nor `describe`.
4. **`drift` is structurally silent on 5 kinds** (RuntimePolicy,
   Secret, Image, App, Gateway): `actual()` returns `last_applied: None`,
   so they always count as `unstamped` and never as drift — and this **is not
   stated** in the `drift.rs`'s `DOC_SCOPED`/`not_checked` honesty mechanism.
5. **`IPPool.spec.interface` is unreachable in any accepted document**: it
   only applies with `announce: l2` (`ippool.rs:234`), and `l2` is refused as
   not built (`:242`, ADR-0046 phase 4).

### Effective fields — the number Phase 2 has to cover

The `RECONCILED_*_FIELDS` are what `plan`/`diff`/`drift` compare. The gap
versus the schema is large and is what the `unconverged_fields_condition`
now **names** instead of silently discarding:

| kind | props in schema | effective |
|---|---:|---:|
| VirtualMachine | 60 | **9** |
| Container | 50 | **12** |
| Volume | 13 | 8 (only `quota` is live) |
| RuntimePolicy | 12 | 12 |
| Network | 9 | 7 (only `peers` is live) |
| Pod | 9 | 3 |
| SystemContainer | 9 | 8 |
| NetworkAccessRule | 6 | 6 |
| IPPool | 3 | 3 |

Five `examples/full-*.yaml` files are missing, which are the per-field
reference: `full-storagepool.yaml`, `full-systemcontainer.yaml`,
`full-networkzone.yaml`, `full-networkgateway.yaml` and the `full-volume`
share variants.

---

## 7. Providers — 9 crates

| provider | ops | live.rs | ops covered live | route trace | failure injection |
|---|---:|---|---|---|---|
| Proxmox | ~300 | 45 cases | ~179 routes | **yes** — 184/675 (27.3%) | **yes**, ~80 cases |
| OPNsense | ~24 | 4 cases | 15 ops | no | **yes**, ~45 cases |
| TrueNAS | 5 | 1 case | 5 ops | no | **no** |
| PowerDNS | 2 | 1 case | 2 ops | no | partial (`tests/mock.rs`) |
| libvirt | ~34 | 1 case | **3**: `boot`, `is_running`, `guest_info` | **unusable** | **no** |
| Cloud Hypervisor | ~21 | 1 case | **2**: `hotplug_add_cpu/memory` | no | **no** |
| LVM | ~11 | **no** | **0** | no | **no** |
| ZFS | ~11 | **no** | **0** | no | **no** |
| btrfs | ~11 | **no** | **0** | no | **no** |

Engine cell metric: **103 of 273 applicable proven (37.7%)**
(`capability_ratchet.py` **rc=0**).

**Seven of the nine providers have no failure injection.** Only Proxmox and
OPNsense have it, and the two suites assert from the mock's **request log**
— "the call returned Ok" is never the whole assertion.

**No CI gate** regenerates or checks `docs/proxmox/matrix-9.2.2.md`, and
`lab.yml` never runs a `--test live`: every live case is `#[ignore]` and run
by hand.

### Defects verified by me in this inventory

1. **The `delonix-test-run=<uuid>` label does not exist anywhere.**
   `grep -rn 'delonix-test-run'` across the whole repo → **0 occurrences**.
   It is a requirement from this mission's own briefing, and it is not
   implemented in any of the nine providers. What exists is OPNsense's
   `fresh_mark()` (`live.rs:50-61`) — but that is a **production** ownership
   mark (ADR-0059 D1.5), not a test label; and PID-derived names in the
   others.
2. **`scripts/libvirt_virsh_inventory.py` reads the wrong files.** `SOURCES`
   (:54-57) points at `crates/adapters/delonix-vm/src/lib.rs` and
   `bins/delonix-runtime-bin/src/cmd/vm.rs`. Measured: the provider crate has
   **199** occurrences of `virsh`, the adapter the script scans has **2**.
   So libvirt's numerator comes from the wrong place and ADR-0050's
   **31/276 (11.2%)** is not reproducible against today's tree.
3. **The Proxmox client has no redirect policy.** `connect_with`
   (`src/lib.rs:1201-1225`) sets `connect_timeout`, `timeout`,
   `pool_max_idle_per_host`, `danger_accept_invalid_certs` and
   `add_root_certificate` — and **no `redirect::Policy`**, so `reqwest`'s
   default applies (follow up to 10). OPNsense has the policy
   (`src/lib.rs:245 .redirect(Policy::none())`), the check (`:292`) and the
   test (`a_302_with_no_body_is_unauthorized_and_never_followed`), and the
   module comment (:17-21) explains why it matters: a client that follows
   redirects **hides** a failure by redirecting to the GUI login. Phase 2's
   matrix case 11 requires exactly "redirect (never followed)".

### Other provider findings, to confirm before asserting

- **Silent skip** in `delonix-proxmox/tests/live.rs:2150`: an
  `if let Ok(callback) = env::var(...)` skips half of a case and the case
  still reports **passed** — against the rule "a missing variable FAILS".
  And there are defaults that mask absence (`_NODE`→`pve`, `_STORAGE`→`local-lvm`
  in 18 places, `_BRIDGE`→`vmbr0`; TrueNAS `_POOL`→`tank`).
- **Teardown is not failure-proof** in OPNsense (0 `Drop` guards), in TrueNAS
  (0) and in ~41 of Proxmox's 45 cases (inline teardown with `.expect()`): a
  panic halfway leaves the object on the target.
- **LVM/ZFS/btrfs are shipped and wired** (`delonix-storage-helper/src/main.rs:120-133`,
  `cmd/storage_pool.rs:125-134`) but `capability-matrix.md:196-197` declares
  `storage.lvm-thin` and `storage.zfs-btrfs` **`not-implemented`**.
- **9 Proxmox live cases never passed against a node** (self-declared in
  `live.rs:33-50`): the three `move`s, the system container's `move`, the IP
  via the agent, DNS, vCPUs/memory via the agent, node release and gateway
  repair. So `move_to_node`, `move_to` and
  `DnsProvider::prepare_zone/observe` are implemented and **unproven**.
- CRI's `ListPodSandboxStats` reads only `f.id` and **discards
  `PodSandboxStatsFilter.label_selector`** — exactly the defect its sibling
  `list_container_stats` claims to have fixed (`lifecycle.rs:2686`). With no
  coverage, nothing would catch it.
- `scripts/critest.sh` isolates `DELONIX_ROOT` but **not**
  `DELONIX_NET_RUNTIME_DIR`, and `cleanup()` (:49) invokes
  `"$CLI_BIN" netns down` — a spelling that **no longer exists** (it has been
  `net netns down` since the CLI reorganization). Measured: `delonix netns down`
  answers `error: unrecognized subcommand 'netns'`. The error is swallowed by
  `|| true`.

---

## Decisions I need from you

1. **The node contract's 29 unimplemented RPCs.** `ContainerService`,
   `PodService`, `ImageService` and `StackService` are entire declared
   services that are not served. Implementing is far beyond one TDD cycle
   per leaf; the briefing's alternative is "explicit decision to remove/defer
   with an ADR". Which?
2. **`delonix-mgmt` is marked for removal** by ADR-0042 D-E/F. Prove its 40
   operations or leave them as they are until the migration?
3. **The CRI baseline contradicts itself** (79/24 published vs 82/21 with
   logs), and neither is from 5.1.1. Re-measure now and republish, or treat
   as Phase 5?
   **CLOSED (2026-10-24):** re-measured on v5.1.1 → **84/20** and republished
   (`cri-conformance.md`, `gen.py`); see `reports/cri-conformance-2026-10-24/`.
4. **The Phase 5 comparison does not run on this host**: there is no
   `docker` nor `podman`, and `bench_gate.py` **refuses to judge** this
   machine (the baseline is from an AMD Ryzen 9 8940HX/32 threads; this one
   is an Intel i9-13900H/20 threads). It needs a lab VM, like the 2026-08-13
   run did.
