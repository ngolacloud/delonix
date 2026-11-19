# Refactoring backlog — Delonix Runtime (`cebf895`)

Each item satisfies §98: it improves correctness, security, maintenance, or
comprehension by agents. None is cosmetic.

---


## ~~P0-1~~ — `ContainerInitSpec`: type the 37 parameters of `container_init` — **DONE**

**ID:** ARCH-0002 · **Severity:** HIGH · **Breaking:** NO (private fn)
**Status:** **MERGED** into `main` — PR [#170](https://github.com/angolardevops/delonix-runtime/pull/170), merge `5e40074`.

Groups created: `ProcessSpec`, `FilesystemSpec`, `SecuritySpec`,
`NamespaceSpec`, `IoSpec`. The whole spec is `Copy` (all fields are borrows
or scalars), so it travels into the `clone` without touching the child's
allocator — a constraint that `main.rs` documents and that a struct with a
`String` would have violated.

**How it was proven to preserve behaviour:** the body was not touched — the spec
is destructured at the top into the same 37 local names (verified that no
parameter was shadowed or reassigned in the body). The field-by-field
correspondence between the old positional call and the new literal was verified
by script: **37/37 equal, zero missing, zero extra**. `cargo test -p
delonix-runtime`: 52 green.

**Discovery made during the work (see P1-5).**

---

**Problem (as it was).** `crates/delonix-runtime/src/lib.rs:2546` took 37
positional parameters, 9 of them adjacent `bool`s interchangeable in the
compiler's eyes (`read_only`, `seccomp_unconfined`, `seccomp_detect`,
`no_new_privs`, `has_own_netns`, `host_pid`, `inherit_userns`, `privileged`,
`node_cgroup`). Swapping two in a call **compiles** and silently disables a
security barrier.

**Recommended design.** What the FIXME in the file itself already proposes: a
`ContainerInitSpec` with sub-structures by area — `rootfs`, `process`,
`limits`, `security`, `namespaces`, `io`. The 9 security `bool`s move into a
`SecurityFlags` where swapping becomes impossible.

**Affected crates:** `delonix-runtime` (and callers in `delonix-runtime-bin`).
**Migration:** mechanical; the fn is private, there is no external contract.
**Tests required:** one test per security flag proving that turning it on changes
observable behaviour — today the silent swap is not detectable by any test.
**Acceptance:** `#[allow(clippy::too_many_arguments)]` leaves the file; no bare
`bool` in the signature.

**Note this pass uncovered:** the FIXME says «30 positional arguments». It is
**37**. The signature grew by 7 parameters *after* someone had written that it
was a smell. A FIXME without a gate does not stop growth — so this item should
ship together with a test or lint that fixes the ceiling.

---

## P1-5 — Three `#[allow]`s that silenced the wrong function — **DONE**

