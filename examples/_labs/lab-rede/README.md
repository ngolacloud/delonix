# VM network lab

A miniature enterprise network, declared in manifests: authoritative DNS, DHCP,
user authentication, a workstation, NAS storage and a perimeter firewall.

**The complete guide, with the measured output of each step and the pitfalls
encountered, is in [`docs/guia-vm-lab.md`](../../docs/guia-vm-lab.md).** This file
is just the minimum to get the lab up.

| File | VM | Role | LAN address | Memory |
|---|---|---|---|---|
| `10-dns.yaml` | `lab-dns` | BIND 9, zone `lab.ngola` | 10.50.0.10 | 1 GiB |
| `20-dhcp.yaml` | `lab-dhcp` | ISC DHCP | 10.50.0.11 | 1 GiB |
| `30-samba.yaml` | `lab-samba` | Samba (authentication + sharing) | 10.50.0.12 | 1 GiB |
| `40-cliente.yaml` | `lab-cli` | workstation | 10.50.0.50 (via DHCP) | 1 GiB |
| `50-nas.yaml` | `lab-nas` | TrueNAS SCALE | 10.50.0.20 | 4 GiB |
| `60-storage.yaml` | — | dataset + quota + NFS export on the NAS | — | — |
| `70-firewall.yaml` | `lab-fw` | OPNsense | 10.50.0.254 | 3 GiB |

All together it asks for ~11 GiB of RAM. The engine refuses to create a VM that
does not fit (it reserves 2 GiB for the host), so start in phases if the machine
is shared.

## Prerequisites

- `delonix` 0.51.0 or later, with libvirt accessible (user in the `libvirt` group)
- the base image and the *appliance* images:

```bash
delonix vm image ls        # ver o que já existe
delonix vm image ls-remote  # ver o que há publicado
```

The lab uses `delonix-vm-base:ubuntu-24.04`, `truenas-scale:25.10` and
`opnsense:26.1`.

## Step 0 — the lab network

A **libvirt** network, isolated and **without DHCP** (here DHCP is served by
`lab-dhcp`). Note that `kind: Network` does **not** serve this purpose: it creates
container networks, not VM networks — see section 8 of the guide.

```bash
cat > /tmp/labnet.xml <<'EOF'
<network>
  <name>labnet</name>
  <bridge name='virbrlab' stp='on' delay='0'/>
  <ip address='10.50.0.1' netmask='255.255.255.0'>
  </ip>
</network>
EOF
virsh -c qemu:///system net-define /tmp/labnet.xml
virsh -c qemu:///system net-start labnet
virsh -c qemu:///system net-autostart labnet
```

## Step 1 — your SSH key

The cloud-init files ship with `SSH_KEY_PLACEHOLDER` so they can be shared without
anyone's key inside them. Replace it with yours:

```bash
mkdir -p /tmp/labrun/cloud-init
KEY=$(cat ~/.ssh/id_ed25519.pub)
for n in dns dhcp samba cliente; do
  sed "s|SSH_KEY_PLACEHOLDER|$KEY|" cloud-init/$n.yaml > /tmp/labrun/cloud-init/$n.yaml
done
for f in 10-dns 20-dhcp 30-samba 40-cliente; do
  sed "s|./cloud-init/|/tmp/labrun/cloud-init/|" $f.yaml > /tmp/labrun/$f.yaml
done
```

## Step 2 — apply

```bash
for f in 10-dns 20-dhcp 30-samba 40-cliente; do
  delonix stack apply -f /tmp/labrun/$f.yaml
done
```

Each `apply` responds in about 1 s; cloud-init then takes **2 to 4 minutes** to
install and configure the services inside each VM:

```bash
delonix vm ssh lab-dns -- cloud-init status     # → status: done
```

> **Never use `--prune` with these files.** The *stack* name comes from the
> **directory**, so the seven files are the same *stack*: a `--prune` on one of
> them sees the other VMs as orphans and deletes them.

## Step 3 — verify

```bash
delonix vm ssh lab-cli -- '
  ip -4 -o addr show lab0            # 10.50.0.50 — veio do lab-dhcp
  dig +short @10.50.0.10 dc.lab.ngola A
  smbclient -L //10.50.0.12 -U ana%Lab#2026Ngola
'
```

## NAS and firewall

These two have their own steps (creating the data disk and the *pool* on TrueNAS,
configuring the OPNsense LAN from the console). They are described, with the real
output of each command, in steps 6 and 7 of the [guide](../../docs/guia-vm-lab.md#passo-6--o-nas).

## Tear down

```bash
for v in lab-dns lab-dhcp lab-samba lab-cli lab-nas lab-fw; do delonix delete vm $v; done
virsh -c qemu:///system net-destroy labnet
virsh -c qemu:///system net-undefine labnet
```

## Credentials

These are lab credentials and are in the manifests on purpose, so the lab starts
without manual steps. **In a real environment they come from a `kind: Secret`**,
never from a file in git — and the engine warns about this on every `apply`:

| Service | User | Password |
|---|---|---|
| Linux VMs (SSH) | `delonix` | key only |
| Samba | `ana` | `Lab#2026Ngola` |
| TrueNAS (API and web) | `truenas_admin` | `delonix-admin` |
| OPNsense (console and web) | `root` | `opnsense` |
