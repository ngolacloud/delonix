# Audit of the native VM path (Cloud Hypervisor, no libvirt) — 2026-10-24

Sprint 1 of the virtualization briefing: audit, prove live and make
reproducible the native VM path of delonix-runtime.

- **`AUDITORIA.md`** — components, libvirt independence, reproduced failures
  (topology vs. bug), lifecycle result, fix queue, status of the §16 acceptance
  criteria.
- **`MATRIZ-CAPACIDADES.md`** — pinned reference config + CH capabilities and
  what the harness proves (6 graduatable `partial`).
- **`harness-ch-lifecycle.sh`** — reproducible functional harness. Preflight;
  full lifecycle; checks inside and outside the guest; destroy with no orphans;
  cleans only what it creates; `exit != 0` on any failure. Run: **28/28**, RC=0.
- **`evidence/`** — harness output, host facts, raw discovery dump.

Reproduce:  `./harness-ch-lifecycle.sh [delonix]`  (needs `/dev/kvm`,
`cloud-hypervisor`, `cloud-localds` and a golden cloud-init image).

## Next steps (blocked in this session)
Landing code (the `ssh <ip>` hint fix; capability graduation; CI battery) needs
its own branch via GitFlow — blocked by the WIP cap 3/3.
See §7 of `AUDITORIA.md`.
