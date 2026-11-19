# `delonix vm` — administrator's guide, and a complete network lab

**For those who administer systems and networks.** Every command here was run against the
real binary and the output shown is the output that was obtained. What could not be proven
is identified as such, with the reason — a guide that promises what it hasn't verified is
worse than an incomplete guide.

- **Engine version:** `delonix 0.52.0` (the flagged fixes landed in 0.51.x)
- **Validation host:** Ubuntu, 32 vCPU, 30 GiB RAM, rootless, libvirt + Cloud
  Hypervisor installed
- **Date:** August 12, 2026

Table of contents:

1. [Before you start](#1-before-you-start)
2. [Cheatsheet: the lifecycle](#2-cheatsheet-the-lifecycle)
3. [Cheatsheet: inspection](#3-cheatsheet-inspecção)
4. [Cheatsheet: access](#4-cheatsheet-acesso)
5. [Cheatsheet: images](#5-cheatsheet-images)
6. [Cheatsheet: snapshots](#6-cheatsheet-instantâneos)
7. [`kind: VirtualMachine` — the declared VM](#7-kind-virtualmachine--a-vm-declarada)
8. [`kind: Network` — and the boundary that fools everyone](#8-kind-network--and-the-boundary-that-fools-everyone)
9. [The complete lab](#9-the-complete-lab)
10. [Measured pitfalls](#10-armadilhas-medidas)

---

## 1. Before you start

### What the host needs to have

```bash
delonix system info
```

```
Delonix Runtime 0.51.0
  state root:         /home/walter/.local/share/delonix
  mode:               rootless (daemonless)
  cgroup2 delegated:  yes
  network infra:      up (holder pid 71944)
```

For VMs, besides this: `libvirt` (with the user in the `libvirt` group), `qemu-img` and
`cloud-localds`. Without access to the system libvirt, a VM falls back to *user* mode (SLIRP)
and **never gets a visible address** — `vm create` warns when that happens.

### The two engines, and when each one fits

| | **libvirt** | **Cloud Hypervisor** |
|---|---|---|
| Where the VM lives | `virbr*`, on the host network | the engine's SDN, inside the *holder* |
| Address | libvirt DHCP, **observed** | computed from the MAC, **predicted** |
| Graphical console (`--vnc`) | yes | no |
| Snapshots | yes | no (explicit refusal) |
| Appliances (OPNsense, TrueNAS, Proxmox) | yes | **do not boot** |
| `delonix-vm-base:*` images | yes | **do not boot** on this host |

**For serious VM work, use libvirt.** It's what this guide uses throughout the
lab. Cloud Hypervisor serves Linux microVMs that need to be on the same
network as the containers — and it brings the distinction described in
[Pitfalls](#em-cloud-hypervisor-o-endereço-é-calculado--não-observado).

```bash
delonix vm default-backend --set libvirt   # fixa a escolha, uma vez
delonix vm default-backend                 # consulta
```

Effective precedence, from strongest to weakest: explicit `--backend` → the
`HYPERVISOR` recorded on the image → `DELONIX_VM_BACKEND` → `vm default-backend` →
automatic detection.

---

## 2. Cheatsheet: the lifecycle

### Create

```bash
delonix vm create dev \
  --disk delonix-vm-base:ubuntu-24.04 \
  --backend libvirt --net-mode nat \
  --vcpus 1 --memory 1G \
  --hostname dev --ssh-key @$HOME/.ssh/id_ed25519.pub \
  --wait --boot-timeout 180
```

```
Creating VM 'dev'…
 ✓ preparing the overlay disk 💽 0.0s
 ✓ configuring the network 🌐 0.1s
 ✓ defining the domain 📋 0.0s
 ✓ starting the VM ▶ 0.8s
✓ VM 'dev' is up.
info vm 'dev' is up — ip 192.168.122.190
```

**Measured: 11.5 s** from the command to the assigned address; SSH accepted a connection a few
seconds later, already with the *hostname* applied and the key installed by cloud-init.

The base disk becomes a per-VM *overlay* — the original image is never written, which
allows dozens of VMs from a single copy.

**There is resource admission**, and that's good news for anyone running this seriously:

```
error system call `VM admission` failed: host protection: VM 'lab-nas' asks for
6144 MiB but the host only has 7181 MiB available (reserve 2048 MiB). Stop
VMs/containers, reduce the memory, or lower DELONIX_VM_RESERVE_MIB (at your own risk).
```

The refusal happens **before creating anything at all** and reserves 2 GiB for the host. It
was triggered twice while setting up the lab and kept the machine from going into
*swap*.

### Stop, start, restart, remove

```bash
delonix vm stop dev       # 0,4 s — preserva disco e registo
delonix vm start dev      # 0,9 s — mesmo overlay, disco preservado
delonix vm restart dev    # 1,3 s — sempre um arranque real
delonix delete vm dev     # pára e apaga overlay + registo
delonix delete vm dev -f  # descarta o registo local mesmo se o libvirt falhar
```

> **`stop` performs an `undefine` of the libvirt domain** — so as not to leave orphaned
> domains behind. A stopped VM therefore has no domain that `virsh` can query about
> anything. Snapshots **survive** (since v0.51.x — see
> [Pitfalls](#stop-used-to-delete-snapshots-without-saying-so--fixed-in-v051x)).

`start` rebuilds the VM from the persisted record, but **only recovers** disk,
vCPU, memory, network and engine. A custom kernel, cloud-init *seed*, 9p volumes,
fixed address, VNC and the advanced libvirt options only exist as `vm create`
arguments and **do not survive**. A VM that depends on them comes back via the original
`vm create` — which is also idempotent.

---

## 3. Cheatsheet: inspection

```bash
delonix vm ls          # tabela
delonix vm ls --ports  # + sonda TCP real a 22/6443/10250/80/443
delonix vm ls -o json  # para script
delonix describe vm [nome]
delonix vm image describe <nome>...  # detalhe de uma IMAGEM de VM
delonix dashboard --scope vm --once    # KPIs + tabela
```

```
NAME      IMAGE                          BACKEND   VCPUS  MEMORY  STATUS   IP             UPTIME        PORTS OPEN
lab-dns   delonix-vm-base_ubuntu-24.04   libvirt       1  1G      Running  192.168.122.2  Up 2 minutes  22
```

`--ports` makes real TCP connections and is therefore off by default —
it distinguishes *"the VM has an address"* from *"the VM is serving something"*, which isn't the same
question.

> The table sizes columns by content and **omits the ones that end up entirely empty**.
> If no VM has any of those ports open, the `PORTS OPEN` column
> disappears — that's not the flag failing. For a stably shaped output, use
> `-o json`, where the `ports_open` field is always present when `--ports`
> is requested.

`status` reconciles with the engine instead of re-reading the record; `describe` shows the
entire record, including the *overlay* path and its real size on disk.

---

## 4. Cheatsheet: access

```bash
delonix vm ssh dev                        # o nome basta — o IP vem do registo
delonix vm ssh dev -- systemctl status    # um comando e sai
delonix vm ssh 192.168.122.50 -l root     # directo a um endereço
delonix vm console dev                    # consola série (voltar: Ctrl+])
delonix vm vnc dev                        # só para VMs criadas com --vnc
```

The default user is `delonix` on a cloud-init image and `root` on an
*appliance* — the command picks it on its own.

`console` needs a real terminal; without one it says so instead of blocking:

```
error: Cannot run interactive console without a controlling TTY
```

**The console is what saves you when there is no network.** It's what made it possible to configure the
lab's OPNsense, which boots without a usable address.

---

## 5. Cheatsheet: images

```bash
delonix vm image ls          # o que está local
delonix vm image ls-remote   # o que há publicado, sem descarregar nada
delonix vm image pull <ref>  # trazer
delonix vm image build -t <tag> .  # construir a partir de um VMfile
delonix vm image convert <src> --to qcow2|raw|vmdk|vdi|vhdx|vhd
```

`ls-remote` reads only manifests (a few KB) and shows the three official repositories:
`delonix-vm-k8s` (Kubernetes nodes), `delonix-vm-base` (Ubuntu 24.04/26.04, Debian
bookworm, Rocky 9, Fedora 42) and `delonix-vm-appliances` (OPNsense, TrueNAS, the
four Proxmox products).

**`convert` — all six formats were converted and verified with `file(1)`:**

| `--to` | `file(1)` confirmed |
|---|---|
| `raw` | `data` |
| `vmdk` | `VMware4 disk image` |
| `vdi` | `VirtualBox Disk Image` |
| `vhdx` | `Microsoft Disk Image eXtended` |
| `vhd` | `Microsoft Disk Image, Virtual Server` |

An image built here can be imported by VMware, VirtualBox and Hyper-V — without
this engine having a backend for any of them.

### `vm image build` — image from a `VMfile`

```bash
delonix vm image init   # gera VMfile + cloud-init/user-data.yaml
delonix vm image build -t app:1.0 .
```

```
[1/1] stage-1: FROM ubuntu:24.04
FROM ubuntu:24.04: official delonix base delonix-vm-base:ubuntu-24.04 (local)
 ✓ preparing the disk 📦 1.7s
 ✓ 2 steps inside the guest 🔧 2.5s
 ✓ compacting the image 🗜 7.2s
```

**Measured: 12 s.** The `VMfile` has Dockerfile grammar (`FROM`, `RUN`, `COPY`,
`ENV`) plus what only makes sense in a VM: `SIZE`, `VCPUS`, `MEMORY`, `HYPERVISOR`,
`SSHKEY`, `USER`. It supports multi-stage, where each stage is an entire disk.

**`RUN` runs without network by default**, on purpose: a build that reaches the internet produces
a different image depending on the day it ran. To install packages there is
`--network` — which **doesn't work on this host** (see
[Pitfalls](#vm-image-build---network-esbarra-no-host)).

---

## 6. Cheatsheet: snapshots

The four verbs live under the `vm snapshot` group, and they work with the VM **running or
stopped**:

```bash
delonix vm snapshot create  dev antes-da-actualizacao
delonix vm snapshot ls      dev
delonix vm snapshot restore dev antes-da-actualizacao
delonix vm snapshot rm      dev antes-da-actualizacao
```

> Until v0.51.0 these were three flat commands (`vm snapshot`, `vm snapshots`,
> `vm restore`), and there was no way to delete a snapshot via the CLI. The old
> form was removed without an alias: it fails with `unrecognized subcommand`.

With the VM **running** it is a full system restore point: memory *and* disk. With it
**stopped** it is disk only — and the VM remains stopped. Restoring a snapshot taken while
running brings the VM back up running, memory included, and the command says so.

On **Cloud Hypervisor** snapshots are disk-only and require the VM to be stopped: the
running vmm holds the `qcow2` exclusively and CH has no live disk snapshot
API. The verbs that write refuse with an error that says what to do; `ls` always
responds.

Proven by writing a file after the snapshot and restoring:

```
--- restore para base-limpa
real 0m1.394s
--- a marca ainda existe?
cat: /root/marca.txt: No such file or directory
AUSENTE (restore reverteu)
```

**1.4 s** to revert an entire VM.

---

## 7. `kind: VirtualMachine` — the declared VM

```yaml
apiVersion: delonix.io/v1
kind: VirtualMachine
metadata:
  name: dev
spec:
  disk: delonix-vm-base:ubuntu-24.04   # nome de imagem OU caminho para um qcow2
  vcpus: 1
  memory: 1G
  backend: libvirt
  netMode: nat
  hostname: dev
  sshKeys:
    - "@/home/walter/.ssh/id_ed25519.pub"
```

```bash
delonix stack validate -f vm.yaml            # referências resolvem?
delonix stack apply -f vm.yaml --dry-run     # o spec com todos os defaults, e a recusa do apply
delonix stack apply -f vm.yaml               # aplicar
delonix stack plan -f vm.yaml                # o que mudaria
delonix explain Vm                           # todos os campos, gerados do código
```

The declarative cycle was exercised end to end:

```
$ delonix stack apply -f vm.yaml            # 2.ª vez
vm/lab-mani: ensured                        # idempotente

$ delonix stack plan -f vm.yaml --detailed-exitcode
  =   Vm/lab-mani
Summary: 1 unchanged                        # rc=0

# memória 1G → 2G no ficheiro
$ delonix stack plan -f vm.yaml
  -/+ Vm/lab-mani  — does not converge live: memory
        memory: 1G → 2G
Summary: 1 to replace                       # rc=2 com --detailed-exitcode

$ delonix stack apply -f vm.yaml
  ✗ Vm/lab-mani: does not converge live: memory — pass `--replace Vm/lab-mani`
error: stack apply refused: 1 resource(s) need an explicit decision (nothing was changed)

$ delonix stack apply -f vm.yaml --replace Vm/lab-mani
Vm/lab-mani: recreating                     # e a memória passou mesmo a 2G
```

Notice **"nothing was changed"**: the refusal happens before the first change,
not halfway through. That's what lets you put `stack plan --detailed-exitcode` in a *pipeline* as
a configuration-drift detector.

### What the reconciler compares — and what it doesn't

The engine **tells you**, instead of leaving you to find out:

```
VirtualMachine 'lab-nas': Converged=False (FieldsNotCompared) — changed since it was
created and NOT applied to the existing VM: extraNics, netMode — the reconciler
compares only disk, vcpus, memory, network, backend. Recreate it
(`--replace VirtualMachine/lab-nas`, which discards the disk) …
```

Changing network interfaces, injected XML or network mode in a manifest **doesn't change a
VM that already exists**. Either you recreate it (`--replace VirtualMachine/lab-nas`, which discards the
disk), or you use `vm create`. It's worth repeating because in practice it's silent: `apply`
says `ensured` and the VM stays as it was.

The warning compares the manifest against what the VM received **when it was created** (the
`delonix.io/created-spec` annotation, written by `stack apply` on every creation or recreation):
it names only what has changed since then, and stays quiet when nothing changed. On a VM without that
record — created by hand or by an earlier version — it says "declared and not
verifiable" and lists what the manifest declares. `--replace` has to carry the
`Kind/name`: a bare name (or `all`) only authorizes what the plan already proposes, and a
field that isn't compared never proposes it.

### Fields worth knowing

| Field | What it's for |
|---|---|
| `extraNics[]` | additional interfaces — `type` (`network`/`bridge`/`user`), `source`, `mac` |
| `extraDisks[]` | disks besides the overlay |
| `volumes[]` | mount a `kind: Volume` inside the VM, via virtio-9p |
| `ip` | DHCP reservation on the libvirt network (`netMode: nat` only) |
| `userData` | custom cloud-init — **replaces** the generated one |
| `vnc`, `video`, `tpm`, `cpuModel`, `cpuTopology`, `bootOrder`, `machine` | typed equivalents of the libvirt XML |
| `libvirtXmlOverlay[]` | raw XML before `</devices>` |
| `libvirtXml` | replaces the entire `<domain>` |

The last two **are not validated** and can name any host path — only
for trusted manifests. The lab uses `libvirtXmlOverlay` once, and the
[reason](#the-nas-needs-a-disk-with-a-serial-number) is instructive.

**A fixed `mac` is more than a convenience.** Inside the system, `enp1s0` and `enp2s0`
depend on the order in which PCI enumerates the interfaces, and that is not a contract. All
the lab's VMs match the network configuration by MAC:

```yaml
network:
  version: 2
  ethernets:
    lab0:
      match: { macaddress: "52:54:00:5a:b0:10" }
      set-name: lab0
      addresses: [10.50.0.10/24]
```

---

## 8. `kind: Network` — and the boundary that fools everyone

```yaml
apiVersion: delonix.io/v1
kind: Network
metadata:
  name: lab-net
spec:
  driver: bridge
  subnet: 10.233.0.0/16
```

```
$ delonix network describe lab-net
Name:           lab-net
Driver:         bridge
Bridge:         dlxne9623e
Subnet:         10.233.0.0/16
Gateway:        10.233.0.1
```

### A `kind: Network` network is **not** a VM network

This is the confusion that costs the most time, so it's worth measuring it:

```
$ delonix network ls
NAME        DRIVER   BRIDGE       SUBNET
lab-net     bridge   dlxne9623e   10.233.0.0/16

$ virsh -c qemu:///system net-list --all
 Name      State    Autostart
 default   active   yes
 labnet    active   yes            ← a rede que as VMs libvirt usam
                                     (lab-net NÃO aparece)
```

Using one for the other fails, and delonix's message doesn't help:

```
$ delonix vm create x --backend libvirt --net-mode nat --bridge lab-net
error system call `vm` failed: failed to start the libvirt domain (KVM/permissions/image?)

$ virsh start x     # a causa verdadeira
error: Network not found: no network with matching name 'lab-net'
```

**The rule, in two lines:**

- `delonix network create` / `kind: Network` → **container** networks (the engine's
  SDN, rootless, inside the *holder*).
- **libvirt** VMs use **libvirt** networks, created with `virsh net-define`.
  `--bridge`/`spec.bridge` names one of them.

A **Cloud Hypervisor** VM is the exception: it really joins the container
SDN (`--network lab-net` gave it `10.233.254.141`). Except those addresses live inside
the *holder* and are not reachable from the host — only from another container on the same
network.

To cross the boundary there is `delonix vm bridge`, which **requires root** and says what
it's going to do before doing it:

```
$ delonix vm bridge lab-net          # sem --apply: só imprime o plano
warning DRY-RUN — the plan below needs root. Review it, then re-run with `--apply`:
  ip link add vbh72a57c1d type veth peer name vbs72a57c1d
  ip link set vbs72a57c1d netns 71944
  nsenter -t 71944 -n -- ip link set vbs72a57c1d master dlxn0536623e up
  ip addr add 10.233.255.254/16 dev vbh72a57c1d
  sysctl -w net.ipv4.ip_forward=1
  iptables -I FORWARD -s 192.168.122.0/24 -d 10.233.0.0/16 -j ACCEPT
  ...
```

`delonix vm unbridge <network>` prints the undo, rule by rule. A privileged
command that shows the plan and knows how to revert is rare; use the *dry-run* as
documentation.

Without privilege, `delonix vm reach` answers the practical question — which
published ports the VMs actually reach:

```
$ delonix vm reach
VM network gateway(s): 192.168.122.1, 10.10.100.1
warning Published on loopback only — NOT reachable from VMs:
CONTAINER             HOST PORT   BOUND TO
kaeso-db18            5433        127.0.0.1 (host only)
  fix: DELONIX_PUBLISH_ADDR=192.168.122.1 delonix net ingress publish <c> <port>
```

---

## 9. The complete lab

A miniature corporate network: authoritative DNS, DHCP, user
authentication, a workstation, NAS storage and a firewall.

Files in [`examples/_labs/lab-rede/`](../examples/_labs/lab-rede/). Everything that follows was
run; the outputs are the real ones.

### Topology

```
                    ┌───────────────────────── labnet — 10.50.0.0/24 ──┐
                    │  (rede libvirt isolada, SEM DHCP próprio)         │
                    │                                                  │
  ┌──────────┐      │  ┌────────────┐  ┌────────────┐  ┌────────────┐  │
  │ internet │──NAT─┼──│ lab-dns    │  │ lab-dhcp   │  │ lab-samba  │  │
  └──────────┘ default│ 10.50.0.10  │  │ 10.50.0.11 │  │ 10.50.0.12 │  │
                    │  │ BIND 9     │  │ ISC DHCP   │  │ Samba      │  │
                    │  └────────────┘  └────────────┘  └────────────┘  │
                    │                                                  │
                    │  ┌────────────┐  ┌────────────┐  ┌────────────┐  │
                    │  │ lab-cli    │  │ lab-nas    │  │ lab-fw     │  │
                    │  │ 10.50.0.50 │  │ 10.50.0.20 │  │10.50.0.254 │  │
                    │  │ (por DHCP) │  │ TrueNAS    │  │ OPNsense   │  │
                    │  └────────────┘  └────────────┘  └────────────┘  │
                    └──────────────────────────────────────────────────┘
```

Each VM has **two interfaces**, and the reason is practical: `default` gives it
outbound internet access, which cloud-init needs to install packages on first boot; `labnet`
is the lab's isolated LAN, so `lab-dhcp` can serve without
competing with libvirt's DHCP. **Two DHCP servers on the same broadcast
domain deliver whichever answer arrives first** — it's one of the hardest failures
to diagnose, and it's worth not building it on purpose.

| Name | Role | Address | Memory |
|---|---|---|---|
| `lab-dns` | BIND 9, zone `lab.ngola` | 10.50.0.10 | 1 GiB |
| `lab-dhcp` | ISC DHCP | 10.50.0.11 | 1 GiB |
| `lab-samba` | Samba, authentication and sharing | 10.50.0.12 | 1 GiB |
| `lab-cli` | workstation | 10.50.0.50 (via DHCP) | 1 GiB |
| `lab-nas` | TrueNAS SCALE | 10.50.0.20 | 4 GiB |
| `lab-fw` | OPNsense | 10.50.0.254 | 3 GiB |

### Step 0 — the lab network

`kind: Network` doesn't fit here (section 8): a **libvirt** network is needed, and
**without DHCP**, so `lab-dhcp` is the only one serving it.

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

Without `<forward>` the network is **isolated**: the VMs talk to each other and to the host, and don't go out
through this interface. Without a `<dhcp>` block libvirt doesn't serve addresses on it.

> The host becomes a **member of this LAN** — it has `10.50.0.1` on `virbrlab`. It's convenient for
> troubleshooting and worth knowing, because it means the lab's firewall is not
> between the host and the VMs.

### Step 1 — prepare and apply

The manifests carry `SSH_KEY_PLACEHOLDER`, so they can be shared without anyone's
key inside them:

```bash
cd examples/_labs/lab-rede
mkdir -p /tmp/labrun/cloud-init
KEY=$(cat ~/.ssh/id_ed25519.pub)
for n in dns dhcp samba cliente; do
  sed "s|SSH_KEY_PLACEHOLDER|$KEY|" cloud-init/$n.yaml > /tmp/labrun/cloud-init/$n.yaml
done
for f in 10-dns 20-dhcp 30-samba 40-cliente; do
  sed "s|./cloud-init/|/tmp/labrun/cloud-init/|" $f.yaml > /tmp/labrun/$f.yaml
done

delonix stack apply -f /tmp/labrun/10-dns.yaml
delonix stack apply -f /tmp/labrun/20-dhcp.yaml
delonix stack apply -f /tmp/labrun/30-samba.yaml
delonix stack apply -f /tmp/labrun/40-cliente.yaml
```

Each `apply` responds in about **1 s**; cloud-init then takes **2 to 4 minutes**
to install and configure the services. Follow along with:

```bash
delonix vm ssh lab-dns -- cloud-init status
```

> **Don't use `--prune` with these files.** The *stack* name comes from the manifest's
> **directory**, so the six files are the same *stack*: an `apply --prune`
> on one of them sees the other five VMs as orphans and deletes them. Either combine everything into a
> single multi-document file, or use `--name`, or don't use `--prune`.

### Step 2 — DNS

```
$ delonix vm ssh lab-dns -- 'ip -4 -o addr show | grep -v " lo "; systemctl is-active named'
2: enp1s0  inet 192.168.122.2/24 ...      ← saída para a internet
3: lab0    inet 10.50.0.10/24 ...         ← a LAN, por MAC
active

$ dig +short @10.50.0.10 nas.lab.ngola A       →  10.50.0.20
$ dig +short @10.50.0.10 -x 10.50.0.20         →  nas.lab.ngola.
$ dig +short @10.50.0.10 one.one.one.one A     →  1.1.1.1   (recursão)
```

Forward zone, reverse zone and forwarding. BIND listens **only** on `127.0.0.1` and
`10.50.0.10` and only accepts queries from `10.50.0.0/24` — an open resolver on the
internet is one of the easiest contributions to unintentionally make to an
amplification attack.

### Step 3 — DHCP

```
$ delonix vm ssh lab-dhcp -- 'systemctl is-active isc-dhcp-server; sudo ss -ulnp | grep -c ":67"'
active
1
```

`INTERFACESv4="lab0"` limits the service to the lab's interface. The MAC-based
reservation always gives the client the same address, which keeps DNS records and
firewall rules stable.

### Step 4 — Samba

```
$ delonix vm ssh lab-samba -- 'systemctl is-active smbd; sudo pdbedit -L'
active
ana:1001:
```

*Standalone* mode, not an Active Directory domain controller — an AD DC requires
also being the zone's DNS, and here the DNS is `lab-dns`. Changing that requires turning off
BIND and handing the zone to Samba: an architecture decision that deserves to be made on
purpose, not by drift.

### Step 5 — the client proves everything together

```
$ delonix vm ssh lab-cli -- '...'

===== 1. DHCP: o endereço veio do lab-dhcp?
10.50.0.50/24                          ← exactamente a reserva por MAC
ADDRESS=192.168.122.14                 ← a outra placa, servida pelo libvirt
ADDRESS=10.50.0.50

===== 2. DNS: o lab-dns resolve a zona?
10.50.0.12
dc.lab.ngola.

===== 3. SAMBA: autenticação real
	Sharename       Type      Comment
	publico         Disk
	IPC$            IPC       IPC Service (Samba do lab.ngola)
```

And genuinely authenticated writes, end to end:

```
$ smbclient //10.50.0.12/publico -U ana%... -c "put /tmp/prova.txt prova.txt; ls"
putting file /tmp/prova.txt as \prova.txt (56.6 kb/s)
  prova.txt    A    58   Wed Aug 12 15:13:52 2026

# no servidor:
-rwxr--r-- 1 ana labusers 58 Aug 12 15:13 prova.txt
ficheiro criado pelo lab-cli em 2026-08-12T15:13:52+00:00
```

The file arrived owned by `ana` and group `labusers` — authentication was actually
exercised, not bypassed via guest access (`map to guest = never`).

### Step 6 — the NAS

```bash
# o disco de dados tem de existir antes
mkdir -p ~/.local/share/delonix/vms/labdata
qemu-img create -f qcow2 ~/.local/share/delonix/vms/labdata/nas-dados.qcow2 10G

delonix stack apply -f 50-nas.yaml
```

TrueNAS served the interface **80 s** after boot. The API responds with the
`truenas_admin` account (not `root`):

```
$ curl -sk -u truenas_admin:... https://<nas>/api/v2.0/system/info
version: 25.10.5   hostname: truenas   memória: 5.8 GiB   cores: 2
```

#### The NAS needs a disk with a serial number

The first attempt to create the *pool* failed:

```
[EINVAL] pool_create.topology: Disks have duplicate serial numbers: None (vda, vdb).
```

virtio disks come out without a serial number and TrueNAS refuses to pool two
indistinguishable disks. `extraDisks` has no field to set it — it's the case the
XML escape hatch exists for:

```yaml
libvirtXmlOverlay:
  - |
    <disk type='file' device='disk'>
      <driver name='qemu' type='qcow2'/>
      <source file='.../nas-dados.qcow2'/>
      <target dev='vdb' bus='virtio'/>
      <serial>LABDATA01</serial>
    </disk>
```

Creating the *pool* is a one-time step, done via the web UI or the API:

```bash
curl -sk -u truenas_admin:<pass> -H 'Content-Type: application/json' \
  -X POST https://<nas>/api/v2.0/pool \
  -d '{"name":"labpool","topology":{"data":[{"type":"STRIPE","disks":["vdb"]}]}}'
# → labpool  ONLINE  9.5 GiB
```

#### Provisioning storage from the manifest

From here on, `kind: Volume` does the rest **without anyone opening the web UI**:

```yaml
kind: Volume
metadata: { name: lab-dados }
spec:
  provision:
    truenas:
      url: https://<nas>
      username: truenas_admin
      passwordSecret: nas-cred
      insecureTLS: true
      dataset: labpool/lab-dados
      quota: 2G
      owner: { uid: 1000, gid: 1000, mode: "0770" }
      share:
        networks: [10.50.0.0/24, 192.168.122.0/24]
        maprootUser: root
```

```
$ delonix stack apply -f 60-storage.yaml
volume/lab-dados: provisioned 192.168.122.116:/mnt/labpool/lab-dados (quota 2.00 GiB, 2.00 GiB free)
```

Confirmed on the NAS side: `quota: 2 GiB`, active NFS export with exactly the two
declared networks. And mounted from the client:

```
$ delonix vm ssh lab-cli -- 'sudo mount -t nfs <nas>:/mnt/labpool/lab-dados /mnt/nas; df -h /mnt/nas'
<nas>:/mnt/labpool/lab-dados  2.0G  0  2.0G  0%  /mnt/nas    ← a quota é real

$ ... | sudo tee /mnt/nas/do-cliente.txt
# e no dataset ZFS da NAS:
do-cliente.txt  58B  uid=0     used: 96 KiB   quota: 2 GiB
```

Three notes worth the time they cost:

1. **`apply` ends with an error on the host, and provisioning is done.** The
   local `mount -t nfs` needs `CAP_SYS_ADMIN`, which a *rootless* session doesn't
   have. The provisioning report comes out **before** the error, on purpose — an
   `apply` that died silently would leave the administrator not knowing that a
   dataset, a quota and an export already exist on the NAS.
2. **Without `nfs-common` on the client, `mount` fails and the write goes to the
   local disk** instead, looking like success. That's what happened on the first attempt. The
   client's manifest installs it.
3. **NFS does *root-squash*.** Without `owner:` and `maprootUser`, it mounts and you can't
   write. `maprootUser: root` is the classic `no_root_squash`: convenient in a
   lab, and a real concession outside of one.

**The data survives VM recreation.** At some point it was necessary to reduce the
NAS's memory, which forces `--replace` — which discards the system disk. The
*pool* was on the data disk, which is external:

```
$ curl ... /api/v2.0/pool/import_find
[{"name": "labpool", "guid": "10428492264621109666", "status": "ONLINE"}]
```

Imported, and the storage `stack apply` ran again exactly as before. It's a good
argument for keeping data off the system disk.

### Step 7 — the firewall

```bash
delonix stack apply -f 70-firewall.yaml
```

**The order of the interfaces is what decides the roles**, and it's easy to swap it without noticing:
OPNsense calls the first one LAN (`vtnet0`) and the second one WAN (`vtnet1`). So, unlike
the other VMs, here the primary interface is `labnet`:

```yaml
netMode: nat
bridge: labnet          # primária → vtnet0 → LAN
extraNics:
  - { type: network, source: default }   # segunda → vtnet1 → WAN
```

A firewall with WAN on `vtnet0` ends up with the rules inverted — protecting the
internet from the lab.

OPNsense is an *appliance*: it doesn't run cloud-init, and the engine **refuses**
`hostname`/`sshKeys` instead of accepting and ignoring them. It's configured via the console:

```
$ delonix vm console lab-fw
 LAN (vtnet0)    -> v4: 192.168.1.1/24        ← o valor de fábrica
 WAN (vtnet1)    -> v4/DHCP4: 192.168.122.5/24

  2) Set interface IP address
```

Option `2` → LAN → no DHCP → `10.50.0.254` → `24` → no gateway → **do not** activate the
DHCP server (the one serving DHCP in this lab is `lab-dhcp`).

With that done, from the LAN:

```
$ delonix vm ssh lab-dns -- 'ping -c3 10.50.0.254; curl -sk -o /dev/null -w "%{http_code}" https://10.50.0.254/'
3 packets transmitted, 3 received, 0% packet loss
rtt min/avg/max/mdev = 0.276/0.306/0.366/0.042 ms
200
```

And via the WAN, as it should be:

```
$ curl -sk --max-time 8 https://192.168.122.5/     → HTTP 000
```

Administration responds on the LAN and doesn't respond on the WAN. Which is, after all, what's
asked of a firewall.

### Running in phases

The whole lab needs about **11 GiB** of RAM. On a shared host, boot in
groups — the engine's resource admission refuses before creating anything at all,
but it's better not to get there:

```bash
# `vm start` leva UMA VM de cada vez — `start a b` dá "unexpected argument".
# Fase A — infra-estrutura de rede (2 GiB)
for v in lab-dns lab-dhcp; do delonix vm start $v; done

# Fase B — identidade e posto de trabalho (+2 GiB)
for v in lab-samba lab-cli; do delonix vm start $v; done

# Fase C — armazenamento (4 GiB)
delonix vm stop lab-samba && delonix vm start lab-nas

# Fase D — perímetro (2 GiB)
delonix vm start lab-fw

delonix vm ls                            # o que está de pé
delonix dashboard --scope vm             # painel interactivo
```

To tear everything down:

```bash
for v in lab-dns lab-dhcp lab-samba lab-cli lab-nas lab-fw; do delonix delete vm $v; done
virsh -c qemu:///system net-destroy labnet
virsh -c qemu:///system net-undefine labnet
```

---

## 10. Measured pitfalls

Each one cost real time while setting up this lab.

### `stop` used to delete snapshots, without saying so — fixed in v0.51.x

This was it, and it was measured here — **the commands below are the old version's**
(today they are `vm snapshot create`/`ls`/`restore`), kept only for the record:

```
$ delonix vm snapshot lab-probe base-limpa
base-limpa
$ delonix vm stop lab-probe && delonix vm start lab-probe
$ delonix vm snapshots lab-probe
                                  ← vazio, rc=0
$ delonix vm restore lab-probe base-limpa
error: Domain snapshot not found: no domain snapshot with matching name 'base-limpa'
```

`stop` runs `virsh undefine --managed-save --snapshots-metadata --nvram` so as not to
leave orphaned domains behind, and `--snapshots-metadata` was taking the snapshot
bookkeeping with it. The measurement that solved this: **`qemu-img snapshot -l` showed them
intact in the `qcow2` after `stop`** — `undefine` doesn't delete the snapshot, it deletes
only what points to it.

Today `stop` saves the `snapshot-dumpxml` of each one in `vms/<vm>/snapshots/` and the
next start returns them to libvirt. **A snapshot survives `stop`/`start`**,
and `vm snapshot ls` on a stopped VM lists them. What remains from this pitfall is the rule:
never ask libvirt about a stopped VM — it only knows defined domains.

### On Cloud Hypervisor, the address is computed — not observed

On that engine the address comes from arithmetic over the MAC, before the system boots
(that's what allows putting the VM under *namespace* isolation before it
exists). On libvirt it comes from a real *lease*, which is only handed out to a guest that
has booted enough to request it.

The difference matters when the guest does **not** boot. Until v0.51.0 `--wait`
returned in 62 ms announcing `is up` for a VM whose firmware was failing before the
kernel. Today it waits and says what it knows:

```
$ delonix vm create prova-ch --backend cloud-hypervisor --network lab-net \
      --wait --boot-timeout 60
prova-ch
warning vm 'prova-ch' is running but never answered at 10.233.254.240 — that
address is computed from the MAC, not observed, so it exists whether or not the
guest booted; `delonix vm console prova-ch` to watch the boot
real 1m0.322s
```

The warning is the whole story: the process is running, the address exists, and **nobody
answered on it**. That test's VM had a 448 KiB *overlay* — it hadn't written a
byte — and `ip neigh` returning `FAILED` from a container on the same network.

**A `--wait` that times out still exits 0**, so in a *script* check
the warning or probe yourself (`delonix vm ls --ports`, an `ssh`).

About the image, with a control: the golden `delonix-vm-k8s:1.34` **does boot** the kernel
on the same engine, network and firmware — so the network is fine and what varies is the
image. The `delonix-vm-base:*` ones don't boot with the `hypervisor-fw` that the installer places.
**For VMs on Cloud Hypervisor, use the golden one.**

### `vm image build --network` runs into the host

```
virt-customize: error: libguestfs error: passt exited with status 1
```

The error carries the remedy, and it's halfway there:

```bash
mkdir -p /tmp/delonix-run && chmod 700 /tmp/delonix-run
XDG_RUNTIME_DIR=/tmp/delonix-run delonix vm image build --network -t img:1.0 .
```

It gets past the AppArmor problem, but on this host `passt` still doesn't give name
resolution to the *appliance*:

```
W: Failed to fetch http://archive.ubuntu.com/... Temporary failure resolving
E: Unable to locate package bind9-utils
```

**Alternative used in the lab, and better on its own merits:** install via cloud-init on
the first boot. The VM already has libvirt's NAT network, with internet and DNS working
(verified: `HTTP/1.1 200 OK` from `archive.ubuntu.com`), and the configuration stays
declared in the manifest, in plain sight of anyone who reads it.

### `vm pull` on a connection that drops midway *(fixed in v0.51.0)*

While setting up this lab, a 276 MiB `vm image pull` died like this:

```
$ time delonix vm image pull ghcr.io/angolardevops/delonix-vm-base:debian-bookworm
error registry error: blob read: request or response body error
real 8m19s
```

It wasn't the clock — the ceiling has been 4 hours since v0.47.1 — it was the connection dropping. What
made this serious is that **there was no resumption**: the next attempt would restart
at byte zero, so on a bad connection the image might never finish, no matter how many times
it was retried.

Today downloading a blob **resumes with `Range: bytes=<n>-`**, with 5
attempts and one line per resumption. Stitching two ranges together is safe for the same
reason as always: the manifest digest is verified at the end, so either the bytes
check out or they are discarded. Covered by
`blob_retoma_uma_ligacao_cortada_a_meio` and by
`blob_recomeca_quando_o_servidor_ignora_o_range`.

**Resumption is within the same invocation** — it covers the connection dropping, not the process being
killed. Killing a `vm image pull` midway and launching it again still starts from the
beginning.

> A note of honesty about the original measurement: when this failed, this host's
> connection to `ghcr.io` was giving **416 KB/s**; hours later, the same measurement gave
> **9.7 MB/s** and the entire image downloaded in under 75 s. The defect was real and the
> fix is the right one, but the 416 KB/s was a bad moment for the network — not a
> characteristic of the host.

`delonix vm image ls-remote` works fine even on weak connections, because it only reads
manifests. If the network is genuinely bad, bring the images in by another means and
register them with `delonix vm image import`.

### One `apply` per file, one *stack* per directory

The *stack* name comes from the manifest's **directory** (unless there is a named
`kind: Stack`, or `--name`). The lab's six files are, as far as the engine is concerned, the
same *stack*:

```
$ delonix stack plan -f 50-nas.yaml
  -/+ Vm/lab-nas    — does not converge live: memory
  -   Vm/lab-samba  — no longer declared in the manifest — kept, `--prune` removes it
  -   Vm/lab-dns    — no longer declared in the manifest — kept, `--prune` removes it
  -   Vm/lab-cli    — no longer declared in the manifest — kept, `--prune` removes it
```

Without `--prune` nothing is removed — and that's why that option exists and is never the
default behaviour. But the plan shows the risk clearly, and `stack plan --prune`
shows it the way `apply --prune` would: the three VMs turn into removals and count toward
`--detailed-exitcode`.

### `--replace` discards the disk

It's written in the refusal (`--replace, which discards the disk`) and it's worth repeating,
because the change that triggers it is usually innocuous-looking — changing `memory: 6G` to
`4G` recreates the VM and loses the system disk. Important data stays on a
separate disk (that's what saved the NAS's *pool*) or on a volume.

### An `apply` that says `ensured` may not have changed anything

Changing `extraNics`, `libvirtXmlOverlay` or `netMode` in the manifest of a VM that **already
exists** has no effect: the reconciler only compares `disk`, `vcpus`, `memory`,
`network` and `backend`. The engine says so in the `Conditions` block at the end — which is easy
not to read when the previous line says `ensured`.

### `delonix vm ssh` right after `start` refuses the connection

```
ssh: connect to host 192.168.122.14 port 22: Connection refused
```

`vm start` returns in under a second — it defines the domain and starts it. The system
inside takes **30 to 60 s** to get SSH accepting connections. It's not a malfunction; in a
*script*, wait for the condition instead of a fixed time:

```bash
until delonix vm ssh lab-dns -- true 2>/dev/null; do sleep 5; done
```

### Name the image, instead of pointing at the file

While setting up this lab, an *appliance* declared by the `qcow2`'s **path**
escaped the cloud-init refusal that the same *appliance* declared by **name**
received — and it was handed a *seed* CD-ROM that the guest never reads. Today both
forms are refused the same way:

```
# por nome                    spec.disk: opnsense:26.1
# pelo caminho do ficheiro    spec.disk: /.../opnsense_26.1.qcow2
error invalid argument: hostname, sshKeys: this image is an appliance and does not
run cloud-init, so these would be silently ignored — configure it on first boot
(console or web UI), or pass your own `--seed` if you know the guest reads one
```

It's still worth **naming the image**: the name is what the metadata is
associated with that the engine uses without being asked — whether it's an *appliance*, and what vCPU, memory and
engine the image recommends (`resolve_vm_defaults`). `delonix vm image ls` shows the
available names.

---

## Quick reference

```bash
# ciclo de vida
delonix vm create <n> --disk <img> --backend libvirt --net-mode nat \
                      --vcpus N --memory NG --hostname <h> \
                      --ssh-key @~/.ssh/id_ed25519.pub --wait
delonix vm start|stop|restart|rm <n>          # rm -f força

# inspecção
delonix vm ls [--ports] [-o json] · status [n] · describe <n>... · dash [--once]

# acesso
delonix vm ssh <n> [-l user] [-- cmd] · console <n> · vnc <n>

# imagens
delonix vm image ls · ls-remote · pull <ref> · push <n> <alvo>
delonix vm image build -t <tag> [--network] . · convert <src> --to <fmt>
delonix vm image init

# instantâneos (libvirt, VM a correr)
delonix vm snapshot create|ls|restore|rm <n> [<s>]

# declarativo
delonix stack validate|plan|apply -f <f> [--dry-run] [--replace K/n] [--detailed-exitcode]
delonix explain Vm · delonix manifest schema

# rede
delonix network create|ls|describe|rm · vm reach
delonix vm bridge <rede> [--apply] · vm unbridge <rede>    # root
```

**Exit codes:** `0` success · `1` generic error · `2` invalid arguments (or
"there are changes" in `plan --detailed-exitcode`) · `4` not found · `5` conflict (also in
`plan --detailed-exitcode`, when the plan has a resource from another stack).
