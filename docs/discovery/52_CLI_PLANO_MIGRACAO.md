# CLI migration plan — from the 243 current leaves to the spec's 103

Measured on 2026-08-27 against the binary of the slice in progress (243 leaves), not against
the spec read from above. The CLI-0 phase inventory is in
[51_CLI_INVENTARIO.md](51_CLI_INVENTARIO.md); this document is the **sequencing**.

## 1. The two numbers, and what separates them

| | |
|---|---|
| leaves today | **243** |
| leaves the spec asks for | **103** |
| already in the new form | **61** |
| the spec asks for and does not exist | **42** |

**The 140-leaf gap is not excess functionality — it is the same capability written
several times over.** Four groups (`net` 41, `image` 32, `container` 29, `vm` 28)
are 130 leaves, more than half the tree, and that is where almost all of the
reduction lies.

**The "leaves today" count is no longer 243.** `scripts/cli_baseline.tsv`
has, as of 2026-09-01, **249** — it rose with new B3/day-2 capability (and MCP),
fell with the B4-B9 cuts below; the two movements did not cancel out. The
"243→103" ratio and the `57%`/`~80%` of section 4 remain un-remeasured — they were
not recalculated here so as not to invent a number without counting leaf by
leaf again.

## 2. What decides the order

Three constraints, and none is a preference:

**A cut is only honest when the destination already does what the origin did.**
Removing `network ls` before `get networks` covers `Network` removes capability
instead of tidying it up. So: **build first, cut later** — always.

**What is declared stable does not get cut outside of a major.** The
`docs/cli-stability.md` promises the top-level shortcuts (`ps`, `run`, `exec`, `logs`,
`rm`, `images`), the container lifecycle verbs, the exit codes and the `inspect`'s
JSON output. It declares NOT stable the `cluster`, `vm`, `pod`, `workload`,
`storage`, `sharevolume` and `net` — and it is that list that says what can move now.

**CI only runs on a PR against `main`.** Stacked PRs have no checks at all,
and `0 checks` reads as `0 failures`. Every block goes to a PR against `main`.

## 3. The blocks, in order

### B1 — Generic verbs, the rest of the Kinds  ·  ADDITIVE  ·  CLOSED

Bringing `get`/`describe`/`delete` from 9 to the 12 Kinds that have their own state.
**Re-measured before touching code (2026-08-31)**: the original text was
outdated in two places — `describe` of `Secret` and `Image` were already wired
(`SecretCmd::Inspect`/`ImageCmd::Describe`). The real gaps were three:
`NetworkRoute` (describe+delete), `NetworkPolicy` (get/describe/delete, the
design still to decide) and `describe` for `KubernetesCluster` (there was NO
per-cluster describe function at all).

**`NetworkPolicy` resolved**: `get networkpolicies` now combines both
directions per container into a single table (`firewall::list_all_policies`) —
`net ingress ls`/`net egress ls` remain untouched, one direction at a time.
With no registry of its own (`FirewallPolicy` does not persist document
identity — the same reason that led `NetworkAccessRule` to need
`origin`), the identity addressable by `describe`/`delete networkpolicies` is
`<target>/<direction>` (e.g. `web/ingress`), not a document name.
`NetworkRoute` uses the same already-existing logic, `<from>-><to>`
(`netroute::route_name`, the identity `stack plan` already printed).

**Closed CLI-2's exit criterion. Nothing broke** — no new native subcommand
in any of the three Kinds, just parametrization of the already-existing
generic verbs.

### B2 — Renames that do not touch the contract  ·  MINOR BREAK  ·  CLOSED

**Re-measured before touching code (2026-08-31, `Explore` agent +
direct reading of `main.rs`/`scripts/cli_baseline.tsv`): the 9 renames were
ALL already done** — earlier sessions had already carried them out (two
comments in the code cite "B2 of the CLI restructuring" explicitly,
`system.rs:258-263` and `system.rs:271-277`) and the document was never
updated to reflect it.

