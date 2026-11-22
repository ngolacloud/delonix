# B1d — `delonix oci-runtime` at root: the OCI capability model (5 sets) + namespace validation

Measured 2026-11-21 on `h3-root-vm` (qemu/KVM, Ubuntu 26.04, cgroup v2 unified,
uid 0, `cap_last_cap=40`), with a **release** `delonix` built from the working
tree (5-set caps + ns validation + the owner's uncommitted seccomp-args work).
Suite: `runtime-tools@v0.9.1-...bd3b0bd`, through
`scripts/oci-runtime-rt4-wrapper.sh`.

## Result

| binary | ok | not ok |
|---|---|---|
| delonix 0.1.2 (B1c, BEFORE — ambient guard) | **19** | 31 |
| delonix 0.1.2 (THIS run — 5 sets + ambient raise + ns path/type) | **21** | 58 |
| runc 1.5.3 (reference, B1c) | 53 | 53 |

(The `not ok` total rises because more cases now get as far as CREATING the
container and emitting lifecycle assertions that previously did not even run —
the number is not monotonic, as documented.)

## What changed and why (+2, no regression)

All 19 of the original ones hold: `linux_ns_path_type` (14 assertions: 7
negative ones for refusing a path of the wrong type + 7 "matches") is still
14/14 — preserved by the new rule "any namespace with a `path` is refused" (the
engine creates fresh namespaces, it does not setns by path), plus the 5 "operation
MUST error if no container ID" cases.

The **+2** come straight from the ambient guard no longer blocking `create`:

- `kill_no_effect` — creates a container and kills it before `start`; all it
  needed was for `create` to pass (the default config carries ambient).
- `misc_props` — creates a container with an unknown annotation and requires
  that there be NO error; likewise, all it needed was `create`.

Both used to fail at `create` with `error[DX-1000] … process.capabilities.ambient
— ambient capabilities are not raised by this engine`.

## The 5-set model works end to end (independent proof)

The suite cannot observe `runtimetest`'s ~205 capability assertions because
`oci-runtime start` runs the container **detached** (with a log of its own)
instead of wiring stdout to `create`'s fds (runc's inherited-fd model);
`runtimetest`'s TAP never makes it back to `prove` → "No subtests run". That is a
**stdio plumbing** gap, ORTHOGONAL to capabilities/namespaces.

Direct proof, a `delonix oci-runtime` container at root with all five sets equal
to `{CHOWN, KILL, NET_RAW}` (mask `0x2021`), ambient included, reading
`/proc/self/status` from its own init:

```
CapInh: 0000000000002021
CapPrm: 0000000000002021
CapEff: 0000000000002021
CapBnd: 0000000000002021
CapAmb: 0000000000002021   ← ambient RAISED (previously 0 / refused)
```

The five sets install exactly as the bundle asks, and ambient is raised through
`PR_CAP_AMBIENT_RAISE` (precondition `ambient ⊆ permitted ∩ inheritable`,
validated in `create`). It is precisely what
`runtimetest.validateCapabilities` would check (5 × 41 assertions).

## The next lever towards 53 (a new session)

The dominant blocker for the ~30 `RuntimeInsideValidate` cases
(`default`/`process`/`hostname`/`linux_readonly_paths`/`mounts`/…) is the stdout
plumbing of the `create`+`start` pair: the container has to inherit `create`'s
stdio fds (the `stdout-<uuid>`/`stderr-<uuid>` files in the bundle, read by
`ReadStandardStreams`). Beyond that, `process_capabilities` fails upstream with
`config.json: serde failed` (the `oci-spec` crate does not deserialize the
privileged list that `SetupPrivileged(true)` generates) — a gap in the crate, not
in the engine.
