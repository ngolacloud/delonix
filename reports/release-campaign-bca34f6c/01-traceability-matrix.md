# Traceability matrix — campaign `relcamp-bca34f6c`

Consolidates the campaign prompt's requirements with the repository's contracts
(`docs/testing/folhas-inventario.md`, measured 2026-10-20 @ `2663c846`; this
campaign re-measures @ `bca34f6c`). **Requirements coverage ≠ success rate** —
see the final section of the report.

States: `PASSED` · `FAILED` · `BLOCKED` · `NOT EXECUTED` · `N/A (justified)`.
This file is living: the state is updated as each path runs.
Evidence = file under `evidence/` or a report sheet.

## States resolved at the end of the campaign (see `99-final-report.md`)

| ID | Final state | Evidence |
|---|---|---|
| CLI-1,2,3,4 | **PASSED** | `02-cli.md` |
| CLI-5 | **BLOCKED** (docker/podman absent) | `02-cli.md` |
| CONTAINER-1..5 | **PASSED** | `03-containers.md` |
| IMAGE-1,2,3 | **PASSED** | `04-images.md` |
| NETWORK-1,2,3,4 · SCOPE-1,2 | **PASSED** | `05-network.md` |
| IAC-1 (validate/reject/apply/reapply/idempotent/down) | **PASSED** | `07-iac.md` |
| IAC-1 (drift→reconcile) | **FAILED** (DEF-003 P2) | `07-iac.md` |
| IAC-1 (`kind:` stack apply) | NOT EXECUTED | — |
| INIT-1 (+structure/secrets/manifest) | **PASSED** | `06-init-templates.md` |
| TEMPLATE-1 (run generated project) | NOT EXECUTED | — |
| CLUSTER-1 (kind + app + traffic) | **PASSED** | `08-cluster.md` |
| CLUSTER-2 (kind workers/config) | PARTIAL/NOT EXECUTED | `08-cluster.md` |
| CLUSTER-3 (kubeadm offline HA) | **BLOCKED** (disk/time) | `08-cluster.md` |
| CLUSTER-4 (multi-cluster) | NOT EXECUTED | `08-cluster.md` |
| VM-1 (libvirt cycle) | **PASSED** (verified in virsh) | `09-vm-and-api.md` |
| VM-2 (cloud-hypervisor) | NOT EXECUTED | `09-vm-and-api.md` |
| VM-3 (proxmox) | **BLOCKED** (NotConfigured) | `09-vm-and-api.md` |
| VM-4 (cloud image build) | **BLOCKED** (disk) | `09-vm-and-api.md` |
| TUNNEL-1 (pinggy round-trip) | **PASSED** | `10-tunnels.md` |
| TUNNEL-1 (ngrok/cloudflare establish) | **PASSED**/edge-FAILED(environment) | `10-tunnels.md` |
| TUNNEL-2 (stop/remove) | **PASSED** | `10-tunnels.md` |
| API-2 (MCP tools/list + risk) | **PASSED** | `09-vm-and-api.md` |
| API-1 (parity across 4 surfaces) | PARTIAL | `09-vm-and-api.md` |
| RECOVERY-1 (resilience) · RECOVERY-2 (GC) | **PASSED** | `12-recovery-gc.md` |
| PERFORMANCE-1 · §16 security/observ. | NOT EXECUTED | `99-final-report.md` §11 |

