# Per-leaf case plan

> **What this document is.** Which cases every leaf should have, and what
> counts as proof of each one. It is the contract `scripts/folhas_gate.py`
> enforces over `folhas.tsv`.
>
> **Why it is not 538 hand-written blocks.** Because that would be a lie in
> two directions: the `container ls` block and the `vm image push` block
> would be identical and no one would read them, and the 538 copies would
> age without anyone noticing — exactly what happened to the «RUNS 91 —
> 37%» that lived for a month in an `e2e.sh` comment. Hence: **applicability
> rules per surface**, which the gate can verify, and **dedicated blocks
> only where the case is genuinely its own**.

## The twelve cases, and what counts as proof

| # | case | what counts as proof — and what does NOT |
|---|---|---|
| 1 | **Happy path** | the effect on real state: kernel, cgroup, netns, nftables, state-root files, the provider's response. **Not** the exit code alone |
| 2 | **Invalid input** | `../`, `/`, unicode, empty, limits; malformed CIDRs and references → a stable `DX-` code, documented exit code, **and no effect** |
| 3 | **Not found / conflict** | nonexistent, duplicate, delete-while-in-use → the CLASS of the refusal (3 stopped / 4 nonexistent / 5 conflict), not just «it failed» |
| 4 | **Idempotency** | a 2nd `apply` with no changes; `rm` of something already deleted with a documented result; a repeated `Idempotency-Key` → the SAME response |
| 5 | **Daemonless concurrency** | two processes on the same resource: one wins, the other fails cleanly; ledger and locks consistent (ADR-0076) |
| 6 | **Interruption** | SIGINT and SIGKILL mid-mutation; the next run recovers or cleans up; **zero** orphan veths, bridges, rules, mounts, cgroups, processes or disks |
| 7 | **Rootless vs root** | the same leaf in both modes, with the expected differences written down |
| 8 | **Output** | `-o json`/`yaml`/table with a schema stable against `docs/schema/v1`; `--help` consistent with `docs/comandos` |
| 9 | **Isolation** | one engine namespace neither sees nor touches another |
| 10 | **Security** | seccomp/AppArmor/userns as declared; scan and SBOM; signature verified when policy requires it |
| 11 | **Provider failure** | timeout, invalid TLS, 4xx/5xx, **redirect never followed**, truncated response → a clear error and **nothing half-created** |
| 12 | **Upgrade** | state written by the previous version read by the current one |

## Applicability by surface

`●` required · `○` required only if the leaf mutates state · `—` not applicable

| surface | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| CLI | ● | ● | ● | ○ | ○ | ○ | ● | ● | ○ | ○ | ○ | ○ |
| Node API (gRPC and REST) | ● | ● | ● | ● | ○ | ○ | — | ● | ● | ○ | ○ | ○ |
| Mgmt API | ● | ● | ● | ○ | ○ | ○ | — | ● | ○ | ● | — | ○ |
| MCP | ● | ● | ● | ○ | — | — | — | ● | ○ | ● | — | — |
| CRI | ● | ● | ● | ○ | ○ | ○ | ● | — | ● | ● | — | ○ |
| Kinds (× verb) | ● | ● | ● | ● | ○ | ○ | ○ | ● | ● | ○ | ○ | ● |
| Providers (op) | ● | ● | ● | ● | ○ | ○ | — | — | — | ○ | ● | — |

Three decisions baked into this table, and the reason for each:

- **Case 7 does not apply to the APIs.** The server inherits the mode of
  whoever started it; testing it per route would mean measuring the same
  process twice. It is measured once per server, not per leaf.
- **Case 11 is required for the providers and not applicable to mgmt/MCP**,
  which talk to nothing remote — they shell out to the CLI itself.
- **Case 4 is required on the Node API** because it is the only surface that
  honors `Idempotency-Key` (`service.rs:595`). On mgmt it is `○` and would
  always be false today: the string does not exist in the crate.

## The three structural pieces of work — before any leaf

These are not leaves; they are the reason whole surfaces sit at `C`. Each one
converts dozens of cells at once, and doing them first is what makes the
rest cheap.

### E1 — the gRPC leg does not exist in a real run

The battery is all `curl --unix-socket`; there is no `grpcurl` or gRPC
client in `scripts/` or `tests/`. **57 leaves** (each mapped RPC) do not
have the gRPC encoding measured. A battery block that speaks gRPC against
the same socket converts them all at once.

