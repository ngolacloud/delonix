# The PaaS ↔ Runtime boundary in today's contracts (2026-10-08)

Read-only survey, measured against `delonix-runtime` on tag `v5.0.0` + 32 commits
(`1cbe9639`, worktree `naas-kaas-caas`). Objective: confirm whether the
`delonix.node.v1` contract (crate `delonix-node-api`, `proto/delonix/node/v1/*.proto`) already
allows the `PaaS → contract → adapter → backend` path with no SSH, with no PaaS hand-building
firewall rules, with no touching a provider's internal files, and with no need to
fix half-created resources. Every statement below was confirmed against the current code, not only
against `AGENTS.md`'s prose.

## 1. Layer separation

`proto/delonix/node/v1/common.proto` states the rule at the top of the file (guardrail #2):

> «No notion of tenant, account, plan, quota or billing (…). `namespace` is the ENGINE's
> isolation namespace — a client may map its own grouping onto it, the engine never
> learns what that grouping is.»

Confirmed through three independent paths, none of them `scripts/arch_fitness.py`:

1. **`scripts/arch_fitness.py` exists and runs** (`python3 scripts/arch_fitness.py`, ok on
   every check). Its `consumer_mentions()` function (lines 302-329) only refuses the
   **name of a consumer** (`delonix-paas`, `delonix-api`, `ngolacloud`, `paas`,
   `delonixctl`, …) in `crates/`, `bins/`, `proto/` — it does not scan for `tenant`/`billing`/
   `quota`/`customer`/`plan`/`account` as domain concepts. In other words: the gate ensures
   «the engine does not know the PaaS by name», not «the engine has no notion of
   tenancy».
2. **An independent manual search** (`grep -rniE "tenant|billing|customer|\bplan\b|quota|
   account" crates/ bins/ proto/`) returned ~90 occurrences, **all** harmless once read
   one by one:
   - `plan`/`Plan` is always the reconciler's verb (`stack plan`, `fn plan(...)`,
     `PlanStack`), never a billing plan.
   - `quota` is always a TECHNICAL resource quota: cgroup `cpu.max`/`quotactl` (kernel),
     volume quota (`VolumeStore::set_quota`, bytes), a remote dataset's quota
     (`delonix-truenas`). No tenant/plan quota at all.
   - `account` is always the OPERATING SYSTEM account (the account that receives an SSH
     key on a guest, `/etc/subuid`), never a customer account.
   - `tenant`/`customer` appear only in **doctrine comments** and in **test
     variable names** (`crates/contexts/delonix-compute/src/record.rs:290-319`, testing
     that `safe_cgroup_segment` accepts `"tenant-acme"` as just any string and
     refuses `"Tenant"`/spaces — a test about string sanitization, not about
     tenants) — and in the very doc-comment that explains the boundary
     (`record.rs:431-439`: «the engine stays independent of tenants, plans and
     billing…»).
   - `crates/contexts/delonix-security-runtime` has ITS OWN TEST
     (`no_file_in_this_crate_speaks_of_a_tenant`, `lib.rs:71-98`) that fails if a
     `tenant`/`project`/`environment` field lands in that specific crate — stricter
     than `arch_fitness.py`, but scoped to a single crate (the security-events one).
3. **`python3 scripts/contract_gate.py`** runs clean (`buf format`/`buf lint`/`buf
   breaking against v5.0.0`/HTTP mapping of the 57 routes/generated OpenAPI/45 distinct
   REST paths) — the published contract is formally stable and has no path
   reuse.

**Verdict**: the separation is genuinely clean TODAY — there is no field, struct or
route that carries tenant/plan/business-quota/billing. But proof of that is done by
**human reading after a broad `grep`**, not by an automated gate that looks for the
domain terms themselves. `arch_fitness.py` guards against «the engine learning the
consumer's NAME»; there is no equivalent gate that fails if someone introduces a
`tenant_id`/`billing_plan` field under a generic name (`owner`, `group`, `scope`) that does not match
`CONSUMER_NAMES`. It is a **gate** gap, not a gap in the **current state of the
code**.

