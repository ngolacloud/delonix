# ADR-0027 — pidfd for killing `exec`/`attach` children

**Status:** accepted · **Date:** 2026-08-29 · **Scope:** `delonix-cri`

## Context

The CRI server spawns a `delonix` child per `exec`/`attach` session and kills it
when the client disappears — otherwise an abandoned interactive shell runs
forever, and with it the pty, the netns fds, and the references to the
container's namespaces stay stuck. That fix already existed; what it used was
what was wrong.

The killer stored the **PID number**. A number is not a durable name for a
process. While the child is a zombie the slot is held and the number cannot be
reassigned — but whoever waits for it (`child.wait()`, on a detached thread)
**reaps it**, and reaping is exactly what frees the number for reuse.
From that point on, `kill(pid)` names whatever process the kernel has since given
that number.

Three places had the window:

| Place | Window |
|---|---|
| `spdy.rs` `Input::close` | `Input` stored a raw `pid: u32`; the `Child` had already been consumed by the reaping thread |
| `streaming.rs` `exec_tty` | the thread reaps and only then sends the code through a `oneshot`; the `exit_code.is_none()` check still says "hasn't exited" with the PID already free |
| `streaming.rs` `exec_pipe` | same |

The comment that was there claimed the opposite — that because the thread had
already reaped it, the signal would be "a harmless ESRCH, not a signal misdirected
at a reused pid." The reasoning is inverted, which is why no one investigated
it before: whoever reads it finds the guarantee and moves on.

Low probability — `pid_max` is 4,194,304 on a modern kernel, so the counter
needs to wrap around. But the signal is `SIGKILL`, the node runs client
workloads, and the victim would be someone else's work, with nothing in any log
to tie the fact back to us.

## Decision

A `ChildHandle` (`crates/interfaces/delonix-cri/src/child_handle.rs`) wraps
`pidfd_open(2)` and `pidfd_send_signal(2)`. A pidfd refers to the **process**, not
the number: the signal reaches that process or fails with `ESRCH`, never another one.

The handle is opened **immediately after `spawn`**, before anything
can reap the child. Opening it later is not an error but it is not a guarantee either.

## Rejected alternatives

**Store the `Child` and use `Child::kill`.** The blocking `wait()` has to live
somewhere; sharing the `Child` between whoever waits and whoever kills causes a deadlock — the
waiter holds the lock precisely while it is blocked.

**Swap `wait()` for `try_wait()` in a loop.** Closes the window at the cost of
turning a cheap blocking wait into polling, on a path that exists once
per exec session.

**Kill the process group.** The group identifier recycles for the same
reason the PID does.

**Just document it.** This was considered and rejected: an honest SAFETY note here would have to
say "this can signal a recycled PID," which is better than the false
guarantee but leaves the defect standing.

## Consequences

A floor of **Linux 5.3** for the guarantee (`pidfd_send_signal` landed in 5.1,
`pidfd_open` in 5.3). Not a floor to function: if `pidfd_open` is refused,
`ChildHandle` falls back to the usual `kill(pid)` and inherits the window — same as today,
better everywhere the kernel allows it. `is_process_stable()` reports which of the
two is in use, and the test that proves the property skips itself on the
fallback, rather than asserting something that isn't true there.

The three syscalls were already on the seccomp allow-list
(`delonix-runtime/src/seccomp_profile.rs`), so there is no profile to widen.

The `delonix-runtime`'s `HeldChild` does **not** change: it kills and only then does
`waitpid`, and since it never reaps before that the zombie holds the number. It is correct by the
ORDER of the two calls, and it now has that written down — with a warning that swapping the
lines turns it into this very same defect.
