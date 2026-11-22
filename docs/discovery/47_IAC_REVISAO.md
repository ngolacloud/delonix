# 47 — IaC review: what Delonix is missing to be managed by Terraform and Ansible

| Field | Value |
|---|---|
| Date | 2026-08-10 |
| Baseline | `0.46.0` (commit `c097d0d`, branch `ciclo-v046-bloco-a`) |
| Scope | **IaC only.** Declarative model, convergence, management API, contract and documentation. Not a security or runtime audit. |
| Method | Code reading + the documentation site. No production changes. |

> **Reading notice.** Every statement below points to the file and line that backs it.
> Where the conclusion comes from an absence (a route doesn't exist, a command doesn't
> exist), it says how it was searched for. Nothing is inferred from the documentation — this
> repo has already recorded at least two cases where the documentation claimed the opposite
> of the code.

## STATUS — updated 2026-08-10, after running the checklist

This section exists because this repo already paid for not having one. `AUDITORIA-E2E.md`
spent months listing 27 fixed findings as if they were live debt, and
`AGENTS.md` records the lesson: **a findings table that isn't kept up to date lies
in both directions.** Every finding below carries its real status.

| | Finding | Status |
|---|---|---|
| 🔴 | **F1** `apply` doesn't update | ✅ **CLOSED** — `cmd/reconcile.rs`, 3-way diff; 11 of the 12 applicable Kinds converge |
| 🔴 | **F2** no `destroy`/prune | ✅ **CLOSED** — `stack destroy` + `apply --prune`, ownership via `delonix.io/stack` |
| 🔴 | **F3** no drift or `plan` | ✅ **CLOSED** — `stack plan`, `--detailed-exitcode` (0/2/1) |
| 🟠 | **F4** management API is a thin slice | ⬜ **OPEN, and rightly so** — widening it is only worth it once F5 is decided; API surface added before that is surface that has to be maintained afterward |
| 🟠 | **F5** API has no contract and no remote access | 🟨 **FRAMED** — [ADR-0010](../adr/0010-remote-management-api.md) writes down the decision instead of making it, and the answer it expects is that the API **should not** become remote: the consumer that would justify it is a fleet control plane, and that's `delonix-paas`'s job. Meanwhile documented as local and unstable |
| 🟠 | **F6** `apply` output isn't machine-readable | ✅ **CLOSED** — `plan -o json` with stable `action`/`changed` |
| 🟡 | **F7** `--dry-run` only on `stack apply` | 🟨 **PARTIAL** — `plan` covers the declarative path; imperative commands still have no check-mode |
| 🟡 | **F8** identity and reads aren't uniform | 🟨 **PARTIAL** — identity was fixed where it was WRONG (an `Image`'s identity is the ref, not the document's name — that was a bug), but `inspect`/`describe`/`ls -o json` remain three different shapes |
| 🟡 | **F9** no optimistic concurrency | ⬜ **OPEN** — two parallel `apply`s still clobber each other |
| 🟡 | **F10** order by Kind, not by graph | ⬜ **OPEN** — no per-resource `dependsOn`; `stack wait` covers "wait until it's up" downstream |
| 🟡 | **F11** declared schema not stable | ✅ **CLOSED** — promoted to stable, with `scripts/schema-diff.sh` as a gate |
| 🟡 | **F12** no `explain`, schema written by hand | ✅ **CLOSED** — ADR-0007: generated from the code, with a test guaranteeing that what's published IS what's generated |

**About the §7 verdict.** It said the "convergence engine" was missing and that
blocks 1 and 2 were mostly wiring. That was partly confirmed: the wiring did
exist (`cmd_update`, `-o json`, `conditions.rs`), but the real work was
**normalisation** — getting both sides to produce the same string. Every Kind now
has a test proving that an unchanged manifest yields zero diffs, and
most of this series's bugs came from exactly there.

**What the review didn't anticipate**, and only showed up on execution: seven real bugs, all
found by measurement — an `Image`'s identity; two `FirewallPolicy` records silently
deleting each other; the `env: {K: v}` dropped in a published example; two applied Kinds
that `ls` never showed; the mount condition that went silent after a
merge; and three lists of converging Kinds that had drifted. The Kinds review that
followed, and that reordered the work, is in
[48_REVISAO_KINDS.md](48_REVISAO_KINDS.md).