## 2. What the Runtime still trusts the caller for

The prompt's question («a resource name the caller freely chooses, with no
ownership check») was checked both ways: technical traversal/injection, and
logical ownership/scope.

### 2a. Traversal and technical injection — closed for what is served

`CreateNetwork`/`DeleteNetwork` (`crates/interfaces/delonix-node-api/src/
network_ops.rs:174,207`) call `delonix_sdn::NetworkStore::validate_name`/
`validate_subnet` BEFORE any effect, with a test proving `"../evil"`/`"a/b"`/`"bridge"`
are refused (`network_ops.rs:445-448`). The reads (`GetVolume`/
`ListVolumes`, `volumes.rs:218-284`) never build a path from `req.name` —
they enumerate the whole store (`entries(root)` → `VolumeStore::list_all()`, which reads
already-sanitized names on disk) and filter by string equality. There is no path-traversal
reachable through the node-api in the four verbs served today (`Create/DeleteNetwork`, `Get/
ListVolumes`).

### 2b. Ownership and logical scope — NOT checked, by design, and that is a real boundary

No message in the contract carries a field for caller identity or resource
ownership:

- `ResourceMeta` (`common.proto:39-50`) has `name`/`namespace`/`uid`/`labels`/
  `annotations`/`etag` — nothing saying "who may touch this".
- `DeleteNetworkRequest`/`DeleteVolumeRequest` (`infra.proto:202-211,302-308`) only have
  `namespace`/`name`/`etag`/`force`/`request_id`. **Any caller who knows (or
  guesses) the name deletes the resource** — the only brake is the `etag` (if the caller
  passes it) and the dependents check.
- The `delonix.io/stack`/`OwnerMark` that AGENTS.md documents (ownership by label,
  `delonix-stack/src/reconcile.rs`; `OwnerMark` in `delonix-networking/src/
  ownership.rs`) is a mechanism for **prune accounting** (what a `stack apply
  --prune`/`stack destroy` may touch) and for **GC on remote provider objects**
  (Proxmox SDN vnet, OPNsense alias) — it is never read as access control in any
  node-api RPC. `CreateNetwork` accepts `labels`/`annotations` from the
  CALLER itself (`network_ops.rs:124-131`) and writes them verbatim; nothing stops a
  second caller from creating or deleting a resource carrying another's ownership label.
- `ListOperationsRequest` (`operations.proto:82-86`) only has `page`/`active_only` — **zero
  scope**. Any caller with access to the socket sees ALL of the node's operations
  (`GetOperation`/`ListOperations`, `service.rs:268-294`), including the `target`
  (`Network/lab`), the `request_id` and the error of ANY other caller. There is no
  `owner`/`caller_id` on the persisted `Record` (`operations.rs:53-73`).
- `ListVolumesRequest.namespace` accepts the literal `"*"` (`volumes.rs:82-88`,
  `wanted()`), which returns ALL namespaces at once, with no gate at all — it is a
  deliberate escape hatch for a human operator (`delonix volume ls -A`), but
  it is re-exposed as-is over the remote contract: a caller who should only see their own
  namespace and passes (or receives from a buggy PaaS) `namespace: "*"` reads everything.
- By contrast, `NetworkService` fixes the network namespace permanently to `default`
  (`check_namespace`, `network_ops.rs:76-83`) — there is no per-namespace network isolation
  in the node-api today, but there is also no such specific leak, because there is no more than
  one namespace to read.