**ID:** QUALITY-0001 · **Severity:** MEDIUM · **Breaking:** NO
**Status:** **MERGED** in the same commit as P0-1 (PR #170).

**Problem.** `delonix-runtime/src/lib.rs` had four
`#[allow(clippy::too_many_arguments)]`. On removing the one on `container_init`,
it turned out that **three of the remaining ones described nothing**:

| Line | Was on | Real parameters |
|---|---|---|
| 2177 | `apply_env(hostname, env)` | **2** |
| 2203 | `apply_tmpfs(specs)` | **1** — and with the comment «container init: many namespace/security parameters», which belongs to another function |
| 2397 | `mask_slow_node_units()` | **ZERO** |

They had detached from the functions they were supposed to silence. This is the
mechanism that explains the wrong number in the P0-1 FIXME: with the lint turned
off in the wrong places, `too_many_arguments` stopped warning when the
`container_init` signature went from 30 to 37 arguments.

**Proof that they were dead:** with the three removed, `cargo clippy --workspace
--all-targets` still has **zero warnings**. The only one that remains
(`setup_rootfs`, 10 parameters) is legitimate and now carries a written reason.

**Lesson for §88:** an `#[allow]` placed ABOVE the `///` block — as these were —
detaches from the function at the first reordering and nobody notices. Rule to
adopt: the `#[allow]` always sits BETWEEN the doc and the signature.

---

## P1-1 — `tls.hosts` of `kind: Ingress`: warn instead of swallowing

**ID:** ARCH-0003 · **Severity:** MEDIUM · **Breaking:** NO

**Problem.** `httproute.rs:591` — `IngressTls.hosts` has `#[allow(dead_code)]`
without a line of reason. A user who writes two SNI hosts gets **only one**
certificate (the first element of `tls`) and is not warned. It contravenes the
house rule: a field the client writes and the system ignores is worse than a
field that does not exist. It is the only one of the four ignored fields in this
file without a written reason — the other three (`ingressClassName`, `pathType`,
`port.name`) comply.

**Design.** (a) doc saying why; (b) warning in `apply` when
`tls[].hosts.len() > 1` or when `tls.len() > 1`.

**Acceptance:** applying an Ingress with two TLS hosts prints a warning naming
the host left without a certificate.

---

## P1-2 — `pathType: Exact` accepted and served as prefix

**ID:** ARCH-0003 · **Severity:** MEDIUM · **Breaking:** NO

**Problem.** `httproute.rs:504`. Documented in the code as a known limitation,
but a user who writes `Exact` gets **prefix** matching — it routes traffic they
did not ask for. Documenting is not warning.

**Design.** Warning in `apply` when `pathType == "Exact"`, naming the route. A
more expensive and more correct alternative: reject until the proxy supports
`Exact`.

---

## P1-3 — SAFETY on the `unsafe` blocks that depend on invariants

**ID:** DOC-0003 · **Severity:** MEDIUM · **Breaking:** NO

**Problem.** 58 `unsafe` blocks without a SAFETY comment in the preceding 6
lines. Do **not** treat them all alike: a local `libc::close(fd)` is trivial and
does not deserve justification; `libc::kill(pid, SIGKILL)` **depends** on the
invariant that the PID still belongs to the process you think you are killing —
a recycled PID kills the wrong process — and that invariant is not written down.

**Target:** the `kill`, `read`, and `write` calls over captured PIDs and fds in
`delonix-cri/src/spdy.rs`, `delonix-cri/src/streaming.rs`,
`delonix-runtime-bin/src/cmd/container.rs`.
**Acceptance:** each `unsafe` over a captured PID/fd names the invariant and who
guarantees it.

---

## P1-4 — Structured errors on the security path

**ID:** ERROR-0001 · **Severity:** MEDIUM · **Breaking:** yes, in 3 signatures

**Problem.** 12 functions return `Result<_, String>` (§51). Three are on the
security-enforcement path, where the message is the only thing left to diagnose
a rejection:

```
crates/delonix-runtime/src/lib.rs:325       fn install_filter_privileged(…) -> Result<(), String>
crates/delonix-runtime/src/lib.rs:2105      fn apply_apparmor(…)            -> Result<(), String>
crates/delonix-runtime/src/seccomp_profile.rs:578  pub fn compile(…)        -> Result<_, String>
```

**Design.** A structured `SeccompError`/`LsmError` with the syscall, the action,
and the cause — not a sentence. `compile` is `pub`: a breaking change, check
consumers first (§25).
**Remaining 9:** CLI flag parsers (`parse_add_host`, `parse_io_rate`,
`cap_ceiling::parse`, …) — there `String` is defensible, the message goes
straight to the user. **Do not touch.**

---

## P2-1 — 45 `.map_err(|_| …)` throwing the cause away

**ID:** ERROR-0002 · **Severity:** MEDIUM · **Breaking:** NO

Against §24. Not all are equal: on `parse::<u16>()` the cause adds nothing; on
`sign.rs:107` (`c.get_manifest(&sig_tag).map_err(|_| …)`) you lose the
distinction between «signature does not exist» and «the registry answered 500»,
which is exactly what you want to know when diagnosing a failed verification.

**Rule to apply:** keep `|_|` when the discarded error is of pure conversion;
chain the cause in anything that crosses network, disk, or process.

---

## P2-2 — Document the 28 public enums without docs

**ID:** DOC-0001 · **Severity:** MEDIUM · **Breaking:** NO

Enum coverage at 58.8%, the worst of the four categories. Priority: the ones
that appear in an error or resource state first. Do **not** document the 110
`pub mod` — that is percentage without value (§81).

---

## P2-3 — Stop branching on provider name

**ID:** NAMING-0007 · **Severity:** MEDIUM · **Breaking:** NO

```
crates/delonix-runtime-bin/src/cmd/conditions.rs:319  if backend == "libvirt" {
crates/delonix-vm/src/lib.rs:3981                     (vm.backend == "libvirt").then(…)
```

The `VmBackend` port exists; these two sites skip it and ask for the name.
**Design:** a capability method on the trait (e.g. `fn needs_tap_name()`), and
the decision moves to the adapter. In the engine, this closes the leak that
`ngolacloud-arch` records as ARCH-002.

---

## P3-1 — Translate the 3 named private items in PT

**ID:** NAMING-0002 · **Severity:** LOW · **Breaking:** NO

`netdef_path_legado` → `legacy_netdef_path`; `static AVISO` → `WARNED_ONCE`;
`const CONSUMIDOS_TOPO` → `CONSUMED_TOP_LEVEL`. Auto-fixable (§97).
**Do not touch** the lexicon's false positives: `fn todo<T>`, `fn topo_sort`,
`enum StringOrNum`, `pub fn cap_num`.

---

## P3-2 — Translate the ~44 local bindings in PT

**ID:** NAMING-0003 · **Severity:** LOW · **Breaking:** NO

Concentrated in `delonix-net/src/infra.rs` (12) and `cmd/kindmode.rs` (8). Do it
per file, alongside another change in that file; lower the ratchet baseline in
the same commit (the gate requires it).

---

## P4-1 — Translate the 985 test names

**ID:** NAMING-0004 · **Severity:** LOW · **Breaking:** NO

95% of all the identifier debt. The names **are good** — they describe behaviour
as §59 asks — they are just in the wrong language. Do it per crate, one PR per
crate, lowering the baseline with each. Blocks nothing.

---

## P4-2 — One line in `AGENTS.md` about the language of comments

**ID:** DOC-0005 · **Severity:** LOW · **Breaking:** NO

Say that the historical comments are in Portuguese, that new ones are written in
English, and that `scripts/lang_ratchet.py` is the gate. Cost: one line. Gain:
an agent stops treating 3,453 comments as an anomaly.

---

## Explicitly NOT recommended

- **Splitting the 71-field `Container`.** It is persisted state; splitting it is
  migration (§80), and it is documented field by field. Only with an ADR.
- **Creating `NetworkProvider`/`StorageProvider` now.** There is one network
  path and one storage path. A port with a single adapter is YAGNI (§36). Record
  the boundary in writing; create the port when the second one appears.
- **Documenting the 110 `pub mod`.** It raises the percentage, adds no
  information.
- **A newtype campaign.** Do it only where it pays off: inside P0-1.
