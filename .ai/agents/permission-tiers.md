# Permission tiers — the harness permission contract

Shared reference for every agent in `.ai/agents/`. It does not invent a risk
vocabulary: it adopts the one the engine already ships in
`crates/interfaces/delonix-mcp/src/risk.rs`, so an agent and the MCP surface
reason about risk the same way.

## The engine's own vocabulary (source of truth)

`risk.rs` defines, in ascending order:

```
Read  <  SafeWrite  <  Disruptive  <  Destructive  <  Privileged
```

Two rules there are load-bearing for this harness and MUST be mirrored by every
agent:

1. **`requires_confirm()`** — anything above `SafeWrite` (i.e. Disruptive,
   Destructive, Privileged) must not execute without explicit human
   confirmation.
2. **`risk_of()` fails closed** — an UNKNOWN tool/verb maps to `Privileged`,
   never to `Read`. An agent that cannot prove an operation is safe treats it as
   privileged.

The enumerated `TOOL_RISK` table in `risk.rs` tags the MCP tools (15 entries,
e.g. `resource.list`→Read, `container.restart`→Disruptive, `task.cancel`→
SafeWrite). The ~130 CLI verbs are NOT individually tagged in code; agents
classify them by the evidence below, defaulting to the higher tier when unsure.

## Agent tiers (A–D)

| Tier | Maps to `risk.rs` | What it covers | Evidence (crate:symbol) | Agent rule |
|---|---|---|---|---|
| **A — Read-only** | `Read` | ls/describe/get, metrics, logs, scan, parse manifests, policy score | `delonix-mcp/src/risk.rs` (Read rows); `delonix-security` (pure, "no privileges"); foundation `delonix-net-rules` | Auto-allow; no state mutation |
| **B — Controlled (rootless)** | `SafeWrite` | rootless image build, rootless container run, rootless microVM (Cloud Hypervisor), build/test gates | `delonix-oci/src/build.rs`; `delonix-state/src/store.rs` (geteuid!=0 → `~/.local/share/delonix`); `delonix-vm/src/lib.rs`; `delonix-sdn/src/infra.rs` | Allow in a confined workspace; never as root |
| **C — Human approval** | `Disruptive` / `Destructive` | apply/delete, `prune`, delete volume/snapshot, `policy apply`, **release/publish**, **`cluster apply`/`kubeadm` over ssh** | `risk.rs` (Disruptive/Destructive); `bins/delonix/src/cmd/cluster.rs` (`ssh` → `bash -c` via `format!`); SECURITY.md (command-injection in scope) | Propose only; require explicit human confirm; never auto |
| **D — Privileged / root** | `Privileged` | mount/unshare/setns, seccomp, capabilities, nft/ip/netlink, KVM/Cloud-Hypervisor/libvirt, VFIO passthrough, LVM/ZFS pool create, `--wipe-devices`, kubeadm | `delonix-linux/src/{lib.rs,seccomp_profile.rs,capabilities.rs}`; `delonix-sdn/src/{policy_nft,lan,cni,wg,pin_userns}.rs`; providers CH/libvirt; `bins/delonix-storage-helper/src/main.rs` | **NEVER an unrestricted shell or root.** Only via the typed `delonix-pool-helper` contract, SO_PEERCRED-gated |

## MUST-NEVER (applies to every agent, regardless of tier)

1. No unrestricted `sudo` / root shell. Root is deliberately isolated to
   `bins/delonix-storage-helper` behind a typed socket; bypassing it defeats the
   design.
2. Never run `storage-helper create-pool/destroy-pool … --wipe-devices` — it is
   administrator-at-a-terminal only and destroys devices.
3. Never set fail-open flags: `DELONIX_ALLOW_UNENFORCED_*`,
   `DELONIX_*_INSECURE_TLS`, `DELONIX_INSECURE_REGISTRIES`,
   `DELONIX_POLICY_LINT=0`, `DELONIX_CRI_CAP_CEILING*`.
4. Never rename/move/delete state dirs, sockets (`/run/delonix-*.sock`),
   services, `DELONIX_*` vars, or `*.delonix.io` groups (breaks crash-recovery
   and persisted-state invariants; prohibited by the engineer contract).
5. Never edit `$DELONIX_ROOT/policy.json`, `/secrets` (vault) or `/events.jsonl`
   directly — mutate only through the engine's own commands, or you bypass
   admission and the audit trail.
6. Treat `cmd/cluster.rs` ssh → `bash -c` as a command-injection surface: no
   agent-supplied strings flow into it.

## How an agent declares its tier

Each spec in `.ai/agents/` names its **maximum tier** in the body. An agent may
operate at its tier and below. Crossing above its tier is a stop-and-ask, not a
judgement call.

## Tier by real capability, not by prefix

A command's tier is its **effect**, not the word it starts with. `git log` is
Read but `git push` is a remote effect; `cargo tree` is Read but `cargo build`
is SafeWrite; a `bash -c "…"`, an `xargs …`, a `find … -exec` or an `env VAR=x …`
is judged by the command it actually runs, not by the wrapper. Operations that
touch **KVM/Cloud-Hypervisor, a device, a mount/namespace, netfilter, or a
provider credential** are Privileged by their effect even when the verb looks
benign, and an **unknown** verb is Privileged — never Read (fail closed, mirroring
`risk_of`). Anything above SafeWrite is a stop-and-ask, never an automatic run
(mirroring `requires_confirm`).

This is made checkable: `scripts/execution_tier_gate.py` classifies a command
string against a tier and returns ALLOW/REFUSE, decomposing the wrappers above
and failing closed. It is pure logic — it never executes the command — with
benign fixtures in `scripts/execution_tier_fixtures/` and
`scripts/test_execution_tier_gate.py`.

## Read profiles use dedicated tools, not a generic shell

A Tier-A read profile (auditor, security-reviewer) does its file search and
reading through the **dedicated tools** the client provides (Read / Grep / Glob),
not a generic shell. `Bash` stays only for what has no dedicated tool —
`git log/show/diff/status`, `cargo tree`, reading `--help` from an already-built
binary — and it never runs a mutating or privileged verb. A read profile that
reaches for `bash -c`/`xargs`/`find -exec` to do what Grep/Glob/Read already do
is working below its own contract.

## What the executor imposes, and what is only declared (BLOCKED)

This harness **classifies and advises**; it is not, by itself, the hard boundary.
The load-bearing limits — an **unprivileged user**, a **per-task workspace and
network**, **no access to provider credentials**, **no elevation**, and **no
indirect effect via a subprocess** — are imposed by the *executor* that runs the
agent: the client's permission mode, its hooks, and the sandbox / cgroup /
namespace confinement around it. **This repository does not control that
executor.** Where the executor cannot impose a limit, the control downgrades to
stop-and-ask, and `execution_tier_gate.py` is the verifiable check that makes the
intended decision explicit and testable. Treat a REFUSE from the gate as «the
tier says no»; it is the executor that guarantees it. Record this gap as
**BLOCKED** wherever a profile's safety depends on a limit the executor does not
enforce — do not report it as imposed.
