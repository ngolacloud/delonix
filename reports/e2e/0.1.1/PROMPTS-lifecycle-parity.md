# `delonix container` lifecycle — parity defects and fix prompts

**Source:** live functional test, parameter by parameter, of the lifecycle verbs
— `ps`, `logs`, `stats`, `inspect`/`describe`, `cp`, `port`, `stop`, `start`,
`restart`, `rm`, `kill`, `pause`/`unpause`, `top`, `rename`, `create` — against
`docker`/`podman`.
**Environment:** delonix 0.1.2 (commit `8a795077`), rootless host · daemonless,
delegated cgroup2, `alpine:3.19`. Docker 29.10.0 / Podman 4.9.3, kernel
`7.0.0-43-generic`/`x86_64`. Date: 2026-11-19.
**Method:** observed effect — real output, state (`ps`/`ps -a`), signals, cgroup
freezer, `cp` round-trip, `inspect` JSON, `rc` propagation.

**Verdict:** the bulk of the lifecycle is in **good parity**. 9 defects:
2 behavioural divergences (one of them a dangerous flag-semantics conflict),
1 verb with no top-level form, and scripting-flag gaps.

Order of attack: security → functionality → parity → DX.

---

## Per-parameter matrix (observed)

| Verb / parameter | Observed effect | Verdict |
|---|---|---|
| `ps` / `ps -a` / `-q` / `-s` | correct listing; `-s` gives SIZE; `-q` ids only | ✅ parity |
| `ps -o json` | clean structured JSON | ✅ (extra) |
| **`ps -n <N>`** | treats `<N>` as **namespace**; docker `-n N` = last N | ❌ **L2** (semantic conflict) |
| **`ps --filter` / `--format`** | `unexpected argument` | ❌ **L7** (missing) |
| **`logs --tail/--since/--timestamps`** | DX-1000 error unless container has `--log-cri`; docker: always | ❌ **L1** |
| `logs -f` | follows the stream | ✅ parity |
| **`logs --until`** | `unexpected argument` | ❌ **L8** (missing) |
| **`stats` (default)** | one-shot (docker streams); **no** `--no-stream`/`--format`/`-a` | ❌ **L4** |
| `container inspect` | full JSON | ✅ parity |
| `container describe` | human-readable sheet | ✅ (extra) |
| **`inspect` top-level** / **`--format`** | no `delonix inspect`; no `--format` (Go-template) | ❌ **L3** |
| `cp` (ctr→host, host→ctr) | correct round-trip | ✅ parity |
| `stop -t <s>` | stops with timeout; rc=0 | ✅ parity |
| **`stop -s/--signal`** | `unexpected argument` | ❌ **L5** (missing) |
| `kill -s <sig>` | sends the signal; rc=0 | ✅ parity |
| `pause` / `unpause` | cgroup freezer: state `(Paused)` → resumed | ✅ parity |
| `top` | `HOST-PID/STATE/COMMAND` | ✅ parity |
| `rename` | renames; rc=0 | ✅ parity |
| `create` → `start` | `create` stays `Created`; `start` boots | ✅ parity |
| **`start -a/-i`** | `unexpected argument` (start is always detached) | ⚠️ **L9** (deliberate divergence) |
| **`rm -v`** | `unexpected argument` | ❌ **L6** (missing) |

---

## Cross-cutting notes
- **Rootless host, no sudo**; validate with rootless harnesses. `/tmp` ephemeral between turns.
- Binary tests: `cargo test -p delonix-bin --bins`. develop-GitFlow process; push/PR by the owner.
- New flag → **pt.po** (3 strings) + flag order (`delonix-new-code-and-flag-gates`).

---

## L1 — `logs --tail/--since/--timestamps` require `--log-cri`  _(Medium · functionality)_

**Symptom.** `logs --tail 2 <ctr>` fails with
`DX-1000 … require the container to have been run with --log-cri` when the container
was not started with that flag. Docker and Podman honour `--tail`/`--since`/
`--timestamps` **always**. The daily flow `logs --tail 50 web` is broken by default.

**Evidence.**
```
delonix logs --tail 2 lc1 → DX-1000 (lc1 run without --log-cri)
docker  logs --tail 2 <c> → last 2 lines, always
```

