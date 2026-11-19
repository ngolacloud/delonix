# Parity with Docker and Podman — survey and plan

> **Status:** survey done on 2026-08-06 against `delonix 0.42.2`, from a real
> usage session (setting up and operating the Delonix Meet stack with Odoo)
> and from reading the `--help` of each command group.
>
> **This is not an audit.** I did not read the 76 thousand lines across the 10
> crates, did not measure performance, did not exercise `vm` nor `serve cri`.
> Where I did not verify, I say so.

---

## 1. Why this document starts with a correction

The first version of this analysis claimed that Delonix was missing *restart
policies*, the `restart` subcommand, health checks, resource limits, and
login to private registries.

**It was wrong about almost everything.** All of them exist. What happened
was the most common mistake in a tool comparison: inferring absence from a
symptom, without going to check.

The same happened with name resolution. I concluded it "didn't work"
because I had to pin IPs. There is a built-in DNS server
(`delonix-sdn/src/infra.rs`), listening on UDP `:53` on every bridge,
functionally equivalent to dnsmasq — which does not run rootless — and
already with a fix for a bug where a slow query blocked the entire node.

What actually failed me was something else, and it is the central lesson of
this document: **the engine fell back to host-net and I didn't know.** It is
not a networking bug. It is the absence of a signal about a degraded mode —
a different category, and more important than any missing feature.

---

## 2. What ALREADY exists (verified)

| Capability | How | Note |
|---|---|---|
| Restart policies | `--restart no\|on-failure[:max]\|always\|unless-stopped` | A per-container detached supervisor, no daemon — captures the real exit code |
| `restart` | `delonix container restart <ids>` | Reuses the rootfs and the original configuration |
| `wait` | `delonix container wait` | — |
| Health check | `delonix container healthcheck <id>` | Runs the image's `HEALTHCHECK`, **on demand** |
| Resource limits | `--memory`, `--cpus`, `--cpuset` | Require cgroup delegation |
| Events | `delonix system events`, `system monitor` | — |
| Private registry | `delonix image login` / `logout` | — |
| DNS between containers | built into the ingress holder | Equivalent to dnsmasq, rootless |
| `depends_on` with conditions | `service_started` / `service_healthy` / `service_completed_successfully` | Resolved via topological ordering |
| Pods, microVMs, CRI, Docker API | `pod`, `vm`, `serve cri`, `serve docker-api` | Out of Docker/Podman's reach |

Conclusion: **the gap is not one of features.** It is one of *operability* —
knowing what is happening — and of *ergonomics* — the cost of getting there.

---

## 3. Real gaps

### L1 · Silent degradation (the most serious)

When the holder's netns fails, the stack falls back to host-net. The
container starts, looks healthy, and name resolution has stopped existing.
Nothing in `ps`, `inspect`, or the logs says so.

It cost me, in one session: IPs pinned by hand instead of names, an nginx
upstream pointing at a stale IP, and a **502 in demo-production caught by
the user, not by me**.

*SRE lens.* An invisible degraded mode is worse than a failure: a failure can
be diagnosed, degradation poisons everything that follows.

### L2 · Health check is on-demand, not continuous

`container healthcheck` runs the `HEALTHCHECK` and exits with 1 if not
healthy — "usable in a script/CI", as the help itself says. There is no
`--health-interval` that makes the engine monitor it, nor does `ps`'s
`STATUS` distinguish `Up` from `Up (healthy)`.

Direct consequence: compose's `depends_on: service_healthy`, which **is
implemented**, somehow has to resolve health — but whoever operates it has no
way to see that state nor to build alerts on top of it.

### L3 · `--add-host` — DONE, and what the review caught

The container's `/etc/hosts` is recreated at startup, so entries injected by
hand would disappear on every restart. Implemented with persistence in the
record, validated live (survives `restart`, `getent` resolves).

**The security review found a HIGH finding that this feature armed**, and it
is worth recording because the lesson is bigger than the bug:

`write_etc_files` wrote with `format!("{rootfs}/etc/...")` + `fs::write`, and
both `metadata()` and `write()` FOLLOW symlinks. The rootfs is image-controlled
content and, in rootless, the tree belongs to the mapped uid — the container
itself could plant `etc/hosts -> ~/.ssh/authorized_keys` and the next startup
would write it **as the engine**, outside the rootfs.

