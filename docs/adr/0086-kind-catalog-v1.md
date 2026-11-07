# ADR-0086: The Kind catalog v1 — six domain groups, one version per group, and a conversion path

- **Status:** Accepted (2026-11-03, by the owner's catalog brief) — decided here; built in
  campaign phases 3–11. Supersedes the catalog of ADR-0069 D7 and the names/groups of ADR-0020.
- **Date:** 2026-11-03
- **Deciders:** Walter Angolar
- **Relates to / supersedes:** ADR-0069 (D7 catalog, and Pending #2 VM-name scoping — both
  superseded), ADR-0020 (CLI surfaces and Kind names/groups — groups re-laid-out), ADR-0005
  (a renamed Kind keeps answering to its old name), ADR-0007 (generated schema), ADR-0045
  (Stack groups from the Kind table), ADR-0070/0071 (provider block), ADR-0058
  (SystemContainer). Implemented over the node contract of ADR-0087.

## Context

Measured at `develop == main == 64a5953a` (re-measurement in
`docs/discovery/70_ENGINE_CATALOG_V1_REMEASUREMENT.md`): the catalog is **27 Kinds in one
table** (`crates/contexts/delonix-stack/src/kinds.rs`), all at `v1alpha1`, across **nine**
`apiVersion` groups (`core`, `compute`, `networking`, `gateway`, `storage`, `artifact`,
`infrastructure`, plus `security` and `gitops` the earlier briefs missed). There is no
`v1` anywhere despite the name "v1". There is **no version-conversion machinery and no
`delonix manifest convert`**; deprecation is modelled in `enum Form` (`Sunset`/`Sugar`/
`Compat` and silent `KIND_ALIASES`), which handles *renames and lowerings within one version*,
not *conversion across versions*.

The engine is becoming the data plane of a cloud: many nodes, many clients' workloads on the
same hardware, a local agent per node speaking the node contract. Nothing about that consumer
appears in the catalog (AGENTS.md "Identity and boundary"). Every Kind below is a generic
capability in the engine's own vocabulary; tenancy lives in `namespace` + labels + the
capability model (ADR-0003/0087), never in a Kind.

## Decisions

**D1. Six domain groups, suffix `.delonix.io`.** `compute`, `networking`, `storage`, `core`,
`policy`, `cluster`. The nine current groups collapse into these: `security` → `policy`
(RuntimePolicy), `gitops` → `core` (GitOpsSource), `artifact` → `core` (Image; `App` folds
into `Application`), `gateway` → `networking` (Gateway, HTTPRoute), `infrastructure` →
`cluster` (KubernetesCluster). A Kind's group is still a column of the one Kind table — moving
a Kind is a one-row change, as ADR-0020 intended.

**D2. One version per group, shared by all Kinds of the group. Three levels, with written
criteria.**
- `v1alpha1` — may change without notice.
- `v1beta1` — changes only with automatic conversion and one release of warning.
- `v1` — grows only compatibly; requires two consecutive releases with no incompatible change
  **and** each Kind of the group proven live.
The catalog is born at **`v1beta1`**. "v1" is the destination, not the starting label; the
gate (`version_gate.py` extended) refuses promoting a group to `v1` whose Kinds are not all
live-proven over two releases.

**D3. Twenty-four Kinds.** `compute`: Application, Job, VirtualMachine. `networking`: Network,
NetworkRoute, Router, IPPool, IPAddress, NetworkPolicy, Service, Gateway, HTTPRoute.
`storage`: StoragePool, Volume, Bucket, Snapshot. `core`: Secret, Config, Image, Stack,
GitOpsSource, Node. `policy`: RuntimePolicy. `cluster`: KubernetesCluster. Fourteen exist
today, four are partial (Application, Gateway, Bucket, and the Gateway name collision of D4),
six are new (Job, Router, IPAddress, Snapshot, Config, Node).

**D4. `Application` is the only Kind written for container services; `Gateway` is resolved.**
A `Pod` stays **only** as the instance an `Application` creates — it shows in `delonix get
pods` and is not written in manifests; `Container` sunsets into `Application` with
`replicas: 1` (announced, not rewritten — it gains a per-instance shared netns, a change the
owner accepts in this major). The CRI keeps its own pods, unchanged. The **name collision**
on `Gateway` is resolved: the networking `Gateway` is the entry point (Gateway-API shaped,
`listeners[]`, addresses from an `IPPool`, TLS); the former tunnel `Gateway` (ex-`Tunnel`,
cloudflare/pinggy/ngrok) becomes a **tunnel `class`** of that same Kind. One Kind, two classes
(`native` and the tunnel providers), no second `Gateway`.

**D5. Every old version keeps loading, converted, with one release of warning.** `delonix.io/v1`
and the groups `artifact`, `gateway`, `gitops`, `security`, `infrastructure` load, convert and
warn. `delonix manifest convert` rewrites files and **explains each change of behaviour**;
conversion round-trips without losing fields (a gate asserts `convert | convert⁻¹` is the
identity on the field set). The old Kinds leave in the next major. This is the conversion
machinery D2 requires and the measurement found absent — it is the central new mechanism of
this ADR, distinct from the within-version `Form`/`KIND_ALIASES` that stay for renames.

**D6. Provider extensions are their own group and version.** A provider-specific Kind uses the
provider name as group with its own version; the first is `proxmox.delonix.io/SystemContainer`
(the current compute `SystemContainer` moves there). This keeps the six domain groups free of
provider specifics, consistent with ADR-0070/0071.

**D7. Scope, and name uniqueness within a namespace (supersedes ADR-0069 Pending #2).**
Node-scoped (no namespace): `Node`, `StoragePool`, `IPPool`, `Router`, `RuntimePolicy`. Every
other Kind is namespaced, and **the name is unique within the namespace** — including
`VirtualMachine` and `Application` instances. This reverses ADR-0069 Pending #2, which made a
VM name unique on the *node* (a conflict across namespaces) because the on-disk key
(`vms/<name>.json`) and the pod netns (`pod-<name>`) were not namespaced. The cloud data-plane
requirement is that two namespaces may each own a `web`; the store key and netns identity
therefore become namespace-qualified (`<namespace>/<name>`), the same scoping `manifest::
scoped_plan_name` already applies to Container/Service/Volume. ADR-0069's reasoning held for a
single-tenant node; it does not hold for the data plane this catalog serves.

## Consequences

- A manifest written against an old group loads with a warning and `manifest convert` upgrades
  it; after one release the old group is refused (D1 of ADR-0069 still applies: a manifest the
  engine does not understand is refused, not half-applied).
- Scoping VM/Pod names by namespace is a **state-layout change** (`vms/<name>.json` →
  `vms/<namespace>/<name>.json`, pod netns qualified). It is incompatible with persisted state
  and therefore gated by the state-version mechanism of ADR-0090 (D3 of the campaign): N reads
  N-1's node-unique layout and migrates it, and the migration is tested in CI. Without that,
  this decision would strand running VMs — it must land together with state versioning.
- `Application`, `Job`, `Router`, `IPAddress`, `Snapshot`, `Config`, `Node` are new executors;
  the others move group and (Application/Gateway/Bucket) absorb a current Kind's behaviour.
- The schema is still generated (ADR-0007); the generator emits the six groups at `v1beta1`
  and the `x-delonix-update` per-field class of ADR-0088.
- Rollback: the old groups still load for a release, so a bad conversion is recoverable by
  pinning `DELONIX_MANIFEST_LENIENT=1` and reverting the convert.
