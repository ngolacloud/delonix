# Samba — a minimal file server image

This folder builds a Samba file server as a qcow2 with `delonix vm image build`. It is the
example recipe for the directives that keep an image small: `SLIM`, `INSTALL`, `REMOVE`,
`AUTOSTART --only` and `BUDGET` (see
[Declaring a small image](../../docs/dev/delonixfile-and-vmfile.md#declaring-a-small-image)).

```
images/samba/
├── Vmfile      the recipe, in the Dockerfile-shaped format
├── smb.conf    standalone server: SMB3 only, encryption required, no NetBIOS, no printers
└── README.md   this file
```

Family: Debian (`apt`), base `debian:trixie`.

## 1. What you need

| Requirement | Why | Check |
|---|---|---|
| `delonix` | the builder | `delonix version` |
| `virt-customize`, `qemu-img` | edit and convert the disk offline | `command -v virt-customize qemu-img` |
| network inside the guest | `INSTALL samba` reaches the Debian archive | see *Troubleshooting* |
| ~3 GiB free in `$TMPDIR` and in the image store | base image plus a working copy | `df -h /tmp ~` |

## 2. Build it

From the repository root:

```bash
delonix vm image build --network -f images/samba/Vmfile -t samba:trixie images/samba
```

The last argument is the **build context**: `COPY smb.conf` reads from it. `--network` is
required because of `INSTALL`; without it the build is refused before any disk work.

The build prints what it measured and what `AUTOSTART --only` turned off:

```text
footprint: root filesystem … MiB, … packages — largest: …
AUTOSTART --only disabled …: apt-daily.timer, man-db.timer, …
```

Over `BUDGET 1G` the build fails and names the 15 largest packages: that list is where to look
for the next `REMOVE`.

## 3. Run it

The shares live on a **data disk** whose filesystem is labelled `sambadata`, mounted at
`/srv/samba` (`nofail`: without one the VM still boots and the share is empty). Users and
passwords are **not** in the image; give them per instance through user-data:

```yaml
#cloud-config
runcmd:
  - useradd -M -s /usr/sbin/nologin -G smbusers ana
  - printf '%s\n%s\n' "$SMB_PASSWORD" "$SMB_PASSWORD" | smbpasswd -s -a ana
```

Clients connect to `\\<vm-ip>\dados` (SMB3, encrypted). There is no network browsing:
NetBIOS (`nmbd`) is masked.

## 4. What the recipe decides

| Line | Effect |
|---|---|
| `SLIM --docs=off --recommends=off --locales=pt,en` | apt never installs docs, man pages, Recommends/Suggests or other languages' catalogs; what the base already had is removed |
| `INSTALL samba` | through those rules |
| `REMOVE --services=nmbd,samba-ad-dc` | NetBIOS and the AD DC masked: this server is neither |
| `AUTOSTART --only smbd ssh systemd-timesyncd fstrim.timer` | everything else that was enabled is disabled; network, cloud-init, D-Bus, consoles, the guest agent and AppArmor are kept by the engine |
| `BUDGET 1G` | the root filesystem may not grow past 1 GiB |

## 5. What has and has not been run

On the development host (2026-11-27), against the real `debian:trixie` cloud image
(`debian-13-genericcloud-amd64-20250814-2204`), with an isolated `DELONIX_ROOT`:

- **this recipe, end to end** (`--network`, passt built from `passt.top` at `15980a90`, first on
  `PATH`) — rc=0, 1m28s inside the guest:

  ```text
  footprint: root filesystem 552.9 MiB, 338 packages — largest: vim-runtime 37.5 MiB,
    libicu76:amd64 36.5 MiB, linux-image-6.12.41+deb13-cloud-amd64 33.4 MiB, samba-libs:amd64 25.3 MiB, …
  AUTOSTART --only disabled 11: e2scrub_reap.service, grub-common.service, systemd-pstore.service,
    unattended-upgrades.service, uuidd.socket, apt-daily-upgrade.timer, apt-daily.timer,
    apt-listchanges.timer, dpkg-db-backup.timer, e2scrub_all.timer, man-db.timer
  ```

  Read back from the qcow2 (244 MiB, zstd): enabled are `smbd`, `ssh` (with
  `sshd-keygen` in `ssh.service.wants/`), `systemd-timesyncd`, `fstrim.timer`, `apparmor` and the
  network; `nmbd` and `samba-ad-dc` are masked; `smb.conf`, the `sambadata` fstab line, the
  tmpfiles rule and the `smbusers` group are in place; `/usr/share/man` is empty, `/usr/share/doc`
  holds only `copyright` files, and the apt indexes are gone.
- **an offline probe** with every directive except `INSTALL`, and the same probe with
  `BUDGET 100M`, which failed and named the 15 largest packages.

- **two clones of it, booted** directly in Cloud Hypervisor v53 (the image's own kernel and
  initrd, no firmware, no network card, each with its own NoCloud seed made by `cloud-localds`):
  cloud-init finished in ~4 s, `ssh` and `smbd` started, `/srv/samba` was skipped cleanly with
  no data disk (`nofail`), and the two disks came back with **different** machine-ids and SSH
  host keys. (The first boot of an earlier build showed every clone would have shared one
  machine-id; the final stage now resets it — see *Declaring a small image*.)

**Not run:** a client connecting over the network (the boots had no network card). The next
candidates the measurement points at are `vim-runtime` (37.5 MiB, from the base image) and what
pulls `libicu76` in.

## Troubleshooting

`--network` builds depend on libguestfs's passt; the host traps and their remedies are in
[Building microVMs § troubleshooting](../../docs/dev/microvm-setup.md#6-troubleshooting).
Two more seen on the same host:

- **`supermin exited with error status 1`** with a kernel newer than the running one installed:
  that kernel's `/boot/vmlinuz-*` is still `0600`. Point supermin at the running kernel instead
  of changing permissions:
  `SUPERMIN_KERNEL=/boot/vmlinuz-$(uname -r) SUPERMIN_MODULES=/lib/modules/$(uname -r)`.
- **`INSTALL` fails on `Temporary failure resolving`** after a ~300 s pause: the packaged passt
  never leased. Build a current one and put it first on `PATH` for the build only:
  `git clone --depth 1 https://passt.top/passt && make -C passt passt`, then
  `PATH=$PWD/passt:$PATH delonix vm image build --network …`.
- **`socket path too long`**: `XDG_RUNTIME_DIR` (where libguestfs puts its socket) must be a
  short path — `/tmp/delonix-run`, as the passt remedy says, not a deep scratch directory.
