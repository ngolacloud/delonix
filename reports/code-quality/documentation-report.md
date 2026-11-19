# Documentation report — Delonix Runtime (`cebf895`)

## Measured coverage (§81)

Only `pub` items outside test modules. "Documented" = `///` immediately
above (skipping attributes and blank lines).

| Type | Total | Documented | Coverage |
|---|---|---|---|
| `trait` | 2 | 2 | **100.0 %** |
| `struct` | 161 | 141 | **87.6 %** |
| `fn` | 848 | 722 | **85.1 %** |
| `enum` | 68 | 40 | **58.8 %** |
| `mod` | 111 | 1 | 0.9 % |

Crate-level `//!` (§68): **13 of 13** crates with a `lib.rs` have crate
documentation in the first 3 lines.

## DOC-0001 — ~~Enums at 58.8 % is the real gap~~ — **WITHDRAWN** (was MEDIUM)

**Fixed on 2026-08-29, while executing the item.** The number was right and the
reading was wrong.

Of the 28 public enums without docs, **27 are clap `*Cmd` enums** — `NamespaceCmd`,
`VmCmd`, `StackCmd`, `SchemaCmd`, … — and **one** is a domain enum (`Decision`, in
`crates/delonix-runtime-bin/src/cmd/backup.rs`). The `*Cmd` enums have **every
variant documented**, because each `///` is the text that shows up in `--help`:

```rust
#[derive(Subcommand)]
pub enum NamespaceCmd {
    /// List the isolation namespaces IN USE, with what is in each.
    Ls { … },
    /// What is inside ONE namespace, by Kind, `kubectl describe` style.
    Describe { … },
}
```

A `///` on the enum above this adds nothing for anyone: it is exactly the
percentage-chasing that DOC-0002, two sections down, says not to do.
The two sections were written in the same pass and contradicted each other.

The sentence "a public enum is where you declare the semantic meaning of a
domain state" is true in general and **false for this sample**: the 58.8 %
coverage is dominated by CLI plumbing.

**What remains:** document `Decision` — the only domain enum in the list.
The method lesson also remains: coverage by category does not say what the
category CONTAINS, and in this repo `enum` means "subcommand" 27 times out of 28.

## DOC-0002 — 110 `pub mod` without docs — INFO, do not fix for the sake of it

67 of the 110 are in `crates/delonix-runtime-bin/src/cmd/mod.rs` and are
`pub mod x;` declaration lines. Putting `/// Module for X.` above each one raises
the percentage and adds nothing — it is exactly what §81 warns against ("do not
optimize purely for percentage"). Recommendation: **ignore this scoreboard
line**; document a module only when the name is not enough.

If an honest number is wanted, the coverage of items that matter
(struct+enum+trait+fn) is **1,079 of 1,079 analysed → 905 documented = 83.9 %**.

## DOC-0003 — 58 `unsafe` blocks without a SAFETY comment — MEDIUM (§17)

Method: `unsafe {` without the word `SAFETY` in the 6 preceding lines.

Concentrated in `delonix-cri/src/spdy.rs`, `delonix-cri/src/streaming.rs` and
`delonix-runtime-bin/src/cmd/container.rs`, and almost all are one-line FFI:

```rust
unsafe { libc::close(slave) };
unsafe { libc::kill(pid as i32, libc::SIGKILL) };
```

Judgement: §17 says "every **non-trivial** `unsafe` block". `libc::close(fd)`
is trivial and does not deserve six lines of justification. But `libc::kill(pid, …)`
**is not trivial** — it depends on the invariant that the PID still belongs to the
process you believe you are killing (otherwise you kill a recycled PID), and that
invariant is written down nowhere.

Refined recommendation: require SAFETY on `kill`/`read`/`write` over captured fds
and PIDs; leave the local-fd `close` calls alone. Positive note: where the repo
writes SAFETY, it writes it well — `delonix-runtime/src/lib.rs:2590` documents the
userns pipe handshake in three clear lines.

## DOC-0004 — Comment quality — **PASSES, and above average** (§12)

Sampled in `delonix-vm/src/lib.rs`, `delonix-net/src/infra.rs` and
`delonix-runtime/src/lib.rs`: the comments explain **why**, with the measured
cost alongside, which is what §12 asks for and is rarely seen.

```rust
// Measured, and it cost hours: every Proxmox appliance image (the vendor's
```

```rust
/// It was private, so `delonix-proxmox` grew its own copy — and the copy did
/// 2 GiB on libvirt and Cloud Hypervisor and 1 GiB on Proxmox, silently. Same
```

Zero comments that merely repeat syntax found in the sample.

## DOC-0005 — 3,453 comments in Portuguese — LOW (§13)

Against §13 ("all technical comments must be written in English"), it is the
largest debt by volume in the entire repository. Against practical value: they
are **good** comments — they explain why, with evidence — just in the wrong
language.

The real risk is not stylistic: it is that a code agent working in English
reads 3,453 invariant explanations in a language it may not weigh well, and
`AGENTS.md` does not warn it of this. Low-cost recommendation: one line in
`AGENTS.md` saying that historical comments are in PT and that **new ones are
written in English** — which is what the ratchet already enforces in practice.