| ID | Requirement (prompt §) | Repo source | Test case | Preconditions | Expected result | Evidence | State |
|---|---|---|---|---|---|---|---|
| CLI-1 | §6 `--version`, help, help per resource | CLI sheets (300) | `delonix --version`, `--help`, `<grp> --help` | fresh binary | version=HEAD commit; coherent help | `02-cli.md` | NOT EXECUTED |
| CLI-2 | §6 invalid commands/flags, exit codes | plan §2 | invalid arguments → `DX-` + exit code | — | stable DX code, exit≠0, no effect | `02-cli.md` | NOT EXECUTED |
| CLI-3 | §6 structured output `-o json/yaml` | plan §8 | `-o json` against `docs/schema/v1` | — | stable schema | `02-cli.md` | NOT EXECUTED |
| CLI-4 | §6 direct/grouped equivalence (`ps`↔`container ls`) | memory `root-docker-verbs` | compare outputs | — | equivalent | `02-cli.md` | NOT EXECUTED |
| CLI-5 | §6 Docker/Podman comparison | `docs/paridade-docker-podman.md` | swap executable | docker/podman | measured parity | — | **BLOCKED** (docker/podman absent) |
| CONTAINER-1 | §7 pull→rm rootless cycle | CLI sheets class A | nginx-unprivileged full cycle | online/cache | real state in kernel/netns/fs | `03-containers.md` | NOT EXECUTED |
| CONTAINER-2 | §7 exec, logs, wait, exit code | CLI sheets | exec echo; logs; wait | container up | observed result | `03-containers.md` | NOT EXECUTED |
| CONTAINER-3 | §7 effective CPU/mem/PIDs limits | plan §1 | `--memory/--cpus/--pids` + read hierarchy | cgroup v2 | effective values in the hierarchy | `03-containers.md` | NOT EXECUTED |
| CONTAINER-4 | §7 error cases (missing img, name/port conflict, ignored signal) | plan §2/3 | provoke each failure | — | correct refusal class, no orphans | `03-containers.md` | NOT EXECUTED |
| CONTAINER-5 | §7 `--rm`, detached, env, user, entrypoint | CLI sheets | variations | — | documented behaviour | `03-containers.md` | NOT EXECUTED |
| IMAGE-1 | §7 inspect/digest | CLI sheets | `image inspect` | cached image | stable digest | `04-images.md` | NOT EXECUTED |
| IMAGE-2 | §7 build | CLI sheets | `delonix build` minimal Dockerfile | — | correct final artefact | `04-images.md` | NOT EXECUTED |
| IMAGE-3 | §7 push+pull by digest (local registry) | plan §1 | local registry → push → pull | local registry | recovers by digest | `04-images.md` | NOT EXECUTED |
| NETWORK-1 | §8 real topology (`delonix0`/bridges) | `docs/guia-rede.md` | reconstruct topology | — | bridge/veth/nft observed | `05-network.md` | NOT EXECUTED |
| NETWORK-2 | §8 port publishing + external HTTP | CLI sheets | `-p` + external curl | — | HTTP 200 outside the container | `05-network.md` | NOT EXECUTED |
| NETWORK-3 | §8 port re-publishing while running | `e2e_ports.sh` | change port without restarting proc | container up | process intact, new/old port | `05-network.md` | NOT EXECUTED |
| NETWORK-4 | §8 DNS, isolation between scopes | CLI sheets | 2 networks, resolution + blocking | — | DNS ok; cross-scope blocked | `05-network.md` | NOT EXECUTED |
| SCOPE-1 | §8 effective cgroups v2 delegate | plan §1 | limited load, read hierarchy | cpu/mem/pids delegated | effective values | `05-network.md` | NOT EXECUTED |
| SCOPE-2 | §8 logical namespace vs Linux ns vs context | kinds sheets | identical names in different scopes | — | correct resolution | `05-network.md` | NOT EXECUTED |
| INIT-1 | §15 `init` wizard/non-interactive/name | CLI sheets + `init-accept.sh` | generate project | — | structure with no placeholders/secrets | `06-init-templates.md` | NOT EXECUTED |
| TEMPLATE-1 | §15/16 generated project runs (build/test/doctor/dev) | `e2e-all-templates.sh` | make doctor/dev + functional request | toolchains | app responds; Redis/RabbitMQ | `06-init-templates.md` | NOT EXECUTED (risk-based) |
| IAC-1 | §9 compose/stack validate→apply→reapply→drift→rm | `docs/gitops.md` | IaC cycle | — | real state, no recreation, ownership | `07-iac.md` | NOT EXECUTED |
| CLUSTER-1 | §12 simple kind + app + traffic | `e2e_cluster_kind.sh` | create cluster, deploy, curl Service | kindest/node cache | traffic served | `08-cluster.md` | NOT EXECUTED |
| CLUSTER-2 | §12 workers/mounts/portmap/kubeconfig | `kindconfig.rs` | real kind config | — | fields honoured/rejected by name | `08-cluster.md` | NOT EXECUTED |
| CLUSTER-3 | §13 kubeadm offline (k8s-golden, kube-vip, CNI, HA) | `docs/guia-vm-lab.md` | VMs + init + join + CNI + CP failure | k8s-golden image, VM provider | HA within the contract | — | **BLOCKED** (golden image + disk 45G) |
| CLUSTER-4 | §14 multi-cluster + preserved kubeconfig | — | 2 clusters, contexts | resources | entries preserved | `08-cluster.md` | NOT EXECUTED |
| VM-1 | §10 libvirt/KVM cycle create→start→network→rm | `delonix-provider-libvirt` | small VM | virsh + image | guest boots, cloud-init, clean rm | `09-vm.md` | NOT EXECUTED |
| VM-2 | §10 Cloud Hypervisor cycle | `delonix-provider-cloud-hypervisor` | CH VM | CH binary + image | ditto | `09-vm.md` | NOT EXECUTED |
| VM-3 | §10 Proxmox provider | `delonix-proxmox` | create/snapshot/destroy | PVE live | no orphans in the provider | `09-vm.md` | NOT EXECUTED (pve92 powered off) |
| VM-4 | §11 cloud image build + boot + cloud-init | `images/` | public build flow | disk | boots, no builder secrets | — | **BLOCKED** (disk 45G) |
| TUNNEL-1 | §17 cloudflare/pinggy/ngrok HTTP(S) + origin | memory `tunnel-audit` | synthetic app + tunnel + public curl | clients present | public endpoint responds | `10-tunnels.md` | NOT EXECUTED |
| TUNNEL-2 | §17 invalid credential, unavailable origin, reconnection, stop | `tunnel.rs` | provoke failures | — | clear error, supervision | `10-tunnels.md` | NOT EXECUTED |
| API-1 | §18 CLI vs REST vs gRPC vs MCP (allowed/denied op) | node-api/mgmt/mcp sheets | same op across surfaces | servers | coherent identity/authz/validation | `11-api-mcp.md` | NOT EXECUTED |
| API-2 | §18 MCP `tools/call` does not bypass the core | mcp sheets (E3) | MCP client | — | no arbitrary privilege | `11-api-mcp.md` | NOT EXECUTED |
| RECOVERY-1 | §19 interruption (SIGINT/SIGKILL) mid-mutation | case 6 sheets + `chaos.sh` | kill mid-way, run again | — | recovers/cleans, zero orphans | `12-recovery-gc.md` | NOT EXECUTED |
| RECOVERY-2 | §19 GC with active resources | memory `startup-cgroup-gc` | `system prune` with live containers | containers up | used ones not removed | `12-recovery-gc.md` | NOT EXECUTED |
| PERFORMANCE-1 | §16 `make stress`/bench with SLO | `bench.sh` | defined workload | — | measured RPS (not absolute) | `13-performance.md` | NOT EXECUTED (risk-based) |

## Capabilities promised but potentially absent (to be classified as a requirement FAILURE, not N/A)

- CLI-5 (Docker/Podman side-by-side): the reference binaries are not installed → **BLOCKED** (environment), not N/A. The repo has a prior measured comparison (`docs/comparacao-medida.md`); this campaign does not re-measure it live.
- CLUSTER-3 / VM-4: depend on the `k8s-golden` image and on disk space (45 G free). **BLOCKED (environment)** until the image and space are available; it is not N/A.
