# Architecture report — Delonix Runtime (`cebf895`)

## ARCH-0001 — Three traits in 138,500 lines — HIGH (§26, §38, §39)

Measured: the whole workspace declares **3 traits**, of which 2 are public.

```
crates/delonix-vm/src/lib.rs:601        pub trait VmBackend        (16 methods)
crates/delonix-runtime-bin/.../workload.rs:244  pub trait ComputeDriver (5 methods)
crates/delonix-runtime-bin/src/cmd/mapped.rs:690    trait RtIno      (private)
```

An honest reading, and it is twofold:

**The good part.** `VmBackend` is a real port, with real adapters:

```
VmBackend  ←  CloudHypervisorBackend   (delonix-vm)
           ←  LibvirtBackend           (delonix-vm)
           ←  ProxmoxBackend           (delonix-proxmox)
           ←  FakeBackend/Counting/…   (tests)
```

And the dependency direction is **right**: `delonix-proxmox` depends on
`delonix-vm` (adapter → port), never the other way round. This is correct
Ports & Adapters and passes §38 in the VM domain. `ComputeDriver` repeats the
pattern for container-vs-VM, with `FakeDriver` in the tests.

**The unsaid part.** Outside these two domains there is no abstraction at all.
Network (14,740 lines), image (5,724), CRI (5,771), volume (1,890), scan and
security are concrete modules of free functions. There is no
`NetworkProvider`, `StorageProvider`, `ImageProvider`. The closing question from
`ngolacloud-arch` — «if Proxmox is replaced tomorrow, do only the providers
change?» — has, in the engine, the answer: **the VM backend changes and nothing
else needs to; but swapping the network or storage plane is a rewrite, not an
adapter replacement.**

Recommendation: do **not** create speculative traits (§36 — YAGNI). The network
port is only justified once a second real packet path exists; today there is
one. What should be done now is to record this in writing as a known boundary,
not to rediscover it six months from now.

## ARCH-0002 — `container_init` with 37 positional parameters — HIGH (§49)

`crates/delonix-runtime/src/lib.rs:2546`

37 positional parameters, with `#[allow(clippy::too_many_arguments)]`. Among
them, adjacent `bool`s interchangeable in the compiler's eyes:
`read_only`, `seccomp_unconfined`, `seccomp_detect`, `no_new_privs`,
`has_own_netns`, `host_pid`, `inherit_userns`, `privileged`, `node_cgroup`.
Swapping two in a call compiles and **silently disables a security barrier**.
This is why it rises to HIGH and does not stay at MEDIUM with the other
19 cases of `>8` parameters.

The repo already knows — there is a labelled FIXME above it (§16, well written):

```rust
// FIXME(follow-up): 30 positional arguments — a real smell. Refactor to a typed
// `ContainerInitSpec` (groups rootfs/hostname/argv/limits/flags) in a
// dedicated, reviewed change; do not mix with the lint gate.
```

Two notes: (a) the FIXME's proposal is the right one and stands; (b) **the
number in the comment is wrong — it is 37, not 30** (§14, outdated comment),
which suggests the signature grew after the FIXME was written. It is the proof
that a FIXME without a gate does not stop growth.

## ARCH-0003 — Manifest fields accepted and ignored — MEDIUM

`crates/delonix-runtime-bin/src/cmd/httproute.rs`, `kind: Ingress`:

| Field | Line | Written reason? |
|---|---|---|
| `spec.ingressClassName` | 476 | **yes** — «the embedded proxy is the only ingress class» |
| `spec.rules[].http.paths[].pathType` | 504 | **yes** — «`Exact` is accepted but treated as prefix» |
| `spec.rules[].…backend.service.port.name` | 526 | **yes** — and emits a clear error asking for `port.number` |
| `spec.tls[].hosts` | 591 | **NO** — `#[allow(dead_code)]` without a word |

