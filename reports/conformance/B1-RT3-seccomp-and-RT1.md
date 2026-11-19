# B1 isolated gaps — GAP-RT-1 and GAP-RT-3 (fixed)

**Date:** 2026-10-24 · **Binary:** `delonix 5.1.1` (tree at `ee615f3b` + these fixes), rootless.
Decisions: both come from [B1-oci-runtime-baseline.md](B1-oci-runtime-baseline.md).

## GAP-RT-1 — `ociVersion` of the exported bundle · FIXED + verified

- `cmd/image.rs build_runtime_spec`: `.version("1.0.2")` → `.version("1.3.0")` (the version that
  `runc spec` 1.5.2 emits). Test updated.
- **Verified:** `oci-runtime-tool validate` went from *rejecting* ("only handles 1.3.0") to
  **"Bundle validation succeeded"**.

## GAP-RT-3 — seccomp in the exported bundle · FIXED + verified

### Design decision (why NOT the standard moby profile)

I offered two options: (a) **Delonix's own** policy or (b) the **standard containerd/moby** profile.
When implementing I found that the repo already has the material for (a) in a clean way — `const SYSCALLS`
(name↔number table, via `libc::SYS_*`) + `allowed_syscalls()` (the numeric allowlist). So (a)
became **the best choice**: no vendoring of ~300 lines, no transcription risk, and
**zero drift** — the bundle carries **the same policy that Delonix's init enforces**, not a different
upstream policy. (b) would bring a policy diverging from the real one and an external asset to maintain.

### Implementation

- `delonix-linux/seccomp_profile.rs`: new `pub fn syscall_name(nr) -> Option<&str>` (the inverse of
  `syscall_number`).
- `delonix-linux/lib.rs`: new `pub fn allowed_syscall_names()` — resolves `allowed_syscalls()`
  (numbers) to names via the table, sorted and dedup.
- `cmd/image.rs build_runtime_spec`: builds a `LinuxSeccomp` (`defaultAction = SCMP_ACT_ERRNO`,
  arches x86_64+x86, `syscalls = [{names: <allowlist>, action: SCMP_ACT_ALLOW}]`) and wires it into
  `linux.seccomp`. The round-trip test is extended to require seccomp + default-deny + the essentials.

### Verified live (`delonix image export alpine`)

```
seccomp present: True
defaultAction:   SCMP_ACT_ERRNO            (default-deny)
architectures:   SCMP_ARCH_X86_64, SCMP_ARCH_X86
allow count:     263 syscalls
read/write/execve/openat presentes:  True
ptrace/bpf/mount EXCLUÍDOS:           True
oci-runtime-tool validate:           Bundle validation succeeded
```

Before: the bundle had no `seccomp` section → a `runc`/`crun` ran it **with no filter at all**. Now it
runs with the same default-deny allowlist that Delonix applies.

### Documented limitation (follow-up)

The `architectures` are those of the export host (x86_64 + 32-bit compat). A bundle for another
architecture (aarch64) needs the corresponding arch list — `allowed_syscalls()` is already
conditional per arch, only the OCI `Arch` mapping is missing. Small follow-up, does not block x86_64.

## Status

Both **applied, compile, verified live**; on branch `fix/campaign-defects-doctor-rm-compose`,
**not pushed**. Unit test `build_runtime_spec` (now covering RT-1 and RT-3) running.