### E2 — mgmt's `test_state()` points at `/bin/false`

`delonix-mgmt/src/lib.rs:1470`. So **no** shell-out route has its success
path tested anywhere: 7 routes (`run`, `rm`, `action`, `pull`, `build`,
`networks`, `rate`) only have the refusal branch tested. Swapping in a real
binary in an isolated root converts the seven. Owner decision (2026-10-18):
**do it**.

### E3 — there is no `tools/call` in the repository

The 15 MCP tools have never run through the protocol. A single test client
that does `initialize` + `tools/call` converts **15 leaves** and closes the
gap of having no ratchet over the list (which `folhas_gate.py` now keeps).

## The CLI's leaves — dedicated blocks

**Updated on 2026-10-20.** Of the original 19, **nine** are now class `A`:
the four that were `B` with no host blocker (`net l4guard set/clear`,
`system boot enable/disable`) and the five `socat` unlocked (`net netns
exec/detach/firewall/publish/unpublish`). **There is no longer any `B` on
the CLI.** The **10** left are all `E` — waiting on infrastructure, in the
order set by `lab-requisitos.md`.

> **2026-10-22, after the merge of `origin/main`.** The committed trace
> became the one from `6dc31582` (2026-10-21), earlier than this branch's
> checks, so `net l4guard set/clear` and `system boot enable/disable` sit at
> **B** in `folhas.tsv` until the trace is re-recorded from a battery run —
> the checks remain in `scripts/e2e.sh`, and the gate's class is the
> trace's, never the table's. `vm pause`/`unpause` are at **A** by that same
> trace.
> On the same day the trace was re-recorded from the battery run over the
> merge (`fa34383b`, `PASS=1577 FAIL=0 SKIP=14`, 272 of 280), and all four
> went back to **A**.

Mind what `A` means here, because it is easy to read the promotion as
finished work: the 2026-10-20 run (`2663c846`, `PASS=1340 FAIL=0`) proved
**case 1** of each one — the effect — not the full matrix. Cases 2/3/4/6/8/9
below remain outstanding, which is why these blocks stay.

### `net netns exec` · `net netns detach` — **A** (case 1 proven on 2026-10-20)

`net netns attach` is **already run**, and so are `up`/`down`/`gc`/`status`.
Cases: 1 (the command runs inside the right netns — compare `ip netns
identify` or the `/proc/self/ns/net` inode with the container's; `detach`
returns the veth and address to the pool), 2 (name with `../`, nonexistent
netns), 3 (`detach` twice → class 4), 6 (SIGKILL mid-`attach` leaves no
orphan veth — `net-lifecycle-leaks.sh` already has the instrument), 8
(`-o json`).

### `net netns firewall` · `net netns publish` · `net netns unpublish` — **A** (case 1 proven on 2026-10-20)

