# ADR-0103: The kernel is declared in the Vmfile, and an image can boot it directly (`KERNEL`, `INITRD`, `BOOT`)

- **Status:** Accepted (2026-11-27, by the owner). Spike S0 is **GO**. **P1 built** (`KERNEL`,
  `INITRD`, `kernel_version` always recorded) and **P2 built** (`BOOT direct`) — see the
  addenda. P3 (push/pull) to build.
- **Date:** 2026-11-27
- **Deciders:** Walter Angolar
- **Relates to:** the first phase of small images (PR #1056: `SLIM`, `INSTALL`, `REMOVE`,
  `AUTOSTART`, `BUDGET`), ADR-0091 (the `cloud-v1` baseline puts `console=ttyS0` on the kernel
  cmdline through grub), ADR-0076 (VM image defaults), ADR-0018 (VM images stay amd64),
  ADR-0017 (signing VM images).

## Context

The first phase made an image what its recipe declares and measured it. The measurement points at
what no directive can touch yet: **how the image boots**. Measured on the Debian trixie cloud
image (`debian-13-genericcloud-amd64-20250814-2204`) after the samba recipe in `images/samba/`:

| What | Size | Why it is there |
|---|---|---|
| `linux-image-6.12.41+deb13-cloud-amd64` | 33.4 MiB installed | the kernel and its modules |
| `grub-common` + `grub-efi-amd64-signed` + `grub-efi-amd64-unsigned` | 35.8 MiB installed | the bootloader the firmware hands control to |
| `/boot/efi` (vfat) | 124 MiB partition | the EFI system partition the firmware reads |
| `initrd.img-6.12.41+deb13-cloud-amd64` | 17.3 MiB | `MODULES=most`: drivers for hardware a VM never has |

Today the boot path is always: firmware (`CLOUDHV.fd`, or `hypervisor-fw`) → grub inside the
disk → the kernel inside the disk. That costs the bootloader and the ESP in every image, it is a
documented failure mode of its own (`microvm-setup.md`: «CH VM "running" but never answers» when
only `hypervisor-fw` is installed), and a recipe cannot choose its kernel: the `Vmfile` has no
instruction for it, and `kernel_version` is only recorded when the final `FROM` is a local image.

The engine already supports the other path. `VmBootSpec` carries `kernel`, `initrd` and
`cmdline` (`delonix-compute/src/domain/vm_backend.rs`), and `vm create --kernel/--initrd/--cmdline`
and the `VirtualMachine` manifest accept them. What is missing is an **image** that brings its
own.

NixOS is the model here: the kernel is part of the declared system (`boot.kernelPackages`), the
initrd carries exactly the declared modules (`boot.initrd.availableKernelModules`), and the
kernel and initrd are built artefacts beside the root filesystem, not files a bootloader finds
inside it.

### Spike S0 — direct kernel boot in Cloud Hypervisor (GO)

Run on the development host (2026-11-27), with Cloud Hypervisor v53.0, no network and no engine:
the Debian `vmlinuz` (an 11 MiB **bzImage**) and its `initrd.img` were copied out of the built
samba image with `virt-copy-out`, and the disk was converted to raw:

```bash
cloud-hypervisor --kernel vmlinuz-6.12.41+deb13-cloud-amd64 \
  --initramfs initrd.img-6.12.41+deb13-cloud-amd64 \
  --cmdline "console=ttyS0 root=UUID=<rootfs uuid> rw" \
  --disk path=disk.raw,image_type=raw --cpus boot=1 --memory size=1024M \
  --serial file=serial.log --console off
```

- The kernel booted with no firmware. With the disk writable (second run) it mounted the root
  `r/w` at 0.91 s; AppArmor loaded its profiles and the guest reached `samba login:` on `ttyS0`.
