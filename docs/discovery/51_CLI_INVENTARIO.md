# CLI-0 Phase — inventory, destination matrix, and gaps

> Measured on **2026-08-26** against `origin/main` (`a3e7fa1`, v0.63.1), with the binary
> built from that commit — **not** with the `delonix` installed on the host, which was
> four days behind and did not have `namespace`, `net l4guard`, `stack history`,
> `stack rollback`, `system doctor`, or `system features`. Measuring the binary by hand
> would have produced an inventory with six fewer commands and a matrix that left them
> without a destination without anyone noticing.

## 1. The current surface, in numbers

| metric | value |
|---|---|
| commands in the public tree | **263** |
| invokable leaves | **233** |
| top-level groups | **28** |

To reproduce:

```bash
scripts/cli-tree.sh --count      # os totais desta secção
scripts/cli-tree.sh --leaves     # as 233 folhas
scripts/cli-tree.sh --classify   # folha + classe de impacto (secção 4)
```

The script uses the tree's binary and **states which one** in the output — the first measurement
of this phase was taken against the `delonix` on the `PATH` and ended up six commands
behind.

### Verb duplication, counted and not asserted

Point §2 of the specification asks for confirmation that there is overlap. There is, and the
number is this — the count of the second token of each leaf:

| verb | occurrences | where |
|---|---|---|
| `ls` | **12** | cluster, container(`ps`), image, image vm, namespace, network, pod, secret, sharevolume, stack, storage, vm, volumes, workload |
| `apply` | **10** | cluster, container, image, network, secret, sharevolume, stack, storage, vm, volumes |
| `describe` | **10** | container, image vm, namespace, network, pod, sharevolume, stack, vm, volumes, workload |
| `rm` | **10** | container, image, image vm, pod, secret, sharevolume, storage, vm, volumes, workload |
| `prune` | **7** | cluster, container, image, stack, system, vm, volumes |
| `create` | **7** | network, pod, secret, storage, volumes, vm snapshot, volumes snapshot |
| `inspect` | **5** | container, image(`describe`), network, secret, storage, volumes |
| `init` | **5** | cluster, container, image, image vm, stack, vm (+ the top-level `init` = 7) |
| `dash` | **5** | container, image, network, storage, vm (+ the top-level `dash` = 6) |

Nine declarative verbs repeated ten times each is the exact measure of the problem
that the specification exists to close: **the CRUD for each Kind is written once
per group**, and nothing forces the ten copies to agree. It is the same family of
defect that `cmd/kinds.rs` already fixed on the reconciler side — six lists
that had to agree and drifted — except here it is on the public surface.

### The eleven points of §2, verified one by one

| # | claim | verified | evidence |
|---|---|---|---|
| 1 | overlap between `container`/`pod`/`vm`/`workload` | **yes** | `workload ls` lists containers AND vms; `pod ls`/`vm ls`/`container ps` list the same objects through another door |
| 2 | overlap between `volumes`/`storage`/`sharevolume` | **yes** | three groups, 26 leaves, a single `kind: Volume` underneath since the 5th merge (v0.53.x) |
| 3 | overlap between `network`/`net` | **yes** | 13 + 45 leaves; `network route` and `net ingress` act on the same dataplane |
| 4 | `build` separate from `image` | **yes** | `delonix build` and `delonix image build` coexist |
| 5 | `image --vm` switches store | **yes** | global flag that swaps `ImageStore`↔`VmImageStore`; and there are **three** doors to the same thing (`vm pull`, `image vm pull`, `image --vm pull`) |
| 6 | duplicated CRUD group↔`stack` | **yes** | 10 per-group `apply` against `stack apply` |
| 7 | `schema`/`explain` outside manifest management | **yes** | `schema print` and `explain` are top-level groups |
| 8 | `backup`/`restore` in distinct groups | **yes, and worse** | there are **four** doors: `backup`, `restore`, `system backup`, `system restore` |
| 9 | global and repeated `dash` | **yes** | 6 occurrences |
| 10 | top-level argv aliases | **yes** | `ps`/`run`/`exec`/`logs`/`rm`/`images`, via `argv` rewriting before `clap` |
| 11 | divergent `ls`/`ps`/`get`/`status`/`inspect`/`describe` | **yes** | `vm status` and `vm describe` answer the same question with different formats |

Point 8 is the only one where the specification **underestimated** the problem: it listed two
groups and there are four.

## 2. Conflicts with ALREADY PUBLISHED contracts

The specification says it takes precedence over the old CLI. It does — but three of the
changes are not just reorganisation: **they break promises written** in
`docs/cli-stability.md`, which exists precisely to say what does not break. Either
the document is revised in the same commit, or the promise becomes a lie.

### 2.1 The top-level shortcuts are declared STABLE

`docs/cli-stability.md`, section «Stable — does not break without a major»:

> **The top-level shortcuts** (`ps`, `run`, `exec`, `logs`, `rm`, `images`), which are
> literally the same command via argv rewriting.

§3.4 of the specification orders them removed. It is a legitimate decision by the product
owner, but it is **a major**, not a cleanup — and the cost falls entirely on the
adoption audience (those coming from Docker who type `delonix ps`).

### 2.2 Exit codes collide with the v0.49.0 table

Published, with a migration note and a bash example:

