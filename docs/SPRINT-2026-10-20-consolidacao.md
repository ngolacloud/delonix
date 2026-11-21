# Consolidation sprint — delonix-runtime — 2026-10-20

Brings together, into a single front, the five sessions of the
`delonix-runtime` group that were open in parallel. Written to be picked up
**on another machine**: everything that matters is versioned, because
`.claude/` is `gitignored` and per-worktree.

- **Measured HEAD**: `main` at `84c88601`
- **Safety tag of the prior state**: `consolidacao-pre-20261020-1857` → `8565b5f5`
- **Branch for this plan**: `docs/sprint-consolidacao-2026-10-20`

---

## 0. How this was measured

Nothing here is assumed. The sources, in order of authority:

| fact | command that gave it |
|---|---|
| live sessions and their PRs | the session manager's `list_sessions` |
| PR and gate state | `gh pr list --json statusCheckRollup` |
| local-only work | `git rev-list --count <upstream>..<branch>` per branch |
| `develop` vs `main` | `git rev-list --count origin/main..origin/develop` → **0** |
| issues | `gh issue list --state open` → **15** |
| unopened fronts | the 5 drafts in `.claude/sessions/pendentes/` |

> **`main` squash-merges.** A branch's commits disappear from `main`'s
> history and the content stays. To know if something is already there, use
> `git cherry main <branch>`, **never** `git log`.

---

## 1. Snapshot — the group's five sessions

| session | branch | real state |
|---|---|---|
| `unruffled-proskuriakova-b581a6` | `claude/unruffled-proskuriakova-b581a6` | PR **#891** open, **1 red gate**, and **1 commit to push** |
| `nervous-morse-38a32c` | `net/unconditional-guards-are-reconciled` | PR **#893** open and **green**. PR #892 of that session was already merged |
| coding session (primary) | `docs/ingress-phase4-describes-what-exists` | PR **#890** open and **green** |
| coding session (primary) | `net/ra-only-when-ipv6-opted-in` | contract **1/3**, missing the operator measurement. No PR |
| «Communication flowchart» | — | read-only, with no artifact landed yet |

Outside the group but in the same repo: the `delonix-folhas` worktree on
`test/folhas-reais` → PR **#889**, green.

---

## 2. In flight — the 4 open PRs

| PR | branch | gates | what's missing |
|---|---|---|---|
| **#893** | `net/unconditional-guards-are-reconciled` | ✅ 19 pass, 2 declared skip | just the merge |
| **#890** | `docs/ingress-phase4-describes-what-exists` | ✅ green | just the merge |
| **#889** | `test/folhas-reais` | ✅ green (includes `leaf inventory ratchet`) | just the merge |
| **#891** | `claude/unruffled-proskuriakova-b581a6` | ❌ `generated docs and valid examples` | regenerate `docs/` **and** push the local commit |

The exact error from #891, from run `39399365138`'s log:

```
::error::docs/ está desactualizado — corre 'python3 docs/gen.py' e commita.
```

The gate is `git diff --quiet -- docs/` **after** running the generator. Two
known traps before touching it:

1. `docs/gen.py` embeds the binary's **real** `--help`. Run
   `cargo build --release -p delonix-runtime-bin` first, or an old binary
   will invent a diff that is not yours.
2. Touching the EN docs re-renders all **four** handbooks (`en`, `pt-AO`,
   `fr-FR`, `zh-CN`), and the HTML is checked byte-for-byte against
   `markdown==3.10.3`. This machine has `3.10.2` — install the pinned one in
   a separate target and point `PYTHONPATH` at it, or you will generate a
   diff the CI refuses.

> ⚠️ **PR #841** (`consolida/develop-2026-10-14`, 166 files, +13,888/−688)
> is based on `develop`. Resurrecting `develop` from `main` **moves its base
> 55 commits forward**, and `mergeable` can flip to `CONFLICTING`. None of its
> 166 files is in `main` — **do not close it**. Rescuing it means per-line
> rebases, not a merge.

---

## 3. To do — the inventory

### 3.1 Open issues (15)

Grouped by cause, not by number — three of them share the same already
proven root.

| group | issues | note |
|---|---|---|
| **k8s does not start** | #736, #799, #825 | cause **proven**: see 3.3 |
| **Docker API parity** | #826, #827, #881, #776 | unserved routes and an empty `HostConfig` |
| **CLI lies with rc 0** | #804, #809 | same shape as the already-fixed #805-#808; they hit the `library_prints` ratchet |
| **state defects** | #812, #815, #823, #824 | orphan cgroup, partial build-cache, missing secret, duplicate name |
| **platform** | #438 | arm64 — the CI already runs `test (arm64)` |

### 3.2 Unmeasured criteria (contract `net/ra-only-when-ipv6-opted-in`)