| from | to | status |
|---|---|---|
| `dash` | `dashboard` (§22) | done — `main.rs:339` |
| `syntax <editor>` | `completion editor <editor>` (§21) | done — `main.rs:59-73` |
| `completion <shell>` | `completion shell <shell>` | done, same arm |
| `namespace` | `system namespace` | done — `system.rs:265` |
| `net boot` | `system boot` | done — `system.rs:271`, cites B2 in the comment |
| `volumes` | `volume` | done — `main.rs:144` |
| `image ls` / `image rm` | `image list` / `image remove` | **reverted** in v2.0.0 (`docs/releases/v2.0.0.md`) — `image ls`/`image remove` are the canonical form |
| `schema print` | `manifest schema` | done — `schema.rs`'s `SchemaCmd` is no longer wired to any top-level command |
| `restore` (root) | `backup restore` | done — `rbackup.rs:806`, documented in AGENTS.md |

Fixed path in two `AGENTS.md` sections that still used the old spellings as a
title (`## delonix net boot` and `` `delonix volumes` `` in the command
list) — history preserved in the body text, only the title/label fixed to
the current spelling.

**No code touched in this block — it was only a matter of updating the record.**

### B3 — New capability: the groups that do not exist  ·  ADDITIVE  ·  IN PROGRESS

