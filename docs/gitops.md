# GitOps with Delonix

The manifest is the source of truth; the repository is where it lives;
`delonix` is what applies it. No Terraform or Ansible needed on top — this page
is the whole flow.

> Applies to the Kinds that **converge**, which are eleven: `Network`,
> `NetworkRoute`, `Volume`, `Image`, `VirtualMachine`, `Container`, `Pod`,
> `NetworkPolicy`, `HTTPRoute`, `Ingress` and `Gateway`. Only `Secret` remains
> «ensure present» (created if missing, never updated), because the state is
> encrypted values and a plan does not decrypt them to compare — and `plan`
> marks it with `!` instead of hiding it, with the obstacle named. The
> authoritative list is not this paragraph: it is `delonix stack plan --fields`,
> which prints it from the code itself. See also
> [`cli-stability.md`](cli-stability.md).

## The five commands

```bash
delonix stack plan       # what would change — changes nothing (`--prune`: what apply --prune would do)
delonix stack apply      # converges
delonix stack apply --prune   # converges and removes what left the manifest
delonix stack wait       # blocks until it is really up
delonix stack destroy    # removes everything this stack owns
```

`plan` compares three things — the manifest, the machine, and the last spec
that **this stack** applied. It is that third side that tells «you removed
this field from the file» (reverts) apart from «someone set this by hand with
`container update`» (leaves alone).

## The symbols

| | Means |
|---|---|
| `+` | does not exist, will be created |
| `+~` | exists, belongs to no stack — will be **adopted** |
| `~` | converges **hot**, without recreating and without changing the PID |
| `-/+` | has to be **destroyed and recreated** — `apply` refuses without `--replace` |
| `-` | belongs to this stack and left the manifest — only leaves with `--prune`. On a `plan` without `--prune` it shows as **kept** and does not count toward `--detailed-exitcode` |
| `=` | already matches |
| `✗` | belongs to **another** stack — never touched |
| `!` | this Kind does not converge in this version |

If you do not understand why a change of yours is not showing up:

```bash
delonix stack plan --fields
```

Says exactly which fields are compared per Kind, and which are not and why
(`env` and `command` come merged with the image's; `user` is stored as the
resolved uid). And, for the Kind that still does not converge, **why not** —
`Secret`, because the state is encrypted values and a plan does not decrypt
them to compare. A named obstacle is a decision; «still does not converge»
would be just silence.

## Who the owner is

Ownership comes from the `delonix.io/stack` label, stamped on each resource by
`apply` itself. The stack name is, in this order: `--name`, the `metadata.name`
of a `kind: Stack`, or the manifest's **directory**.

Consequences that matter:

* A resource created by hand is **never** deleted by a `--prune` or a
  `destroy` — it does not have the label. It shows up in the plan as `+~` and
  is adopted on the first apply, which is what makes an `import` command
  unnecessary.
* **Not everything is ownable, and that is deliberate.** An `Image` is shared,
  content-addressed cache — the same `alpine:latest` serves every stack on the
  host, so stamping it for one and removing it when that one stopped declaring
  it would pull it out from under the others. A `NetworkPolicy`, an
  `HTTPRoute` and a `Gateway` have no record of their own to stamp. None of
  the four is adopted or pruned; all of them converge.
* Two stacks declaring the same name get a **conflict** (`✗`), not a race.
  `apply` refuses before touching anything.

## The PR: `plan` as review

```yaml
# .github/workflows/delonix.yml
name: delonix
on:
  pull_request:
  push: { branches: [main] }

jobs:
  plan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: validate the manifest
        run: delonix stack validate
      - name: plan
        run: delonix stack plan | tee plan.txt
      - name: comment the plan on the PR
        if: github.event_name == 'pull_request'
        run: gh pr comment ${{ github.event.number }} -F plan.txt
        env: { GH_TOKEN: '${{ github.token }}' }
```

`stack validate` runs first on purpose: it resolves the cross-references
(`Container.network`, `.volumes`, `Ingress`/`Egress` targets) against what the
manifest declares **plus** what already exists on the machine. Since `apply`
is fail-fast with no rollback, a broken reference has to stop everything
before the first creation, not halfway through.