| published (v0.49.0) | proposed (§19) | collision |
|---|---|---|
| `3` = exists but is not running | — | `3` disappears with no named replacement |
| `4` = **does not exist** | `66` = not found | **a script written against the published table starts silently misclassifying** |
| `5` = conflict | `73` = conflict | same |

The `4` case is the serious one: the value does not become invalid, it becomes
*something else* — a `case $? in 4)` that today creates the missing resource will
stop matching anything, and the `*) exit 1` branch swallows it. The published table
literally carries that `case` as a recommended example.

**Recommendation**: keep `3`/`4`/`5` and add the new codes only for
classes that do not exist today (`69` capability unavailable, `75` retryable, `77`
permission, `124` timeout). This meets the goal of §19 — giving a class to what
doesn't have one — without invalidating what has already been promised. The alternative
(adopting `64`–`77` wholesale) requires its own migration note and a major.

### 2.3 `delonix build` is declared stable

The same section lists `image pull ls rm build (delonix build)`. §10 moves it to
`image build --type container`. Same treatment as 2.1.

### 2.5 The manifest schema — the most serious break, and the least visible

`docs/cli-stability.md` has a section titled **«The manifest schema —
stable, and it matters most»**. For `Container`, `Pod`, `Volume`, and
`Network` it promises, within `0.x`:

* a field is **never** removed, nor does it change type or meaning;
* a renamed name **remains accepted as an alias**;
* **`apiVersion: delonix.io/v1` only changes with a `v2`, and a `v2` does not ship unless
  `v1` continues to be accepted.**

The restructuring of the 12 Kinds breaks all three:

| promise | what the restructuring does |
|---|---|
| field never removed | `kind: Container` disappears — **all** of its fields with it, and it is one of the four Kinds covered by the promise |
| alias for renamed name | `Vm`→`VirtualMachine`, `Ingress`/`FirewallPolicy`→`NetworkPolicy` with no declared step |
| `v1` only changes with `v2` | it moves to `compute.delonix.io/v1alpha1` — which is not `v2` and is a step **down** in announced maturity |

There is also a CI gate that this triggers: `scripts/schema-diff.sh` compares field
by field between two tags and **flags a removed field as a contract
break**. The restructuring triggers it by design.

**And the repo already has the right answer written down**, at the end of that same section, about
three Kinds that were merged:

> the old names continue to load, with a deprecation warning — the «clean cut»
> rule applies to commands, and a manifest in git deserves a step down
> instead of an error.

It is the distinction that §3.4 of the specification does not make. **Clean cut in commands:
correct, and it is the v0.30.0 precedent.** Clean cut in manifests: breaks
files that are in git, reviewed in a PR and referenced via
`$schema` in editors — and breaks them without anyone running a command.

**Recommendation**: `delonix.io/v1` continues to LOAD with a deprecation warning and
to downgrade to the new Kinds in `load` — exactly the mechanism that
`Egress`→`FirewallPolicy` and `Storage`→`Volume` already use, and that `cmd/kinds.rs`
models as `Form::Deprecated`. It costs one branch in `load` per old Kind and
keeps the promise. `kind: Container` downgrades to `kind: Pod` for a container,
which is literally what §3.3 says it is.

### 2.4 `--l18n` → `--language`

It is not on the stable list, so it is the cheapest change of the four. But
`DELONIX_L18N` appears in published documentation and in `scripts/e2e.sh`. Accepting
both spellings for one cycle costs three lines and avoids breaking someone else's CI;
silently rejecting the old one is the trap this repo has already catalogued.

## 3. Destination matrix — the 233 leaves

Legend for the **impact** column:
`=` no break (transparent rename or already equivalent) ·
`~` spelling break, same capability ·
`!` break of a published contract ·
`+` new capability required by the specification ·
`?` **no destination in the proposed tree** — open decision

### 3.1 Declarative — the CRUD that collapses into generic verbs

These 47 leaves are the raison d'être of the restructuring: ten copies of a CRUD that
now gets written once.

