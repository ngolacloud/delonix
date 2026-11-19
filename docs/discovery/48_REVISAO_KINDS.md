# 48 — Review of the 18 Kinds: duplicates, merges, and what to expand

| Field | Value |
|---|---|
| Date | 2026-08-10 |
| Baseline | `0.46.0` + the four native IaC commits (`664454b`) |
| Lenses | SRE · Platform Engineering · DevOps · Cloud Architect |
| Scope | **The declarative model only.** Not a security or runtime audit. |
| Production changes | None. This document is the only artefact. |

> Every claim points to the file that backs it. Where the conclusion comes from
> absence, it says how the absence was searched for.

## STATUS — executed on 2026-08-10

| Proposal | Result |
|---|---|
| **D1** merge `Egress` into `FirewallPolicy` | ✅ done — reduced at `load`, with `direction: egress` written into the spec and fail-closed on a contradiction |
| **D2** `Dependency` → sugar for `FirewallPolicy` | ✅ done — merged by TARGET (several accumulate allows), with the origin in an annotation |
| **D3** `Ingress`/`HTTPRoute` — keep both, name the canonical one | ✅ done — and both now CONVERGE, after the routes gained provenance |
| **D4** `Storage` → a form of `Volume` | ✅ done — `nfs:`/`cifs:`/`webdav:` blocks, with the translation split half-pure/half-effectful |
| **D5** Pod-form of `Container` → `Pod` | ❌ **NOT DONE — the premise was wrong.** See below |
| **D6** `Workload` is the right direction | 🟨 unchanged — still underused in the documentation |
| `status` (§3) | ✅ decided — **not a manifest field**; entered as plan conditions and as `stack wait` |
| VM providers (§4) | 🟨 half-done — `vm convert` for the five ecosystems; VirtualBox/VMware backends blocked (tools absent on the host, measured) |

**D5 is this review's most important finding, and it contradicts the review itself.** The
proposal assumed that a `kind: Container` with a single-element `spec.containers` and a
single-element `kind: Pod` are the same object. Reading the creation code for both: the first
creates a container named `<name>`, with no shared namespace; the
second creates the `pod-<name>` netns and calls the member `<name>-c0`. Rewriting one
would rename the container and break internal DNS, `HTTPRoute` backends, a
`Dependency`'s `from`/`to`, and `validate`'s cross-references.

Renaming someone's container as a side effect of a tidy-up is not a
merge. What stayed was the deprecation warning — and the Kinds went from 18 to **15**, not 14.

**The recommended sequencing was confirmed.** Merging first saved us from converging three
Kinds that stopped existing, and `cli-stability.md` only promoted to stable what
was already merged. Had the order been reversed, each merge would have cost a contract
break.

---

## 0. The finding that reorders the work

**18 Kinds. 8 mechanisms.**

The manifest has 18 nouns, but underneath there are far fewer distinct things.
Seven Kinds have no mechanism of their own — they compile to another Kind or
write into the same store as a sibling:

| Kind | What it IS, underneath | Where it ends up |
|---|---|---|
| `Ingress` | **compiles to `HttpRouteSpec`** (`httproute::ingress_to_httproute:452`) | L7 proxy |
| `Egress` | **the SAME struct** as `FirewallPolicy` (`firewall::FwDocSpec:913`) | nft firewall |
| `FirewallPolicy` | same, with explicit `direction` | nft firewall |
| `Dependency` | sugar → per-container ingress rules (`dependency::apply:72`) | nft firewall |
| `Storage` | a `Volume` with a network driver + a friendly declaration (`storage::build_mount`) | `VolumeStore` |
| `ShareVolume` | `VolumeStore::register_external` + its own `JsonStore` | `VolumeStore` |
| `Workload` | **sugar** → `Container`/`Vm`/`Pod` (`workload::lower_workload:48`) | the others |

This has a direct consequence for what was asked: **converging the remaining 14
to the same format would be giving convergence, a schema, and a stability promise
to Kinds that should not exist separately.** Merging afterwards
costs a break; merging first costs nothing.

