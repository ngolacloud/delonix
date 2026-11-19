# DEF-001 — `system doctor`/`system resources` advertise the `--pids-limit` flag, which does not exist

- **Requirement / test:** CLI-1/§6 (coherent help), SCOPE-1/§8 (limits). CONTAINER-3.
- **Severity:** **P3 (low)** — documentation/UX defect, not functional. No data loss and no undue refusal.
- **Type:** product defect (incorrect user-facing text).
- **Environment:** `delonix 5.1.1` @ `bca34f6c3`, `target/release/delonix`, rootless.

## Minimal steps

```
$ delonix system doctor
  ✓ ok       cgroup2 delegation (--memory/--cpus/--pids-limit)
```

And the doc-comment of `cmd_doctor` (`bins/delonix-runtime-bin/src/cmd/system.rs:2189`):
> "Without it `-m`/`--cpus`/`--pids-limit` are accepted and ignored"

## Expected vs observed

- **Expected:** the user-facing text only names flags that exist.
- **Observed:** `--pids-limit` is advertised in `system.rs:2189` and `:2232`, but `container run`/`update` **do not have** that flag. It is deliberately the Docker spelling — `bins/delonix-runtime-bin/src/main.rs:1688-1697` has a test (`the_docker_spellings_are_not_ours`) that guarantees `--pids-limit` **never** exists as a flag.

## Hypothesised cause

The comment in `main.rs:1640-1647` identifies the exact class: "they are string literals in another crate, and the compiler has no opinion about the inside of a string." The test `named_flags_all_exist` covers `flags_of_controller`, but **not** the free text of `doctor`/`resources` in `system.rs`. When the flag was removed from `run`, these two literals in `system.rs` were left behind.

## Closure criterion

- Swap `--pids-limit` for `--memory`/`--cpus` (the ones that exist) in the two literals of `system.rs`, OR extend the gate to also scan the text of `system doctor`/`resources` against the set of real flags.
- Re-run `delonix system doctor` and confirm `--pids-limit` no longer appears.

## Status

FIXED (local, working tree, verified live on 2026-10-23). See the «Fix» block below.

## Fix

- `bins/delonix-runtime-bin/src/cmd/system.rs`: introduced 3 testable consts
  (`DELEGATION_CHECK_LABEL`, `DELEGATION_LIMITS_APPLY`, `DELEGATION_LIMITS_INERT`),
  all with `--memory/--cpus` (no `--pids-limit`), used in `doctor` and `resources`;
  also fixed the help of `Setup`, the doc-comment of `cmd_doctor` and the `DELEGATE_BODY`.
- `bins/delonix-runtime-bin/data/pt.po`: `--pids-limit` removed from the corresponding translations.
- Test: `main.rs::advisory_flag_tests::delegation_labels_name_only_real_flags` — asserts that
  the 3 consts contain none of the 4 Docker spellings. **PASSES.**
- Live: `delonix system doctor` → `cgroup2 delegation (--memory/--cpus)`; `grep -c pids-limit`
  in `doctor`+`resources` = **0**.

**Commit:** `e489852b` (branch `fix/campaign-defects-doctor-rm-compose`, not pushed).
