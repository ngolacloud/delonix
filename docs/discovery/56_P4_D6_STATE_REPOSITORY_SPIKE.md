# 56 — P4/D6: `StateRepository<T>` spike (ADR-0044)

**Date:** 2026-09-18 · **Base:** `origin/main` (`d16b2f69`, v4.0.0 +7 commits) ·
**Verdict: GO, with the D6 sketch fixed in four points — and a new finding that
reduces the scope of what D6 had taken for granted.**

## The question

ADR-0044 D6 proposes a `StateRepository<T>` port to close five twin exceptions in
`scripts/arch_fitness.py` (`delonix-linux`, `delonix-vm`, `delonix-sdn`, `delonix-oci`,
`delonix-volume` → `delonix-state`), all with the same written reason: "the adapter opens
its own store directly; P4 hands it a `StateRepository` port from the composition root."
Spike #4, which the ADR requires before "Accepted", asks two things:

1. Can the port be introduced **without changing** the `JsonStore::update`/
   `Store::update` guarantee (lock + re-read under the lock + write)?
2. Are the five exceptions really the same form of debt?

## How it was measured

- A real implementation of the D6 sketch, against the `Store<Container>` that
  `delonix-linux` already uses (see `docs/discovery/56_P4_D6_STATE_REPOSITORY_SPIKE/repository.rs.snapshot`,
  the `crates/adapters/delonix-state/src/repository.rs` file from a spike branch).
- Two concurrency tests, same shape as the existing
  `update_concorrente_nao_perde_escritas`/`jsonstore_update_concorrente_nao_perde_escritas`
  (`store.rs`): 24 threads incrementing a counter on the SAME container through the port,
  each opening its own `Store::open` (mimicking CLI and CRI as two processes), with a
  2 ms pause between the read and the write to force the race window.
- **Verification by reversion** (the same discipline this repo already applies to every
  fix): the real `FileLock::acquire` inside `Store::update` was commented out and the
  "real" test (through the port, with `Store` behind it) was run — if the guarantee truly
  comes from `flock` and not from the trait's shape, the test MUST FAIL without it. The
  `FileLock::acquire` was then restored and the crate's full suite run.

## What was measured

| Run | `state_repository_update_atraves_da_porta_nao_perde_escritas` | `sem_lock_por_baixo_a_mesma_porta_perde_escritas` (control) |
|---|---|---|
| `Store::update` without `flock` (reversion) | **FAILED** — 5 of 24 (`docs/discovery/56_.../without-lock.txt`) | passed (lost writes, as expected from the control) |
| `Store::update` with `flock` restored | passed — 24 of 24 | passed (the control still loses writes, proving the race is real) |

Full `delonix-state` suite with the lock restored: **34/34** (`with-lock.txt`).

**1. GO for question 1** — the guarantee survives the trait: the proposed
`StateRepository<T>` does not need to reinvent anything of the `flock`/read-modify-write,
only to expose it.

**2. The D6 sketch had four shapes that do not survive the first real caller**
(`repository.rs.snapshot`, the module's doc-comment, all confirmed when actually trying
to migrate `delonix-linux::wait_and_record`/`persist_stop`):

1. The sketch's `f` returns `Result<()>`; the real callers use `false` to say
   "this record no longer describes the incarnation I am reporting about — abort,
   and this is NOT an error". Forcing that into `Result<()>` would require an `Err` for a
   healthy no-op — exactly the "dishonest reporting in reverse" that this repo's doctrine
   avoids.
2. `update` has to return the FINAL value (`Result<T>`), not `Result<()>` — `Store::update`
   itself already does this precisely so the caller does not need a 2nd read outside the
   lock, which would reopen the very race window the port exists to close.
3. **A `set`/`save` is missing.** `update` does not serve the creation of the FIRST record
   (it returns `NotFound`) — and `create_with` creates new containers via
   `store.save(...)`. Without `set` the port does not cover the largest use `delonix-linux`
   makes of the store.
4. `impl Trait` in the argument position of a trait method is **not object-safe** — it is
   only callable via a generic bound, never via `dyn StateRepository<T>`. Accepted as a
   sketch compromise (D9/P4a does not need a runtime-swappable `dyn`), but written down for
   whoever one day needs a test fake behind a `dyn`.