---

## 0. The criterion

A "complete" IaC tool is not a declarative format. It's six things, and
Delonix has two fully, two halfway, and two not yet started:

| # | Capability | Status |
|---|---|---|
| 1 | **Describe** the desired state in a versionable file | ✅ strong |
| 2 | **Predict** what will change before changing it (`plan`) | 🟡 partial (`--dry-run` shows the desired state, not the diff) |
| 3 | **Converge** — create *and update* until it matches the declared state | ❌ **create only** |
| 4 | **Remove** what dropped out of the file | ❌ non-existent |
| 5 | **API** with a contract, for an external tool to drive all of this | 🟡 small slice, local, with no published contract |
| 6 | **Documentation** that frees you from memorising anything | ✅ strong for the CLI, ❌ absent for the API |

Point 3 is the structural defect. Everything else is work; 3 is the difference between
"a declarative installer" and "IaC".

---

## 1. Strengths (measured, not assumed)

### 1.1 The declarative model is broad and coherent

18 Kinds in the k8s shape (`apiVersion`/`kind`/`metadata`/`spec`), documented in
`docs/kinds.html` and with 26 YAML examples under `examples/`:

```
Container · Pod · Vm · Workload · Image · Volume · Storage · ShareVolume · Network
Secret · Ingress · Egress · FirewallPolicy · HTTPRoute · Dependency · Tunnel · Cluster · Stack
```