| old | new | semantic reason | impact |
|---|---|---|---|
| `container apply` | `apply -f` | Kind `Container` ceases to exist (§3.3) | `!` |
| `image apply` | `apply -f` | unified declarative CRUD | `~` |
| `network apply` | `apply -f` | same | `~` |
| `secret apply` | `apply -f` | same | `~` |
| `sharevolume apply` | `apply -f` | group eliminated (§12) | `~` |
| `storage apply` | `apply -f` | group eliminated (§12) | `~` |
| `volumes apply` | `apply -f` | same | `~` |
| `vm apply` | `apply -f` | same | `~` |
| `cluster apply` | `apply -f` | `KubernetesCluster` (§13) | `~` |
| `stack apply` | `apply` | canonical verb | `~` |
| `stack plan` | `plan` | canonical verb | `~` |
| `stack destroy` | `delete -f --owner` | canonical verb (§5.7) | `~` |
| `stack wait` | `wait -f` | canonical verb (§5.8) | `~` |
| `stack validate` | `manifest validate` | §6.1 | `~` |
| `stack ls` | `get -f` / `get <kind>` | generic listing | `~` |
| `stack describe` | `describe -f` | §5.6 | `~` |
| `stack prune` | `apply --prune` | pruning is a mode of apply, not a command | `~` |
| `stack init` | `init` | the top-level `init` already delegates | `=` |
| `stack history` | `manifest history` **or** `get stackrevisions` | ADR-0019; **the specification does not provide for it** | `?` |
| `stack rollback` | `apply --revision <n>` | ADR-0019; **not provided for** | `?` |
| `schema print` | `manifest schema` | §6.4 | `~` |
| `explain` | `explain` | stays at the top level (§4) | `=` |
| `pod create` | `apply -f` | §8 | `~` |
| `pod ls` | `get pods` | §8 | `~` |
| `pod rm` | `delete pod` | §8 | `~` |
| `pod describe` | `describe pod` | §8 | `~` |
| `pod logs` | `pod logs` | day-2, stays | `=` |
| `vm create` | `apply -f` | §9 | `~` |
| `vm ls` | `get virtualmachines` | §9 | `~` |
| `vm rm` | `delete virtualmachine` | §9 | `~` |
| `vm status` | `get vm <name>` | §9 — merges with `describe` | `~` |
| `vm describe` | `describe vm` | §9 | `~` |
| `network create` | `apply -f` | §11 | `~` |
| `network ls` | `get networks` | §11 | `~` |
| `network rm` | `delete network` | §11 | `~` |
| `network describe` | `describe network` | §11 | `~` |
| `network inspect` | `get network -o json` | `inspect`≡`get -o json` | `~` |
| `network route` | `apply -f` (`NetworkRoute`) | §11 | `~` |
| `volumes create` | `apply -f` | §12 | `~` |
| `volumes ls` | `get volumes` | §12 | `~` |
| `volumes rm` | `delete volume` | §12 | `~` |
| `volumes describe` | `describe volume` | §12 | `~` |
| `volumes inspect` | `get volume -o json` | §12 | `~` |
| `storage create/ls/rm/inspect` | `apply`/`get`/`delete` (`Volume`) | group eliminated (§12) | `~` |
| `sharevolume ls/rm/describe` | `apply`/`get`/`delete` (`Volume`) | group eliminated (§12) | `~` |
| `cluster create` | `apply -f` (`KubernetesCluster`) | §13 | `~` |
| `cluster kubeadm` | `apply -f` (provider `kubeadm`) | §13 | `~` |
| `cluster ls` | `get kubernetesclusters` | §13 | `~` |
| `cluster delete` | `delete kubernetescluster` | §13 | `~` |
| `secret ls/rm/inspect` | `get`/`delete`/`get -o json` | §5 | `~` |
| `workload ls/rm/stop/describe` | `get`/`delete`/… over Pod+VM | group eliminated (§5) | `~` |

### 3.2 Imperative compatibility — `container` stays

The 27 leaves of the `container` group stay where they are (§7), with four exceptions
listed in 3.5. It is the adoption bridge and the specification explicitly
preserves it.

`run · ps · start · stop · restart · kill · wait · rm · exec · logs · attach ·
inspect · update · port · rename · pause · unpause · cp · diff · commit · top ·
stats · healthcheck · ssh` — **all `=`**.

### 3.3 Day-2 — survive because they are not CRUD

| old | new | impact |
|---|---|---|
| `pod logs` / `pod exec`¹ | `pod logs` / `pod exec` | `=` |
| `vm start/stop/restart/console/ssh` | same | `=` |
| `vm snapshot create/ls/rm/restore` | `vm snapshot …` | `=` |
| `volumes snapshot create/ls/rm/restore` | `volume snapshot …` | `~` (plural→singular) |
| `image pull/push/build/scan/verify/convert/import/export` | `image …` | `=` |
| `cluster load` | `cluster load` | `=` |
| `secret create` | `secret create` | `=` |
| `secret rotate-key` | `secret rotate` | `~` |
| `net flow` | `network flow` | `~` |
| `net boot enable/disable/status` | `system boot …` | `~` |
| `namespace ls/describe` | `system namespace …` | `~` |
| `system info/events/df/prune/doctor/features` | same | `=` |
| `dash` | `dashboard` | `~` |

¹ `pod exec`/`attach`/`cp`/`port-forward` **do not exist today** — they are new
capability (§8), marked `+` in the total.

### 3.4 Policy — what collapses into new Kinds

| old | new | impact |
|---|---|---|
| `net ingress allow/deny/policy/clear/rm/ls` | `apply -f` (`NetworkPolicy`) | `~` |
| `net egress allow/deny/host/net/policy/clear/rm/ls/show` | `apply -f` (`NetworkPolicy`) | `~` |
| `net ingress publish/unpublish` | `apply -f` (`Service`) | `~` |
| `net httproute apply/ls/rm` | `apply -f` (`HTTPRoute`) | `~` |
| `net tunnel expose/rm/ls/describe/apply` | `apply -f` (`Gateway`) | `~` |

### 3.5 **Gaps — leaves with no destination in the proposed tree**

This is the finding that the CLI-0 Phase exists to produce. **31 leaves** have no
destination in the §4 tree, and each one requires a decision before the CLI-5 Phase
(removal) can run — otherwise they are eliminated by omission, which is the most
expensive way to break a CLI.