**Root cause.** Two log formats (`bins/delonix/src/cmd/dockerapi.rs:1251-1375`,
`log_cri`): only the CRI format has a per-line timestamp; the default does not, and the filter
rejects instead of degrading. `LogsArgs` in `bins/delonix/src/cmd/container.rs:1654`,
`cmd_logs` in `container.rs:8188`.

**Fix.** Make the default record logs with a per-line timestamp (or derive the
timestamp from the write) so that `--tail`/`--since`/`--timestamps` work without
`--log-cri`; worst case, `--tail N` (line count) must always work,
since it needs no timestamp.

**Done when.**
1. Container run **without** `--log-cri`: `logs --tail 2` returns 2 lines, rc=0.
2. `--timestamps` returns stamped lines (or documented degradation, never an error).
3. e2e test of the path without `--log-cri`.

**Gates.** Do not regress the CRI format. Rootless gates. **Agent:** general-purpose, high tier.

---

## L2 — `ps -n` clashes with Docker's semantics (`-n` = last N)  _(Medium · functionality/parity)_

**Symptom.** `delonix ps -n 3` interprets `3` as a **namespace** and returns an
empty list; `docker ps -n 3` returns the **last 3** containers. A command copied from
Docker silently produces the wrong result.

**Evidence.**
```
delonix ps -n 1 → empty table (namespace "1" does not exist)
docker  ps -n 1 → the last container created
```

**Root cause.** `PsArgs` binds `-n` to `--namespace`
(`bins/delonix/src/cmd/container.rs:1567`, `short = 'n', long = "namespace"`).
Docker uses `-n/--last N`.

**Fix.** Resolve the clash: keep `--namespace` only in long form (drop
the short `-n`) **or** move the namespace to another letter, and bind `-n/--last <N>` to
Docker semantics (last N, implies `-a`). Owner's decision on which letter.

**Done when.**
1. `ps -n 1` returns **1** container (the most recent), like Docker.
2. `--namespace <ns>` still filters by namespace (long form).
3. Test covering both.

**Gates.** Flag change → **pt.po** + order + completers. Rootless gates.
**Agent:** general-purpose, medium tier.

---

## L3 — no top-level `delonix inspect` and no `--format`  _(Medium · parity/scripting)_

**Symptom.** `delonix inspect <x>` → `unrecognized subcommand`; only
`container inspect` (JSON) and `container describe` (human) exist. None accepts `--format`
(Go-template), the pillar of Docker scripting (`docker inspect --format '{{.State.Pid}}'`).

**Evidence.**
```
delonix inspect lc2                  → unrecognized subcommand
delonix container inspect --format … → unexpected argument '--format'
```

**Root cause.** `ContainerCmd::Inspect` (`container.rs:1876`, `cmd_inspect:6631`)
emits fixed JSON without `--format`; there is no top-level `inspect` verb (`mod.rs`).

**Fix.** (a) Expose a top-level `delonix inspect` (alias of `container inspect`,
ideally also resolving images/volumes/networks like Docker). (b) Add
`--format <template>` to `cmd_inspect` (Go-template render over the same JSON).

**Done when.**
1. `delonix inspect <ctr>` returns the same JSON as `container inspect`.
2. `inspect --format '{{.Name}}' <ctr>` prints the name, rc=0.
3. Test of the template + the top-level alias.

**Gates.** New flag/verb → **pt.po** + order. Rootless gates. **Agent:** general-purpose, medium tier.

---

## L4 — `stats` without `--no-stream`/`--format`/`-a` (and one-shot default)  _(Low-medium · parity)_

**Symptom.** `delonix stats <ctr>` gives **one** sample and exits (docker **streams**
until Ctrl-C). The flags a Docker script passes — `--no-stream`, `--format`, `-a` —
are **rejected** by the parser.

**Evidence.**
```
delonix stats lc2              → 1 sample, rc=0 (does not stream)
delonix stats --no-stream lc2  → unexpected argument '--no-stream'
```

**Root cause.** `StatsArgs` (`container.rs:1704`) only accepts ids; `cmd_stats`
(`container.rs:8063`) is one-shot.

