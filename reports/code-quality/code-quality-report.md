# Code quality review — Delonix Runtime

Skill: `delonix-code-quality-architecture`
Repository: `delonix-runtime`
Measured commit: `cebf895` (= `origin/main`, 0 commits behind)
Version: `1.0.0`
Date: 2026-08-29

## Measured inventory

| Metric | Value |
|---|---|
| Crates in the workspace | 14 (+ `delonix-runtime-bin`) |
| `.rs` files tracked by git | 147 |
| Lines of Rust | ~138,500 |
| `pub` items in production analysed | 1,190 (161 struct, 68 enum, 2 trait, 848 fn, 111 mod) |
| Identifiers flagged as PT by the gate | 1,039 |
| Comments flagged as PT by the gate | 3,453 |

## Automatic gates — what is GREEN, run now

| Gate | Command | Result |
|---|---|---|
| Formatting (§89) | `cargo fmt --all --check` | **GREEN**, rc=0 |
| Clippy (§87, §88) | `cargo clippy --workspace --all-targets` | **GREEN**, zero warnings |
| Language gate (ratchet) | `python3 scripts/lang_ratchet.py` | **aligned**, rc=0 |

Reading caveat: the language gate is a **ratchet**, not a ceiling. "Aligned"
means "the debt did not go up", it does **not** mean "it meets Golden Rule §1".

## Scoreboard

| Dimension | Score | Why |
|---|---|---|
| English conformance (§1–§2) | **82/100** | Zero public identifiers in PT; 54 local `let` + 985 test names + 3,453 comments remain |
| Naming quality (§4–§11) | **90/100** | Rust conventions respected (clippy green); discount for local PT names and for `util.rs` |
| Documentation (§18–§23, §68) | **86/100** | 13/13 crates with `//!`; structs 87.6 %, fn 85.1 %; enums 58.8 % — but see DOC-0001, WITHDRAWN: 27 of 28 are clap subcommands with documented variants |
| Architecture (§26–§39) | **71/100** | Only **3 traits** in 138k lines; `VmBackend` is a real port, but the rest is concrete modules |
| Error handling (§23–§24, §51) | **74/100** | 45 `.map_err(\|_\| …)` losing the cause; 12 `Result<_, String>`, 3 on the security path |
| Agent readability (§84) | **88/100** | `ARCHITECTURE.md` + `AGENTS.md` exist and are **tested** (`tests/architecture.rs`) |

## Findings by severity

| Severity | Count | Already closed |
|---|---|---|
| CRITICAL | 0 | — |
| HIGH | 2 | **1** (ARCH-0002, PR #170) |
| MEDIUM | 6 | **1** (QUALITY-0001, PR #170) |
| LOW | 6 | — |
| INFO | 3 | — |

Two findings were fixed during this audit, in PR
[#170](https://github.com/angolardevops/delonix-runtime/pull/170):
the 37 positional arguments of `container_init` moved to a grouped type,
and three `#[allow(clippy::too_many_arguments)]` that silenced the wrong function
were removed. The rest is in `refactoring-backlog.md`, prioritised.

Detail in `architecture-report.md`, `naming-report.md`, `documentation-report.md`
and `refactoring-backlog.md`.

## What was NOT validated (the second half of the report)

- **No tests were run.** Coverage was not measured, nor was anything validated live.
- Documentation coverage was measured by our own syntactic analysis
  (`///` immediately above the item), **not** by `cargo doc`. Items documented
  with `/** */` count as not documented.
- The production-vs-test classification of identifiers uses brace counting
  from `#[cfg(test)]`; files with macros that open blocks may
  misalign (no cases were found, but it was not proven exhaustively).
- **No security review was done.** The `unsafe` findings are about
  *documentation* (§17), not correctness.
- The other repos in the workspace (`delonix-paas`, `delonix-portal`, `delonix-admin`,
  …) were **not analysed**. This report is about the engine only.
