# GAP-RT-2 — exportable rootless bundle · FIXED + verified

**Date:** 2026-10-24 · **Binary:** `delonix 5.1.1` (tree `ee615f3b` + fixes), rootless.

## The gap (from B1)

`delonix image export` produced a bundle **without uid/gid mappings** (root-only); `runc`/`crun`
rejected it rootless with `rootless container requires user namespaces`. Portable only as root.

## Fix

New flag **`image export --rootless`**: adds a **user namespace** and the uid/gid mappings
**of this user**, mirroring the map that the engine's own init uses
(`write_userns_maps`): container 0 → our euid (1 id), then container 1.. → the delegated
subuids (`USERNS_UID_BASE`). The default is still the root bundle (no maps). Files:
`cmd/image.rs` (flag, `build_runtime_spec(rootless)`, adjusted warning), reuses
`delonix_linux::USERNS_UID_BASE/USERNS_RANGE`.

## Verified live (`image export --rootless alpine`)

```
has user ns: True
uidMappings: [{hostID:1000, containerID:0, size:1}, {hostID:100000, containerID:1, size:65535}]
gidMappings: [{hostID:1000, containerID:0, size:1}, {hostID:100000, containerID:1, size:65535}]
oci-runtime-tool validate: Bundle validation succeeded
runc --root <tmp> run  (rootless):  rc=0        ← era "requires user namespaces"
```

Default (without `--rootless`): no user ns, no maps — unchanged.

## Tests and gates

- Test `bundle_exportado_e_conformante_oci_runtime` extended: default without userns/maps;
  `--rootless` with userns + map container-0→euid-size-1 + container-1→`USERNS_UID_BASE`. **Passes.**
- Green gates: `lang_ratchet` (2 new strings → `pt.po` + baseline 114→116), `docs_cli_gate`,
  `cli_exec_ratchet` (90.7%), `dev_docs --check`. **No pending gate** for the new flag.

## Documented limitation (follow-up)

The seccomp `architectures` and the map assume the export host (x86_64, subuid 100000:65536 from
`/etc/subuid`). A bundle for another architecture / another `/etc/subuid` needs to read those values
instead of the `USERNS_*` convention. Small; does not block the common case.

## Track B status (OCI runtime)

| Gap | Status |
|---|---|
| RT-1 ociVersion 1.3.0 | ✅ fixed + validate OK |
| RT-2 rootless bundle | ✅ fixed + `runc run` rootless OK |
| RT-3 seccomp in bundle | ✅ fixed (own policy, 263 syscalls) |
| B2 mount propagation | ✅ already existed (verified) |
| RT-4 runc-style CLI (create/start/state/kill/delete) | ⬜ open (what is missing to run the runtime-tools *validation suite* against Delonix) |

Everything on branch `fix/campaign-defects-doctor-rm-compose`, **not pushed**.