| leaf | what it does | proposal | why |
|---|---|---|---|
| `net l4guard set/clear/status` | per-origin L4 limiter | `apply -f` (`NetworkPolicy.spec.rateLimit`) | it is policy; it arrived after the specification was written |
| `net netns` (9 leaves) | holder plumbing | **hidden subcommand** | §11 mandates hiding it, but doesn't say where — it becomes `__netns`, outside the public tree |
| `system setup` | prepares cgroup delegation on the host | `system setup` | local administration, fits in §15 |
| `system virt` | reports virtualization support | `system doctor` | it is a capability question |
| `system thermal` | hardware temperatures | `system metrics` | §15 already provides for `metrics` |
| `system monitor` | continuous monitor | `dashboard` | duplicated the dash |
| `system backup` / `system restore` | engine-state backup | `backup create --scope engine` | 4th and 3rd door to the same thing (see §2 point 8) |
| `cluster kube generate` | generates k8s YAML from a manifest | `manifest render --target kubernetes` | §6.2 is the natural place |
| `cluster init` / `container init` / `image init` / `image vm init` / `vm init` | scaffold by type | `init` (top level, already delegates) | §4 keeps a single `init` |
| `container prune` / `image prune` / `vm prune` / `volumes prune` / `cluster prune` | per-domain GC | `system prune --scope <x>` | one GC, one command (§15) |
| `container dash` / `image dash` / `network dash` / `storage dash` / `vm dash` | per-resource dash | `dashboard --scope <x>` | §22 mandates eliminating the duplicates |
| `image tag` | retag a local image | `image tag` | legitimate day-2, missing from the tree |
| `image login` / `image logout` | registry credentials | `image login`/`logout` | legitimate day-2, missing from the tree |
| `image load` / `image save` | Docker archive | `image import`/`export` | §10 already has both verbs |
| `image history` | image layers | `image inspect` | same question |
| `image ls-remote` / `vm ls-remote` | tags of a repository | `image list --remote` | missing from the tree |
| `vm vnc` | graphical console | `vm console --vnc` | console mode, not a command |
| `vm reach` | VM→container diagnostic | `network diagnose` | §11 already has the place |
| `vm bridge` / `vm unbridge` | privileged VM↔SDN bridge | `network bridge`¹ | privileged and EXPERIMENTAL |
| `vm default-backend` | hypervisor preference | `config set-context --vm-backend` | §16 is the place for preferences |
| `vm build/convert/pull/push` | the 3rd door of `image --vm` | `image … --type virtual-machine` | §10 closes the three doors into one |
| `network vlan` | VLAN of a network | `apply -f` (`Network.spec.vlan`) | it's a field, not a command |
| `network node key` / `node init` | WireGuard overlay keys | `network node …` | missing from the tree |
| `sharevolume migrate` | absorbs legacy `ShareRecord` | `manifest migrate` | §6.3 is the place |
| `serve api` | local management API | `serve management-api` | §4 renames it |
| `syntax` | VMfile highlighting | `completion editor` | §21 |
| `container ssh` | SSH into the container | `container exec` with `--ssh`? | **open** — duplicates `exec` |
| `stack history` / `stack rollback` | ADR-0019 | see 3.1 | **open** |

¹ `vm bridge` continues to require root and to be the only declared exception to the
rootless model. Moving it to `network` does not change that, and the proposed tree does not
have a place for a privileged command — open decision.

## 4. Final count of the matrix

Derived from the leaf-by-leaf classification (`scripts/cli-tree.sh --classify`), not
estimated — a first version of this section carried numbers that summed correctly and
had been invented, which is the dishonest reporting this repo chases down.

| class | leaves |
|---|---|
| `~` spelling break, same capability | **144** |
| `=` no break | **70** |
| `?` no destination / open decision | **17** |
| `!` break of a published contract (leaves) | **2** |
| **total** | **233** |

To the 2 `!` leaves are added **four breaks that are not leaves** and therefore do not
appear in this count: the six top-level shortcuts (they are `argv` rewriting, not
`clap` subcommands), the exit code table, the `--l18n`, and — the most serious —
the manifest schema (§2.5).

**CLI-0 Phase exit criterion met**: the 233 leaves have a documented
destination; 17 of them with the destination marked as an open decision, which is
precisely what this phase exists to expose before someone deletes them.

## 5. Blocking dependency

The CLI-2 Phase (declarative verbs) **cannot start** before the restructuring
of the 12 Kinds. Measured: `origin/main` serves `apiVersion: delonix.io/v1` with 15
Kinds, and `Pod`, `VirtualMachine`, `Service`, `Gateway`,
`NetworkPolicy`, and `KubernetesCluster` do not exist. Building `get pods` against a Kind that
does not exist is writing a command that can have no caller — the dead-code
pattern this repo has already deleted four times (`publish_port_allow`, `Net`,
`mount_live`, `set_net_rate`).

Order that respects the dependency:

```
Kinds (12)  →  CLI-1 (contratos)  →  CLI-2 (verbos)  →  CLI-3 (day-2)
                     ↑
              pode correr JÁ, em paralelo com os Kinds
```

The CLI-1 Phase (ResourceRef, contexts, output, JSON errors, exit codes,
cancellation) is independent of the Kinds and is the work that can start today.

## 6. Two methodological traps this phase paid for

They are recorded here because neither is about the CLI — they are about how one measures,
and both were already catalogued in `AGENTS.md` before I fell into them again.