- CH v53 accepts the distro's bzImage as is: no `extract-vmlinux`, no PVH ELF needed.
- Two findings for the implementation:
  - A raw disk needs `image_type=raw`. Without it, CH v53 refuses writes to sector 0 and the guest
    stalls.
  - The same boot exposed that `Vmfile` builds left a `machine-id` in the image (fixed in
    PR #1056: the final stage now resets identity).

## Decision

Three instructions, in the `Vmfile` grammar and fail-closed like the rest:

```dockerfile
KERNEL linux-image-cloud-amd64 --purge-others   # or: KERNEL --file=./vmlinux
INITRD --modules=virtio_pci,virtio_blk,virtio_net,ext4
BOOT direct --cmdline="console=ttyS0 quiet"     # or: BOOT firmware (today's behaviour)
```

1. **`KERNEL <package> [--purge-others]`** — a step. It installs the package through the same
   path as `INSTALL` (so it needs `--network`, and `SLIM` applies). With `--purge-others`, every
   other installed kernel image package is purged together with its modules (`linux-image-*` on
   apt; the other `kernel-core` versions on dnf). The kernel version is read from the files the
   package owns (`dpkg -L` / `rpm -ql`, the `vmlinuz-<version>` it installs), never from
   `uname -r`, which inside `virt-customize` is the appliance's kernel. It is recorded as
   `kernel_version` for every `Vmfile` build, not only the ones whose base was local.
   **`KERNEL --file=<path>`** brings a kernel built elsewhere (a microVM config with virtio built
   in, no modules). It is context-confined like `COPY`, and it implies `BOOT direct`.
   At most one `KERNEL` per stage.
2. **`INITRD --modules=<list>`** — a step after `KERNEL`. It regenerates the initrd for **that**
   kernel version, with that list only:
   - initramfs-tools: `MODULES=list` plus the list in `/etc/initramfs-tools/modules`, then
     `update-initramfs -c -k <version>`;
   - dracut: `dracut --force --no-hostonly --add-drivers "<list>" … <version>`.

   Never host-only: inside `virt-customize`, the host is the libguestfs appliance, and an initrd
   tailored to it would not boot the VM. Module names are validated (`[a-z0-9_-]`).
3. **`BOOT firmware|direct [--cmdline=<…>]`** — an image property.
   - `firmware` is today's path and the default.
   - With `direct`, after the final stage the build copies the kernel and the initrd out of
     `/boot` with `virt-copy-out` into the image store, beside the qcow2:
     `<name>.vmlinuz` and `<name>.initrd`, each with its sha256.
   - `VmImage` gains `boot: { kernel_sha256, initrd_sha256, cmdline }`.
   - The build adds `root=UUID=<uuid>` itself, read from the disk (`virt-filesystems --uuid`).
     It never guesses `/dev/vda1`: virtio-blk and virtio-scsi name the disk differently.
   - `vm create` uses the image's kernel, initrd and cmdline **only** when the caller gave neither
     `--kernel` nor `--firmware`. Same precedence rule as `default_vcpus`.
   - Every raw disk the engine hands CH carries `image_type=raw` (finding of S0).
   - The kernel package is held in the guest (`apt-mark hold` / dnf `versionlock`): with direct
     boot, a kernel upgraded inside the VM is never booted. Its modules would replace the running
     kernel's on the next `autoremove`, and the VM would lose drivers after a reboot. Changing the
     kernel means rebuilding the image, as in NixOS.

`BOOT direct` does **not** remove grub or the ESP by itself. The image keeps booting through
firmware too, until the recipe says otherwise with `REMOVE --packages=grub-…`. Removing what makes
an image bootable is the recipe's explicit choice, not a side effect.

**Distribution.** A `BOOT direct` image whose bootloader was removed cannot boot from its qcow2
alone, and `vm push` today ships only the qcow2. So `vm push` of an image with `boot:` is
**refused**, naming why, until the OCI artifact carries the kernel and initrd as two more layers
(`application/vnd.delonix.vmimage.v1.kernel`, `….initrd`). Publishing an image that does not boot
would be the silent failure this repo names as its worst. That is phase P3.

`vm.yaml` gets the same three fields (`kernel:`, `initrd:`, `boot:`) when it gets `slim:` and
`autostart:`: one follow-up for the whole front end, so the two never disagree on semantics.

### Phases

| Phase | What | Proof |
|---|---|---|
| S0 | Spike: CH boots a distro kernel and initrd directly | **done, GO** (above) |
| P1 | `KERNEL`, `INITRD`; `kernel_version` always recorded | recipe-level tests; a real build that swaps the kernel and shrinks the initrd, read back from the disk |
| P2 | `BOOT direct`: extraction, `VmImage.boot`, `vm create` defaults, `image_type=raw`, kernel hold | a real build plus `vm create` booting it on CH and on libvirt, reaching ssh |
| P3 | `vm push`/`pull` carry the kernel and initrd (refused until then) | a push/pull round trip that boots |

## Alternatives considered

- **Keep firmware boot and only let `KERNEL` swap the package.** It still pays grub and the ESP
  in every image and keeps the firmware failure mode. It is P1 of this ADR, not the whole of it.
- **A Unified Kernel Image (UKI) booted by firmware.** Kernel, initrd and cmdline become one
  signed EFI binary, which is the right answer for Secure Boot. But it still needs the firmware
  and the ESP, and `systemd-ukify` in every guest. It stays open as a later `BOOT uki` mode.
- **Extract the kernel at `vm create` time instead of build time.** It needs libguestfs on every
  create, makes create slower, and leaves the kernel unverified (no sha256 recorded at build).
- **Build the kernel from source in the build.** `KERNEL --file` covers bringing one. Compiling
  kernels is not the image builder's job.
- **Build the root filesystem from scratch** (`mmdebstrap --variant=minbase`, `dnf --installroot`,
  or an OCI image as the base). This is the next lever after this one. It is a separate decision,
  because it is a second way to make a disk.

## Consequences

- An image can drop the bootloader and the ESP. A `BOOT direct` image no longer depends on which
  firmware the host has installed.
- The initrd shrinks to the drivers a VM has. The measured default is 17.3 MiB with `MODULES=most`;
  the reduced size will be measured in P1, not promised here.
- The kernel is part of the image. Patching it is a rebuild, enforced by the hold, so kernel CVEs
  become an image-rebuild cadence. The inventory already tells you which images carry which
  kernel (`kernel_version`, `packages.tsv`).
- The image store gains two files per direct-boot image, and `VmImage` gains one optional field
  (`#[serde(default)]`, so older records still load).
- `vm push` refuses direct-boot images until P3, and says so.
- libvirt also supports direct kernel boot, but it has not been measured with these artefacts.
  P2 measures it before claiming it.

## Addendum — what building P1 measured (2026-11-27)

Debian trixie, `images/samba`, built with the network and booted directly in Cloud Hypervisor v53:

- `KERNEL linux-image-cloud-amd64 --purge-others` followed the metapackage to the archive's
  current `linux-image-6.12.118+deb13-cloud-amd64`, purged the base's 6.12.41, and recorded
  `6.12.118+deb13-cloud-amd64`. `KERNEL linux-image-amd64 --purge-others` swapped in the generic
  kernel: 106.4 MiB installed, against 34.1 MiB for the cloud one. That measurement is the reason
  the example recipe declares the cloud kernel.
- `INITRD --modules=virtio_pci,virtio_blk,virtio_scsi,ext4`: **18.6 MiB → 10.7 MiB** for the
  same kernel (`MODULES=most` → `MODULES=list`). Six modules remain. The rest is
  initramfs-tools' userspace; udev's `hwdb.bin` alone is 12.4 MiB uncompressed. That is the next
  lever, not the module list.
- A/B boot test, with the same kernel and image, comparing the `list` and `most` initrds, two
  boots each: there is no difference attributable to the initrd. Both varied with the host's load.
- **A distro kernel cannot drop the initrd.** Debian's cloud kernel builds in `ext4` and
  `virtio_pci`, but `virtio_blk` is a module. Booted with no initrd, it waits for its root
  forever, with or without `rootwait`. So `BOOT direct` (P2) keeps an initrd for distro kernels,
  and only `KERNEL --file=` (a kernel with virtio built in) may go without one. P2 must not
  promise otherwise.
- Two bugs in the generated shell were caught by the real build: a `; ;` and a loop exiting 1.
  Both now have tests that execute or parse every generated command.



## Addendum — what building P2 measured (2026-11-27)

- `BOOT direct` on `images/samba`: the kernel (11.2 MiB) and initrd (10.7 MiB) were extracted
  beside the qcow2 with their sha256. The command line was recorded with the root filesystem's
  UUID, read through libguestfs' `inspect-os` + `vfs-uuid`.
- `vm create` from that image, with no `--kernel`, on both backends:
  - **libvirt** (`qemu:///system`, NAT): `<kernel>`, `<initrd>` and `<cmdline>` were in the
    domain;
  - **Cloud Hypervisor** (rootless): `--kernel`, `--initramfs` and `--cmdline` were on the
    process.

  In both, ssh logged in, `/proc/cmdline` was the image's own (not grub's), `smbd` and `ssh` were
  active, the kernel packages were held, and the machine-id was the VM's own.
- **Not needed after all:** the engine already gives every Cloud Hypervisor disk an explicit
  `image_type` (the boot disk is a qcow2 overlay), so the `image_type=raw` finding of S0 needed no
  change.
- **`KERNEL --file=` is not built.** P1 measured that a distro kernel cannot boot without an
  initrd, so a kernel from outside the image needs its own decision about where its initrd comes
  from. The parser refuses it and says so.
- **A trap in the proof, not in the code:** several sessions on one machine share `target/`.
  Mid-proof another session's build replaced `target/debug/delonix`, and three `vm create`s ran
  without this code. The proof was re-run with a pinned copy of the binary.