| group | leaves | nature | status |
|---|---|---|---|
| `backup` | 6 | consolidates the root `backup`/`restore` + `list`/`inspect`/`schedule`/`remove` | already done before this plan (see AGENTS.md) |
| `diff` | 1 | the three faces (desired/last-applied/observed) of ONE named resource | **CLOSED** — PR #200 |
| `system metrics` | 1 | raw `DashSummary`, `-o json`/table | **CLOSED** — PR #201 |
| `cluster` day-2 | 5 | `kubeconfig`/`health`/`upgrade`/`drain`/`uncordon` | `kubeconfig` already existed; **`health` CLOSED** — PR #202; `upgrade`/`drain`/`uncordon` still to build — **the ADR-0010 citation here was a MISATTRIBUTION** (measured 2026-09-03): that ADR refuses a REMOTE management API (`delonix-mgmt` reachable from outside the host); `drain`/`uncordon` do not even need SSH (`cluster kubeconfig` already caches it locally — they are plain `kubectl --kubeconfig=<cache> drain/uncordon`) and `upgrade` reuses the SAME `cmd/remote.rs::SshTarget` that `kubeadm_init`/`join` already use. Nothing blocked; still to build |
| `config` | 5 | contexts — **needs an ADR**, see §5 | **`output` CLOSED** (local-only, without reopening the ADR) — PR #203; `namespace` stays out on purpose, with no single read point |
| `system state` | — | — | **not built** — already answered by `system info` (see slice 1's plan) |
| `pod` day-2 | 4 | `exec`/`attach`/`cp`/`port-forward` | `exec`/`attach`/`cp` already existed before this plan; **`port-forward` DONE** — PR #207 (to be merged) |
| `vm` day-2 | 3 | `pause`/`resume`/`migrate` | **`pause`/`unpause` DONE** — PR #206 (to be merged); **`migrate` DONE** — stop-copy-start MVP (2026-09-03; see note below), PR to be merged |
| `network` | 2 | `diagnose`/`capture` (`flow` exists as `net flow`) | `diagnose` already existed; **`capture` DONE** — PR #208 (to be merged) |
| `image sign` · `secret rotate` | 2 | | **`secret rotate` was already DONE** (PR #205, 2026-08-xx) — citing it here as "to do" was outdated, the same misattribution pattern as B2/B4-B9; **`image sign` DONE** — PR #209 (to be merged) |

**B3 is THUS 100% covered by already-written code** (2026-09-03) — what remains
is not implementation, it is manual review: `pod port-forward`/`vm pause`/`net
capture`/`image sign`/`cluster upgrade+drain+uncordon`/`vm migrate` all touch
sensitive infrastructure (private key, packet capture, remote execution,
VM suspension) and are flagged for review before merge, the same pattern
already used in this session.

**Slice 1 closed (2026-08-31, PRs #200/#201/#202/#203)**: `diff`, `system
metrics`, `cluster health` and `config` (only `output`) — the scope decision for
each one is in that slice's execution plan, not repeated here. `config`
was born **local-only**, without reopening ADR-0010. `cluster drain`/`uncordon`
do NOT stay for the same reason — see the correction on the `cluster` day-2
line above: the ADR-0010 citation there was also a misattribution, measured and
fixed on 2026-09-03. `diff` reuses the 3-way diff engine `stack plan`
already had (`cmd/reconcile.rs::diff_fields`) — zero new engine.

**`vm migrate` — what was measured before designing (2026-09-03).** A
spike confirmed: Cloud Hypervisor has NO disk migration mechanism at all
(only memory/state), and even the libvirt/QEMU NBD path (which exists and is
mature) would require `VmBackend::migrate` + network reachability between
hosts + convergence management this engine does not have — it remains its own
ADR, out of scope. What IS feasible and got built: an MVP
**stop-copy-start** (real downtime) — `vm stop` → flattens the overlay into a
standalone qcow2 → `scp` to the target → `image vm import --appliance`
(registers it without assigning it a new cloud-init, the disk already carries
its own state) → `vm create --disk <imported-image>` on the target. **Real
bug caught while designing, not while validating live**: `remote::ssh_run`
ALWAYS wraps the command in `sudo -n bash -c` — correct for `cluster apply`
(installs system packages), but running the `delonix` CLI ITSELF this way
would run it as ROOT on the target, against the WRONG state root
(`Store::default_root` branches on `geteuid()`) — invisible to a normal
`vm ls` on that host. Fixed with `remote::ssh_run_as_user` (no `sudo`), new,
alongside the existing `ssh_run`.

**Nothing left to do in this slice — only to be merged.** The previous text
here listed the four big pieces as pending design/implementation; all have
code written and tested today (see the table and the two notes above), waiting
only for manual review before merge because they touch sensitive infrastructure.

### B4–B9 — re-measured on 2026-09-01 (`Explore` agent, fresh `origin/main`)

**This section described B4–B9 as blocks yet to start, and it was wrong.**
All six had already been measured and partially executed — v0.68.0/v0.69.0
(29-30/08) cut B4/B5/B6/B7, and v1.0.0 (29/08) executed B8 and **rejected**
B9 — all of this BEFORE this round's B1/B3 had even begun (31/08 onward).
Anyone reading this section at a glance would re-derive conclusions already
settled, or could reopen an already-made decision (B9). Numbers confirmed by
direct reading of the code and commits, not from `docs/releases/*` alone.

### B4 — The `net` collapse  ·  BREAK  ·  promised −41, delivered **−4**

The two blockers this section cited (`get networkpolicies`, `network
diagnose`) are closed and confirmed real — but they were not the real
blocker. **The real blocker, measured in v0.68.0**: `net ingress`/`net egress`
are **incremental** (`allow`/`deny` add a rule, "last command wins"), while
the only declarative Kind that existed — `FirewallPolicy` — **replaces the
entire state of one direction** on every apply, and refuses two documents for
the same (target, direction). Two incremental calls do not turn into two
`FirewallPolicy` applications without the second erasing the first.

Only **4 of the 41** were genuinely duplicated: `net tunnel ls/describe/rm` →
`get/describe/delete gateways`, `net httproute ls` → `get httproutes`
(`cmd/tunnel.rs`/`cmd/httproute.rs`, the `Ls`/`Describe`/`Rm` left the enums
with a comment citing B4). `kind: NetworkAccessRule` (ADR-0028,
31/08) was born precisely to reopen this block — an INCREMENTAL,
per-document rule, with `origin` so it can be removed without erasing other
rules — but collapsing `net ingress`/`net egress` onto it **was not done**;
ADR-0028 itself says so: "whether it is worth trading 17 imperative leaves
for manifests needs its own measurement, not an assumption."

**Missing from the plan's own number**: `l4guard` (3 leaves — `set`/`clear`/
`status`) never entered the count of 41, and ADR-0028 says in so many words
that it is "a different mechanism" — outside the scope of `NetworkAccessRule`.

**The three open decisions — RESOLVED (`docs/adr/0029-net-ingress-egress-collapse.md`)**:
(1) `net ingress/egress allow/deny` will write through the SAME
accounting as `NetworkAccessRule` (`origin` synthesized from the
`(dir, proto, port, src)`) — unifies the two mutation paths without removing
a single CLI leaf nor changing any UX; (2) `publish`/`unpublish` (DNAT)
remain PERMANENTLY outside `NetworkAccessRule` — they are a different grain
(port mapping, not an allow/deny decision) and should never have counted
toward the promised "−41"; (3) `net netns` stays VISIBLE — it is a diagnostic
tool documented as such, not internal plumbing (that is already hidden before
clap). None of the three opens new leaf-cutting work — the only code change
is merging the two mutation paths from point (1).

### B5 — The storage collapse  ·  BREAK  ·  promised −22, delivered **−7**

`storage apply` shipped in v0.68.0 (byte-for-byte identical to `volume::apply`);
`storage dash/inspect/rm` shipped in v1.1.0 ("collapse into `volume` — genuine
duplicates, measured field by field", `docs/releases/v1.1.0.md`; `volume inspect`
gained `device`/`options` so as not to lose what only `storage inspect` had).

**What remains (`storage create`/`ls` — 2 leaves; `sharevolume` apply/ls/
describe/rm/migrate — 5 leaves) was measured and is not duplicate**: `storage
create` has NFS/CIFS/SMB/WebDAV credential onboarding that `volume create`
does not have; `sharevolume` has namespace/quota/parent-storage semantics and
a `migrate` with no equivalent in any generic verb. Collapsing further needs
NEW design (network credentials inside `volume apply -f`, a `--network-only`
filter/`DEVICE` column in `volume ls`, a home for `migrate`), not code
deletion.

### B6 — `image --vm` and `build`  ·  BREAK  ·  promised −11, delivered **−1**

`vm build` (root group) shipped in v0.69.0 — identical to `image --vm build`
(the only apparent difference, `-v/--verbose`, was already a no-op in both:
both read `$DELONIX_VERBOSE` via `Progress::new()`).

**New finding in this re-measurement, still to cut**: `image --vm build` (the
old flag) and `image vm build` (the more recent nested subcommand) are today
**two spellings of the SAME operation** — `ImageCmd::Vm` mirrors `VmSub` 1:1
(`image.rs`). No release cut either of the two; it is the cheap duplicate that
remains to be measured and closed before this block's expensive part.

`image build --type container|virtual-machine` **does not exist and is NEW
design, not a cut**: the flag sets genuinely diverge (`BuildArgs` is the
Dockerfile/Delonixfile form; the VM variant has ~18 flags of its own —
distro, cloud-init, golden recipe — with no container-side equivalent).
v0.69.0 already measured this and concluded unifying under a `--type` is new
engineering, outside the scope of "just cut duplicates".

### B7 — Pure day-2: `vm`, `cluster`, `pod`  ·  BREAK  ·  **CLOSED** — promised −25, delivered **−8**

`vm rm`/`vm describe`, `pod rm`/`pod describe`, `cluster ls`/`cluster delete`
shipped in v0.69.0 — confirmed identical to the generic `get`/`describe`/
`delete`, body extracted into a `pub(crate)` function the generic verb calls
directly.

**Closed on 2026-09-03 with the discipline that was missing: the 40 real
leaves of the three groups (not the ~37 estimated — the count itself had
never been recounted from the `enum`s), measured ONE BY ONE against what
`get`/`describe`/`delete` already cover**, not by block judgement. Found: **38
are IRREDUCIBLE** (most already suspected, but never confirmed leaf by
leaf) and **2 were cuttable and had slipped through the previous pass**:

- **`vm status` — CUT, no alias.** Duplicated `get vms` (same call,
  `delonix_vm::list` → `status()` per VM) with no name, and `describe vms <name>`
  with the name — zero flag of its own, zero data difference.
- **`pod ls` — CUT, no alias, with a "build before cutting" step first.**
  It only differed from `get pods` by accepting `--namespace`, which the
  generic `get` had for NO Kind. Fixed at the source: `delonix
  get <kind> -n <namespace>` now exists (refused, never silently ignored,
  on Kinds without a namespace — `Secret`, for example), and only afterward
  was `pod ls` removed, consistent with this document's §2 rule ("a cut
  is only honest when the destination already does what the origin did").

**The remaining 38 stay, now with the reason written next to each one** (the
full audit table lives outside this document — the summary by category):
real interactive sessions (`console`/`ssh`/`vnc`/`exec`/
`attach`), side effects a read verb cannot have
(`stop`/`start`/`restart`/`bridge`/`unbridge`/`snapshot create`/`rm`/
`restore`), live network I/O a `get` refuses on purpose
(`vm ls --ports`, `cluster health`), or objects with no Kind at all
(VM images, snapshots, kubeconfig file, `kube generate`,
`vm default-backend`). `cluster kubeadm`/`kube`/`kubeconfig`/`health` stay
for this last reason. `upgrade`/`drain`/`uncordon` are not B7 candidates
— they are new B3 capability (see the correction in the B3 table above: the
ADR-0010 citation was a misattribution).

### B8 — Root shortcuts and `workload`  ·  CONTRACT BREAK  ·  **CLOSED in v1.0.0**

**No longer blocked nor pending — it already happened.** `ps`, `run`, `exec`,
`logs`, `rm`, `images` left the root in v1.0.0 (29/08), a clean cut with no
alias: the old spelling fails with `unrecognized subcommand`, never silently.
`docs/cli-stability.md` no longer lists them as shortcuts — it records the
break itself and the migration table (`ps`→`container ps`, etc.).

**`workload` was evaluated and kept on purpose**, not forgotten: it is the
only path that disambiguates a Container and a VM with the SAME name —
`get`/`describe`/`delete` answer by Kind, `workload` answers by name across
both. Cutting it needs a successor ADR naming the loss; a plan line does not
suffice (`docs/releases/v1.0.0.md`).

### B9 — Colliding exit codes  ·  **REJECTED in v1.0.0, decision closed**

`exitcode.rs` already had, as of this proposal's date, a closed and
exhaustively tested scheme (LSB: `3` not-running, `4` not-found, `5` conflict;
`sysexits.h`: `69`/`74`/`77`/`124`) — built AFTER the B9 text, and the
module itself evaluates and refuses the literal request: remapping `2/4/5`
would require first splitting `Error::Invalid` (643 sites, two distinct
classes merged) rather than a simple remap. The exit codes had already been
in `cli-stability.md`'s *Stable* table since v0.49.0. **Nothing changes
here** — a closed decision, not a pending item.

## 4. What can go out in the next bump

**This section was partly overtaken by execution (2026-08-31)**: `config`
and `diff` — which here appeared as "does NOT fit" — were already closed in
B3's slice 1, with reduced scope (`config` only `output`, local-only, without
reopening ADR-0010). The original text remains below as a record of the
reasoning, not as the current pending list — that is in the B3 table, above.

**B1 + B2 + part of B3.** This is what needs no new decision nor breaks a
published contract:

* generic verbs across the 12 Kinds;
* the nine renames;
* the consolidated `backup` group;
* `system metrics`/`state`, `network diagnose`, `secret rotate`.

It would take conformance from **57% to ~80%**, and is a **minor** (`0.67.0`)
with a migration note for the nine renamed spellings.

**What did NOT fit in this original reading:** `config` (needs an ADR), `diff`
(new output), `pod`/`vm`/`cluster` day-2 (real capability), and all the
collapse blocks — which depend on B1/B3 being complete and, in B8's case, on
a major.

## 5. The three decisions that remain

**`Service` — IMPLEMENTED on 2026-09-05, `docs/adr/0032-service-kind-dns-round-robin.md`.**
Selects a set of containers by `matchLabels` (the same primitive ADR-0024
still designs for `FirewallPolicy`, which remains to be built) and publishes
it as several round-robin DNS `A` records, under the already-existing
internal name — no VIP, no new dataplane, no daemon. `delonix get/describe/
delete services`, converges (`hot_fields`), `stack plan`/`apply`, and
generated schema. Validated live: label selection, real multi-record DNS
(`nslookup` returning both IPs), per-namespace isolation (including the
`default` exception = public), empty selector warning rather than failing,
and hot port updates. Closes the count of "12 operable Kinds".

**`config` and contexts — CLOSED, confirmed in the code.** `config.rs`
says, in its own module comment: a small local preference, never a context —
§16 wants `endpoint`/`identity`/`tls` in a context, and that is exactly what
ADR-0010 already rejected. `namespace` (the plan's other candidate) stays out
on purpose: unlike `output`, there is no single read point for a namespace
default to attach to. `KNOWN_KEYS` has a single key: `"output"`. ADR-0010
remains `Status: Rejected (2026-08-10)` — "the API stays local, and that is
the answer, not a deferral."

**When is the major — already answered by EXECUTION, not by a document.**
v1.0.0 happened on 29/08, BEFORE B1 had even been merged (31/08) — this
section's question ("do B4–B7 go before or join the major?") became moot:
the major already shipped, narrow and decoupled, carrying only the least
controversial part of B8 (root shortcuts) and **rejecting** B9 (exit code
remap). B4–B7 remain `0.x` form, additive-and-measured, as if the decision
had been "ship the major early and narrow, keep collapsing the rest
afterward, gradually". Recorded here as fact, not as an open question.

## 6. What this plan does not promise

There is no time estimate. **B3 is now 100% coded** — what remains
is review, not implementation. `pod port-forward` (PR #207), `net capture`
(PR #208), `vm pause/unpause` (PR #206), `image sign` (PR #209),
`cluster upgrade`/`drain`/`uncordon` and `vm migrate` (PRs to be opened/already
opened as of 2026-09-03) have code awaiting manual review — all touch
sensitive infrastructure (private key, packet capture, remote execution,
VM suspension).

**`vm migrate` — investigated AND built on 2026-09-03.** Measured against
the real `VmBackend` and the upstream Cloud Hypervisor and
libvirt/QEMU documentation: **real live migration is out of reach** — Cloud
Hypervisor has no DISK migration mechanism (only memory/state), and even the
libvirt/QEMU NBD path (which exists and is mature) would require
`VmBackend::migrate` + network reachability between hosts + convergence
management this code does not have — that remains its own ADR, not an
incremental extension. What got built is the **stop-copy-start** MVP
(real downtime, documented as such): `vm stop` → flattens the overlay into a
standalone qcow2 → `scp` to the target → `image vm import --appliance` (the
disk already carries its own state — a new cloud-init pass would be wrong)
→ `vm create --disk <imported-image>` on the target. **Real bug closed at
design time, not only at validation**: `remote::ssh_run` always wraps the
command in `sudo -n bash -c` (correct for `cluster apply`); running it this
way against the target's own `delonix` CLI would operate on the WRONG
state root (`Store::default_root` branches on `geteuid()`) — invisible to a
normal `vm ls` there. `remote::ssh_run_as_user` (no sudo) closes it, new
alongside the existing `ssh_run`.

**`cluster upgrade`/`drain`/`uncordon` — were never actually blocked, and
are built.** The ADR-0010 citation was a misattribution (see the correction
in the B3/B7 section above): `drain`/`uncordon` do not need SSH (kubeconfig
already cached locally); `upgrade` reuses the `SshTarget` that `kubeadm_init`/
`join` already use.