**The `$?` after a pipe belongs to the last command.** I ran
`cargo test --workspace 2>&1 | tee log | grep …` and the harness announced **exit code
0**. `grep` exited 0; `cargo` had failed. Had I accepted the number, I would have
reported the battery green over a suite with a failing test. What caught it
was counting the suites (**4**, when a single run of the binary already had 672
tests) — a number too good to be true is the signal to turn off the filter and recount.

**Half-isolation is worse than none.** The test that failed
(`delonix-mgmt::redes_lista_get_e_estado`) passes **29/29 twice** when the
suite runs alone, and fails when `--workspace` runs several test
binaries in parallel against the same real machine state. It is not test
flakiness nor an effect of the change — it is the shared `DELONIX_ROOT` and
`DELONIX_NET_RUNTIME_DIR`, exactly the 2026-08-12 incident that restarted a
production container. This phase's battery runs with **both** redirected:

```bash
DELONIX_ROOT=<iso>/root DELONIX_NET_RUNTIME_DIR=<iso>/netrt cargo test --workspace
```

This is debt to close before the CLI-2 Phase: plain `cargo test --workspace` is not
reproducible in this repo and nobody says so anywhere.

## 7. CLI-1 Phase — what has already landed

| contract | status | real consumer today |
|---|---|---|
| exit codes (`69`, `124`) | **done** | `stack wait`, `wg`, `virt-customize`, `ngrok`, `cloudflared`, `systemd-run` |
| textual identity `DX_*` | **done** | `serve api` error body |
| shortnames from the registry | **done** | `explain`, `stack apply --replace` |
| `ResourceRef` (the TYPE) | **deferred, on purpose** | — |
| contexts (`config set-context`) | **in conflict with ADR-0010** | — |
| output formats, cancellation, request IDs | to do | — |

### `ResourceRef` was written and deleted

The type (`kind`/`kind/name`, with tests) did come to exist and was removed before the
commit: nothing consumes a resource REFERENCE until the declarative verbs exist, and
this repo has already deleted four public APIs that ended up with no caller and created
latent bugs nobody could notice — `publish_port_allow`, `Net`, and the parameters
ignored in `mount_live` and in `reexec_start`. A fifth, written by the same
hand writing this sentence, would be no better.

What was recorded in the module's doc, so it need not be re-derived: `kind/name` is
one argument and `kind name` is two; naming the resource twice is rejected
instead of resolved; and `pod/` is rejected instead of being read as the collection —
otherwise a typo in a `delete` becomes all pods.

What remained is what HAS a caller: `resolve_kind`, plus the `plural` and `short` columns in
the Kinds table. Two real edges were closed along the way:

- **`explain` only accepted the exact canonical spelling.** `explain pods` and
  `explain po` failed — and `pods` is precisely what is on one's fingers right
  after typing `get pods`. All four spellings now work, and the two rejections
  (Kind with no typed schema, token that is not a Kind) keep their targeted hints.
- **`stack apply --replace` compared strings.** `--replace container/web`
  (lowercase) did not match: the authorization was silently ignored and the apply
  rejected the flag the person believed they had passed. At a destructive
  gate, that is the worst way to fail. The KIND goes through the registry; the **NAME
  never** — resolving names is how a `--replace` starts authorizing a resource
  nobody mentioned.

### Contexts: §16 collides with ADR-0010

§16 asks for `config set-context --endpoint`, with identity and TLS, «to prepare
the CLI for local and remote management». This repo's **ADR-0010 rejected the remote
management API**, and the reason applies just as much to the client: *remoteness*
without identity, authorization, and auditing is not remoteness worth having, and that
half lives in `delonix-paas`.

Building the client side of a capability the engine decided not to have is the same
class of consumer-less code that `ResourceRef` just came out of. What
**does not** collide, and is the useful part, is a purely LOCAL context: default
namespace, preferred output format, `DELONIX_ROOT`. It stands as a decision to be made
before the CLI-1 closes.

## 8. The §18 output contract — measured before touching anything

The specification lists eleven rules for what automation reads. I measured them against the
binary **before** writing code, and the result reversed the plan: of those that are
testable today, **all were already being met**.

| §18 rule | measured state |
|---|---|
| empty arrays stay arrays | **met** — `[]` in all six `ls`/`ps` |
| JSON with no translated text | **met** — `-o json` byte-for-byte identical in EN and PT, in listing and in plan |
| no ANSI when stdout is not a TTY | **met** — zero escapes in a pipe |
| data on stdout, everything else on stderr | **met** — `-o json` writes nothing to stderr |
| secrets redacted | **met** — `ls` carries key names, `inspect` redacts and says how to reveal |
| `table`/`wide`/`name`/`yaml` | **`wide`, `name`, and `yaml` do not exist** |

So the useful work here **was not adding formats**. First because
`yaml`/`name`/`wide` on the current `ls` commands is work CLI-2 throws away once
`get` replaces them; and second because what was really missing was something else:
**none of the five properties already being met was locked in by a gate**.
What nobody checks is what breaks again, and each of them breaks
silently — a `[]` that turns into `""` breaks every `jq '.[]'` out there with no
message, and an ANSI escape that starts leaking into a pipe makes a `grep` stop
matching for a reason invisible to the naked eye.

