# Pre-release test campaign — SUT and environment

**Campaign ID:** `relcamp-bca34f6c` · prefix of created resources: `relc-` · network: `relc-net`
**Start date:** 2026-10-23

## 1. System under test (SUT)

| Field | Value |
|---|---|
| Repository | `angolardevops/delonix-runtime` (public, Apache-2.0) |
| Directory | `/home/walter/workspaces/NgolaCloud/projects/delonix-runtime` |
| Branch | `develop` |
| Commit | `bca34f6c3cfc366e32e4fb5650fbaf541cb46eed` |
| Working tree | **clean** at start (`git status --short` = 0 lines) — the SHA represents the tested state |
| Announced version | `delonix 5.1.1` (`commit: bca34f6c3 (+243 commits since v5.1.1)`, built 2026-10-23) |
| **Tested binary** | `target/release/delonix` built **at this commit** (`cargo build --release -p delonix-runtime-bin`, 3m17s) |
| **Digest (sha256)** | `725112b6454f6431bf6229137f01a10538e012cf64483a89fb9edd986213e328` |
| Install method | local build from the tree; **NOT** the binary in `~/.local/bin/delonix` (that one is `f49d3570c`, from 2026-10-18 — stale) |

> **Anti-mixing rule.** All tests run the freshly built binary at
> `target/release/delonix` (digest above). The binary installed in `~/.local/bin`
> (commit `f49d3570c`) was only used for the initial inventory of pre-existing
> resources and does **not** count as SUT.

## 2. Environment

| Field | Value |
|---|---|
| Host | `delonix` |
| OS | Zorin OS 18.1 (Ubuntu 24.04 "noble" base) |
| Kernel | `7.0.0-38-generic` x86_64 |
| CPU | 20 threads |
| RAM | 62 GiB total, ~30 GiB available |
| Disk `/` | 938 G, **95% used, 45 G free** — constrains large pulls, cloud image builds and VMs |
| Privileges | user `walter` (uid 1000); groups: `sudo`, `docker`, `kvm`, `libvirt`, `render`, `ollama` |
| cgroups | **unified v2**; controllers delegated to the user: **`cpu memory pids`** (io/cpuset **not** delegated) |
| Rootless | `subuid/subgid` = 100000:65536; `unprivileged_userns_clone=1`; `apparmor_restrict_unprivileged_userns=0` → rootless userns OK |
| TMPDIR | `/tmp` |

### Auxiliary tools

| Present | Absent |
|---|---|
| `runc`, `nft`, `iptables`, `slirp4netns`, `newuidmap` | `crun`, `skopeo` |
| `kind`, `kubectl`, `kubeadm` | — |
| `virsh`, `qemu-system-x86_64`, `cloud-hypervisor` | — |
| `mkcert`, `cloudflared`, `ngrok`, `pinggy` | — |
| **`docker` and `podman` ABSENT** (expected: delonix is the runtime) | — |

### VM providers

| Provider | State |
|---|---|
| libvirt/KVM | **available** (`virsh qemu:///system`): VM `opnsense` running, `pve92` powered off; many lab networks |
| Cloud Hypervisor | binary present |
| Proxmox | `pve92` appliance (powered off) in libvirt; PVE target documented in the project memory |
| OpenStack | not available on the host |

### Connectivity

- `registry-1.docker.io/v2/` → HTTP 401 (reachable; 401 = no auth, expected)
- `github.com` resolves (140.82.121.3)
- Online — offline tests (section 13) require a deliberate egress block in the test environment

## 3. PRE-EXISTING resources — to PRESERVE (do not touch)

> The campaign does **not** remove or alter anything below. It only touches resources with the `relc-` prefix.

**Running delonix containers (~1h):**

- `kaeso-odoo` (`kaeso-odoo:16`, `127.0.0.1:8069->8069`) — live Odoo stack
- `kaeso-db` (`pgvector/pgvector:pg15`) — on the `kaeso-net` network

**delonix networks:** `demos-net`, `k8s-net`, `kaeso-net` (in use), `kambacode-odoo20-net`, `kind`, `nsnet`, `pnet`

**libvirt:** VM `opnsense` (running), `pve92` (powered off); networks `br-lan/br-dev/br-backup/br-dmz/br-wan/br-pmx-*/cluster1`

**System service:** `delonix-engine prod --bind 0.0.0.0:9443` (pid 1682) — not the SUT; do not restart

**Images already cached (reusable without pull):** `hello-world`, `nginxinc/nginx-unprivileged:1.30`, `redis:7.4-alpine`, `ubuntu:22.04`, `python:3.13-slim`, `rabbitmq:4.0-management-alpine`, `mariadb:11`, `wordpress:6.7`, `kindest/node:v1.34.0`, `postgres:17-alpine`

## 4. Campaign limits and policy

- Small images preferred (hello-world, nginx-unprivileged, redis-alpine) — disk at 95%.
- No large cloud image pulls/builds without need; do not fill the disk.
- No publishing to public registries; push/pull only against the local registry.
- Exclusive resources with the `relc-` prefix; cleanup verified at the end of each path.
- No destructive operation on `kaeso-*`, the libvirt lab, or the `delonix-engine prod` service.