This already existed, but it was a primitive of *truncate with the engine's
own fixed content*. `--add-host` made the content **attacker-chosen** — which
turns it into code execution. Proven live before the fix.

Closed with `safe_bind_target` (which already existed in the SAME file, and
which this repo had already applied twice — to bind mounts and to the
build's `COPY`; this path had been left out) + boundary validation: the
address parsed as `IpAddr`, the name checked against an LDH whitelist,
`\n`/`\t`/space refused, and an error instead of a silent drop.

**Lesson**: adding user-controlled content to an already-existing write path
changes that path's risk class. It is not enough to review the new code — you
have to reassess what it now feeds into.

### L4-bis · A gap I made up

While writing the stability promise I claimed `--format json` was missing
from the listing commands. **It is not**: `-o json` has existed since
ADR-0005 and works across all nine listing commands, verified one by one.

This is recorded because it is the SECOND time in this document that the same
mistake appears — and the first is right in section 1. Inferring absence
from a symptom, without going to check, is the characteristic failure mode of
tool comparisons.

### L4 · Divergent ergonomics

`delonix container ps`, not `delonix ps`. In isolation it is a detail;
multiplied by verbs used dozens of times a day, it is constant friction for
anyone coming from Docker or Podman.

### L5 · cgroup delegation is the user's job

The engine warns that without delegation there are no memory/CPU limits —
honest — but solving it means knowing how to set up a `systemd --user` with
`Delegate=yes`. It should be a command.

### L6 · `serve docker-api` is an undocumented slice

As long as it stays partial and with no published matrix, third-party tools
(Testcontainers, CI plugins, IDEs) break unpredictably. Failing with an
explicit "not implemented" is worth more than approximating completeness.

---

## 4. Plan

### Phase 1 — operable

1. **Explicit network state.** `bridge` vs `host (degraded: <reason>)` in
   `ps`, in `inspect`, and a one-time warning in the log. Closes L1.
2. **Continuous health check.** DONE. `--health-cmd`/`--health-interval`/
   `--health-timeout`/`--health-retries`/`--health-start-period`; `ps`'s
   `STATUS` now reads `Up 21 seconds (healthy)`, `describe` shows the probe,
   the policy and the last verdict, and a state change emits an event.
   Closes L2.

   **Who monitors, in an engine with no daemon** — that was the design
   decision. It is the **detached container's own supervisor**: already
   exists, one per container, already survives the CLI, and dies with
   whatever it watches. No fleet process, no new lifecycle. The cost is that
   a foreground container is not monitored, and that is correct — you are
   looking at it.

   **The probe limits only itself**, inside the container (`sh` with a
   watchdog). This was not an aesthetic choice: `runtime::exec` blocks on an
   intermediary, and killing that intermediary leaves the probe alive in the
   container's pid-namespace — a leak that would repeat every interval,
   forever. A probe that kills itself has no such hole and needs no new
   host-side plumbing.

   Validated live: `starting` during the 6s the file did not exist,
   `healthy` the exact second it appeared; and with `retries 3`, the sequence
   1 → 2 → 3 before flipping to `unhealthy`, with a success resetting the
   count.
3. **`--add-host` persisted** in the spec and reapplied at startup. Closes L3.
4. **`run --wait`** — DONE. Blocks until the image's `HEALTHCHECK` passes.
   Measured: 64 ms without the flag (with the service still starting up)
   against 6086 ms with it, and the service ready on return. With no
   `HEALTHCHECK` in the image it is an ERROR, never an instant return.
   Reuses the command resolution that compose's `depends_on: service_healthy`
   already used, so the two can never diverge.

   **Two real bugs found while wiring this up**, both silent:

   * **The supervisor took over the entire `-d` path**, not just
     `--restart` — and returns early. The first version of the flag was
     accepted and silently ignored in the overwhelming majority of `run -d`
     cases. It only surfaced because the debug `eprintln!` never actually
     printed.
   * **`commit_flat_rootfs_from_tar` hard-wrote `healthcheck: None`.** A
     `HEALTHCHECK` in the Dockerfile was parsed, accepted, and DISCARDED — on
     the rootless path, which is the normal one. The overlay (root) path had
     always honoured it, so the same file produced different images
     depending on the engine's mode, and compose's `depends_on:
     service_healthy` failed with "declares no healthcheck" against an image
     whose source declares one.

### Phase 2 — adoptable

5. **Top-level aliases** — DONE. `ps`, `run`, `exec`, `logs`, `rm`, and
   `images`. Implemented by **argv rewriting**, not by duplicated clap
   variants: a second declaration of `run`'s ~70 flags is a second
   declaration to keep in sync, and on the day they diverged the shortcut
   would silently stop accepting a flag. This way they are the SAME command
   by construction, `--help` included. `stop`/`start` are deliberately left
   out: this engine also stops VMs and pods, and a `delonix stop <name>`
   silently meaning "container" would be guessing — `workload stop` already
   covers the case and refuses the ambiguity out loud. Closes L4.
6. **`system setup [--delegate]`** — DONE. Diagnoses and fixes cgroup
   delegation. **Two remedies, because there are two problems** — the half
   every StackOverflow answer misses: the drop-in in `user@.service` fixes
   user services and future logins, but NOT the shell you are typing in,
   because a `session-N.scope` is a SIBLING of `user@.service` and inherits
   nothing from it (measured on this host: `subtree_control` is `root:root`
   in one, the user's in the other). Closes L5.
7. **`serve docker-api` matrix** — DONE. `delonix serve docker-api --matrix`
   prints the 14 implemented routes and the 7 deliberately missing ones with
   each one's reason; the 404 points there instead of just refusing. **The
   matrix cannot diverge**: a test reads the dispatch's own source code and
   fails if an arm exists with no line in the table. That test has already
   caught a real ambiguity (a ONE-FIELD limitation of `create` was filed as if
   the whole route were missing). Closes L6.

### Phase 3 — defensible

8. **CRI validation** — DONE, and the item that paid off the most. Upstream
   `critest` v1.36.0, run for real: **65 of 103 specs**, published in
   [cri-conformance.md](cri-conformance.md) with the script to reproduce it.

   **Found a serious, long-standing bug.** `delonix-cri` was calling `delonix
   netns attach`; the v0.30.0 reorganization moved it to `net netns attach`
   with a clean cut, and this caller was never updated — **rootless pod
   creation had been broken since then**. Hidden because `delonix_detached`
   sent stderr to `/dev/null` and returned a `bool`: the message that reached
   the kubelet named the victim and hid the culprit. A one-line fix took the
   run from **19 to 65 passes**.

   Of the remaining 38 failures, 9 are environmental (rootless AppArmor) and
   the rest is surface yet to be written — 8 of them in Security Context,
   which is where to start.

9. **CLI stability promise** — DONE, in
   [cli-stability.md](cli-stability.md). States what is stable (lifecycle
   verbs + `inspect`'s JSON), what is stable in CONTENT but not in format
   (the tables — use `inspect` for automation), and what makes no promise at
   all (`serve *`, `cluster`, `vm`, manifest schemas).
10. **Smoke tests** — DONE. `tests/compat/docker_api_smoke.py` runs the
   sequence third-party tools actually use (`create` → `start` → `inspect` →
   `rename` → `stop` → `remove`, one call at a time), with the official
   Python SDK — the same protocol as the Java/Go/Node clients Testcontainers
   uses. **14/14 green** against our socket. Not `docker run` on purpose: no
   test harness uses `docker run`.

---

## 5. What NOT to do

**Do not chase parity with Docker.** It is a race with no prize and no win:
whoever wants Docker uses Docker. Parity only matters where its absence
*prevents* using what is differentiating — microVMs and CRI.

**Do not build a Docker Desktop.** Huge cost, return for an audience that
is not our target.

**Do not promise production to third parties before Phase 1.** An engine
whose degraded mode is invisible is not operable by anyone who didn't write it.

---

## 6. The four lenses, in summary

| Lens | What it sees | Priority |
|---|---|---|
| **SRE** | You cannot tell what state it's in. Silent degradation, unobservable health | Phase 1 (1, 2) |
| **DevOps** | Daily friction: a long path to hot verbs, hand-written waits | Phase 1 (4), Phase 2 (5) |
| **Platform Eng** | Already ahead on capabilities; missing a **contract** — what is stable, what is covered | Phase 2 (7), Phase 3 (9) |
| **Cloud Native** | `serve cri` is the structurally strongest bet; demands demonstrated conformance | Phase 3 (8) |

---

## 7. Method

Every item closes with: implementation, a test that fails without it, and
review by that area's reviewer (`delonix-code`, `delonix-devops`,
`delonix-security-compliance`). Nothing is taken as done just because it
compiles — that is how the first version of this document was born wrong.

---

## 8. Testcontainers against `serve docker-api`, measured (2026-10-12)

Item 10 above runs the Docker SDK through the lifecycle, and it is green. That
is not the same as Testcontainers working: the SDK smoke removed a container
only after stopping it, and never asked for a port the daemon picks. This
section is what testcontainers-python **4.8.2** (docker-py **7.2.0**) actually
calls, captured through a logging proxy in front of the socket, in an isolated
root, with Ryuk disabled.

The test: `DockerContainer("alpine:3.20").with_command(...).with_exposed_ports(80)`,
then `get_container_host_ip`, `get_exposed_port(80)`, `wait_for_logs("ready")`,
`exec(["echo","hi"])`, `get_logs`, `stop`.

| Call | Route | Before (b1eab234) | After (S20) |
|---|---|---|---|
| `start` | `GET /version`, `POST /containers/create`, `POST …/start` (304), `GET …/json` | works | works |
| `get_container_host_ip` | none | `localhost` | `localhost` |
| `get_exposed_port(80)` | `GET …/json`, polled | **fails** after polling: «Port mapping … not available». An empty `HostPort` was dropped, the port never published, and the inspect had no `NetworkSettings.Ports` | `45821`, published on loopback |
| `wait_for_logs`, `get_logs` | `GET …/json` then `GET …/logs` | **fails** in the client: `KeyError: 'Tty'` (docker-py reads `Config.Tty` before every logs call) | **fails** with 404: the logs route is not written |
| `exec` | `POST …/exec` | 404, refused by design (needs a hijacked connection) | 404, same |
| `stop` | `DELETE /containers/<id>?v=True&link=False&force=True` | **409 «use --force»**: `force=True` was read as false, and the container stayed running after the test | forced; 504 (DX-8101) while the host's writeback held the exit, then 204 |

Fixed here, each with a unit test that fails with the fix reverted, and three
checks added to `tests/compat/docker_api_smoke.py` (the base binary fails all
three, the fixed one passes 17/17):

- **Query booleans** are read the way the daemon reads them (`BoolValue`):
  case-insensitive, false only for `""`, `0`, `no`, `false`, `none`.
- **An empty `HostPort`** is published on a free host port chosen at create
  time, as the compose translator does for `ports: ["80"]`. A non-empty
  `HostIp` is kept; an empty one keeps the engine's loopback default.
- **The inspect** carries `Config.Tty` (always `false`: no container here has a
  TTY) and `NetworkSettings.Ports` in the shape `docker port` reads.

Still not working, and none of it is small:

- **`GET /containers/{id}/logs`.** `wait_for_logs` is Testcontainers' most
  common wait strategy, so this is the gap that stops most suites. The route
  has to frame stdout and stderr separately (the multiplexed stream of a
  non-TTY container), and a detached container's log file here keeps them
  mixed unless it was started with `--log-cri`. That is a design decision about
  the log format, not a missing handler.
- **`POST /containers/{id}/exec`.** Refused by design: it needs a hijacked
  connection (see the module doc).
- **`v=True` on remove** (anonymous volumes) is not read: the engine's `rm`
  has no separate «with its anonymous volumes» mode.
- **Ryuk** (the reaper sidecar, enabled by default) was not tried. It needs
  the Docker socket mounted inside a container, which this layer does not map.

## 9. Testcontainers runs end to end now (2026-10-27)

The three "still not working" routes above are served, and the **real
testcontainers-python flow is green against `delonix serve docker-api`** —
driven by the same docker-py 7.2.0, with Ryuk disabled:

| testcontainers call | route added | result |
|---|---|---|
| `DockerContainer(...).start()` | `GET /images/{name}/json` (image inspect — docker-py calls it before create; its 404 was the next wall after the §8 fixes) | green |
| `get_exposed_port(80)` | (§8) | green — a host port |
| `wait_for_logs("ready")` | `GET /containers/{id}/logs` — Docker stdcopy framing (stdout/stderr split under `--log-cri`, else one stdout stream), follow/tail/since/timestamps | green |
| `exec(["echo","hi"])` | `POST /containers/{id}/exec` → `POST /exec/{id}/start` (101 hijack, stdcopy-framed, re-exec of `container exec`) → `GET /exec/{id}/json` (ExitCode) | green, exit 0 |
| `get_logs()` | `GET …/logs` | green |
| `stop()` | (§8) | green |

Containers created over this API now log in CRI format by default, so the logs
route frames stdout and stderr on SEPARATE streams: a client asking for only
stderr gets only stderr (proven — see below).

Checked two ways: `tests/compat/docker_api_smoke.py` (extended with logs, exec
and the stdout/stderr-split assertions — the base binary 404s or mixes them)
passes, and a testcontainers harness exercising the list above passes every
call.

Added since, so the Docker-API layer is fuller (each proven live, in
`docker_api_smoke.py` and by hand):

- **`GET /containers/{id}/stats`.** A one-shot sample (`?stream=false`) in
  Docker's shape — cpu/memory from the container's cgroup, read twice ~100 ms
  apart so `precpu_stats`/`cpu_stats` yield a real CPU %. Used by Dev Containers
  and GitLab Runner. (A streaming request gets one sample; live rates are what
  `/metrics` is for.)
- **Anonymous volumes (`Config.Volumes` + `rm ?v=1`).** The create maps each
  source-less target to a deterministic `dlxanon-<name>-…` volume (so the
  container gets real isolated storage there), and remove with `v=1` deletes
  them by that prefix — Docker's auto-create/auto-remove lifecycle, in the
  compat layer. The native `-v` CLI still refuses a source-less spec by design
  (DX-1209); this is Docker-API fidelity only.

- **Label filtering on `GET /containers/json`.** `?filters={"label":[…]}` is
  honoured server-side now (`label` ANDed; `status`/`name`/`id` ORed; unknown
  keys ignored). This is the reaping primitive — list by label, remove what
  comes back.

### Ryuk runs on delonix — and safer than on Docker

An earlier draft of this section said Ryuk could not run here. That was wrong,
and `tests/compat/ryuk_reap.py` proves it: Testcontainers with Ryuk **enabled**
pulls and runs the real `testcontainers/ryuk` image, binds the control socket
into it, and Ryuk reaps the session's labelled resources — measured end to end,
including an orphan from a crashed session reaped in ~4 s, after which Ryuk
removes itself too.

Three properties make it work, none of them a hole:

- **The socket accepts Ryuk because Ryuk IS the engine.** `SO_PEERCRED` still
  admits only the engine's own uid. A rootless container's `root` (uid 0) is
  mapped to the engine's uid on the host (`newuidmap`: container 0 → our euid),
  so Ryuk — root in its image — is recognised as the engine itself. A process
  belonging to any *other* user is still refused. The `access.transport-verified`
  boundary holds.
- **Safer than Docker, by construction.** On Docker the daemon is root, so a
  container with the socket bound in is host-root — the canonical escape. On
  delonix the socket peer is the UNPRIVILEGED engine user (euid 1000 here, not
  0): a socket-bearing container can do only what that rootless user can, never
  touch host root. The most dangerous bind mount in the container world stops
  being a privilege escalation.
- **Reaping is label-scoped, so it is exact.** Ryuk removes by
  `label=org.testcontainers=…`, and the engine honours that filter server-side
  (above) — so it reaps the session's resources and nothing else. Without the
  server-side filter Ryuk would have had to trust a client-side list; with it,
  the engine is the one that scopes the blast radius.

So Testcontainers runs with its defaults — no `TESTCONTAINERS_RYUK_DISABLED`
needed — and the reaper is less dangerous here than on the daemon it was written
for.