13 new checks land in `scripts/e2e.sh`, and they are properties of the OUTPUT and not of
a command: when `get` replaces these `ls` commands, the block changes target, not
meaning. The three assertions whose logic could have been subtly wrong were
verified to discriminate against deliberately bad input — above all the
ANSI one, whose `$'\033'` inside nested quoting was the fragile point.

**To be decided**: `wide`, `name`, and `yaml` come in with `get` (CLI-2) and not before.

## 9. Cancellation — and the only real bug CLI-1 found

§20 asks for «TTY restored after failure». Measured on **2026-08-26**, it was not:

```
antes:   raw=False
durante: raw=True     (sessão interactiva, correcto)
SIGTERM →
depois:  raw=True     ← a shell de quem chamou fica sem eco e sem edição de linha
```

A `SIGTERM` to a `container exec -it` left the terminal in raw mode. Whoever
was hit by it ended up typing blind until remembering `reset`.

**The cause is not carelessness.** `restore_mode` runs on every normal exit —
including the error one, which is why the `exec`'s `?` is placed AFTER it, something
someone thought through carefully. What runs in no Rust code at all is a signal: no
destructors, no unwinding. It happens with any death by signal — a `kill`,
a CI timeout, a session teardown, the OOM killer.

**The fix lives at the boundary** (`set_raw_mode`, in the engine) and not in the command,
so every caller of the interactive path inherits it — the same discipline as
`missing_wg`. An `AtomicPtr` and not a `static mut`, because the handler reads this from
an asynchronous context and an atomic read is one of the few legal things there;
`tcsetattr`, `signal`, and `raise` are on POSIX's async-signal-safe list, and
nothing else in the handler allocates, locks, or formats.

**Re-raising with default disposition is what keeps the exit status honest**:
a process killed by `SIGTERM` has to keep looking killed by `SIGTERM`
(`128+15`) to whoever is waiting on it. Swallowing the signal to exit cleanly would trade a
broken terminal for a lie about how the process ended. Measured across all
four: `rc=-15`, `-2`, `-1`, `-3`.

**The gate measures the TERMINAL and not the command** — a `check` on exit code stayed
green over the bug, because the process still died with the same status.
Verified to fail with the fix reverted: `15: the terminal STAYED in raw
mode`. `SIGKILL` is left out on purpose: it is not catchable, and promising to restore
in that case would be a lie.

### What was NOT done, and why

`delonix` still has **no SIGINT handler** for non-interactive operations.
That is deliberate for now: the question «does a Ctrl-C mid-`stack apply` leave the
stack half-converged?» has a known, written answer — apply is fail-fast
with no rollback, and whatever has already been applied STAYS applied. Giving it a handler that
tried to undo it would be inventing transactionality the engine does not have, and that is
ADR material, not a CLI slice.

## 10. The open decisions, closed (2026-08-26)

The CLI-0 Phase exposed 17 leaves with no destination and two specification conflicts. All of them
are decided here, so CLI-5 does not delete them by omission and CLI-2 can
start with nothing left hanging.

### 10.1 `net netns` (9 leaves) → HIDDEN subcommand

`net netns {up,down,status,exec,attach,detach,firewall,publish,unpublish}` becomes
`__netns`, outside the public tree, as `ingress-proxy` already is.

It is not a capability removal: `docs/cli-stability.md` already declares «**everything
starting with `net netns`** — internal plumbing exposed for debugging
convenience» as NOT stable. What changes is that it stops being at the same distance from
the user as `container run`. `net netns down` remains the recovery
command for an in-place upgrade, and the `stale_holder_message` message has to
start naming the new form — otherwise the diagnosis tells someone to type a command that
no longer exists.

### 10.2 `net l4guard` (3 leaves) → `network l4guard`

It is a **node-level** L4 guard (per-origin connection rate and a cap on
concurrents), ingress-wide, not a per-workload policy. That is why it does **not**
become a `NetworkPolicy` field: that Kind describes what is allowed to ONE target, and
squeezing a global threshold in there would be the same merging of two questions that this
engine rejects between `NetworkRoute` and `FirewallPolicy`.

It stays in the day-2 network group, alongside `diagnose`/`flow`/`capture`. It arrived
after the specification was written, which is the reason it is not in its
tree.

### 10.3 `stack history` / `stack rollback` → `get revisions` and `apply --revision`

`rollback` **is an apply** — the `--help` itself says so: it replays the revision N
manifest through the normal path and gains its own revision. A dedicated verb for
«apply with different input» would be a second path to keep in agreement with the
first.

* `stack rollback --to N` → **`apply --revision N`**
* `stack history` → **`get revisions`**, with `--show N` staying

This adds a line to the Kinds registry (`Revision`, `revisions`, with no
abbreviation). It is a RECORD and never a source of truth — ADR-0019 is explicit — and
its `presence` is `Registry`, because there really is a store underneath.

### 10.4 `container ssh` → REMOVED, with the behaviour absorbed by `exec`

The name promises SSH and delivers something else: the `--help` says «shortcut for
`exec -t`» and what it does is try `bash` and fall back to `sh`. Neither Docker nor
Podman have this verb, and `container` exists precisely to mirror what they taught.

The useful capability is not the command, it is the **fallback**: `container exec -it <id>`
with no command now tries `bash` and falls back to `sh`, which is what the person
wanted. With that done, `ssh` adds nothing and leaves in CLI-5's clean cut.

