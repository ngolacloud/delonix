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
(`debian-13-genericcloud-amd64-20250814-2204`):

- **an offline probe** with every directive except `INSTALL` (`SLIM`, `REMOVE`,
  `AUTOSTART --only ssh fstrim.timer`, `BUDGET`) built and was read back from the disk:
  `/usr/share/man` went from 33 entries to 0, `/usr/share/locale` from 116 directories to 13
  (pt and en only), the dpkg/apt rules were in place, `apparmor.service` and `ssh`'s
  `sshd-keygen.service` stayed enabled, and 11 units were disabled (the apt and man-db timers,
  `e2scrub`, `grub-common`, `systemd-pstore`, `systemd-timesyncd`, `uuidd.socket`, …). Root
  filesystem: 480.5 MiB. The same probe with `BUDGET 100M` failed and named the 15 largest
  packages.
- **this recipe end to end was not run**: on that host the packaged `passt` starts but never
  hands out a DHCP lease, so `INSTALL samba` failed on `Temporary failure resolving
  'deb.debian.org'` — trap 3 of [Building microVMs](../../docs/dev/microvm-setup.md). The
  remedy there (a current passt first on `PATH`) was not applied.

Treat the first network build on a capable host as the test of the `INSTALL` path and of
`BUDGET 1G` for this recipe.

## Troubleshooting

`--network` builds depend on libguestfs's passt; the host traps and their remedies are in
[Building microVMs § troubleshooting](../../docs/dev/microvm-setup.md#6-troubleshooting).
Two more seen on the same host:

- **`supermin exited with error status 1`** with a kernel newer than the running one installed:
  that kernel's `/boot/vmlinuz-*` is still `0600`. Point supermin at the running kernel instead
  of changing permissions:
  `SUPERMIN_KERNEL=/boot/vmlinuz-$(uname -r) SUPERMIN_MODULES=/lib/modules/$(uname -r)`.
- **`socket path too long`**: `XDG_RUNTIME_DIR` (where libguestfs puts its socket) must be a
  short path — `/tmp/delonix-run`, as the passt remedy says, not a deep scratch directory.