`kind: Stack` aggregates everything in one document and expands on
`load` ([manifest.rs:145](../../crates/delonix-runtime-bin/src/cmd/manifest.rs#L145)); `kind:
Workload` unifies container/vm/pod/microvm (ADR-0001, ADR-0006). Both are **sugar
on top** of the base Kinds — not a second engine diverging on its own. It's the
right decision, and it's made.

Coverage: `ContainerSpec` has ~35 fields, `VmSpec` has 32. This isn't a toy
subset of what the CLI does.

### 1.2 `stack validate` — resolves cross-references before touching anything

[stack.rs:74](../../crates/delonix-runtime-bin/src/cmd/stack.rs#L74) resolves
`Container.network`/`.volumes`, `Vm.network`, `Ingress`/`Egress` targets against what
the manifest declares **plus** what already exists in the stores, and exits with an error if a
reference is left unresolved. It's the safety net against an `apply` that fails fast with no
rollback. Few projects of this size have this.

### 1.3 `--dry-run` materialises the defaults

`render_with_defaults`
([manifest.rs:56](../../crates/delonix-runtime-bin/src/cmd/manifest.rs#L56)) round-trips
each spec through the typed struct, so the printed YAML shows what will
**actually** be applied, with the `#[serde(default)]`s visible, Stacks already
expanded, and Kinds canonicalised. It covers every Kind except `Secret`
(deliberately). It's `kubectl apply --dry-run=client -o yaml`, and it works.

### 1.4 `conditions.rs` — a resource is forbidden to lie by omission

[conditions.rs](../../crates/delonix-runtime-bin/src/cmd/conditions.rs) computes, from
the spec plus a probe of the environment, kubectl-style conditions (`ok` + stable
`reason` + actionable message) for cases where a resource is created but **doesn't do
what it appears to**: an NFS `Storage` in rootless that doesn't mount, a quota that is
only monitored, a macvlan `Network` that sits in the record with no physical plan, a
`restartPolicy` on a Cloud Hypervisor VM that nobody supervises.

**This is a genuine competitive advantage and it's worth saying out loud.** It's
exactly what Terraform *doesn't* do: a `terraform apply` says `Creation complete`
over a resource that ended up inert. Delonix already knows how to tell "created" apart from "created
and working". The problem is that today it only shows up in `stack describe` — it's not a
state field an external tool can read.

### 1.5 `-o json` with a written contract (ADR-0005)

Ten listing commands emit a JSON array of typed rows, with **field names in
`snake_case` independent of language** — explicitly never the translated
table headers. The ADR reasons about this through three lenses (Platform/DevOps/SRE) and
the compromise is fixed: fields can be added, never removed or have their type changed.

### 1.6 A written stability promise

[docs/cli-stability.md](../cli-stability.md) says what breaks and what doesn't within
`0.x`, and the rule for how a break is made ("fail loud, no aliases"). This is
rare in 0.x and it is **exactly** the document anyone writing a provider needs to
read before deciding to adopt it.

### 1.7 Declarative hygiene already in place

- an unknown `apiVersion` **refuses** instead of proceeding
  ([manifest.rs:120](../../crates/delonix-runtime-bin/src/cmd/manifest.rs#L120)).
- `warn_unknown_fields` warns about a misspelled field **before** the early-continue,
  so a typo keeps showing up on re-applies and not just on first creation
  ([container.rs:1709](../../crates/delonix-runtime-bin/src/cmd/container.rs#L1709)).
- `canonical_kind` is case-insensitive throughout — a half-measure would leave `kind: vm`
  silently ignored.
- Idempotency with no state file in `cluster apply` (each step has a `check` and
  an `apply`) — it never drifts out of sync with a `.tfstate` because there isn't one.

---

## 2. Weaknesses, by severity

### 🔴 F1 — `apply` **doesn't update**. Create only.

```rust
// container.rs:1717
if store.list()?.iter().any(|c| &c.name == name) {
    println!("container/{name}: already exists, nothing to do");
    continue;
}
```

Change the image in the manifest, change the memory, add a port, run `stack apply`:
**nothing happens, and the command returns 0**. The same pattern in
[volume.rs:165](../../crates/delonix-runtime-bin/src/cmd/volume.rs#L165) and
[network.rs](../../crates/delonix-runtime-bin/src/cmd/network.rs) ("ensure present",
as the `manifest.rs` doc-comment itself assumes up front).

This is the IaC version of the **dishonest reporting** that v0.37.0 fixed in the
imperative CLI: reporting success for an operation that did nothing. Here it's worse because
the user changed the file *on purpose* — the signal of intent is explicit and it's discarded.

It's made worse by the fact that the engine **already has** the capability:
`container update` reconfigures ports, volumes, networks, memory and CPU **live, without
changing the PID** — this engine's fundamental difference from Docker. `apply` simply
doesn't call it. It's the same family of pitfall this repo has already paid for four times
(`mount_live`, `set_net_rate`, `update_limits`, `JsonStore::update`): the capability exists, tested,
with no caller.

**Without this there is no possible Terraform provider.** Terraform's cycle is
Create/Read/**Update**/Delete; an `Update` that does nothing and returns success produces
permanent, silent drift — the worst failure mode an IaC tool can have.

### 🔴 F2 — There is no declarative way to remove

`StackCmd` has `Init`/`Apply`/`Ls`/`Describe`/`Validate`
([stack.rs:17](../../crates/delonix-runtime-bin/src/cmd/stack.rs#L17)). There is no
`destroy`, no `prune`, no `--prune`. A resource dropped from the manifest stays alive
forever and nothing flags it — not even `describe`, which only looks at what the file
declares.

Practical consequence: the manifest stops being the source of truth the moment
someone deletes a block. And tearing down an environment today is a hand-written `rm`
script, in reverse order, written by the user — exactly what IaC exists to eliminate.

### 🔴 F3 — There is no drift detection, nor `plan`

`stack describe` answers **presence** (does it exist / doesn't it), not **conformance**
(does what's there match what was declared?). The doc-comment is honest about it: "the
column that matters is PRESENCE". `--dry-run` prints the desired state, not the diff
against the real one.

The central IaC operation is missing: `delonix stack plan` → "container `web`: image
`nginx:1.24` → `nginx:1.27`, memory `256M` → `512M`; volume `dados`: no change;
network `interna`: **missing**". Without this there is no PR review, no
approval before applying, no CI that fails on drift.

### 🟠 F4 — The management API is a small slice of the engine

Existing routes ([lib.rs:186-229](../../crates/delonix-mgmt/src/lib.rs#L186)):

| Resource | Create | Read (get) | List | Update | Delete |
|---|---|---|---|---|---|
| Volume | ✅ | ✅ | ✅ | ❌ | ✅ |
| Container | ✅ | ✅ | ✅ | 🟡 only `publish-add/rm` | ✅ |
| Image | 🟡 pull/build | ❌ | ✅ | ❌ | ✅ |
| Network | ✅ | ❌ | ❌ | ❌ | ✅ |
| VM | ❌ | ❌ | ❌ | ❌ | 🟡 only `action` |
| Pod, Secret, Storage, ShareVolume, HTTPRoute, Ingress, Egress, FirewallPolicy, Dependency, Workload, Cluster | ❌ | ❌ | ❌ | ❌ | ❌ |

**No route applies a manifest.** The API is imperative (POST `/containers` rebuilds
a command line) while the declarative model lives only in the CLI — they're two
surfaces with different powers.

And the fidelity of what exists is low: `RunSpecBody` has **11 fields** (`image`, `name`,
`ports`, `env`, `network`, `memory`, `restart`, `command`, `volumes`, `knows`,
`knows_none`) against **71 flags** of `container run` and ~35 fields of `ContainerSpec`.
No labels, no CPU, no user/workdir/entrypoint, no secrets, no healthcheck, no
namespace, no capabilities, no devices/GPU, no tmpfs/sysctl/ulimits.

A Terraform provider written on top of this API would express **less** than the YAML already
expresses today. The missing `Read` for networks and VMs is a blocker on its own: Terraform
does a `Read` on every refresh of *every* resource, and without it there's no `import` or
detection of out-of-band changes.

### 🟠 F5 — The API isn't remote, isn't authenticated, and has no published contract

- **Transport**: unix socket only, no TCP/TLS option
  ([serve.rs:26](../../crates/delonix-runtime-bin/src/cmd/serve.rs#L26)).
- **Authentication**: `SO_PEERCRED`, and the peer must be the server's **same euid**. There
  is no token, no mTLS, no notion of identity whatsoever. This is **correct** for the
  current model (it's the runtime's highest-privilege surface — `/exec` is arbitrary
  execution inside any container) and the decision is well argued in the code.
- **Contract**: zero OpenAPI, zero JSON-Schema (searched via `openapi|schemars|
  JsonSchema` across the whole repo — no occurrences). Zero route documentation on the site:
  the only trace in `README.rst` is one line in a crate table.
- **Stability**: `docs/cli-stability.md` explicitly classifies `serve api` as
  **NOT stable** — "may change in any version".

Adding it up: Terraform and Ansible typically run **from outside** the managed node.
Today the only path is `ssh node -- delonix ...`, and the API doesn't help. The goal "API
ready to be managed by Terraform" requires deciding where the remote plane lives — and
that decision collides with the PaaS boundary (multi-tenant/authz belongs to
`delonix-paas`). **It's an architecture decision, not a task; it deserves an ADR
before any code.**

### 🟠 F6 — `apply` output isn't machine-readable, and is only partly translated

```
volume.rs:179     println!("volume/{name}: {}", po::t("ensured"));       ← traduzido
network.rs:203    println!("network/{name}: {}", po::t("created"));      ← traduzido
container.rs:1718 println!("container/{name}: already exists, ...");     ← EN cru
```

Two consequences:

1. **i18n inconsistency** — part of `apply`'s output changes with `--l18n=pt` and part
   doesn't. It's the same class of bug v0.32.2 fixed in 380 strings, surviving
   on this path.
2. **There is no structured `changed`/`unchanged` signal.** ADR-0005's `-o json`
   covers listings; **mutations were left out.** An Ansible module lives on knowing whether
   something changed (`changed_when`) — today it would have to `grep` a string that
   changes with the operator's language.

### 🟡 F7 — `--dry-run` only exists on `stack apply`

No imperative command has check-mode. Ansible's `--check` would have nowhere to
map to in `container run`, `network create`, `volumes create`, `vm create`.

### 🟡 F8 — Identity and reads aren't uniform across Kinds

Terraform's `import` requires, for every type, "give me this resource by id, in JSON".
Today: containers have id **and** name; volumes/networks/secrets/storage are by name; VMs by
name; pod members are derived from labels; stacks and clusters **have no dedicated
record** — they're derived from the file and the labels. And reads have three forms
(`inspect` JSON, `describe` text, `ls -o json` summary) that don't match.

It's not wrong — the decision not to invent registries that drift out of sync is well made
and defensible. But it means a provider has to treat `stack` and `cluster` as
resources with no stable identity, and that has to be said up front.

### 🟡 F9 — No resource versioning or optimistic concurrency

There is no `resourceVersion`/`generation`/ETag. The `Store`'s `flock` protects the
**file** against concurrent writes; it doesn't protect **intent**. Two parallel `apply`s — or
a Terraform run and an operator using the CLI — clobber each other undetected. For a single
operator it's irrelevant; for a shared CI pipeline, it isn't.

### 🟡 F10 — Order by Kind, not by a dependency graph

`stack apply` runs in a **fixed order of 13 Kinds**
([stack.rs:126](../../crates/delonix-runtime-bin/src/cmd/stack.rs#L126)). Within a
Kind, the order is the file's order. There is no per-resource `dependsOn` or
readiness gate between resources — and `spec.detach` defaults to `true`, so `apply` doesn't
wait for anything to become healthy before moving on.

Compare with `compose`, which **already has** topological ordering with the three
`depends_on` conditions and waits for the real healthcheck. The capability exists in the
repo; the native declarative path doesn't have it. (`kind: Dependency` is network
reachability, not ordering — similar names, different things.)

### 🟡 F11 — The manifest schema is declared **not stable**

`cli-stability.md`: "the manifest schema (`kind: *`) — fields are additive in
practice, but it's not a promise." For IaC this is exactly backwards from what's
needed: the schema **is** the artefact users put in git and review in a PR. The
CLI is better protected than the declarative format, when it should be the other way around.

### 🟡 F12 — There's no `explain`, and the schema is documented by hand

There is no `delonix explain container.spec.ports`. The schema for the 18 Kinds lives in two
independent places — in the Rust structs and in the hand-written `docs/kinds.html` — with no
generation of one from the other. This repo has already recorded the cost of that
divergence (pages describing `serve docker-api` as read-only, `cluster kubeadm` with no HA,
`network` with no overlay). It's the same pitfall, in a place where the user has no way to
verify it.

---

## 3. What's specifically missing for Terraform

Required order — each line depends on the previous one:

| # | Provider requirement | Status | Blocked by |
|---|---|---|---|
| 1 | Read of **every** type, by id, in JSON | 🟡 partial | F4, F8 |
| 2 | **Update** (in-place or declared replace) | ❌ | **F1** |
| 3 | Delete by id | 🟡 CLI yes, API partial | F2, F4 |
| 4 | Calculable diff before applying (`plan`) | ❌ | F3 |
| 5 | Versioned, stable API contract | ❌ | F5 |
| 6 | Authenticated remote access | ❌ | F5 |
| 7 | `import` of pre-existing resources | ❌ | F8 |
| 8 | Safe concurrency (lock/version) | ❌ | F9 |

**The pragmatic shortcut, and it's a good option:** a provider that speaks no HTTP at
all and runs the local CLI over SSH — the same model as the `null`/`external` provider or
`terraform-provider-shell`. It only needs (1), (2) and (4), i.e.: **`-o json` on
mutations, an `apply` that updates, and `stack plan`**. No remote API, no
authentication, nothing colliding with the PaaS boundary. It becomes functional well before
F5's architecture decision is made.

## 4. What's specifically missing for Ansible

Ansible is **easier** than Terraform here, because it already runs over SSH and keeps no
state. Three things are missing, and only three:

1. **Structured `changed` signal.** Every mutation should emit, with `-o json`,
   `{"kind","name","action":"created|updated|unchanged|deleted","changed":true|false}`.
   It's the heart of an Ansible module and it doesn't exist today (F6).
2. **Check-mode.** `--dry-run` on the imperative commands, not just on `stack apply` (F7).
3. **Convergence.** An Ansible module is idempotent by definition: running it twice
   gives the same result, and running it after a change **converges**. Today it converges
   to "already exists, nothing to do" (F1).

With these three, an `angolardevops.delonix` collection with `delonix_container`,
`delonix_network`, `delonix_volume`, `delonix_stack` modules gets written in days, not months —
each module is a thin wrapper over the CLI. Without them, every module would have to parse
translatable text, which is the anti-pattern ADR-0005's `-o json` exists to kill.

---

## 5. Documentation

### Strong

- **26 command pages** + `kinds.html` (1171 lines, the 18 Kinds) + `cheatsheet` +
  `comparacao` + `c4`/`arquitectura` + `cloud`/`labs`, generated by `docs/gen.py`
  (4670 lines), **bilingual EN/PT** with a switcher.
- **26 YAML examples** under `examples/`, one per Kind, runnable.
- **6 ADRs** with alternatives considered and consequences — the *why* trail, which is
  what 90% of projects lack and what makes someone trust the format.
- `docs/cli-stability.md` — the contract.
- `stack init --template` generates a project that's **already filled in**, which is the
  best documentation there is (it works without reading anything).

### Weak — and this is where the "nothing to memorise" goal fails

1. **The management API is documented nowhere.** Not a page, not a route
   list, not a `curl` example. Anyone wanting to automate via the API has to read
   `crates/interfaces/delonix-mgmt/src/lib.rs`. This is the opposite of the stated goal.
2. **There's no field reference generated from the code.** `kinds.html` is hand
   written; the structs are the source of truth. As long as they're two sources, they'll
   diverge — and the repo already has a history of that. The remedy is generation (a
   `delonix explain`, or a JSON-Schema emitted by the binary and consumed by `gen.py`),
   not more manual reviews.
3. **The page that answers "how do I actually operate this" is missing.** There are
   per-command pages; the cross-cutting guide is missing: GitOps with Delonix, what to
   put in CI, how to roll back when `apply` fails halfway (today the answer is "there
   isn't one — run `stack describe` and clean up by hand", and that should be written down).
4. **There's no schema changelog.** `RELEASES.md` is large and detailed, but anyone
   versioning manifests needs "what changed in the Kinds in v0.46" separate from the rest.
5. **No error catalogue.** The messages are good and actionable (a real merit), but there's
   no page where you can look up an error's text. In a CLI with ~208 subcommands, that's
   what keeps you from getting stuck.

---

## 6. Proposed roadmap, in order of value

Every block ships on its own and unblocks the next ones.

**Block 1 — Convergence.** `apply` starts distinguishing declared spec from real state and
calls the `container update` that already exists. Where a live update isn't possible, recreate
**only with explicit opt-in** (`--replace`), never silently. Closes F1. It's the piece without
which nothing else counts.

**Block 2 — `stack plan` + `-o json` on mutations.** `plan` falls out of Block 1 almost
for free (the diff already has to be computed to converge — all that's missing is printing it
instead of applying it). `-o json` extends ADR-0005 to mutations with the `changed`
field. Closes F3 and F6. **With blocks 1 and 2, Ansible becomes viable.**

**Block 3 — `stack destroy` + `apply --prune`.** Closes F2. Requires deciding what
"belongs to this stack" means without inventing a registry — the likely answer is the same one
`compose` and `cluster` already use: a deterministic label.

**Block 4 — Remote API ADR.** Don't write code before this. Decide: transport, identity
model, and where the PaaS boundary sits. Only then expand routes, publish
OpenAPI, and promote `serve api` to stable in `cli-stability.md`. Closes F5, and only then
is F4 worth the effort.

**Block 5 — `delonix explain` generated from the code**, with `gen.py` consuming the same
JSON-Schema. Closes F12 and documentation points 2 and 5 in one go.

**Cross-cutting, cheap, do it now:** promote the manifest schema to stable in
`cli-stability.md` (F11) — it's the commitment users need, and in practice it's
already being honoured.

---

## 7. Verdict

Delonix has the **model** of a good IaC tool — 18 coherent Kinds, graph
validation, dry-run with materialised defaults, honesty conditions Terraform
doesn't have, and a written stability contract. What it's missing is the **convergence
engine**: today it's a declarative installer that's idempotent-by-presence, not a
reconciler.

The good news is the distance is smaller than it looks, and it doesn't go through any API: the
hard pieces (live reconfiguration without changing the PID, topological ordering with
health waits, `-o json` with a contract) **are already written and tested in this repo** — on
other paths. Blocks 1 and 2 are mostly wiring, not invention, and they are
what separates "Delonix is interesting" from "Delonix is manageable by Terraform and Ansible".