**DONE (2026-09-12, Sprint 2 of the CLI restructuring requested in a
DevOps/SRE audit).** `container ssh` removed (clean cut — fails with `unrecognized
subcommand`, never silently); `container exec` with no `COMMAND` does the fallback.
**Real inherited bug, caught by live validation and not by reading**: the original
argv of `cmd_ssh` was `exec /bin/bash 2>/dev/null || exec /bin/sh` — and a
failed `exec` in a NON-interactive POSIX shell terminates that shell on the spot, without
reaching the `||` (confirmed with `busybox sh`/`dash`, not merely assumed). Against
any image without `bash` — `alpine` included — the fallback `container ssh`
promised never ran; it always returned the entire shell's `exec` failing.
Fixed along the way to `command -v bash >/dev/null 2>&1 && exec bash || exec
sh`, which only tests the binary's presence and never reaches an `exec` that could
fail. `vm ssh` (real SSH, over the network, into a VM) stays untouched — the
name collision this also resolved was with this `container ssh`, never
with that one.

### 10.5 `vm bridge` / `vm unbridge` → `network bridge` / `network unbridge`

**They take a NETWORK as an argument** (`vm bridge <NETWORK>`), which settles the
question: it is an operation on the network, not on a VM. They move into the
day-2 network group with everything that protects them intact — the default dry-run, the
root requirement, and the EXPERIMENTAL label.

It remains the **only declared exception to the rootless model**, and changing
group does not change that. The proposed tree had no place for a privileged
command; now it does, and it is the same place where `capture` already lives, which
also requires authorization.

### 10.6 Contexts (§16) → **only the LOCAL context**

§16 asks for `endpoint`, `identity`, and `tls configuration`. **ADR-0010 rejected the
remote management API**, and the reason applies just as much to the client side:
*remoteness* without identity, authorization, and auditing is not remoteness worth
having, and that half lives in `delonix-paas`. Building the client of a capability
the engine decided not to have is the same class of consumer-less code that
`ResourceRef` already came out of.

What remains is what is useful and does not conflict — a **purely local** context:

```
namespace   o `-n` por omissão
output      o `-o` por omissão
root        o `DELONIX_ROOT` a usar
```

`endpoint`, `identity`, and `tls` **do not enter**. They reopen with the successor ADR to
0010, which will have to name the concrete consumer — and at that point the field is born
with a server on the other side, instead of waiting for one.

### 10.7 The matrix ends with no open leaves

| class | before | now |
|---|---|---|
| `~` spelling change | 144 | 144 |
| `=` no break | 70 | 70 |
| `?` open | **17** | **0** |
| `!` contract break | 2 | 2 |
| `→` moved by this section's decision | — | **17** |

**The CLI-0 Phase closes here.** What remains before CLI-2 is no decision at all —
it is the restructuring of the 12 Kinds, which is the other prompt.

## 11. The first CLI-2 command: `api-resources`

CLI-2 is blocked on the 12 Kinds — **except for one command**. `api-resources`
lists whatever is in the registry, so the ROWS change with the restructuring without
the mechanism changing. That is why it is the first to land, and it is what gives
visible surface to everything CLI-0 and CLI-1 built.

```
NAME               SHORTNAMES   APIVERSION      KIND             NAMESPACED   DOMAIN         FORM
secrets            sec          delonix.io/v1   Secret           false        artifact       primary
pods               po           delonix.io/v1   Pod              true         compute        primary
egresses                        delonix.io/v1   Egress           false        net-policy     deprecated → FirewallPolicy
```

**There is no second table underneath.** The listing derives from the same `cmd::kinds` that
the parser, the schema, completion, and the reconciler read — a hand-written listing
alongside it is how the two start disagreeing about which Kinds exist, which is the
defect that module was written to remove.

**`FORM` is not in `kubectl`'s table and is the column one cannot guess:**
it says whether a document of that Kind survives loading under its own name. It is the
answer to «why does my `kind: Egress` never show up in the plan with that
name».

**`apiVersion` became a COLUMN and not a constant**, even though today all 19 rows
say the same thing. The restructuring splits it by domain, and a Kind whose version lives
in a shared `const` cannot be moved one at a time — which is the only way
for that migration to be reviewed.

### Two invariants that got locked in

* **`NAMESPACED` cannot disagree with the loader.** `honors_namespace` is
  what decides whether a `metadata.namespace` is honoured or warned about; the table
  saying something else was worse than saying nothing — the person writes the namespace because the
  table said the Kind takes it, and load silently ignores it. `PerDocument`
  is what makes this non-trivial.
* **Every listed plural resolves in `explain`** (E2E). A documented name that
  did not work would be the table publishing an invalid spelling.

### Two notes of honesty

`api-resources` **did not get a site page**, and that is a choice: of 234
leaves, the generator documents 32 GROUPS, and `explain` — its closest sibling — also
has no page. A new entry there costs a title, a tagline, an intro, and examples in
PT and EN, and it makes more sense written once for the whole set of CLI-2 verbs
than for a single isolated command.