Three of the four comply with the house rule («a field the client writes and the
system ignores is worse than a field that does not exist») because they *say so*.
The fourth does not: `tls.hosts` is a list of SNI names the user writes and the
engine throws away without warning — it always serves a single certificate (the
first element of `tls`). A user with two hosts in the list gets broken TLS for
the second and is not warned anywhere.

A separate note on `pathType: Exact`: it is documented in the code, but a user
who writes `Exact` still gets **prefix** matching — it routes traffic they did
not ask for. Documenting is not warning; a warning should be emitted in `apply`.

Positive to record: `kind: Ingress` **does** have a list of known fields
(`INGRESS_SPEC_FIELDS`, used by `manifest.rs:422` and `schema.rs:218`), unlike
compose, which per `CLAUDE.md` silently swallows unknown keys for lack of
`deny_unknown_fields`.

## ARCH-0004 — `Container`: 71 fields, 8 `bool`s — MEDIUM (§31, §41)

`crates/delonix-runtime-core/src/lib.rs:528`. It is the god-object of the domain
model: identity, image, network, limits, security, and state in the same type,
serialized to disk. Real mitigations: **it is documented field by field**, and
it is the persisted record — splitting it is state migration (§80), not cosmetic
refactoring. I do not recommend touching it without an ADR.

## ARCH-0005 — Dependency direction — **PASSES** (§90, §91)

Graph declared in the `Cargo.toml` files, verified:

```
delonix-runtime-core  ← everyone (foundation, depends on no one)
delonix-vm            ← delonix-proxmox            (port ← adapter) ✔
delonix-net           ← delonix-runtime, -cri, -bin
delonix-runtime-bin   → all 13                      (the CLI is the top)  ✔
```

No inversion: no domain crate depends on the CLI, no port depends on an adapter,
and `delonix-proxmox` is reached by no one except the binary and the port crate.
The only mention of «Proxmox» inside `delonix-vm` is in **documentation and
error messages** — deliberate, and among the best I have read in this repo:

```rust
/// The distinction is not pedantry. `delonix-proxmox` exists in this workspace
/// and implements the trait — answering `--backend proxmox` with «unknown
/// backend» would be a lie.
```

## ARCH-0006 — Executable architecture tests — **PASSES** (§66, §67, §85)

`crates/delonix-runtime-bin/tests/architecture.rs` verifies that
`ARCHITECTURE.md` and `AGENTS.md` **name every crate that exists on disk** and
that the declared count is the real one. It is the only defence I saw in this
workspace against architecture documentation rotting, and it is exactly what §85
asks for. This is what pushes the agent-readability score up to 88.

## Patterns identified (§27, §95)

| Pattern | Location | Quality | Note |
|---|---|---|---|
| Ports & Adapters / Provider | `delonix-vm::VmBackend` + `delonix-proxmox` | **GOOD** | 3 real adapters, fakes in tests, right direction |
| Strategy | `workload.rs::ComputeDriver` | **GOOD** | container vs VM behind a contract |
| Adapter | `httproute.rs::ingress_to_httproute` | **GOOD** | translates k8s shape → internal model |
| Newtype (§42) | — | **MISSING** | ids pass as raw `String` |
| State machine (§40) | — | **REVIEW** | state by combination of `bool`s in `Container` |
| God Object (§31) | `Container` (71 fields) | **REVIEW** | mitigated by documentation and by being persisted state |

## Anti-patterns searched for and NOT found

- Commented-out code (§54): **0** — 40 raw candidates, all false positives
  (continuations of English prose starting with `for`/`while`/`if`).
- `AbstractXFactory` / Java-in-Rust (§30): **0**.
- Unjustified blanket `#![allow]` (§88): **0**. The two that exist have a written
  reason — `result_large_err` (tonic's `Status` is large by nature) and
  `clippy::all` scoped to the module of protobuf-generated stubs.
- Bare TODO/FIXME (§15): **0** real ones — 3 in the whole repo, 2 labelled, and
  the «third» is the Portuguese word «TODOS» in a comment.
