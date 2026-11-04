# ADR-0091: A common VM-image hardening baseline (`baseline: cloud-v1`)

- **Status:** Proposed (2026-11-04) — the mechanism is built as an OPT-IN recipe field, green and
  tested at the recipe level on `track/vm-cloud`; awaiting the owner's decision to default it on
  and to apply it to the shipped `images/debian` and `images/ubuntu` recipes, and lab confirmation
  of the actual guest build (it installs packages, so it needs `network: true`).
- **Date:** 2026-11-04
- **Deciders:** Walter Angolar
- **Relates to:** ADR-0076 (VM image defaults), the "VM structure for cloud" review of 2026-11-04
  (finding #7: the recipes only REMOVE packages; there is no common hardening baseline).

## Context

The review of 2026-11-04 found the VM-image mechanics sound (verified downloads, boot probe,
identity cleanup on clone) but the recipes thin: `images/*/vm.yaml` describe what to REMOVE and
little of what every cloud image should CARRY. Three gaps stood out:

- **No guest agent.** `qemu-guest-agent` is how the engine talks to a running VM (graceful
  shutdown, filesystem freeze, in-guest queries) — "real compute". The generic cloud images do not
  ship it, and each recipe was expected to remember it.
- **No serial console contract.** `vm console` needs a login getty on `ttyS0`; the cloud images put
  `console=ttyS0` on the kernel cmdline but do not guarantee the getty.
- **No common hardening.** sshd/sysctl/time/audit/trim settings an operator wants on every image
  were nowhere, so every recipe either omitted them or reinvented them.

## Decision

Introduce a named, closed hardening baseline folded into the recipe by one field:

```yaml
images:
  u:
    distro: debian
    release: trixie
    network: true          # the baseline installs packages
    baseline: cloud-v1
```

`cloud-v1` (Debian/Ubuntu via apt, Rocky/Fedora via dnf) adds:

- **Packages** (through the same verified package path and `network: true` requirement as
  `packages:`): `qemu-guest-agent`, `chrony`, and the audit daemon — `auditd` on apt, `audit` on dnf.
- **Serial console:** the kernel cmdline gets `console=tty1 console=ttyS0,115200n8` and
  `serial-getty@ttyS0.service` is enabled, so `vm console` reaches a login prompt. apt sets the
  cmdline with a grub drop-in applied by `update-grub`; dnf uses `grubby --update-kernel=ALL`,
  which edits the Boot Loader Spec entries of the cloud image in place. Both are tolerated-failing.
- **Drop-ins:** sshd (`PermitRootLogin no`, `PasswordAuthentication no`,
  `KbdInteractiveAuthentication no`, `X11Forwarding no`) and sysctl (rp_filter, syncookies, no
  redirects/source-routing, restricted kptr/dmesg) — identical across both families.
- **Units enabled:** `qemu-guest-agent`, the time-sync service (`chrony` on apt, `chronyd` on dnf),
  `auditd`, `fstrim.timer`.

The baseline is a CLOSED enum (`parse_baseline`), so an unknown name is a clear error, not a silent
no-op. It is refused on the golden profiles and the appliance/`build:` routes — those have their
own fixed shape. The baseline steps run after the recipe's own `run:` (so a recipe step cannot be
undone) and before removal/cleanup (so the machine-id reset stays the last operation, per the
`Cleanup` invariant). It is OPT-IN by default: turning it on for all custom images, or baking it
into the shipped recipes, is a behaviour change the owner decides (both force `network: true`).

Both apt and dnf bases are supported; any other package manager is refused rather than shipped as a
half-baseline. The dnf package and service names (`audit`, `chronyd`) and the `grubby` cmdline edit
still need confirmation on a real Rocky/Fedora cloud boot.

## Consequences

- One field gives every image the guest agent, serial console and a conservative hardening floor,
  checkable at the recipe level (`baseline_steps` is pure and tested).
- It installs packages, so a `cloud-v1` recipe requires `network: true`; the shipped base recipes
  stay offline-buildable until the owner opts them in.
- The named baseline is the single place to evolve the floor; bumping it is a deliberate edit.