**But «the site does not require coverage» is not «nothing requires it»**, and I wrote
that before measuring. The **manual** requires it: `todo_o_comando_tem_entrada_no_manual`
failed with «1 command(s) with no entry in manual_entries.rs: api-resources», and
`a_descricao_curta_cabe_numa_linha` failed alongside it, because the `about` I wrote
had 140 characters against a 110 cap. Both are real requirements I had
assumed did not exist after looking only at `docs/gen.py`. Both
closed — the short description split in two (the rest became `long_about`,
nothing was lost) and the entry written, with the example comments translated.

The lesson is the usual one in this repo: **searching in one place and
concluding about all of them**. CLI coverage is watched in two different places, and
I had seen one.

And the surface gate **fired on a real change**, not a fabricated one: the new
leaf showed up as unclassified and forced deciding its class (`=`, because
it is born at the destination and never had another spelling). It is the first proof that it
works outside of a negative test.

## 12. A Kind's name now has one place

The first rename (`Tunnel`→`Gateway`) cost **15 string sites across five
files**, and it measured the problem: a Kind's name was a repeated literal, with
nothing to catch a forgotten site. And a forgotten site **does not fail loudly** — it makes
a code path stop recognizing a Kind the rest of the engine keeps
serving.

It is the same defect that `cmd/kinds.rs` had already removed for the FACTS about a
Kind — six lists that had to agree and drifted — still standing for the
NAME.

### What the measurement changed in the plan

The initial estimate was 106 literals. **It is 460.** The earlier number was only for the
four Kinds being renamed. But the raw count is not the answer, because most
of it is not Kind identity:

| category | sites | decision |
|---|---|---|
| dispatch (`match`, `of_kind`, `kind:`) | **113** | migrated to constants |
| test fixtures | ~138 | **stay literals** |
| labels, documentation, `manual_entries` | the rest | untouched |

**Tests keep literals, and that is deliberate.** A test that writes
`"Gateway"` by hand tests the REAL format that goes to disk; swapping it for the
constant would make it tautological — it would start FOLLOWING a rename instead of
CATCHING it. The safety net has to stand outside of what it watches.

### The idiom is safe because of a setting in this repo

A `&'static str` constant is a legal `match` pattern, and a misspelled name
degrades into a BINDING that catches everything — a known footgun. I verified it with a
two-minute program before adopting the idiom, instead of assuming: the binding
makes the following arms unreachable, and this repo runs with `-D warnings`,
so it is a compile error. **Whoever turns off `-D warnings` loses this
guarantee**, and that is why it is written in the constants' doc-comment.

### The payoff, measured

```
pub(crate) const CLUSTER: &str = "KubernetesCluster";   ← uma linha
→ 0 erros de compilação
```

A rename went from 15 sites across five files to **one line**, plus the
alias in `canonical_kind` and its line in the test that keeps the old names
loading. The three remaining — including `Vm`, which was the most expensive at 54
sites — now have the same cost.

---

## 10. CLI-2 Phase — the first slice, and why it is ADDITIVE

The §3 matrix describes a cut: `network ls` **becomes** `get networks`, and
the old one disappears. This slice does half of that — it adds the generic verb and
**removes nothing** —, and the reason is not caution, it is an ordering the matrix does not state.

**A cut is only honest when the destination already does everything the source did.**
Today `get` routes five of the sixteen Kinds. Removing `network ls` now
would leave seven Kinds with no way to be listed at all — the restructuring
taking away capability instead of tidying it up. The cut is the next slice, and it can
only happen once the three routing lists cover everything that has `ls` today.

### What landed

`get`, `describe`, and `delete` at the top level, resolved by the SAME registry that
`api-resources` prints — so `pods`, `pod`, and `po` are the same question, and a
Kind renamed in v0.64.0 still answers to the old name.

They route to each group's `run` instead of reimplementing. That is
verifiable and is verified: the battery compares the JSON output of both spellings and
demands **byte-for-byte equality** — five pairs, all equal. A unit test
could not have shown this.

### Three things the slice decided

**`delete` with no name REFUSES.** Never «all of that Kind». A missing argument
is a typo far more often than an intention, and this verb gets no
second chance to ask.

**A Kind that cannot be asked about this way explains itself.** `get stacks` says that a Stack
is read from a manifest; `get workloads` says the Kind downgrades to three others.
Answering with an empty list would be worse than refusing: it reads as «there are none».

**The routing lists GOVERN dispatch.** The first version had them sitting beside
a `match` and used them only in tests — the second list this repo has already paid for
several times. Now the list decides, and a `match` with no matching arm returns an
error that identifies itself as an internal defect, not a mistyped command.

### The defect the slice found in itself

The recovery message was built with the registry's PLURAL, and told people to use
`virtualmachines ls` — a command this CLI never had. The group a
person actually types is `vm`. A wrong road sign is worse than none: it costs the
time of discovering it was wrong.

Closed with `cli_group()` and a gate that walks the REAL `clap` tree and rejects
a suggestion that does not exist there. Verified by reverting: with the plural back,
it fails saying «suggests `virtualmachines`, and `virtualmachines` does not exist in the
tree».

### What is missing for the cut

Eleven Kinds still to route (`Image`, `Container`, `HTTPRoute`, `Gateway`,
`NetworkPolicy`, `NetworkRoute`, `KubernetesCluster`, and the four that are
self-explanatory), plus `apply`/`plan`/`wait`/`diff` promoted from `stack`, plus the
`manifest` group. Only after that does removing `network ls` stop taking away
capability.