## The merge: `apply`

```yaml
  apply:
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: delonix stack apply --prune
      - run: delonix stack wait --timeout 180
```

`wait` is not decoration: `apply` returns as soon as it **created** things,
which is not the same as the stack working. Without it, every pipeline
invents its own `sleep`. It blocks until every declared resource exists and —
where that means something — is running and healthy; it fails naming exactly
what did not come up. A missing prerequisite (the plan's `!`) is **warned
about, not waited on**: a volume that does not mount under rootless does not
start mounting after ninety seconds, and blocking on that would turn an
honest warning into a hang.

A recreation **does not pass on its own**. If the plan carries a `-/+`, the
job fails naming the field that forces it, and someone has to decide:

```bash
delonix stack apply --replace Container/api
```

This is deliberate: recreating means downtime, and on a volume it means losing
the data. A careless `apply` should not be able to do it.

And the value is **verified**, in two steps: the shape (`<Kind>/<name>`, a bare
name, or `all`) and then whether it even names something from this manifest. A
`--replace Container/wev` with a typo is **refused for naming the value**,
instead of being read as authorized and letting the next refusal talk about
the resource — never about the mistake that caused it. On a flag that
authorizes a destructive operation, accepting a value that cannot match
anything is giving the illusion of having authorized it.

## The drift gate

Drift is `plan` saying something with the manifest unchanged. The exit code
serves it directly — the same contract as
`terraform plan -detailed-exitcode`:

| exit | means |
|---|---|
| `0` | nothing to do |
| `2` | there are changes (= drift, if the file did not change) |
| `5` | the plan has an **ownership conflict** (`DX-5601`) — `stack apply` would refuse it whole |
| `1` | the command failed |

**The gate plans with the flags of the `apply` that accompanies it.** `plan`
answers for what that `apply` would do, and an `apply` without `--prune`
removes nothing. So, without `--prune`, a resource that left the manifest
shows up as **kept** and does not count: the gate stays green, because `apply`
also leaves it alone. A pipeline that applies with `--prune`, like the one
above, plans with `stack plan --prune --detailed-exitcode`, and there the
removal counts as a change (`2`). Until now `plan` had no `--prune`, it always
counted the removal, and a gate without pruning stayed at `2` forever.

Since v0.49.0 a failure can come CLASSIFIED instead of `1` — `4` if the
manifest names a resource that does not exist, for example (the full table is
in [cli-stability.md](cli-stability.md)). The two positive signals do not
change: `0` is «nothing to do», `2` is «there are changes», and no error path
returns `2`. A gate written as «`0` passes, `2` fails for drift, **everything
else** fails for error» remains correct; one written with `elif [ $? -eq 1 ]`
for the error now needs an `else`.

`5` has existed since a conflict stopped passing as clean: a resource another
stack has already stamped is not a change (its `changed` is `false`), and
until now the plan answered `0` — the gate stayed green and the following
`apply` refused with `5`. The conflict beats the changes: with a conflict in
the plan, the answer is `5` even if there are also resources changing, because
`apply` would not apply any of them.

```yaml
  drift:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: delonix stack plan --prune --detailed-exitcode
```

```yaml
on:
  schedule: [{ cron: '0 7 * * *' }]
```

To feed another tool instead of failing a job:

```bash
delonix stack plan -o json | jq -r '.[] | select(.changed) | "\(.kind)/\(.name) \(.action)"'
```

The JSON field names are stable and **do not change with `--l18n`**
(ADR-0005) — it is the table that is translated, never the keys.

## When an `apply` dies halfway

It happens, and it is a normal state: `apply` is **fail-fast with no
rollback**. There is no transaction; what was already created stays created.

Recovery is not guesswork:

```bash
delonix stack plan      # shows exactly what was left undone
delonix stack apply     # continues from there — it is idempotent
```

`plan` is the diagnostic tool because it compares against the real thing, not
against a state file that could be out of sync. **There is no `.tfstate`**:
ownership and the last applied spec live on the resource itself
(`delonix.io/stack` and `delonix.io/last-applied`), so the failure mode «the
state says one thing and the machine says another» does not exist.

If the run died after creating but before stamping, the resource shows up as
`+~` (to be adopted) and the next apply resolves it.

## Writing the manifest without memorizing anything

```yaml
# yaml-language-server: $schema=https://ngolacloud.github.io/delonix/schema/v1/delonix.json
```

One line, and the editor starts giving completion, type checking and the
documentation of every field as you write. The schema is generated from the
code itself (ADR-0007), so it cannot drift from it — it catches a typo in a
field name, a `kind` the engine does not know, and an `apiVersion` that is not
that Kind's group.

In a PR, this is worth more than it looks: a manifest the editor underlines
never reaches `plan`, and the error shows up to whoever wrote it instead of
showing up in someone else's CI.

On the terminal:

```bash
delonix explain Container
delonix explain Container.ports
delonix explain Pod.containers.image
delonix manifest render         # the manifest with ALL defaults filled in, without reading the machine
delonix stack apply --dry-run   # the same, and exits the way apply would exit (5 on an ownership conflict)
```

## Validated live, with a real container

The whole cycle, in an isolated `DELONIX_ROOT` — and the proof that matters is
not the command returning 0:

| Step | Result |
|---|---|
| `apply` of a new manifest | container created, PID `618350` |
| `plan` again | «no changes» |
| change `memory: 64M` → `128M` | plan says `~ update`, with both values |
| `apply` | **unchanged PID (618350)** and the cgroup's REAL `memory.max` becomes `134217728` |
| change the `image` | plan says `-/+`, naming `image` |
| `apply` without `--replace` | **REFUSES**, and the PID stays `618350` — nothing was touched |
| `container update` from outside | the next `plan` catches the drift (`256M → 128M`) |
| `stack wait` | returns immediately |
| `stack destroy` | container removed, `ps -a` empty |

The unchanged PID is the point: it is what distinguishes hot convergence from
a disguised restart, and it is the fundamental difference between this engine
and recreating the container the way Docker would.

## What this flow does not do

* **It does not reconcile continuously by default.** It converges when you
  call it. The pull half has existed since 2026-10-12 and is opt-in: a
  `kind: GitOpsSource` ([ADR-0021](adr/0021-gitops-pull-reconciler.md),
  `examples/gitops/gitopssource.yaml`) names a repository, a ref and a path,
  and with `sync.automated: true` a systemd timer runs ONE pass and exits —
  the engine remains daemonless. `delonix gitops reconcile` runs one pass by
  hand, `delonix gitops status` tells the commit each source last saw and
  what is applied. A commit the guard or the security policy refuses leaves
  the running stack intact.

  **They are two stacks, and never the same one.** The stack that applies the
  source by hand (the bootstrap) declares only the source; the stack the
  passes apply the repository into is `spec.stack`, by default
  `gitops.<namespace>.<name>`. If they were the same, a bootstrap `stack plan`
  would give rc=2 forever and a `stack apply --prune` would delete what the
  pass applied (and the next pass would recreate it empty). That is why
  `stack apply`, `plan`, `prune`, `rollback` and `validate` refuse (DX-1601) a
  manifest declaring a source whose passes land on the stack the command is
  acting on — give the bootstrap another `--name`, or the source another
  `spec.stack`:

  ```bash
  delonix stack apply -f source.yaml --name bootstrap   # registers the source
  delonix gitops reconcile production --namespace production
  delonix stack plan -f source.yaml --name bootstrap --detailed-exitcode   # 0
  ```

  If the repository declares itself, the first pass takes over the source's
  record from the bootstrap stack («taken over from stack 'bootstrap'»), and
  from then on it is the passes' stack that declares everything; the
  bootstrap stops owning anything.
* **It has no transactional rollback** (see above).
* **It does not manage a fleet.** It is a node runtime: one manifest, one
  machine. Several nodes are several runs.