The `repository.rs` in the spike branch already implements the port fixed on these four
points, with `impl StateRepository<Container> for Store` and a generic `impl<T>
StateRepository<T> for JsonStore<T>` (verbatim forwarding, no new logic).

**3. New finding, outside what D6 had taken for granted: the five exceptions are NOT the
same form of debt.** Measured by direct reading (not by `arch_fitness.py`'s text, which
generalises the same phrase for all five):

| Crate | What it uses today | Shape of the debt |
|---|---|---|
| `delonix-linux` | `delonix_state::Store<Container>` | Exactly the shape D6 assumes — `Store::update` called directly; migrates to the port without changing mechanism. |
| `delonix-vm` | `delonix_state::JsonStore<Vm>` | Same shape, different type — the generic `impl<T> StateRepository<T> for JsonStore<T>` already covers it. |
| `delonix-sdn` | **Its OWN `flock` lock** in `ipam.rs` (not the `delonix-state` `FileLock` — the file's own comment says so: "`Store`'s `FileLock` degrades the same way, but it at least says so"), plus raw `delonix_state::write_atomic` in `infra.rs` (routes/services) with no read-modify-write at all. | A DIFFERENT debt: it is not "calls the store directly instead of the port", it is "has its own parallel locking mechanism, never connected to `delonix-state`'s". Migrating to `StateRepository<T>` here is not a call swap — it is replacing one lock with another, with IPAM having to decide whether it accepts losing the degradation message its own comment values. |
| `delonix-oci` | `delonix_state::write_atomic_mode`, **once**, for the ECDSA signing key (`sign.rs::ensure_signing_key`) — one-time "if it doesn't exist" creation, no record collection at all. | Nothing here for `StateRepository<T>` to wrap — there is no `T` to be listed, updated, or removed. The `arch_fitness.py` exception for this crate is quoting the generic reason of the other four; this file's real reason is something else (a `chmod` TOCTOU, already closed, not a missing port). |
| `delonix-volume` | Its own `VolumeStore` (`lib.rs`), over raw `delonix_state::write_atomic` — no `FileLock` visible either in the file or in the usages grepped. | Read-modify-write with NO lock at all, which `StateRepository<T>` does not fix by itself — giving it the port without first giving it `flock` would be dressing up the safety debt, not closing it. |

In other words: of the five, **two** (`delonix-linux`, `delonix-vm`) migrate directly via
the fixed sketch. The other **three** each need a separate decision — it is not
mechanical, and treating them as if it were (the reading D6/D9 made before this spike)
underestimates the real P4a.

## What this changes in ADR-0044

- **D6 and spike item #4** can move to "measured" only for the
  `delonix-linux`/`delonix-vm` part (`Store<Container>`/generic `JsonStore<T>`) — proof
  above.
- **`delonix-sdn`/`delonix-oci`/`delonix-volume` need their own sentence in D6**, not the
  same generic line repeated five times — and possibly their own slice in D9 (P4a is
  bigger than "five calls swapped for a trait").
- Neither of the three blocks P4b–P4e (which do not depend on D6) — only
  `arch_fitness.py` removing its own three corresponding exceptions.

## Proven vs not validated

**Proven** (measured in this session, against `origin/main` `d16b2f69`): the
`Store::update` guarantee survives the port and really comes from `flock` (proof by
reversion); the four divergences of the D6 sketch; the real persistence shape of
`delonix-sdn`/`delonix-oci`/`delonix-volume`, by direct code reading, not by
`arch_fitness.py`'s phrase.

**Not validated:** the same concurrency test through `JsonStore<T>` (the generic instance
was only verified by reading — it forwards verbatim to `JsonStore::update`, which already
has its own proof, but never ran through the port); any decision on HOW
`delonix-sdn`/`delonix-oci`/`delonix-volume` adopt (or not) the port — left for whoever
writes the D6/D9 slice that covers them.

## What happened to the spike code

Marked `// SPIKE ... not for merge as-is` in the module itself, in the tradition of
`53_P1B_LAUNCHER_SPIKE` (spike code does not stay on `main` — what stays is this report +
the evidence alongside it). `repository.rs` stays as `repository.rs.snapshot` in this
directory; the `spike/p4-state-repository` branch that produced it is not merged.