**Fix.** Accept `--no-stream` (no-op that confirms the one-shot, for
script compatibility), `--format <template>` and `-a/--all`. Optional: a real
`stream` mode behind the absence of `--no-stream` (lower priority; one-shot is a good default).

**Done when.**
1. `stats --no-stream <ctr>` → one sample, rc=0 (accepted).
2. `stats --format '{{.Name}} {{.MemUsage}}'` prints the requested fields.
3. Test of the flags.

**Gates.** New flags → **pt.po** + order. Rootless gates. **Agent:** general-purpose, low tier.

---

## L5 — `stop` without `-s/--signal`  _(Low · parity)_

**Symptom.** `stop -s SIGKILL <ctr>` → `unexpected argument '-s'`. Docker/Podman
let you choose the stop signal; delonix only has `-t/--time` (`StopArgs`,
`container.rs:1592`).

**Fix.** Add `-s/--signal <SIG>` to `StopArgs`, used on the first send
before the timeout `SIGKILL` (reuse the signal parsing from `kill`).

**Done when.** `stop -s SIGINT <ctr>` terminates via SIGINT (observable in rc/state);
`-t` still works; test covered.
**Gates.** New flag → **pt.po** + order. **Agent:** general-purpose, low tier.

---

## L6 — `rm` without `-v` (anonymous volumes)  _(Low · parity)_

**Symptom.** `rm -v <ctr>` → `unexpected argument '-v'`. Docker removes the container's
anonymous volumes with `-v`. `RmArgs` (`container.rs`, right after `RestartArgs`)
only has `-f/--force` and `-n/--namespace`.

**Fix.** Add `-v/--volumes`: on removal, also delete the anonymous volumes
created by this container (not the named ones).

**Done when.** `run -v /data … ; rm -v <ctr>` removes the anonymous volume; a
**named** volume survives; test covered.
**Gates.** New flag → **pt.po** + order. Do not delete named volumes. **Agent:** general-purpose, medium tier.

---

## L7 — `ps` without `--filter`/`--format`/`-l`/`--no-trunc`  _(Low · parity/scripting)_

**Symptom.** `ps --filter status=running` and `ps --format '{{.Names}}'` → `unexpected
argument`. `-o json` exists (good for machines), but the Go-template `--format` and
Docker's `--filter` are missing; also `-l` (latest) and `--no-trunc`.

**Fix.** Add to `PsArgs` (`container.rs:1555`, `cmd_ps:4814`):
`--filter <k=v>` (repeatable: `status`, `name`, `label`, …), `--format <template>`,
`-l/--latest`, `--no-trunc`. Reuse the data model from `-o json`.

**Done when.** `ps --filter status=exited` lists only the stopped ones; `ps --format
'{{.Names}}'` prints names; test covered.
**Gates.** New flags → **pt.po** + order. **Agent:** general-purpose, medium tier.

---

## L8 — `logs` without `--until`  _(Low · parity)_

**Symptom.** `logs --until <t>` → `unexpected argument`. Add to `LogsArgs`
(`container.rs:1654`) the upper time bound, counterpart of the already-existing `--since`
(depends on L1 to have per-line timestamps in the default).

**Done when.** `logs --since T1 --until T2` returns only the window; test covered.
**Gates.** New flag → **pt.po** + order. **Agent:** general-purpose, low tier.

---

## L9 — `start` without `-a/-i` (always detached)  _(DX · deliberate divergence)_

**Symptom.** `start -a <ctr>` → `unexpected argument`. By design, `delonix start` is
**always** detached (documented in `--help`). Docker allows `start -a` (attach to
stdio) and `-i`.

**Decision.** Probable **deliberate divergence** (post-boot attach is done with
`delonix attach`). Recorded for the publication menu; if the owner wants parity,
`start -a` should reuse the `attach` path. Not actionable without the owner's decision.

---

## What is NOT a defect (parity confirmed live)
- `ps`/`ps -a`/`-q`/`-s` and `-o json`; `cp` round-trip; `stop -t`; `kill -s`;
  `pause`/`unpause` (cgroup freezer, state `(Paused)`); `top`; `rename`;
  `create`→`start`; `container inspect` (JSON) and `container describe` (human).
- Typed DX-* errors with dedicated rc, generally clearer than Docker.