The code criterion is **met and observed** at `8565b5f5`: the
`the_ra_emitter_only_runs_when_ipv6_is_opted_in` test fails without the
change (rc=101) and passes with it; `clippy`, `arch_fitness`
(`library_prints` stable at 82), `cargo test` 3424/0 and the rest of `make
ci`'s gates returned rc=0.

Two operator measurements are still missing, and they are not mine — they
touch the engine:

```bash
cargo build --release -p delonix-runtime-bin
bash .claude/sessions/medir-ra-net-ra-only-when-ipv6-opted-in.sh
```

Arm A: without `DELONIX_ENABLE_IPV6`, **0** RAs on `delonix0`. Arm B: with
the variable, **≥3**. The script isolates `DELONIX_ROOT`/
`DELONIX_NET_RUNTIME_DIR`, keeps the path short (the socket's `SUN_LEN` is
108 bytes) and enters the infra netns via `nsenter`. This branch **still has
no PR**.

### 3.3 Drafts ready to become a session (`.claude/sessions/pendentes/`)

| draft | state | value |
|---|---|---|
| `bind-source-is-created-on-demand` | cause proven, **design undecided** | 🔥 the highest |
| `golden-preseeds-cni-agent-images` | **hypothesis REFUTED**, reduced | do not run as-is |
| `dedup-measured-reports-credential-value` | 2 confirmed duplications | cheap debt |
| `decisoes-duplicacao-menor` | **a decision**, not execution | do not open without deciding |
| `handbook-zh-cn-inalcancavel` | 25 orphan pages | cheap debt |

**The finding that ties everything together.** `calico-node` does not fail
for lack of image — the live cluster's `describe` for `probe` has no
`Pulling`, `Pulled` or `ImagePullBackOff`. It has this:

```
Error: failed to start container upgrade-ipam:
  delonix: no such bind path /var/lib/cni/networks
```

`VolumeStore::resolve_spec` (`crates/adapters/delonix-volume/src/lib.rs`, the
`src.starts_with('/')` branch) `canonicalize`s a **bind**'s origin and
returns `DX-4000` if it does not exist. The branch right below it, for a
**named volume**, creates it on demand — and has the comment «creates on
demand, like Docker». The engine is inconsistent with itself; Docker creates
it in both cases. A Kubernetes `HostPath` with an empty `HostPathType`
counts on the runtime creating the directory.

Reproduced in isolation, with the real `rc`:

| case | result |
|---|---|
| bind, origin missing | `rc=4`, `DX-4000`, directory **not** created |
| bind, origin present | `rc=0` |
| named volume, equally missing | `rc=0`, **created** |

So pre-seeding images in the golden image does **not** fix the `NotReady`.

---

## 4. The sprint plan

Six blocks, in dependency order. Blocks 0 to 3 empty out what is in flight;
block 4 is the only one that attacks product, and it is the one that pays
off.

### Block 0 — pre-flight: save what is local-only  ⏱ ~10 min

Nothing below can run before this, because it deletes branches.

1. Push the commit ahead on `claude/unruffled-proskuriakova-b581a6`.
2. Push `cons/integration-test-2026-10-20` (no upstream) or declare it
   disposable — see §5.
3. `git push origin main:develop` — fast-forward of 55 commits.

**Done when**: `git rev-list --count <upstream>..<branch>` gives 0 for every
local branch, and `git ls-remote origin develop` matches `origin/main`.

### Block 1 — land what is already green  ⏱ ~20 min

Merge **#893**, **#890** and **#889**, in that order. Between each one,
confirm the next is still `MERGEABLE` — the base moves.

**Done when**: all three `MERGED` and `git cherry main <branch>` shows no `+`
for any of them.

### Block 2 — unblock #891  ⏱ ~30 min

Build the release, run `python3 docs/gen.py`, commit `docs/` and push it
along with the local commit still sitting there. Respect the two traps from
§2.

**Done when**: `generated docs and valid examples` turns green and #891
becomes `MERGEABLE` with no failures.

### Block 3 — close `net/ra`  ⏱ ~15 min, **the operator's**

Run both arms from §3.2. If they give the expected result, tick both
criteria, open a PR for the `net/ra-only-when-ipv6-opted-in` branch (already
on the remote) and close the contract.

**Done when**: arm A reads 0, arm B reads ≥3, and a PR exists.

### Block 4 — the bind creates the origin  ⏱ 1 full session  🔥

The highest-return slice of this whole inventory: a ~10-line inconsistency
that keeps three issues open and Kubernetes at `NotReady`.

The cause is proven; **the design is not decided**. Decide before writing
code:

- always create, like Docker — simple, but a `src` with a typo starts
  creating garbage instead of refusing;