- Outside the node-api: `docs/adr/0069-kind-catalog-reassessment-and-strict-manifests.md`
  (lines 147-150) already records, for the `stack`/CLI layer, that a scoped
  resource's identity (`namespace/name`) is today **string convention**
  (`destroy_one` does a `find` over the plan's name) and that a structural
  `ResourceKey` «would make that structural instead of conventional» — nobody has
  measured needing it yet, but it is the SAME class of fragility.

**Network/socket isolation**: the only identity control that exists is at the TRANSPORT
layer — `SO_PEERCRED` against the server process's own uid, socket `0600`
(`crates/interfaces/delonix-node-api/src/lib.rs:46-47,117-134`). That is: today the node-api
is only reachable by a process running with the SAME uid as the server on the SAME host — which
implicitly assumes «one node-api per host, one fully-trusted caller (the
PaaS's local agent on that host)». There is no notion of "caller A" vs "caller B" on the
socket — `SO_PEERCRED` only guarantees "it is the process's owner", not "it is WHO it claims to be".

**Verdict for question 9**: the low-level TECHNICAL boundaries (file names,
subnets, operation ids) are already validated on the server, regardless of what the PaaS
has already checked upstream — that part is fine. But the LOGICAL boundary («can this caller
touch this resource, in this namespace, see this operation») does not exist in the
contract: the Runtime trusts entirely that (a) only a trusted agent talks to the
socket, and (b) that agent only asks for what it has authority to ask for. It is a position
consistent with the doctrine («the engine has no notion of tenants»), but it means ANY bug on
the PaaS side that forgets to filter `namespace`/`name`/`request_id` before passing it through grants
cross-access with no notice or refusal from the Runtime.

## 3. Contract maturity matrix

| Area | State today (evidence) | Gap | Where to close it |
|---|---|---|---|
| **Schemas/versioning** | Single `delonix.node.v1` package (`common.proto` + `node/operations/infra/compute.proto`), `buf lint`/`buf breaking against v5.0.0`/generated and verified HTTP mapping and OpenAPI by `scripts/contract_gate.py` (run: `ok` on all 6 checks, 57 routes mapped, 45 distinct REST paths). No per-resource-group versioning (unlike the CLI's manifests, which use `compute.delonix.io/v1alpha1` etc.) — it is all flat `v1`. | A breaking change in ANY message forces bumping the whole package to `v2`, even if only one resource changed. Acceptable while the contract is young; sooner or later it gets expensive. | Decide, via ADR, whether `delonix.node.v1` stays monolithic or each `message`/service gets its own version cycle (would be a big structural change — not urgent today). |
| **Idempotency** | `request_id` on EVERY creation/deletion message in the proto (`CreateNetworkRequest.request_id`, `DeleteVolumeRequest.request_id`, etc. — confirmed by grep in `infra.proto`/`compute.proto`). Implemented and tested for the 2 served routes: `operations::replay`/`begin` (`network_ops.rs:212-224,253-291`; tests `the_same_create_sent_twice_does_the_work_once`, `the_same_delete_sent_twice_is_answered_once_not_with_not_found`). REST reads `Idempotency-Key` and injects it as `request_id` (`service.rs:396-404`). | The FIELD exists on all 22 messages that return an `Operation`, but the MECHANISM (`operations::begin`/`replay`) is wired to only 2 of them (`CreateNetwork`/`DeleteNetwork`). The other 20 (Volume, Image, Stack, Container, Pod, VM, Snapshot) are `UNIMPLEMENTED` — the field is accepted by the parser and never used. | `crates/interfaces/delonix-node-api/src/service.rs` — wire each new verb to the SAME already-written `operations::begin/finish/replay`, do not reinvent it. |
| **Async operations** | See section 4 — `OperationService` (on-disk ledger, `Interrupted` on a dead owner, 7-day retention) is done and tested for what already exists. | Only 2 of 22 mutation RPCs that return an `Operation` go through the mechanism. `WatchOperation`/`CancelOperation` are `UNIMPLEMENTED` (`service.rs:295-316`) — a client can only *poll* via `GetOperation`, never subscribe or cancel. | `operations.rs` already has the `Record`; missing is the `WatchOperation` `stream` (see the design note below) and `CancelOperation` (today there is not even a concept of "ask it to stop" midway through a `create_work`/`delete_work`, which are synchronous, non-cooperative functions). |
| **Structured errors** | RFC 9457 `application/problem+json` on REST (`service.rs:684-715`, `delonix_model::codes::problem`), `tonic::Status` with a class code + `dx-number` metadata on gRPC (`network_ops.rs:42-73`), central, stable `DX-CDNN` dictionary (`delonix-model/src/codes.rs:1-10`: «a number never changes meaning and is never reused»). | The proto PROMISES (`common.proto:114-122`) an `ErrorDetail` (`reason`/`resource`/`metadata`/`retryable`/`retry_after`) «attached to every non-OK gRPC status as a binary detail» — but it is only built inside `Operation.error` (`operations.rs:340-349`, confirmed by grep — it is the ONLY construction of `ErrorDetail` in the whole crate). A SYNCHRONOUS error (`GetNetwork` not-found, `DeleteNetwork` in use) never gets the binary detail; it only carries the `dx-number` string in ASCII metadata. A generic gRPC client expecting `google.rpc.Status.details[]` (the pattern used by `tonic_types`/other services) does not find it; it has to know how to read the proprietary metadata. | `network_ops.rs::status_of` / the equivalent for each new service — build and attach an `ErrorDetail` via `Status::with_details` (or the `grpc-status-details-bin` convention) on the synchronous path too, not only inside `Operation.error`. |
| **Desired-vs-observed** | The 3-way reconciler already exists and is mature (`delonix-stack/src/reconcile.rs`, `kinds.rs` — the doc-comment explains the single `KindFacts` table and the 3 real bugs that motivated it; `plan()` is pure, tested, compares manifest/machine/`delonix.io/last-applied`). `ResourceMeta.etag`/`Condition` in the proto already model observed-vs-expected at the contract level (`common.proto:39-66`). `NetworkService.GetNetwork` already reads `Realized` from the real dataplane (ADR-0042 slice E1). | DECLARATIVE `plan`/`apply` (what resolves "what has to change to match the manifest") lives ENTIRELY in `delonix-stack`/the local CLI — `StackService.PlanStack`/`ApplyStack`/`DestroyStack` are the 3 contract RPCs that expose it, and all 3 are `UNIMPLEMENTED` (not even registered in `service.rs`). A PaaS cannot ask over the network to "converge to this state"; it can only issue imperative RPC-by-RPC operations (and even those, today, only Network create/delete). | `StackService` in `service.rs` + a new `crates/interfaces/delonix-node-api/src/stack_ops.rs` calling `delonix_stack::reconcile::plan`/the `apply` the CLI already uses — ADR-0042 names this as dependent on ADR-0040's P5 (see AGENTS.md: «ADR-0059's F6 DEFERRED until P5», the same dependency). |
| **Create/adopt/update/replace/delete** | The proto already draws the pattern well for `Container` (`UpdateContainerRequest` with `FieldMask update_mask` + `etag`, `infra.proto`/`compute.proto:285-300`) and `etag`/`force` on every `Delete*Request`. | Per resource: **Network** — Create/Delete served, **no Update** in the proto (there is no `UpdateNetworkRequest` at all — labels can only be set at creation) and **no adoption**: `CreateNetwork` over an already-existing name is always `AlreadyExists` (`network_ops.rs:215-220`), never the adoption semantics the CLI (`stack apply`) already has via `delonix.io/last-applied`. **Volume** — read-only; Create/Delete `UNIMPLEMENTED`. **Container/Pod/VM/Stack/Image** — no verb served at all (neither Create nor Get). | A short ADR decides whether `CreateNetwork` gets an "adopt-if-exists-and-unowned" mode (replicating the reconciler's rule) before the Container/VM wave arrives — otherwise each wave reinvents the decision. |
| **Stable events** | `WatchEvents` is IN the contract (`node.proto:38`) and has a registered handler — but it always returns `Err(not_yet("WatchEvents"))` (`service.rs:40-46,147-153`). The `delonix-node::events` crate (mentioned in AGENTS.md's crate table) exists as an internal registry, but is not wired to this stream. | Zero stable events delivered over the network today. A PaaS has no way to know "VM X changed state" without continuous *poll* of `Get*`. | Wire `delonix_node::events` (which already exists) to `WatchEvents` — probably the SAME streaming infrastructure `WatchOperation` will need; makes sense to design them together. |
| **Per-provider capability discovery** | **Done and served** — `ListProviders` (`NodeService`, ADR-0050 D5) returns `ProviderInfo{id, kind, available, capabilities[], health, catalog_version}` with `Capability{name, supported, detail, state}` across 6 states (`supported`/`partial`/`unsupported-by-provider`/`requires-external-component`/`not-implemented`/`unavailable-on-host`) — confirmed in `service.rs:78-108,137-146` and testable today (gRPC + REST + `/docs`/`/redoc`). The rule «`supported` cites evidence that exists» is enforced by a test (`the_declared_providers_are_the_published_matrix`, catalogue documented in AGENTS.md). | None. This is the MOST mature area of the contract — the PaaS can already ask "does this node support snapshot with memory?" before trying, instead of discovering it by error. | — |

## 4. Async operations

`OperationService` (ADR-0042 D4/E2, `crates/interfaces/delonix-node-api/src/
operations.rs`) covers, WELL, FOR WHAT IS WIRED INTO IT, the four questions in
the prompt:

1. **Created with an ID before the work.** `begin()` writes the `Record` to
   `<root>/operations/<id>.json` with `state: Running` BEFORE the `work` closure runs
   (`network_ops.rs:221-229`); proven by the test
   `create_is_persisted_running_before_the_work_and_succeeds_with_the_target`, which
   peeks at the list DURING the `work` and confirms it is already there.
2. **Progress query.** `GetOperation`/`ListOperations` served (gRPC + REST,
   `service.rs:268-294`), with `ETag`/pagination — a client who lost the initial
   response comes back and reads the state by `id`.
3. **Idempotent result by `request_id`.** `operations::replay` (called BEFORE
   any check the first request itself already made true — e.g. "already
   exists") returns the SAME `Operation` for the same `(verb, target, request_id)`, without
   repeating the effect — tested (`the_same_create_sent_twice_does_the_work_once`: `runs`
   stays at 1).
4. **A client that loses the connection and asks again later, without repeating the effect.**
   Two cases, both covered: (a) the client has the `Operation`'s `id` — it does
   `GetOperation`; if the process doing the work died midway, `settled()`
   (`operations.rs:140-161`) marks it `Failed/Interrupted` on the NEXT read, never
   `Running` forever — and says "read the target to see what is left, and resend the request".
   (b) the client only has the original `request_id` — it resends the SAME request and
   `operations::replay` returns the response already given, without re-running `work`.

**What is missing, measured per RPC**: 24 contract RPCs return an `Operation` (grep in
`infra.proto`+`compute.proto`+`operations.proto`); of those, `GetOperation` is the read that
already serves the mechanism itself and `CancelOperation`/`WatchOperation` (the latter returns
`stream Operation`, so it falls outside the count of 24, but it is the same family) are the other 3
RPCs of `OperationService`, always `UNIMPLEMENTED`. That leaves **22 mutation RPCs** (`Create`/
`Delete`/`Apply`/`Pull`/… that create or terminate a resource) — and of those only **2**
(`CreateNetwork`, `DeleteNetwork`) go through `operations::begin`/`finish`/`replay`. The
other 20 — `Create/DeleteVolume`, `Pull/Push/Build/Scan/DeleteImage`, `Apply/
DestroyStack`, `Create/DeleteContainer`, `Create/DeletePod`, `Create/Delete/Start/
StopVirtualMachine`, `Create/Restore/DeleteSnapshot` — respond `UNIMPLEMENTED`
directly in `service.rs` (they don't even get as far as creating a `Record`). Numerically: **2 of 22
(≈9%)** of the contract's async mutations are actually wired to the mechanism that
makes them safe for a client on unstable networking — the rest is not even reachable.

## 5. The 3 most serious gaps (for the PaaS to be able to delegate without bypassing the contract)

1. **Container/Pod/VM/Stack/Image have NO verb served at all in the node-api — the PaaS has
   no way to create anything "real" over the network today.** `ContainerService` (12 RPCs),
   `PodService` (4), `VirtualMachineService` (13), `StackService` (3), `ImageService`
   (7) — 39 of 59 contract RPCs, 0 implemented (`service.rs` has no `impl
   ContainerService for NodeApi` at all; REST always falls into the `_ => Err(not_served)` at
   line 414). The PaaS, today, can only: create/delete a network, read volumes, list
   the node's capabilities. For everything else (running a container, creating a VM, applying
   a stack) the path remains the local CLI — exactly the SSH/hand-typed-command
   this design wants to eliminate.
   - **Where**: `crates/interfaces/delonix-node-api/src/service.rs` (register the
     `impl *Service for NodeApi`) + a `compute_ops.rs` module calling
     `delonix_compute::run`/the container engine the CLI already uses (`cmd/
     container.rs`), the way `network_ops.rs` calls `delonix_sdn::netops`.
   - **Test that would prove it**: a real gRPC integration test (like
     `crates/interfaces/delonix-cri/tests/grpc_status.rs` already does for CRI) that brings up
     the server on a test unix socket, calls `CreateContainer` with a `request_id`,
     confirms `Operation.state == Succeeded` and that the container exists in the real `Store` —
     and repeats the SAME `request_id` confirming it does not run twice.

2. **No RPC, synchronous or asynchronous, carries a notion of "who asked for this", and
   that is why `ListOperations`/`DeleteNetwork`/`ListVolumes(namespace="*")` are
   cross-access surfaces if the PaaS ever passes through a `namespace`/`name` without filtering
   it first.** Today this is safe BECAUSE the socket only accepts the server's own uid
   (`SO_PEERCRED`) — that is, it assumes 1 fully-trusted agent per node. The day
   the design needs more than one caller per node (several PaaS processes, a
   multiplexed proxy, a second agent), there is NOTHING in the contract to stop one of them
   from seeing/deleting what another created.
   - **Where**: `proto/delonix/node/v1/common.proto` — an optional field, say
     `string caller_scope` in `ResourceMeta`/`ListOperationsRequest`, checked against
     the already-existing `delonix.io/stack`/`OwnerMark` on the server side (reuse the
     ownership mechanism that already exists for pruning, rather than inventing a second one). Design
     decision: continuing with "one fully-trusted agent per node" is acceptable and should
     be WRITTEN DOWN as such in an ADR, so it is not discovered late as an
     implicit assumption.
   - **Test that would prove it**: two distinct `request_id`s/callers create two networks with
     different ownership labels; a `DeleteNetwork` from the second caller against the
     FIRST one's name fails with an ownership reason — today that test cannot even be
     written, because there is no field to express it in.

3. **The structured `ErrorDetail` the contract promises ("attached to every non-OK gRPC
   status") only exists inside `Operation.error` — the synchronous error path (the
   majority of real errors: not-found, conflict, stale etag) never builds it, and
   so a generic gRPC client (not hand-written against this API) cannot
   classify the failure without knowing how to read the proprietary ASCII metadata `dx-number`.**
   - **Where**: `network_ops.rs::status_of` (and the equivalent any new service will
     need) — build an `ErrorDetail{reason, resource, metadata, retryable,
     retry_after}` and attach it to the `tonic::Status` via `Status::with_details`/
     `grpc-status-details-bin`, alongside (not replacing) the `dx-number` metadata
     already used by REST.
   - **Test that would prove it**: a test that calls `GetNetwork` over a nonexistent name
     and decodes the binary detail of the `tonic::Status` (not the ASCII metadata) expecting an
     `ErrorDetail.reason == "DX_NOT_FOUND"` and `resource == "Network/<name>"` — today that
     test would fail because the detail is never attached.
