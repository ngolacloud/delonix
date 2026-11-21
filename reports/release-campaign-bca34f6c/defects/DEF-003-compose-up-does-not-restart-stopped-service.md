# DEF-003 — `compose up` does not reconcile a stopped service back to Running

- **Requirement / test:** IAC-1 / §9 steps 10-11 ("introduce drift; detect and reconcile"). Also §6 docker-compose parity.
- **Severity:** **P2 (medium)** — silent divergence from the documented behaviour and from docker-compose; prevents recovery by reapplying. No data loss; a workaround exists.
- **Type:** product defect (incomplete reconciliation).
- **Environment:** `delonix 5.1.1` @ `bca34f6c3`, rootless.

## Minimal steps

```
# compose.yaml with services web (nginx) and cache (redis)
delonix compose up -d                 # both Running
delonix stop relc-stack-cache         # drift: cache -> Exited (0)
delonix compose up -d                 # "cache: already exists, nothing to do"
delonix compose ps                    # cache STILL Stopped
delonix start relc-stack-cache        # workaround -> cache Running
```

## Expected vs observed

- **Expected:** `compose up` — whose own `--help` says *"Creates/**starts** every service"* and which
  aims for compatibility with `docker compose up` (which restarts stopped containers of the project) —
  should bring the stopped service back to `Running`.
- **Observed:** the existence check short-circuits the start: "already exists, nothing
  to do". The stopped service stays `Stopped` across successive reapplications. Confirmed over 2
  consecutive `up -d` invocations.

## Impact

In a daemonless model with no supervisor by default, if a service crashes/stops, **reapplying the
manifest does not recover it** — the operator has to discover and manually start each stopped
service. It also breaks the expectation of anyone bringing `docker compose` scripts across.

## Hypothesised cause

In the `compose up` logic, the "container already exists" branch returns early without comparing/forcing
the desired `running` state. Reconciliation covers creation, not execution.

## Closure criterion

`compose up` starts existing-but-stopped services (matching `docker compose up`).
Re-run the minimal steps: after the 2nd `up -d`, `cache` must be `Running` without a manual `start`.

## Status

OPEN. Reproduction preserved. Not fixed this round (behaviour defect, candidate for a
decision: it may be intentional — in which case `--help` and the parity promise have to change).

## Fix

- `bins/delonix-runtime-bin/src/cmd/compose.rs`: in `up`, an existing container that is NOT running is
  reconciled — `reconcile_status` + a new pure helper `compose_up_should_start(status)` (starts
  `Stopped`/`Failed`/`Crashed`/`Created`; leaves `Running` and `Paused` alone) → `cmd_start`. New
  message `started`/`started, connected to …`.
- Test: `tests::up_restarts_down_services_but_not_running_or_paused`. **PASSES.**
- Live: `up` → stop `cache` (Exited) → `up` again prints `cache: started` and `web: already exists,
  nothing to do`; `compose ps` shows `cache` **Running**.

## Integration note (commit blocker)

The three fixes were built and verified live from `bca34f6c` + patch (binary digest
`a45e71a5…`). The current `develop` (`69c349c3`, merge of #940 «develop-GitFlow») **does not compile** for a
reason UNRELATED to these fixes: `bins/delonix-runtime-bin/src/cmd/prune.rs:370` calls
`delonix_linux::user_service_base` at a visibility the merge left inconsistent (E0603) — this is the
partial integration of the prune branches that the project memory warned had to go in «both together».
So the changes stay in the working tree (local fix requested), **uncommitted**: committing requires
`develop` to compile (owner's fix to #940) and consent for the commit message.

**Commit:** `ee615f3b` (branch `fix/campaign-defects-doctor-rm-compose`, not pushed).