- create only when the caller asks for it (a flag in the spec) — safe, but
  Kubernetes does not pass it, so it does not fix the `NotReady`;
- create when the origin is under a prefix the runtime knows.

**Done when**: a `HostPath` with an empty `HostPathType` boots; there is a
test that fails without the change; and `probe` cluster's `calico-node`
leaves `NotReady` — measured on the cluster, not in the test.

### Block 5 — cheap debt, if there's time left  ⏱ ~40 min

`measured_reports()` duplicated byte-for-byte between
`bins/delonix-runtime-bin/src/cmd/provider.rs:119` and
`crates/interfaces/delonix-node-api/src/providers.rs:18` — 34 lines with no
compiler protecting against drift. And the handbook landing page, which has
the languages *hardcoded* to three and leaves 25 `zh-CN` pages unreachable
(`scripts/dev_docs_site.py:812-819`); generate it from `LANGS`.

**Do not** open `decisoes-duplicacao-menor` as execution — it is a decision.

---

## 5. What only exists on this machine

**Re-measured on 2026-10-20 18:3x against the remote, after a `git fetch
--prune --tags`: nothing.** The first version of this section listed four
risks; they were readings of **stale** tracking refs, not of the remote.
What is actually there:

| artifact | real state |
|---|---|
| `cons/integration-test-2026-10-20` | on the remote, **identical** sha (`0a3e9224`) |
| `claude/unruffled-proskuriakova-b581a6` | on the remote at `62738979`, = head of PR #891 |
| `829f17fe` (orphan commit) | at `salvado/829f17fe-testes-cli`, tag **already on the remote** |
| `docs/sprint-consolidacao-2026-10-20` | on the remote, and `develop` points to it |
| contracts in `.claude/sessions/` | `gitignored` — which is why this plan exists |

> **The lesson, which matters more than the table:** `git rev-list
> <upstream>..<branch>` measures against the **local** tracking ref, which a
> partial `fetch` leaves stale. A `fetch origin main` does not update the
> others. Before declaring anything «local-only» — and especially before
> deleting anything because of it — run `git fetch origin --prune --tags`
> and confirm with `git ls-remote`.

Nothing was lost, but it came close: **archiving the sessions deleted the
worktrees' branches**, not just the directories. If the stale measurement
had been right, `claude/unruffled-proskuriakova-b581a6`'s commit would have
gone with them.

### What `main` still does not have

Everything is committed and on the remote; what's missing is the **merge**,
not the commit:

| branch | commits to land | how |
|---|---|---|
| `net/unconditional-guards-are-reconciled` | 1 | PR #893, green |
| `docs/ingress-phase4-describes-what-exists` | 1 | PR #890, green |
| `test/folhas-reais` | 12 | PR #889, green |
| `claude/unruffled-proskuriakova-b581a6` | 2 | PR #891, **red gate** |
| `net/ra-only-when-ipv6-opted-in` | 1 | **no PR** — block 3 |
| `docs/sprint-consolidacao-2026-10-20` | 1 | this plan |

`cons/integration-test-2026-10-20` shows 15, but they are the same commits
from the PRs above — it is the local merge that brought them together for
testing. It adds nothing of its own.

`develop` is at `a5537fcc`: `main` plus this plan's commit, 0 behind.

**`829f17fe`** — «cli: pin the two message decisions #806 and #807 turn on,
in tests». These are the 8 tests from the `fix/cli-commands-say-what-they-did`
session, whose push was rejected by the classifier. **The four issues
#805-#808 are already CLOSED** and the fix is in `main` via #885. So the
commit is *only* the test net, which never landed. It is worth its own
slice — do not let it die without a decision.

**`cons/integration-test-2026-10-20`** — local merge of the four in-flight
branches, to test them together. The 15 commits `git cherry main` marks
with `+` are the ones from PRs #889/#890/#891/#893; once they land, this
branch adds nothing. It is disposable **after** block 1, not before.

---

## 6. Startup order on another machine

```bash
git clone git@github.com:angolardevops/delonix-runtime.git
cd delonix-runtime
git checkout develop          # or main — they are the same content after block 0
cat docs/SPRINT-2026-10-20-consolidacao.md
```

Then, from `AGENTS.md`, read only the section for the front you are about to
tackle. The guide has 120 sections and is not startup reading.

Before the first commit, confirm the environment:

```bash
make ci                       # what the CI runs
df -h /                       # this machine's disk is at 96%
git worktree list             # the ceiling is 3 worktrees
```

> `cargo test` wants an **empty and exclusively its own** `TMPDIR` — the
> `tmp_roots_gate.py` judges whatever is left there. Do not share it with
> the `scripts/test_*.py`: they leave ~7 directories behind, and the
> measurement ends up accusing `cargo test` of a leak that is not its own.
