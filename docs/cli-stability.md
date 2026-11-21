# CLI stability promise

> Applies from **v0.42.3** onward. Written for `0.x` — every break listed
> below had to wait for a major. **v1.0.0 is that major**: it closed the
> only pending break (the top-level shortcuts), and from here on this list
> stands as the real semver promise for everything else — a breaking change
> no longer fits in a `1.x`.

An engine without a contract does not get automated. Whoever writes a `Makefile`, a CI
step or a deploy script needs to know what can break on an upgrade — and the
answer "it's 0.x, anything can break" is true and useless: it guarantees that nobody
depends on anything, which is the same as nobody adopting it.

This used to list what is committed to and what is not within `0.x`,
which is what most projects at that stage are missing. Since v1.0.0 it is
more than that: it is the project's semver contract.

## Stable — does not break without a major

**The container lifecycle verbs**, with the names and the semantics that
Docker and Podman give them:

```
container run  create  ps  stop  start  restart  kill  rm  exec  logs  attach  stats
                wait   inspect   port   rename   pause   unpause
image     pull  ls  remove (rm)  tag  save  load  push   build (delonix build)
root      run  create  ps  start  stop  restart  kill  rm  exec  logs  attach  stats
          pull  images  rmi  tag  save  load  push            (aliases of the above)
          cp  port  login  logout  info  events              (aliases of the above)
          wait                                        (polymorphic — see below)
```

Specifically, what is guaranteed:

* **The command name and the order of positional arguments.**
* **The short and long flags listed above and their meanings** — on
  `run`: `-d`, `-i`, `-t`, `-p`, `-v`, `-e`, `--name`, `--rm`, `--net`
  (alias `--network`), `--pull`, `--platform`, `--umask`, `--cgroupns`, `--cidfile`, `--shm-size`, `--restart`,
  `--memory`, `--memory-reservation`, `--memory-swap`, `--pids-limit`, `--cpus`,
  `--cpu-shares`, `--blkio-weight`, `--oom-score-adj`,
  `--entrypoint`, `-w`, `-u`, `--cpuset` (alias `--cpuset-cpus`), `--ipc`,
  `--pid`, `--uts`, `--stop-signal`, `--stop-timeout`, `--init`,
  `--volumes-from`, `--mount`, `--label`, `--label-file`, `--annotation`,
  `-P` (alias `--publish-all`), `--no-healthcheck`, `--add-host`, `--wait`,
  `--health-*`; on `exec`:
  `-i`, `-t`, `-e`, `-w`, `-u`. (This list
  used to be flattened, without saying which verb each flag belongs to; a test in
  `main.rs` now checks both lists against the real `clap` tree, instead of
  relying on reading it.) **`-i`/`-t` on `run`/`create` entered along the
  line of the CLI parity review**: `run` used to refuse `-it` in the parser and broke
  the first line of any interactive script (`error: unexpected argument
  '-i'`), measured against docker 29.9.2 and podman 4.9.3. They are accepted for parity
  — a foreground `run`, being *daemonless*, already INHERITS the operator's stdio,
  so the interactive shell, `test -t 0` and the exit code already matched
  `docker run -it`; what was missing was the recognition, not the behaviour.
  `-t` also now changes behaviour: a foreground `run -t` synthesizes a pty (the same
  `setup_console` machinery as PID 1's console), connects it to the
  process's stdio (`setsid` + `TIOCSCTTY` + `dup2`) and pumps from the
  master to the operator's stdio in raw mode. So `isatty(1)` is true
  inside the container **even when the operator's stdout is a pipe** —
  `echo x | run -t … sh -c 'test -t 1'` passes, the limit this section
  used to document as deliberate and which no longer exists. (Without `-t` nothing
  changes: inherited stdio, no pty.)
* **The exit codes** — see the "Exit codes" section below.
* **The JSON output of `inspect`** — fields may be ADDED, never removed
  nor have their type changed.
* **The top-level (`root`) verbs are aliases of the group ones** — `delonix run` IS
  `delonix container run`: same flag struct, same handler, same exit
  codes, by construction (there is no second contract to break). They inherit the promise
  above; what is NOT YET promised is full flag and behaviour parity
  with a reference version of Docker/Podman — that measurement is work in
  progress, outside this set.

> **Contract break in v1.0.0.** Until v0.69.0 the top-level shortcuts (`ps`,
> `run`, `exec`, `logs`, `rm`, `images`) were here, as an argv rewrite
> to `container <verb>`/`image list`. They were removed — a clean cut, no alias: the
> old spelling fails with `unrecognized subcommand`, never silently, the
> same rule the v0.30.0 reorganization already followed. The groups remain:
> `delonix container ps`, `delonix container run`, `delonix container exec`,
> `delonix container logs`, `delonix container rm`, `delonix image list`
> (see the v2.0.0 note below — that spelling changed again).
> **Exit codes do not change in this version** — see the section below.

> **Reintroduced in v5.x.** The top-level shortcuts CAME BACK — `delonix run`,
> `create`, `ps`, `start`, `stop`, `restart`, `rm`, `exec`, `logs`, `pull` — for a reason the
> v1.0.0 note did not weigh: the familiarity of someone coming from Docker/Podman is worth
> more than the purity of a CLI without synonyms, and the stated goal is
> to swap the executable and keep the flow. This is NOT the argv rewrite of before (the
> old, fragile spelling, which this same note was right to cut): each
> top-level verb is now a full `clap` subcommand that carries the SAME
> argument struct and dispatches to the SAME handler as the group — there is no second
> code path to diverge, and `--help` is the same module for module. That is
> why they fall under the stability promise above, and not under a separate one: what
> `delonix container run` promises, `delonix run` promises along the same line. The
> set reintroduced in the first pass was that of the most frequent flows.

> **Table completed on the 0.1.0 line** (the renumbering following the historical v5.1.x baseline). The remaining verbs of the
> Docker/Podman table entered through the same mechanism — a full `clap`
> subcommand, SAME argument struct, SAME group handler: the container ones
> `kill`, `attach`, `stats`, and the image ones `images` (alias of `image ls`),
> `rmi` (alias of `image rm`), `tag`, `save`, `load`, `push`.

> **Root completed in the CLI parity review.** Differential measurement against
> docker 29.9.2 and podman 4.9.3 found more root verbs refused by the parser
> whose group form already worked. They entered through the SAME mechanism
> (`clap` subcommand, SAME handler, verbatim docs): `cp` and `port` (alias of
> `container cp`/`container port`), `login` and `logout` (alias of `image
> login`/`logout`), and `info` and `events` (alias of `system info`/`system
> events`). An operator who writes `docker cp`, `docker login`, `docker info`
> or `docker events` only has to swap the executable. `inspect` and `diff` did NOT enter the
> root — see the resolution policy right below.
>
> **Second root wave.** Five more container verbs gained a root form through
> the same mechanism (`clap` alias, SAME handler): `pause`, `unpause`,
> `commit`, `top` and `rename` (alias of `container pause`/`unpause`/`commit`/
> `top`/`rename`). `docker pause`, `docker commit`, `docker top` and `docker
> rename` just swap the executable. `update` did NOT enter the root: `docker update`
> is about resource LIMITS, whereas Delonix's `container update` is about
> live reconfiguration (ports/volumes/bandwidth) — distinct
> semantics, so it stays only in group form. Two more image verbs also entered,
> `history` (alias of `image history`, like `docker history`) and `untag`
> (alias of `image untag`, like `podman untag`).
>
> **Generic command names: a resolution policy, never a guess.** Three names
> in the table collide with meanings this engine already had or that would be
> ambiguous; they are resolved by a written rule, not by guesswork:
>
> * **`wait` is a single verb with two forms, chosen by GRAMMAR.** One or
>   more container positional operands ⇒ the `docker wait` form (blocks
>   until each one exits and prints the exit code — the SAME handler as
>   `container wait`). Zero operands ⇒ the manifest form (`delonix wait -f
>   <file>`, the one this engine already had). The two grammars are disjoint;
>   mixing them — a container operand alongside `-f`/`--timeout` — is
>   REFUSED out loud, never resolved silently. The container form
>   inherits the `container` group's promise; the manifest one is still classified
>   with the rest of the manifest surface (NOT stable).
> * **`inspect` and `diff` are TYPE-SCOPED.** There is no generic `delonix
>   inspect <name>` that decides on its own whether the name is a container or an
>   image: `container inspect` resolves only containers (the JSON of `docker
>   inspect`), `image describe` only images, `container diff` only containers, and
>   `delonix diff`/`delonix wait -f` at the root are of the MANIFEST. It is the same
>   rule that keeps separate operations with different effects (removing a resource vs
>   cleaning up unused ones): a generic resolver that guessed the Kind is
>   exactly what this CLI refuses to do.

> **Contract break in the next major (ADR-0062).** `container run` of an
> image that declares `USER` will run as that user, as in Docker
> and Podman; until v4.5.0 it ran as uid 0 inside the container, regardless of
> what the image declared. `--user 0` (or `user: "0"` in a manifest) is the
> explicit way to stay root. An image without `USER` is unchanged. And a container with a
> non-root user stopped owning its rootfs and re-claiming host bind
> mounts: it writes only where the image gives it ownership, in an empty named
> volume, or wherever the host's permissions allow.

> **Contract break in v2.0.0.** The B2 of the CLI restructuring (v0.67.0)
> had renamed `image ls`/`image rm` to `image list`/`image remove` — the
> only naming exception in a CLI where the other 15 "list"-type leaves use
> `ls` (`network ls`, `volume ls`, `vm ls`, `pod ls`, `secret
> ls`, `stack ls`, …), following the Docker/Podman pattern this same
> document already cites for the container verbs. Reverted: `image list`
> goes back to `image ls` (`image remove` stays). Clean cut, no alias — the
> `list` spelling fails with `unrecognized subcommand`, never silently.

## Exit codes

> **Contract change in v0.49.0.** Until v0.48.0 **every** engine failure
> was `1` — "does not exist" and "crashed" were the same number. It is still true that
> `0` is success and non-zero is failure, so an `if delonix …; then` does not change
> behaviour; what changes is for whoever tests `[ $? -eq 1 ]` **expecting a
> specific failure**. See the migration note at the end of this section.

> **Contract change on the 0.1.0 line.** With `--detailed-exitcode`, a
> `stack plan` whose plan has a `Conflict` — a resource with the `delonix.io/stack` stamp
> of another stack — used to answer `0`, because a conflict is not a
> change. It now answers `5` (`DX-5601`), the same as `stack apply` would
> refuse with, and outranks `2`. A gate written as "`0` passes, `2` is drift, the
> rest is error" stays correct without change; whoever only tested `[ $? -eq 0 ]` now sees
> red exactly where `apply` would have failed. Without the flag, `plan` still
> answers `0`.

> **Contract change on the 0.1.0 line (2).** `stack plan` (and `plan`)
> gained `--prune`, and started planning with `apply`'s flags. Without `--prune`, a
> resource this stack owns that the manifest no longer declares is still listed
> (`-`), but as **kept**: `changed: false` in `-o json` and outside
> `--detailed-exitcode`. It used to count as a change, and a drift gate without
> pruning answered `2` forever, because the accompanying `apply` never removes it.
> With `--prune` it answers as before: the removal is a change (`2`). Whoever made the
> gate return `2` with resources that left the manifest now needs
> `--prune`. `action` is still `delete`, and `planDigest` does not change.

> **Contract change on the 0.1.0 line (3).** `stack apply --dry-run` (and
> `delonix apply --dry-run`, the same path) used to only print the manifest with
> defaults and answer `0` without reading the machine — a plan that `apply` refuses
> used to pass the preview. It still prints the same manifest to stdout,
> but now runs everything `apply` runs before the first write and exits
> as it would:
>
> * `5` (`DX-5601`) with a resource another stack stamped;
> * `5` (`DX-5390`) with a `--plan-digest` that is no longer the plan;
> * `1` (`DX-1000`) with a Replace or a Reboot without `--replace` or `--reboot`,
>   with a typo in those flags, or with a value that does not have the
>   `<Kind>/<name>` shape, a name or `all`;
> * `77` (`DX-7000`) when the state root cannot be read and prepared.
>
> No resource is created nor changed and no revision is recorded. But the root
> is not left untouched: the stores create their directories and the tunnels'
> keyring key, as `stack validate` already did. Whoever only wanted the manifest, without
> the machine, has `delonix manifest render`, which still answers `0`.

| code | meaning |
|---|---|
| `0` | success |
| `1` | failure without its own class (the usual default) |
| `2` | invalid usage (the `clap` one) — and, in `stack plan --detailed-exitcode`, "there are changes" |
| `3` | the resource exists but **is not running** |
| `4` | that resource **does not exist** |
| `5` | **conflict** — the name is already taken; and, in `stack plan --detailed-exitcode` and `stack apply --dry-run`, a plan with a resource another stack stamped (`DX-5601`) |
| `69` | **a capability this host does not have** — a tool not installed, a backend unavailable |
| `74` | **the filesystem said no** — disk full, impossible path |
| `77` | **permission denied** — the fix is a permission, then retry |
| `124` | **the deadline ran out** — `stack wait --timeout`, and whatever else gets a deadline |

`3` and `4` are not made-up numbers: they are the LSB status codes that
`systemctl` still speaks (`3` = the program is not running, `4` = no such
unit). `5` has no convention behind it — it is the next free number, below
the range the shell uses (`126`/`127`, and `128+N` for signals).

**`74` and `77` came out of the `1` bucket, not out of any published class.** Until
v0.66.1 an I/O failure answered `1`, which the line above describes as
"a failure without its own class" — giving it a class is what this table exists to
do, and a script with `*) exit 1` still falls into the same branch. `77` is
carved out of `74` by the error's `kind`, because that is how it arrives: measured
against a state root with no write bit, `volumes create` and `secret create`
both come back as permission denied. It is the most actionable failure there
is — fix the permission and retry — and it used to answer the same number as
a full disk.

**`69` and `124` are not either.** `69` is `sysexits.h`'s `EX_UNAVAILABLE`; `124` is
what `timeout(1)` returns when the deadline passes, and is therefore already at the
fingertips of whoever wraps a command in one. They entered because they had **real
producers misclassified**, not to fill out a table: a `stack wait` that ran out of
time answered `1` — the same number as a broken apply, in the command whose
whole function is to be read by CI — and a missing `wg`/`virt-customize`/`ngrok`
also answered `1`, indistinguishable from a typo error in a flag. A reconciler's
next two calls are opposite: wait longer, or stop and
install something.

```bash
delonix stack wait -f delonix-manifest.yaml --timeout 120
case $? in
  0)   ;;                     # everything up
  124) exit 0 ;;              # still coming up — the next pipeline run retries
  69)  echo "this node is missing a tool" >&2; exit 1 ;;
  *)   exit 1 ;;              # anything else: stop
esac
```

### The dictionary number: `DX-CDNN`

The exit code and the `DX_*` say the **class**. The `DX-CDNN` number says **which failure**
(ADR-0043): the thousand is the class, the hundred the domain, the last two digits the failure, and `00` the
class itself. The CLI prints it on the error line — `error[DX-4501] no such VM: dev` — and
`delonix explain DX-4501` says what it means and what to do (`--json` for scripts). The
full dictionary is in `delonix explain codes` and on the
[Codes dictionary](codigos.html) page, generated from the same table.

**A number never changes meaning nor is reused**: one can be added, and a failure
that ceases to exist has its number retired. The accompanying message may be rewritten and is
translated; the number is the contract. While a spot in the engine does not yet have its own entry,
it answers with its class's generic entry (`DX-4000`, `DX-1000`, …).

### The textual identity: `DX_*`

The number serves whoever reads `$?`. Whoever reads **text** — an HTTP client, a `-o
json` consumer, a logging pipeline — has the `DX_*` code, which is the same
classification spelled differently:

| `DX_*` | code | when |
|---|---|---|
| `DX_NOT_FOUND` | `4` | that resource does not exist |
| `DX_NOT_RUNNING` | `3` | exists, but is not running |
| `DX_CONFLICT` | `5` | the name is already taken |
| `DX_UNAVAILABLE` | `69` | a capability this host does not have |
| `DX_TIMEOUT` | `124` | the deadline ran out |
| `DX_INVALID_ARGUMENT` · `DX_REGISTRY` · `DX_SYSCALL_FAILED` · `DX_INVALID_STATE` · `DX_IO` | `1` | failures without their own number |

**These names are a contract**: a code may be ADDED; an existing one
never changes spelling or meaning.

The relation is **asymmetric on purpose**. A `DX_*` always maps to ONE
number — if it mapped to two, `$?` and the text would contradict each other for the same
failure. But `1` carries SEVERAL codes, because it is the "no own class" bucket and
the text can afford to be finer-grained: each NUMBER is a promise that has to
hold for the rest of `0.x`, while `DX_REGISTRY` next to
`DX_INVALID_ARGUMENT` costs nothing. There is a test requiring both halves of this
rule, including that the bucket stays a bucket.

Today the code goes out in the **management API** (`delonix serve api`), as a field
added to the error body — `{"error": "...", "code": "DX_NOT_FOUND"}`. The
`error` field was not touched: the ADR-0005 rule (adding yes, removing or
changing type no) holds here as it holds in `-o json`.

**What still has no code of its own, and it is honest to say so:** *permission denied* and
*temporary failure* were considered and left out — there is today no
error variant that produces them (permission failures arrive wrapped in the
`errno` of a syscall, and the retry that exists happens inside the engine and never
reaches the caller). Publishing a number that nothing builds is a number that can never
come back.

What this solves is concrete: a reconciler — the `Makefile`, the CI step, the
bash loop driving this CLI — used not to be able to tell "create, because it's missing"
from "stop, because it failed" without reading the error MESSAGE. And the message is
translated (`--l18n=pt`), so a script doing `grep 'no such'` works on the
machine where it was written and stops classifying on a node with a different locale.

```bash
delonix container inspect web >/dev/null 2>&1
case $? in
  0) ;;                       # it's there
  4) delonix container run -d --name web nginx ;;   # missing — create it
  3) delonix container start web ;;                 # exists, stopped — start it
  *) exit 1 ;;                # anything else: stop, don't guess
esac
```

**The exit code of a `run`/`exec` is still that of the WORKLOAD, not of the engine.**
A foreground `run` returns the container process's code, `exec` the
command's, and `healthcheck` exits `1` when unhealthy — the three promises of
old, unchanged. The consequence to remember: a container that exits `4` exits `4`,
so **the `$?` of a `run`/`exec` is never read as one of the classes above** — that
number was chosen by the workload, not by the engine.

**What is left out**, and it is honest to say so: a host precondition still to
be satisfied (a session without cgroup delegation, for instance) has no code
of its own — the engine warns and continues, so there is no failure to classify. And
the groups still not stable (see the list further below) may answer `1` where
a stable command already answers `4`: `workload describe` of a name that does not
exist is the known case.

### Migrating

* `if delonix …; then` / `|| exit 1` / `set -e` — **nothing changes**.
* `[ $? -eq 1 ]` testing "this command failed" — becomes `[ $? -ne 0 ]`.
* `[ $? -eq 1 ]` testing "does not exist" — becomes `[ $? -eq 4 ]`, the
  test there previously was no way to write.

## Stable in content, not in format

**The `ls`/`ps` tables.** The columns may change width, order or
gain siblings — they are made for humans and are measured by actual content. A script
that does `awk '{print $3}'` on them breaks, and that does not count as a contract
break.

**For automation there is `-o json`**, and it is the stable one: a JSON array per
resource, fields may be ADDED but not removed nor have their type changed
(ADR-0005). Verified working (2026-09-25, v4.4.0) in `container ps`,
`image ls`, `volume ls`, `network ls`, `vm ls`, `secret ls`, `workload ls` and in the
generic `get <plural>` verb (`get pods`, `get volumes`, …), which is the listing
path of Kinds without their own `ls` — `pod ls` no longer exists, and the
`storage` group merged into `volume`. Also `inspect` and `-q`/`--quiet`. **Measured
exception:** `backup ls` does not yet accept `-o json`.

> An earlier version of this document said `-o json` was "still to be done and is the
> gap acknowledged here". **That was wrong** — it has existed since ADR-0005. It is
> exactly the error `paridade-docker-podman.md` opens to fix: inferring
> absence from a symptom, without going to check.

## The manifest schema — stable, and the most important one

> This section **replaces** the line saying the schema "is additive in practice,
> but is not a promise". It was the commitment the wrong way round: the CLI was more
> protected than the declarative format, when it is the format that people put into
> git and review in a PR. A `delonix-manifest.yaml` only gets versioned if it is known
> not to break on its own.

For Kinds with a typed spec — **`Container`, `Pod`, `Volume`, `Network`** —
it is guaranteed, within `0.x`:

* **A field is never removed, nor changes type, nor changes meaning.**
* **A new field is always optional** and has a default that preserves the
  previous behaviour. A manifest written today still does the same thing
  tomorrow.
* **An old name that gets renamed is still accepted as an alias** (this is what already
  happens with `restart`→`restartPolicy`, `options`→`mountOptions`,
  `wg_ip`→`wgIp`).
* **`apiVersion: delonix.io/v1` only changes with a `v2`**, and a `v2` does not ship
  without `v1` continuing to be accepted. Since v0.64.0 each Kind also has its
  domain group (`compute.delonix.io/v1alpha1`, `networking.…`, …) — the
  `delonix api-resources` command says which one each one has. Both spellings are accepted;
  the group one is canonical, and the Kind only accepts **its own group**
  or the legacy one.

The truth is not this text, it is the schema: **`delonix manifest schema`** emits it
from the code itself (ADR-0007), and the same file is published at
[`schema/v1/delonix.json`](schema/v1/delonix.json). A repository test
fails if the published one stops matching the generated one, precisely so this page cannot
lie again.

What the schema REFUSES, and it is worth knowing because it is what shows up
underlined in the editor: an unknown field (a typo), **a `kind` the engine does not
know** — a typo in the name, or a removed Kind — and **an `apiVersion` that is not
the group for that Kind**. Until v0.65.x, `kind` was a free string and any
name passed, so `kind: Contaner` validated cleanly and an already-removed `Egress` did too; and
`apiVersion` was fixed to the legacy one, so the group spelling — the canonical
one — was underlined as if it were an error. A validator that disagrees with the engine is
worse than none, because the green check is what people follow.

Point the editor and write manifests with completion and validation:

```yaml
# yaml-language-server: $schema=https://ngolacloud.github.io/delonix/schema/v1/delonix.json
apiVersion: compute.delonix.io/v1alpha1
kind: Pod
metadata: { name: web }
spec:
  containers:
    - name: web
      image: nginx:1.27
```

There is also a VS Code extension that brings this already wired, plus a template per
Kind: [`angolardevops/delonix-vscode`](https://github.com/angolardevops/delonix-vscode).
It points to the SAME file published above — fetched live, not embedded —
so the editor and `stack apply` cannot disagree.

To know what changed between two versions, there is no hand-written page —
that would be the second source of truth ADR-0007 abolished. There is a command:

```bash
scripts/schema-diff.sh v0.46.0          # from that tag to the current tree
scripts/schema-diff.sh v0.46.0 v0.47.0  # between two tags
```

It compares field by field (name and type), not the raw JSON, and exits **1** when there
are differences — it serves directly as a CI gate. A field removed or with its
type changed is flagged as a **contract break**, which is what this section
promises will not happen.

**All Kinds now have a generated schema.** This section used to say that `Vm`,
`Cluster`, `ShareVolume`, `Image`, `Secret`, `Ingress`, `FirewallPolicy`,
`HTTPRoute`, `Tunnel`, `Workload` and `Stack` did not have one — that is no longer
true: `delonix explain <Kind>` answers for every Kind that
`delonix api-resources` lists (verified 2026-09-25, v4.4.0), with the
canonical names (`VirtualMachine`, `KubernetesCluster`, `NetworkPolicy`, `Gateway`; the
old names `Vm`/`Cluster`/`FirewallPolicy`/`Tunnel` still resolve as an
alias). `ShareVolume` no longer exists: it is a `kind: Volume` with a `share:` block. If
a future Kind arrives without a schema, `delonix explain` refuses it, saying which Kinds
have one, instead of omitting it.

> **Three Kinds stopped existing** in this series, merged into what they already did:
> `Egress` → `FirewallPolicy` with `direction: egress`; `Dependency` →
> `FirewallPolicy` (sugar, reduced on load); `Storage` → `kind: Volume` with a
> `nfs:`/`cifs:`/`webdav:` block. The old names still load, with a
> deprecation warning — the "clean cut" rule applies to commands, and a
> manifest in git deserves a ramp rather than an error.

## The node's providers file — `config.delonix.io/v1`

`providers.yaml` (ADR-0054) says which providers a node has, how the engine reaches
each one, and which one serves a request that names none. Since catalog 1.1.0
(ADR-0059 F1) it also accepts an OPNsense appliance (`type: opnsense`) and the
`networkDefaults` block, which says which provider answers each network role
(`segment`, `gateway`; `nat`, `ipam` and `dns` are refused until someone
serves them exists). Since ADR-0064 D6 it also accepts `type: powerdns`: the
credential of the ENGINE ITSELF for a PowerDNS server (`url`, `controllers`, `auth.keyFile`,
`allowPlainHttp`), used only to remove the gateway records the Proxmox
node writes and never removes. `defaultProvider` is still only the compute one. It is looked for in
this order, and **the first file that exists wins, without merging**:
`DELONIX_PROVIDERS_CONFIG`, `$XDG_CONFIG_HOME/delonix/providers.yaml` (or
`~/.config/…`), `/etc/delonix/providers.yaml`.

It is a published format, with the same promise as the manifest schema within
`0.x`: **a key is never removed nor changes meaning, a new key is
always optional, and `apiVersion: config.delonix.io/v1` only changes with a `v2`**
that does not ship without `v1` continuing to be read.

The truth is the schema, generated from the types the engine reads:
**`delonix provider config schema`**, published at
[`schema/v1/providers.json`](schema/v1/providers.json), with the same test
failing if the published one stops matching the generated one. The schema refuses an
unknown key, another version, a provider type that does not exist, and a secret
written in the file itself (`tokenSecret`, `password`, `secret` — only by
reference: `tokenSecretFile`, `passwordFile`, `secretFile`, `secretRef`).

The schema says whether the file is **well written**; `delonix provider config
validate` says whether it **works**: a
`defaultProvider` or a `networkDefaults` without an entry in the file, a token
file or secret readable by other users, a CA that does not exist. Neither of the two contacts a node.

```yaml
# yaml-language-server: $schema=https://ngolacloud.github.io/delonix/schema/v1/providers.json
apiVersion: config.delonix.io/v1
defaultProvider: libvirt
providers:
  - type: libvirt
  - type: opnsense
    url: https://fw.example
    auth: { keyFile: /etc/delonix/opnsense.key, secretFile: /etc/delonix/opnsense.secret }
networkDefaults:
  gateway: opnsense
```

## NOT stable — may change in any version

* **`serve cri`, `serve api`, `serve node-api`, `serve docker-api`** — protocol surfaces
  under construction. `docker-api` publishes its coverage at
  `delonix serve docker-api --matrix`, and that table is what says what exists
  today, not this promise. The **management API** (`serve api`) is local (unix
  socket, only the same uid) and has no published contract: do not build automation
  on it — the CLI exists for that, with `-o json`. Its routes are
  **frozen** (fixes only) while the local node contract that replaces it
  is decided (ADR-0040 and ADR-0041, both *Proposed*); a promise for that
  contract only enters this page once its first wave closes.
  The routes that changed the network **without the container's record** —
  `/v1/net/firewall/:ip`, `/v1/net/egress[/:bridge]`, `/v1/net/attach-extra…` and
  `DELETE /v1/net/attach/:id/:ip` — now answer **`501` with
  `DX-6302`**: the next reapply would undo what they wrote, and they only covered the
  primary IP of a container with multiple networks. The path is the CLI (`net ingress`,
  `net egress`, `network connect`/`disconnect`), which goes through the record.
* **The `delonix` executable name belongs to this engine.** The control plane no
  longer produces a binary with that name (ADR 0038 of the control plane); a `delonix`
  installed by this project is always the CLI described on this page.
* **`compatibility`** — the same table as `serve docker-api --matrix`, but
  as a top-level verb and with `-o json`. Still only covers `docker`; `compose`/
  `cri`/`oci` are future surfaces, each with its own "three
  states" still to build. No promise on fields until `compose`/`cri`/`oci`
  are added — the format may change shape to accommodate them.
* **`provider`** — `ls`/`describe`/`matrix`: what each provider (libvirt,
  cloud-hypervisor, proxmox, linux, opnsense) declares against the capability
  catalog (ADR-0050, 1.1.0 since ADR-0059), measured on the host. The types are
  `compute`, `network`, `storage`, `image` and `gateway`; a `gateway` answers the
  network rows. The capability NAMES (`vm.snapshot.memory`,
  `net.namespace-isolation`, …) and the six states are what a script reads, and
  they follow the catalog version (`catalog_version` in the JSON): a new entry bumps
  the minor, renaming or removing one bumps the major. The table columns and the
  JSON shape beyond those fields may still change.
* **`cluster`, `vm`, `pod`, `workload`, `net`, `systemcontainer`** —
  the surface is still settling. (The `kind: Pod` *schema* is stable, see
  above; what is not stable is the `delonix pod` command group.)
  > **`storage`/`sharevolume` stopped being groups** (#216, "fold
  > storage/sharevolume into volume") — this section kept naming them
  > for a while after they no longer existed, which is exactly the error
  > the list below exists to prevent (a classified name that is no longer
  > a real group). Their surface now lives in `delonix volume`.
* **`volume`, `network`, `secret`** — only the `-o json` output and (for `Volume`/
  `Network`) the manifest schema have a promise (see the two sections
  above); the rest of the imperative surface (`create`/`rm`/`inspect` and their
  flags) has never had its own promise reviewed. `volume` in particular
  just absorbed `storage`/`sharevolume` (#216) — it is literally mid-
  settling, the worst possible moment to promise anything about it.
* **`apply`, `plan`, `wait`, `stack`, `manifest`, `get`, `describe`,
  `delete`, `diff`, `drift`** — the native reconciliation cycle (IaC, v0.47.0+). The
  SCHEMA of the manifests these commands consume is already stable (section
  above); the VERBS and flags of these commands themselves (`--detailed-exitcode`,
  `--prune`, `--replace`, …) have never had their own review, and the most
  recent Kind (`Service`) is still gaining wiring in some of these paths.
* **`migrate`** — `migrate assess` reads a compose file you already have and says
  what this engine serves, refuses and does not implement. It is a REPORT (it does not create, does not
  pull, does not run), and the table behind it is `compatibility compose`'s —
  a dated measurement, which moves as the engine gains keys.
* **`compose`** — native `docker-compose.yml` support, still gaining
  Compose Spec keys (`profiles`, `configs`/`secrets`, multi-file —
  see `AGENTS.md` for the full list of what is missing).
* **`system`, `dashboard`, `completion`, `init`, `man`, `config`,
  `explain`, `api-resources`, `version`** — utilities/introspection, no
  promise declared in any version.
* **`hosts`** — `hosts sync` publishes in `/etc/hosts` the service names of
  exposed containers (ADR-0048); new surface, no promise yet.
* **`mcp`** — the Model Context Protocol server (ADR-0025), new surface. The
  `stdio` transport is the supported one (one AI client child process per
  session, never a daemon); a local HTTP transport, should it come to exist, would be
  loopback-only with a local token, as `serve api` is already local-only and without a
  published contract — do not build automation on it yet.
* **`policy`** — `delonix policy unset` (M04, `docs/roadmap/
  13-improvements-traceability.md`), the only imperative verb of
  `kind: RuntimePolicy`. New surface, no declared promise — the
  `policy.json` it removes already existed and was already read by `container run`/
  `vm create`; what is new is the declarative form (`stack apply`) and this
  removal command. Reading goes through `delonix get runtimepolicies`/
  `delonix describe runtimepolicy <name>`, the generic verbs, which inherit
  the promise (or lack thereof) of those verbs and not one of their own for this group.
* **`backup`** — the group was not declared on either side, and that
  omission is itself a defect: whoever wanted to know if they could depend on
  `delonix backup vm x` had no answer. It stays NOT stable while the
  archives gain verbs. The **archive format** is a different thing and has its
  own guard: `backup.json` carries a version number and a reader that does not
  recognise it REFUSES instead of guessing a layout it never saw.

  **Changed in v0.67.0:** `delonix backup <kind> <name>` became
  `delonix backup create <kind> <name>`, and the root `delonix restore` became
  `delonix backup restore <archive>` — without the positional `<kind>`, which the archive
  already records (it can still be asserted with `--kind` so a mismatch is
  refused). Scheduling moved from backup flags to
  `delonix backup schedule`, with the same semantics: it installs the timer **and**
  takes the first archive. Clean cut, no aliases — the old form fails with
  "unrecognized subcommand", never silently.

  **`delonix system backup` was not touched on that date.** It is a different scope —
  the whole state root of a node — and not a second door for this group.

  **Changed later (Sprint 4 of the CLI restructuring, 2026-09-12):** `system
  backup`/`system restore` itself was renamed to `system snapshot
  create`/`system snapshot restore` — the same vocabulary as `vm snapshot`/
  `volume snapshot` (a point-in-time capture), only at node scale. The SCOPE
  did not change nor merge with this group — only the SHARED WORD ("backup")
  disappeared, which was the real source of the confusion the previous paragraph was
  already explaining in writing. Clean cut: the old form fails with
  "unrecognized subcommand".
* **`gitops`** (`reconcile`/`status`/`suspend`/`resume`, ADR-0021) — new group
  as of 2026-10-12, NOT stable. Acts on a `kind: GitOpsSource` that already exists;
  the source is declared with `stack apply`, like any other Kind. The record and the
  state of each pass in `$DELONIX_ROOT/gitops/` are internal state (read
  with `gitops status -o json`, never from disk).
* **`oci-runtime`** (`create`/`start`/`state`/`kill`/`delete`, ADR-0080) — the
  `runc`-compatible CLI: the engine's own init behind the runc contract,
  so it can take the place of `runc`/`crun`. New and EXPERIMENTAL surface — the
  contract it imitates is runc's, not a promise of its own by this engine, and
  the verbs/states may still change while conformance settles.
* **Everything starting with `net netns`** — internal plumbing exposed for
  debugging convenience.
* **The state file format** in `$DELONIX_ROOT`. Read via `inspect`,
  never from disk.
* **`stack history`**, **`stack rollback`** and the content of `$DELONIX_ROOT/stacks/` (ADR-0019). It is a
  record of what was applied, and the design explicitly promises that **nothing
  reads it to decide what exists**: deleting that folder does not change what `plan`, the
  `apply`, `prune` or `destroy` do — only history is lost, and there is a gate in
  the E2E battery requiring it. Automation that depends on the presence of a revision is
  betting against that promise.

## Deliberate divergence — `docker run` flags this engine does NOT bring

This table is documentation, not a stability promise: it lists the
`docker run` flags delonix does **not** implement on purpose, because they make no
sense in a *daemonless*, *rootless* engine built on **cgroup v2**. Each
line gives the reason in one sentence and, where one exists, the equivalent path in this
engine. The absence is a design decision, not a gap to fill — asking for
any of them should be refused clearly, never accepted silently.

| `docker run` flag | Why it is N/A here | Delonix equivalent |
| --- | --- | --- |
| `--cpuset-mems` | The `cpuset` controller is not delegated to cgroup v2 in rootless mode (the host does not hand over the NUMA memory nodes). | No direct equivalent; `delonix run --cpuset` covers the part (CPUs) that rootless does hand over. |
| `--memory-swappiness` | cgroup v1 only; v2 has no per-cgroup `memory.swappiness`. | No equivalent; the total memory+swap is gauged with `delonix run --memory-swap`. |
| `--oom-kill-disable` | cgroup v1 only (`memory.oom_control`); v2 removed that knob. | No equivalent; priority before the OOM-killer is done with `delonix run --oom-score-adj`. |
| `--cgroupns` | In rootless the cgroup namespace is already isolated by construction — there is no honest `host`/`private` choice to offer. | No equivalent — isolation is the default state. |
| `--isolation` | Windows/Hyper-V specific; this engine is Linux only. | No equivalent. |
| `--runtime` | Delonix **is** the OCI runtime (`delonix oci-runtime`), it does not delegate to another one (`runc`/`crun`). | No equivalent — the engine itself is the target. |
| `--storage-opt` | Storage is the engine's own overlay, not a per-container configurable driver. | No equivalent; managed volumes live in `delonix volume`. |
| `--volume-driver` | There are no external volume driver *plugins* in the daemonless model. | No equivalent; `delonix volume` is what manages volumes. |
| `--use-api-socket` | There is no daemon nor API socket to mount inside the container. | No equivalent — the model is daemonless. |
| `--link` | Legacy, deprecated upstream (Docker itself discourages it). | `delonix network connect` and network aliases (`delonix run --network-alias`). |
| `--sig-proxy` | A foreground `run`, being daemonless, already inherits and forwards the operator's signals — there is no proxy to switch on/off. | No equivalent — it is the default behaviour. |

## How a break is made, when it has to happen

Precedent already set by the project: the v0.30.0 reorganization (`netns` →
`net netns`, `cri` → `serve cri`, …) was a **clean cut, no aliases** — the
old form fails with "unrecognized subcommand", never silently.

That stays as the rule: **fail loud**. A compatibility alias that
changes behaviour is worse than an error.

And a lesson that cost this session: that break left an INTERNAL caller
out of date (`delonix-cri` kept invoking `delonix netns attach`), which
broke rootless pod creation for months — see
[cri-conformance.md](cri-conformance.md). A clean cut requires grepping
for callers across the ENTIRE workspace, not just in the documentation.

## Discovering commands via TAB

In Fish, run `delonix completion shell fish | source` to enable completion
in the current session. After `delonix ` or `delonix container `, press TAB to see
`ps` and `ls` with the same description. In `delonix image `, both `remove` and `rm`
should appear. The shell controls the column layout. In Bash, completion
is registered with `source <(delonix completion shell bash)`.