Cases: 1 (the rule **shows up** in `nft list ruleset` and the DNAT actually
forwards; `unpublish` removes the rule **and** slirp's hostfwd), 2 (invalid
spec: `abc:80`, port 0, port 70000, unknown protocol → a stable `DX-`), 3
(`unpublish` of an unpublished port), 4 (publishing the same port twice), 6
(SIGKILL between `add_hostfwd` and the DNAT leaves nothing half-published),
9 (one namespace's rule does not affect another).

### `net l4guard set` · `net l4guard clear` — **A** (case 1 proven on 2026-10-20)

`net l4guard status` already runs. Cases: 1 (the guard **exists** in the
ruleset with both counters, and `status` reads them back), 2 (`set 0 0`,
negatives, non-numeric), 4 (`set` twice = update, not duplicate; `clear`
twice), 6 (SIGKILL during `set` leaves no half-guard).

### `system boot enable` · `system boot disable` — **A** (case 1 proven on 2026-10-20), with a caution

**The caution is the case itself:** they write user units and enable
linger, outside `DELONIX_ROOT`. The run has to redirect `XDG_CONFIG_HOME` to
the isolated root and prove that **nothing** showed up in
`~/.config/systemd/user` — otherwise it violates «every run leaves the host
as it found it».
Cases: 1 (the unit exists, with the container policy's `Restart=`), 2
(invalid `--restart`), 3 (`disable` with nothing installed), 4 (`enable`
twice), 1-bis (stale units for containers that no longer exist are removed).

### `vm pause` · `vm unpause` — **E** (image); **A** on the `6dc31582` trace (2026-10-21)

Real on all three backends; the trait's default fails closed on purpose.
Cases: 1 (the VM becomes `Paused` **and** the VMM stays alive holding the
memory — distinguishing it from `stop`; `unpause` returns it to `Running`),
3 (`pause` of a stopped VM → a named refusal; `unpause` of one that is not
paused), 7 (both modes), 12.

### `image push` · `image sign` — **E** (registry)

Cases: 1 (the manifest **is** in the registry and the digest matches; the
signature sits alongside and `image verify` accepts it), 2 (malformed
reference, `latest` tag under a policy that forbids it), 3 (push of a
nonexistent image), 4 (push twice = same digest; `sign` without `--force`
does not re-sign), 10 (the private key is **never** printed — only the
public one's path), 11 (registry returning 500, invalid TLS, **redirect not
followed**, truncated response).

### `vm image pull` · `vm image push` · `vm image ls-remote` — **E** (registry)

Cases: 1, 2 (malformed source), 3 (nonexistent tag), 4, 11 (the same five
registry failure modes).

### `vm image build` — **E** (network)

Cases: 1 (the resulting image boots), 2 (invalid `VMfile`), 6 (SIGKILL
mid-way leaves no half-built disk in the store).

### `cluster create` — **E** (`kindest/node` image)

Cases: 1 (the apiserver answers and `kubectl get nodes` returns `Ready`), 2
(negative `--workers`, `--control-planes 2` without an LB → the refusal
`--help` promises), 3 (two `create`s with the same `--name`), 6 (SIGKILL
mid-way leaves no orphan nodes or kubeconfig).

### `cluster kubeadm` — **E** (golden image + VMs)

The same ones, over VMs instead of containers, plus: the cluster's own
network is created **and** removed by `cluster destroy`, and a network named
by the caller is **never** removed.

## The remaining surfaces — what the gate requires, by class

- **`D` (29 on the Node API, 6 on mgmt, 2 on CRI, 23 in Kinds):** until the
  owner's decision lands in code, the required case is **3** on the refusal
  — that it is the right refusal (`501 DX-6001` / `grpc-status 12` /
  `DX-6302`) and not a 404 or a 500. 2026-10-18 decision: implement the 29 of
  the node contract; until then, the refusal is what gets proven.
- **`C` (15 + 19 + 10 + 5 + 8):** the missing case is **1** at the layer
  that is missing — most of it is solved by E1, E2 and E3 above, not leaf by
  leaf.
- **`E` (10 on the CLI, 8 in Kinds):** nothing to do until the
  infrastructure exists. The gate accepts them **with the reason written
  down**, and `lab-requisitos.md` names the missing piece.

## Work order and estimate

| # | work | leaves converted | estimate |
|---|---|---:|---|
| 1 | E3 — `tools/call` client for the MCP | 15 | 1 session |
| 2 | ~~The CLI's `B` leaves (`net netns`, `l4guard`, `system boot`)~~ — **DONE 2026-10-20** (`87992e4f`): 9 leaves, not 7, because the five `net netns` ones were at `E`, not `B` | ~~9~~ | — |
| 3 | E2 — `test_state()` with a real binary | 7 | 1 session |
| 4 | E1 — the gRPC leg in the battery | 57 | 1–2 sessions |
| 5 | Local registry → the 5 push/pull leaves | 5 | 1 session |
| 6 | VM image → `vm pause`/`unpause` | 2 | 1 session |
| 7 | The 7 defects found (see the inventory) | — | 1–2 sessions |
| 8 | **The 29 node-contract RPCs** (owner decision) | 29 | **weeks** |
| 9 | The 40 mgmt operations for real (owner decision) | 32 | 2–3 sessions |
| 10 | Kinds: the 9 with no `apply` in a real run | ~45 cells | 2–3 sessions |

Step 2 is already in the total: **341/538 (63.4%)** as of 2026-10-20
(**349/550** as of 2026-10-22, after the merge of `origin/main` — see the
note above about the four leaves that went back to `B`). Steps 1 through 7
that remain take it to about **427/538 (79%)** with no new line of
functionality. Steps 8 to 10 are projects, not sessions, and 8 is the
biggest of all: four entire services.

**An honesty note about 8 and 9.** They were decided on 2026-10-18 in their
maximal form («implement the 29», «the 40 for real»). Neither fits the
45-minute-per-leaf time-box the plan uses for the rest, and neither fits in
this session. They go in as their **own contract**, not a silent extension
of this one.
