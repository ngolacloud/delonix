# Agent readability — Delonix Runtime (`cebf895`)

**Score: 88/100** (§84)

## What an agent can answer without reverse engineering

| Question (§85) | Answer available? | Where |
|---|---|---|
| What does this crate do? | **yes** | `//!` in 13/13 `lib.rs` |
| Which crates exist and what do they do? | **yes, and it is tested** | `ARCHITECTURE.md` + `AGENTS.md`, verified by `tests/architecture.rs` |
| Which provider abstraction do I use for VMs? | **yes** | `VmBackend`, with 3 adapters and 5 fakes |
| …for network / storage / image? | **no** | no abstraction exists (ARCH-0001) |
| Which decision locked in this design? | **yes** | `docs/adr/`, cited from within the code (`0008-proxmox-vm-backend.md`) |
| Which security invariant applies here? | **partially** | where there is SAFETY it is good; 58 `unsafe` without it (DOC-0003) |
| Which test proves this path? | **yes** | test names describe behaviour (§59), even if in PT |

## What pulls the score up

**The architecture tests.** `crates/delonix-runtime-bin/tests/architecture.rs`
fails the build if `ARCHITECTURE.md` or `AGENTS.md` stop naming a crate that
exists, or if the declared count diverges from the real one. It is the difference
between architecture documentation you believe and documentation you verify, and
it is rare. Without it the score would be ~78.

**Comments that explain why, with the measured cost alongside.** An agent that
reads `delonix-vm/src/lib.rs:267` learns not only what the function does, but that
a private copy of it already caused a silent 2 GiB vs 1 GiB drift between
backends. That prevents the error recurring; a syntax comment does not.

**Error messages that teach.** `unknown_backend("proxmox")` does not say
"unknown backend" — it says that the backend exists, what is missing to configure
it, and in which ADR it is decided.

## What pulls the score down

1. **3,453 comments in Portuguese** without `AGENTS.md` saying so. An agent that
   works in English finds most of the invariant explanations in a language that is
   not the language of the rest of the contract. Cost to fix: one line.
2. **28 public enums without docs** (DOC-0001) — the agent has to reconstruct the
   semantics of each variant from the `match` arms.
3. **Absence of a network/storage port** (ARCH-0001): to the question "where do I
   implement this?" the answer is "inside an 8,859-line file", which is not an
   answer.
4. **`container_init` with 37 parameters** (ARCH-0002): the most important
   signature in the engine is the least legible, and the `bool` args are
   interchangeable as far as the compiler can see.
