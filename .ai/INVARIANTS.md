# Delonix — invariants (read this before editing)

The minimal set a session needs **before** it touches the tree. It is the
canonical first contact; `AGENTS.md` is the ~10k-line **archive** of decisions
and their why, consulted by section through its index, never end to end. When a
detail here needs its history, follow the pointer into `AGENTS.md`.

Each rule below points at the one file that owns it; this page does not fork a
policy, it routes to it.

## What the engine is (and is not)

A per-node execution layer: it runs **containers and microVMs** and governs the
network and storage they need. Declarative, with its **own Kinds** in published
groups (`delonix api-resources`). It talks to providers (Linux kernel, libvirt,
Cloud Hypervisor, Proxmox, the Kubernetes CRI/kubeadm) only through each one's
management interface, behind ports — a new provider is a port implementation,
never an `if provider == …`. **It knows no consumer**: no platform, control
plane, tenant, quota or billing, in code, comments, deps or `proto/`.
→ `AGENTS.md` «Identidade e fronteira do motor» (canonical), enforced by
`scripts/arch_fitness.py`.

Principles, and what each forbids: **cloud-native** (declarative, API-first,
open-telemetry observable), **daemonless** (no resident process by default; a new
daemon needs an ADR), **rootless-first** — the normal path runs without root;
privilege is explicit opt-in, and rootless-first is **not** rootless-only (a
«needs root» capability is built behind a root mode, never dropped).

## Authority (who may do what)

- **Collaboration acts** — push, open/operate a PR, merge, release — are governed
  by `.ai/agents/authorization.md`. Everything above `commit` needs **explicit**
  owner authorization, per operation **and** target; absent it, deliver the
  command, never run it. No commit carries a `Co-Authored-By: Claude` trailer.
- **Operation risk** against the host/engine (tiers A–D, Read→Privileged) is
  governed by `.ai/agents/permission-tiers.md`, judged by a command's **real
  effect** not its prefix (`scripts/execution_tier_gate.py`); above SafeWrite is
  always a stop-and-ask.

## Flow

`develop`-GitFlow (owner decision 2026-10-23). `feat/chore/refactor/perf/docs/
test/ci/build` cut from `origin/develop`; `fix/bug/hotfix` cut from `origin/main`.
One worktree per task; never edit the shared `main`/`develop` checkout. The
canonical model is `docs/dev/engineering-process.md`, run by `scripts/gitflow.py`.

## Definition of done

Observed, not presumed: `make ci` passes (or the gates it touches, with `rc=0`
quoted); a test fails without the change and passes with it; the docs of the same
behaviour travel with it (`scripts/dev_docs.py --check`); no new `print!`/
`eprintln!` in `crates/` (the `library_prints` ratchet only descends).

## Minimal commands

```bash
make ci        # fmt + script gates + clippy + tests — what CI runs
make build     # the five binaries (release)
make install   # ~/.local/bin + ~/.config/delonix/env.sh
```

## Measurement traps (each has cost a CI round)

- `cargo test` wants an empty, private `TMPDIR`; sharing it with `scripts/test_*.py`
  frames it for a leak that is not its (`scripts/tmp_roots_gate.py`).
- `docs_cli_gate.py` / `cli-tree.sh --gate` resolve against `target/release/delonix`;
  a stale binary **invents** failures — rebuild first.
- Integration is **squash-merge**: use `git cherry develop <branch>`, not
  `git log`, to know whether something already landed.
- A pushed branch runs CI only with a PR; "CI green" always implies a PR.

## Commits, language, scope, WIP

- **Commits**: Conventional Commits, subject in Portuguese, body with **Porquê**
  and **Como validar** (commands and their `rc`).
- **Language (LANG-01)**: identifiers, comments and user-facing text in **English**,
  even among Portuguese neighbours; the `lang_ratchet` counts.
- **Scope**: a session opened here works **only** on `delonix-runtime`; what
  belongs to another repo is reported, not fixed from here.
- **WIP cap**: at most **3** open worktrees/sessions; a worktree isolates the
  code, not the one shared Delonix engine, nor ports/volumes/state.
- `.claude/` and `CLAUDE.md` are gitignored — this repository is public.

## The archive

Everything else — the ~124 sections of decisions, CLI surface, Kinds, VM/cluster,
security audits, templates, releases — lives in `AGENTS.md`, navigable through its
generated index (`<!-- agents-toc:begin -->`). Read the section you need; do not
read it end to end. The agent and skill specs live in `.ai/agents/` and
`.ai/skills/` (projected to `.claude/` for clients).
