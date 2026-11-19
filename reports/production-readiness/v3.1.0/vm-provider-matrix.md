# VM provider matrix — measured against `fa8c6297`

Binary from `fa8c6297`, isolated roots, `audvm-*` resources, read-only verification with `virsh -r` and
`qemu-img snapshot -l`. 2026-09-15. Logs: `scratchpad/aud/` (libvirt.log, report.tsv, provider-calls.log).

| Provider | Result | State |
|---|---|---|
| libvirt (`qemu:///system`) | **51 PASS · 1 FAIL** (CLI, `kind: VirtualMachine`, real Ubuntu 24.04 guest with SSH) | PARTIAL |
| Cloud Hypervisor (EDK2 `CLOUDHV.fd`) | create/stop/start/restart/snapshots/delete PASS; guest responds to ARP+ping | PARTIAL (subset) |
| Proxmox VE | `DELONIX_PROXMOX_URL` absent; `--backend proxmox` refuses, naming the variables to set | ENVIRONMENT BLOCKED |
| OpenStack | zero occurrences in `crates/*/src`; `--backend openstack` → `unknown VM backend` rc=1, no fallback | ABSENT |
| Live migration | ADR-0031 Accepted: NO-GO; only `vm migrate` stop-copy-start (not run) | UNSUPPORTED |

## libvirt

| Area | Checks | State |
|---|---|---|
| idempotent create, describe (rc=4 nonexistent), stop, start, restart ×2 | confirmed by libvirt | PASS |
| snapshot create/ls/duplicate rc=5/restore/stop+start post-restore/restore nonexistent rc=4/rm | libvirt `snapshot-list` | PASS |
| `kind: VirtualMachine`: validate, plan 2→0, idempotent apply, `spec.backend`, memory drift rc=2, `get`, destroy | | PASS |
| Real guest `--ssh-key --hostname --wait` + `vm ssh -- hostname` | returned the hostname from inside | PASS |
| Non-honourable option (`--namespace` on libvirt) | explicitly refused, nothing created | PASS |
| No action outside delonix | 427 calls to the provider tools, all with delonix as parent | PASS |
| **`vm pause` → `vm ls` says `Stopped`** (libvirt says `paused`) | reproduced by hand | **FAIL (G-041)** — fixed in `origin/main` (`58fdc4e3`), outside `fa8c6297` |

## Cloud Hypervisor

| Area | State |
|---|---|
| create `--wait` with IP verified by ARP/ping from inside the holder | PASS |
| snapshot with VM running → explicit refusal naming the alternative | PASS |
| snapshots with VM stopped (create, duplicate rc=5, restore, rm confirmed in the qcow2) | PASS |
| start/restart with guest responding ~4 s later | PASS |
| `vm stop`/`restart` print `qemu-img check` output to stdout before the name | PARTIAL (G-042, cosmetic) |
| `kind: VirtualMachine`, pause, `--ssh-key`/`vm ssh` | NOT TESTED |

## Proxmox — known defect not measured here

The duplicated `ipconfig0` that made every create fail (`got ARRAY`) entered in `23af3c45`, present
in `fa8c6297`, and was fixed in `a9c41cb9`/`d66dd486` (`origin/main`). Not verified live
(no Proxmox node) — **INFERENCE** from the git history.

## Host incident to record

During this phase the `vmip-ctl` domain of `qemu:///system` disappeared. **It was neither created nor touched
by this audit**: every brokered destructive call targeted only `audvm-*`. Circumstantial
evidence of another session: worktree `/tmp/wt-integra-vmip` (branch `integra/vm-create-stale-ip`),
`target-vmip`, and `vmip-ctl.log` (21:42) followed by `vmip-fix.log` (21:47) in libvirt. **Not proven** —
to be confirmed with the owner of that session.
