# 57 — P4/D5: `LaunchSpec` transport spike (ADR-0044)

**Date:** 2026-09-18 · **Base:** `origin/main` (`d16b2f69`, v4.0.0 +7 commits) ·
**Verdict: the fd wins — it closes a real window that the file cannot close by design,
not by oversight.** Contrary to what ADR-0044 had assumed as the most likely hypothesis
("an fd needs to earn its added complexity against that baseline").

## The question

D5 leaves open whether an inherited fd (`memfd_create`/pipe, passed through a `fork`)
closes a real window that the already-existing precedent — a `0600` file +
`O_CREAT|O_EXCL` + unique name + `unlink` on both sides, which `__apirun` already uses
(`delonix-cri::write_run_spec`, `cmd/dockerapi.rs`) — does not close. If it does not,
D5 stays shaped like the file, generalised.

## How it was measured

A standalone harness (`harness.rs`, with no dependency on any `delonix-runtime` crate —
only `std` + `libc`, which is already a workspace dependency via
`delonix-linux`/`delonix-vm`/`delonix-sdn`/`delonix-state`, so zero new supply-chain
surface), with four verbs: `write-file`/`read-file` (replicates `write_run_spec`'s
pattern byte for byte: `0700` directory, `0600` file with `create_new`, unique name,
`unlink` on both sides) and `write-fd`/`read-fd` (`memfd_create` **without**
`MFD_CLOEXEC`, offset reset to zero before the `fork`, the fd number passed as a string
argument — never the content, never a name to look up on `PATH`).

1. **Correctness**: one pair of each, confirming the fd survives a REAL `execve` into a
   different binary (not just a `fork` within the same process) — the point D5 requires
   and that P1b had never measured for this mechanism.
2. **Concurrency**: 50 pairs in parallel of each transport, each with a distinct payload
   and its own check (an FNV-1a `checksum` compared by the WRITER against what the
   reader returned) — with no reliance on a human reading the log.
3. **Crash window**: the writer dies (via `--die-after-write`) right after writing,
   before spawning the reader and before its own defensive `unlink` — the same failure
   class an OOM-kill or an external `SIGKILL` inflicts on any process.
4. **Structural proof**: `strace -f -e trace=openat,open,unlink,unlinkat` over both
   paths — not "by construction it shouldn't touch disk", but the real system calls.

## What was measured

| | FILE (precedent) | FD (`memfd_create`) |
|---|---|---|
| Correctness, 1 pair | OK, 21 bytes | OK, 19 bytes, survives a real `execve` |
| Concurrency, 50 pairs | **50/50 ok, 0 cross-talk, 0 leftovers** (`concurrency-50.txt`) | **50/50 ok, 0 cross-talk** |
| Writer dies right after writing | **A 32-byte secret stays on disk, readable by its owner, indefinitely** (`crash-window.txt`) | **Zero trace on disk** — confirmed by a scan AND by `strace` |
| Real `openat`/`unlink` (`*.strace`) | `openat(..., O_WRONLY\|O_CREAT\|O_EXCL\|O_CLOEXEC, 0600)` + `unlink(...)` × 2 (the reader, then the writer with `ENOENT` — the same double race the real code's `let _ =` already tolerates) | **Zero `openat`/`unlink` on any payload path** — only `/proc/self/maps` and `/dev/null` (the Rust runtime's own bookkeeping) |

## Interpretation

**The file precedent already closes the risk class ADR-0044 itself had named** ("a spec
readable by another user" — `chmod` TOCTOU, symlink attack): `create_new` refuses a
pre-existing path, the `0600` is atomic at creation, the `0700` directory even prevents
listing. That is correct and does not change.

**But there is a SECOND risk class that no way of "writing a file well" can close**:
between the write and the `unlink`, the secret exists on disk. If EITHER side dies before
running its own `unlink` — an OOM-kill, an external `SIGKILL`, a crash of the engine
itself — the file is left orphaned, with the secret inside, and **nothing in the engine
today sweeps this up** (there is no reaper for `cri/run/*.json` the way there is for IPAM
leases or reference markers — see `AGENTS.md`, "IPAM leaks"). It is not a bug in
`write_run_spec`; it is a property of any transport that writes the secret to a path
before delivering it. Measured, not hypothetical: the simulation above reproduces exactly
this death, and the secret is left there.

A `memfd` has no such class: there is no `unlink` to forget, because there is no `open()`
to make, because there is no path. When the last fd pointing at it closes — for any
reason, including the process's death — the kernel frees the memory. There is no reaper
to write because there is nothing to sweep.

**Cost, measured and not hypothetical**: a call to `libc::memfd_create` (already a
workspace dependency), no new dependency; the fd number still travels as a string
argument — this is not zero argv surface, it is a reduction of it (from "the whole
payload, or a path to find it" to "a small integer, which is not a secret"). SCM_RIGHTS
**was not measured, nor is it necessary for this case**: `delonix-launcher` is always
`fork`+`exec`ed directly by the process that invokes it (CLI/CRI), never receiving an fd
from a process that is not its parent — simple inheritance through `fork` is enough.
SCM_RIGHTS would only matter to a consumer that received fds from non-child processes
over an already-existing socket, which is not this case (that is the
`delonix-netns-holder`'s case, but it does not transport `LaunchSpec`).

## Decision for D5

**D5's code stays as it is** — the ADR's sketch already proposes the fd, not the file;
this spike only confirms it instead of downgrading it. What changes is the certainty: the
original text treated this as "declared intent, not measured" and as possibly needing to
regress to the file form; it becomes **measured, and the file is now documented as the
alternative with a residual window that D5 exists to close**.

## Proven vs not validated

**Proven**: the four rows of the table above, measured on this host, with this harness's
binary (not production code — see the note below) and real `strace`.

**Not validated**: the same harness inside a real `unshare(2)`/userns (the
`fork`+`exec` of `delonix-launcher` happens from inside a process that has already
created its own namespaces — this spike ran in the user's own process, without
namespaces); `SCM_RIGHTS` (deliberately out of scope, see above); behaviour under
AppArmor (left for spike #2, `53_P1B_LAUNCHER_SPIKE` repeated across three binaries —
item 2 of ADR-0044's spike list).

## What happened to the spike code

`harness.rs` stays in this directory as evidence (it does not compile as part of the
workspace — it is a standalone `std`+`libc` crate, run from `/tmp` in this session). There
is no branch to merge or delete: this spike never touched a `delonix-runtime` tree.