And there's a closing window: the stability promise that just landed in
`cli-stability.md` covers **only** `Container`/`Pod`/`Volume`/`Network`. All the
merge candidates are **outside** it. Once they're given a typed schema and
stability, they can no longer be merged without breaking something.

---

## 1. The duplicates, by severity

### 🔴 D1 — `Egress` and `FirewallPolicy` are literally the same object

They share the struct, the validator, the `apply`, and the dataplane. The only difference is
where the direction comes from: `Egress` implies `egress`, `FirewallPolicy` reads
`spec.direction`.

```rust
// firewall.rs:913 — one struct, two Kinds
struct FwDocSpec { direction: Option<String>, scope: Option<String>, target: String, ... }
```

Worse: **`Ingress` already was this Kind** and changed meaning in an earlier
version (today it's k8s-style L7). In other words, whoever read the v0.6 docs and
the v0.8 docs saw `kind: Ingress` meaning two different things.

How the four lenses see this:
- **SRE**: three names for "network policy" means three places to look
  during an incident, and the `describe` of one doesn't show the others.
- **DevOps**: a code review cannot tell whether a PR opened up traffic without
  knowing which of the three Kinds is authoritative.
- **Cloud Architect**: no known network model (k8s NetworkPolicy, AWS
  SG, Azure NSG) separates ingress and egress into different *types* — it separates them with a
  field. We're diverging from the convention with no gain.

**Proposed merge**: `kind: FirewallPolicy` becomes the only one; `direction` required
(`ingress`|`egress`). `kind: Egress` becomes a **canonicalized alias** (the
`canonical_kind` mechanism already exists and already does this for `KnowDepends`) with a
deprecation warning. Cost: an entry in `canonical_kind` and a line in
`apply`.

### 🔴 D2 — `Dependency` is a third way of writing the same firewall

`Dependency app→db` compiles into "on `db`: ingress default-deny + allow from `app`'s
IP" — exactly what a `FirewallPolicy` with `direction: ingress`
writes by hand. And the two **collide**: AGENTS.md already documents that a `to` targeted
by both warns, because `Dependency` is authoritative and takes over.

That two constructs of the same language have to warn about each other is the
symptom; the cause is that they are the same thing.

**But it's not a candidate for deletion** — it's the only one that expresses *directional*
intent readably (`from`/`to`), and it's what a user writes
naturally. The mistake is that they're siblings.

**Proposed merge**: `Dependency` becomes **explicitly documented sugar** that lowers to
`FirewallPolicy` at `manifest::load` time — the same treatment `Workload` and `Stack` already
get (they don't survive the load). Concrete gains: (1) the collision disappears, because there's
only one object left at the end; (2) `plan`/`describe` show the REAL rule that will be
applied, instead of an intent whose translation is invisible; (3) it converges for free when
`FirewallPolicy` converges.

### 🟠 D3 — `Ingress` and `HTTPRoute` are the same proxy with two grammars

`Ingress` (the `networking.k8s.io/v1` form) compiles to `HttpRouteSpec` and both
feed the same process. The limitations are identical because it's the same
code (a single certificate, no SNI; `pathType: Exact` treated as a prefix).

**Here the duplication is defensible** — and this is the important difference from D1/D2. They
are not two names for the same grammar: they are **two grammars for the same
mechanism**, and each serves a real audience. Whoever comes from k8s pastes an
`Ingress` they already have; whoever starts here writes the `HTTPRoute`, which is shorter.

**Proposal**: keep both, but **name which one is canonical** (`HTTPRoute`) and
treat `Ingress` as a *foreign-schema translator* — the same category this repo already
put `docker-compose.yml` and the Docker API in, with the same contract:
the translator documents what it does not translate, and never pretends. It is already like that in the code; what's
missing is saying so in the documentation, where the two appear side by side as if
they were equivalent alternatives.

### 🟠 D4 — `Volume`, `Storage`, and `ShareVolume`: three Kinds, one store

All three end up in `VolumeStore`:

* `Volume` — `driver: local|nfs`, with raw `device`/`mountOptions`.
* `Storage` — the SAME thing, with a friendly declaration
  (`type`/`server`/`share`/`username`/`passwordSecret`) translated into
  `device`+`options` by `build_mount`.
* `ShareVolume` — an isolated subdirectory of a `Storage`, via
  `register_external` + a `JsonStore` just for the share registry.

The problem isn't there being three; it's that **`Volume` and `Storage` overlap
entirely**. `kind: Volume` with `driver: nfs` and `device: nas:/export` is the
same object as `kind: Storage` with `type: nfs`/`server: nas`/`share:
/export`. Two ways of writing the same mount, and nothing says which to use.

- **Platform Engineering**: a self-service catalog cannot expose two
  primitives that do the same thing — it has to choose, and that choice is left
  undocumented in the template.
- **SRE**: `volumes ls` shows both mixed together (it's the same store), but
  `storage ls` only shows some — the same question gets different answers
  depending on the command.

**Proposed merge**: `Storage` becomes a **form of `Volume`** — `spec.nfs`/
`spec.cifs`/`spec.webdav` as named blocks inside `kind: Volume`, with `driver`
derived from whichever block is present (exactly the pattern `Workload` already
uses: the block has the name of the type, and the type can't contradict it). `kind:
Storage` becomes an alias with a warning. **`ShareVolume` stays** — it's genuinely
a different thing (subdivides an existing volume, with isolation between tenants), and
the name says so.

### 🟡 D5 — `kind: Container` with `spec.containers[]` is a Pod

`container::apply` chooses between two forms based on whether
`spec.containers` is present — and the Pod form accepts **exactly one**
container, otherwise it tells you to use `kind: Pod`.

In other words: `kind: Container` with a single-element `spec.containers` and `kind: Pod`
with one element are the same object, written in two ways, and the boundary
between them is a count.

This has a measured cost in this very piece of work: the generated schema had to offer
`anyOf: [ContainerSpec, PodSpec]` for the same `kind`, and `explain Container`
shows the union of both — autocomplete gets worse because of the
ambiguity.

**Proposal**: **don't merge, deprecate one direction.** The Pod form inside
`kind: Container` exists for compatibility with people pasting k8s YAML; the
right path is for that YAML to become `kind: Pod` at `load` time (a rewrite, as
`Workload` already does), leaving `kind: Container` with a single grammar. Direct gain: the
schema stops being ambiguous, `explain` becomes useful, and the Container's
convergence stops having two normalization paths.

### 🟡 D6 — `Workload` is sugar over four Kinds, and is the only one with a future

`type: container|vm|pod|microvm` + a block named after the type, which is
**exactly** the autonomous Kind's spec. It doesn't redefine a field, so it can't
diverge. Lowers at `load` time.

**It's not a duplicate — it's the right direction**, and it's underused: it's the only
object that expresses "compute" without the user having to know whether it's a container
or a VM, which is the promise of the Runtime Abstraction Layer that `AGENTS.md` sets as the
product's north star. What it's missing is being the **recommended** path in the
documentation instead of an alternative listed at the end.

---

## 2. The Kinds to expand, and the directives

### `Vm` — the biggest deficit, and the explicit request

Today: two backends (`cloud-hypervisor`, `libvirt`) behind the `VmBackend` trait,
which is already the pluggable-driver pattern. See §4 — it has its own section.

### `Secret` — the only Kind with no typed round-trip, and the most sensitive one

`manifest::filled_spec` deliberately leaves it in the raw spec ("do not reformat the
`stringData`"). Consequence: it's the only Kind with no real `--dry-run`, no schema,
and that will remain without convergence.

**Directive**: give it a typed spec with a `SecretValue` type that **never**
serializes the value (only `<redacted>`), which solves both things at once —
the dry-run comes into existence and still doesn't print secrets. Without this, the
most sensitive Kind is the only one outside every guarantee.

### `Image` — idempotency differs depending on the field, and no one says so

`spec.pull` is idempotent (`resolve_or_pull`); `spec.build` **rebuilds and
replaces the tag on every apply** (no build cache). Two opposite behaviours on the
same Kind, decided by which field is filled in.

**Directive**: to converge, `Image` needs a notion of identity that is not
the tag — the **digest**. An honest `plan` for `spec.pull` compares
digests (the tag can move); for `spec.build`, with no build cache the plan can
only say "will rebuild", and should say that instead of showing `=`.

### `Cluster` — it's not a resource, it's a procedure

`cluster apply` is a `kubeadm` bootstrap over SSH, idempotent via `check`/`apply`
step by step. It has no registry, no state, and the "resource" is a set of
remote machines.

**Directive**: **don't converge, and say so.** That's the only honest answer — the
`plan` cannot predict what 6 remote hosts will say without talking to them, and talking
to them would stop being a plan. It stays permanently `NotConverged`, with an explicit
reason ("remote procedure, not a local resource").

### `Tunnel` — the only Kind whose desired state depends on a third party

It has its own `JsonStore`, with identity by PID. The public URL is assigned by the
provider and changes on every respawn — it's not comparable to anything declared.

**Directive**: convergible only on the declared fields (`localPort`, `provider`);
the URL is **status**, not spec. That calls for the distinction the model does not yet have —
see §3.

### `Network` — the only convergent one with an admitted gap

`converge` warns that an overlay peer removed from the manifest is not removed
(`add_overlay_peer` has no inverse). It's honest, but it's a promise not yet
fulfilled.

**Directive**: `remove_overlay_peer`, and `peers` becomes convergent in both
directions.

---

## 3. The cross-cutting gap: `status` is missing

No Kind separates **spec** (the desired state) from **status** (the observed state). The
`describe` mixes the two, `plan` compares everything it can read, and
fields that are a result — a container's IP, a tunnel's URL, a VM's DHCP
lease — have nowhere to live.

The four lenses converge here:
- **SRE** wants `status.conditions` to alert on (`conditions.rs` already computes
  conditions; they have nowhere to live on the object).
- **Platform** wants `status.phase` for a catalog to show progress.
- **DevOps** wants a `kubectl wait`-equivalent, which is impossible without status.
- **Cloud Architect**: it's the convention of the entire declarative ecosystem since
  k8s; diverging from it is gratuitous.

**Directive**: before converging the 14, decide whether `status` comes in. If it does, the
`conditions.rs` output stops being a command's output and becomes a field — and
`plan` can then say "created, but not healthy", which today cannot be said.

---

## 4. `kind: Vm` and the requested providers — what fits and what doesn't

The `VmBackend` trait (`delonix-vm/src/lib.rs:438`) is already the extension point.
Adding a backend means implementing it. But the six requested providers **are not
the same class of thing**, and treating them as if they were would be the mistake:

| Provider | Fits? | Why |
|---|---|---|
| **Cloud Hypervisor** | ✅ exists | microVM, local, rootless |
| **KVM/libvirt** | ✅ exists | it's the `libvirt` backend |
| **VirtualBox** | 🟡 fits | local `VBoxManage`, no daemon of our own. But **it does not coexist with KVM** on the same host (both want `/dev/kvm`), and it's not rootless — it's a *workstation* backend, not a node backend |
| **VMware** | 🟡 two different products | local Workstation/Fusion (`vmrun`) fits through the same door as VirtualBox; **vSphere/ESXi is REMOTE** (a datacenter API) and is a different category — see below |
| **Proxmox** | 🔴 remote | a cluster's REST API. `AGENTS.md` has **already** settled that multi-cluster inventory/scheduling belongs to `delonix-paas`. A **single-node** `ProxmoxBackend` fits; the Proxmox product does not |
| **Hyper-V** | ❌ doesn't fit | It's **Windows**. This engine is Linux/KVM/rootless, with namespaces and cgroups v2 at its core. It's not a missing backend — it's a different operating system |

**The boundary this reveals**, and which deserves an ADR before any code: the
current `VmBackend` assumes a **local** hypervisor, invoked as a child process.
VirtualBox and VMware Workstation respect that. vSphere and Proxmox do **not** —
they speak HTTP to a datacenter, have authentication, and the "host" stops being this
machine. Cramming them into the same trait would make `delonix vm create` mean two
different things depending on the backend, which is exactly the confusion this
review is trying to remove from the Kinds.

> **SUPERSEDED by [ADR-0008](../adr/0008-proxmox-vm-backend.md)**, written in
> parallel and with a better design than what follows. The proposal below was **two
> traits** — one local, one remote. The ADR shows that **one** is enough: the
> `VmBackend` stays, and backend selection becomes REGISTERABLE (today
> `backend_for` is a private `match` over two strings, with a `_ =>` that silently
> falls back to Cloud Hypervisor). A Proxmox backend implements the same trait
> and speaks HTTP internally, in its own crate (`delonix-proxmox`), so
> `delonix-vm` gains no new dependency. The scope remains a single NODE, with no
> inventory or scheduler — the boundary with the PaaS this review also
> identified. I leave the original text below because the analysis of the six
> tools is still valid; the design conclusion is the ADR's.

**Design proposal (superseded, see above)**: two traits, not one.
- **`VmBackend` (local)** — what exists. Gains `VirtualBox` and `VMwareWorkstation`.
- **`VmProvider` (remote)** — new, for vSphere/Proxmox: creates on a fleet that isn't
  this machine, and therefore needs credentials, an endpoint, and a notion
  of "where". **This is where the boundary with `delonix-paas` has to be decided**,
  and it's an architecture decision, not a task.

**The `VMfile` is the piece that unifies** and is where the request has the most immediate payoff:
today it builds a `.qcow2` (`vm build`), and `HYPERVISOR` is already a field. What's
missing for "ready for the major providers" is not one backend per provider —
it's the **artefact** coming out in the format each one consumes (`.vmdk` for VMware,
`.vdi` for VirtualBox, `.vhdx` for Hyper-V, `.raw` for Proxmox), and
`qemu-img convert` already does all of them. `vm convert` exists and today only offers
`qcow2|raw`.

---

## 5. Sequencing recommendation

The requested order (14 Kinds → then VM) is reversed at two points, for a
concrete reason: **merging after stabilizing costs a break.**

1. **Merges first** (D1, D2, D4, D5) — all in `manifest::load`/
   `canonical_kind`, the mechanism that already exists and that `Stack`/`Workload` already
   use. None of it touches the dataplane. After this the 18 Kinds become ~13, and the
   convergence work shrinks by the same proportion.
2. **Decide `status`** (§3) — changes the shape of every object; doing it after
   converging 13 Kinds means redoing them.
3. **Converge what's left**, in order of value: `Image` (digest), `Secret`
   (with a redacted spec), `Storage`→`Volume` (already merged in step 1),
   `HTTPRoute`+`FirewallPolicy` (already unified), `Vm`. `Cluster` stays
   explicitly out, with a reason.
4. **`vm convert` for each provider's formats** — immediate payoff, zero new
   boundary, uses the `qemu-img` already there.
5. **New local backends** (VirtualBox, VMware Workstation) — each with its own
   GO/NO-GO spike, as the repo requires.
6. **ADR for the remote `VmProvider`** (vSphere/Proxmox) — decide the boundary with
   `delonix-paas` **before** writing code. Hyper-V doesn't come in.

---

## 6. Executive summary

| | |
|---|---|
| Kinds today | 18 |
| Distinct mechanisms | ~8 |
| To merge | `Egress`→`FirewallPolicy` · `Dependency`→sugar for `FirewallPolicy` · `Storage`→form of `Volume` · Pod-form of `Container`→`Pod` |
| To keep, with the stated role | `Ingress` (k8s translator) · `Workload` (the recommended path) · `ShareVolume` |
| Not to converge, with a reason | `Cluster` (remote procedure, not a resource) |
| Cross-cutting gap | no `status` exists — without it, "created but not healthy" cannot be said |
| VM providers | 2 already fit (VBox, VMware Workstation) · 2 need a new trait and an ADR (vSphere, Proxmox) · 1 doesn't fit (Hyper-V, it's Windows) |
| Biggest immediate payoff | `vm convert` for `.vmdk`/`.vdi`/`.vhdx`/`.raw` — `qemu-img` already does them |
