# Final report — pre-release test campaign `relcamp-bca34f6c`

> This campaign provides **evidence** for the release decision; it does **not** publish the product.
> Requirements coverage and the success rate of executed tests are presented **separately**.

## 1. Exact artefact and state tested

| | |
|---|---|
| Binary | `target/release/delonix` built **at this commit** (`cargo build --release`, 3m17s) |
| Version | `delonix 5.1.1` · `commit: bca34f6c3 (+243 since v5.1.1)` · built 2026-10-23 |
| Digest sha256 | `725112b6454f6431bf6229137f01a10538e012cf64483a89fb9edd986213e328` |
| Branch/commit | `develop` @ `bca34f6c3cfc366e32e4fb5650fbaf541cb46eed`, **clean working tree** |
| MCP sidecar | `delonix-mcp` built at the same commit (for §18) |
| Anti-mixing | binary installed in `~/.local/bin` (`f49d3570c`, stale) **not** used as SUT |

## 2. Environments and versions

Zorin OS 18.1 (Ubuntu 24.04), kernel 7.0.0-38, 20 threads, 62 GiB RAM, disk **96%/44 G free**.
cgroup v2 (`cpu memory pids` delegated; io/cpuset **not**). Rootless OK (subuid/userns).
Auxiliaries: runc, nft, slirp4netns, kind, kubectl, kubeadm, virsh/qemu, cloud-hypervisor, mkcert,
cloudflared, ngrok, pinggy. **docker/podman absent** (delonix is the runtime). Cached images
(nginx-unprivileged, redis-alpine, ubuntu, kindest/node v1.34.0, k8s-golden). Online.
Providers: libvirt+cloud-hypervisor available/Ok; proxmox/opnsense NotConfigured.

## 3 & 4. Requirements coverage vs success rate (SEPARATE)

**Coverage** (of the prompt's requirements, not "percentage of greens"):

| State | IDs | No. |
|---|---|---:|
| **PASSED** | CLI-1..4, CONTAINER-1..5, IMAGE-1..3, NETWORK-1..4, SCOPE-1..2, IAC(validate/reject/apply/reapply/idempotent/down), INIT-1..6, CLUSTER-1, VM-1, TUNNEL(pinggy full, ngrok/cloudflare establish), API-2(MCP), RECOVERY-1..2 | ~30 |
| **FAILED** | IAC drift-reconcile (DEF-003); TUNNEL cloudflare edge round-trip (environment) | 2 |
| **BLOCKED (environment)** | CLI-5 (docker/podman absent), CLUSTER-3 (kubeadm offline HA — disk/time), VM-3 (proxmox NotConfigured), VM-4 (cloud image build — disk) | 4 |
| **PARTIAL / NOT EXECUTED** | IAC(kind stack apply), TEMPLATE-1 (run generated project), CLUSTER-2 (kind workers/config), CLUSTER-4 (multi-cluster), VM-2 (cloud-hypervisor), API-1 (parity across 4 surfaces), PERFORMANCE-1, §16 security scan/sbom/sign, §16 observability | ~9 |

**Success rate of EXECUTED tests:** of the ~32 cases actually run, **~30 PASSED, 1 product FAILURE
(DEF-003), 1 environment FAILURE (cloudflare edge)**. The rate was **not** inflated by excluding the
blocked cases — they are listed above and count against coverage.

## 5. Complete paths demonstrated (end-to-end, with independent verification)

1. **Rootless container**: pull→run→exec→**HTTP 200**→limits **effective in the kernel** (mem 128 MiB/cpu 0.5)→logs→RW bind-mount→wait(42)→stop→start→restart→**rm with no orphans** (cgroup/port/proc).
2. **Image**: build→push to local registry→**digest matches the registry's**→rm→**pull by digest**→identical content.
3. **Network**: **hot port re-publishing without restarting the PID**; isolation between namespaces **by name AND by IP** (nft drop); DNS; effective cgroup delegate.
4. **IaC compose**: validate(fail-closed)→apply→**idempotent reapply (same id)**→down with ownership.
5. **init**: 2 templates generated, 0 placeholders, **random secrets per project**, `.env` gitignored, valid manifest offline.
6. **kind cluster (without Docker)**: create→node Ready→**deploy app + traffic (port-forward 200 + DNS pod→service)**→clean destroy.
7. **libvirt VM**: create k8s-golden→**IP via DHCP + ping 2/2**, confirmed in `virsh`→stop/start→**destroy (qcow2/overlay/xml) verified in libvirt**.
8. **pinggy tunnel**: expose→public URL→**public fetch 200 correlated in the origin log (edge XFF)**→delete.
9. **Daemonless GC**: orphan cgroup leaf **opportunistically reclaimed** (even by a `ps`) without touching the live ones.
10. **MCP**: initialize+tools/list over the protocol (15 tools, classified risk).

## 6. Defects and severity

| ID | Sev | Summary | State |
|---|---|---|---|
| [DEF-001](defects/DEF-001-doctor-advertises-pids-limit-flag.md) | P3 | `system doctor`/`resources` advertise a non-existent `--pids-limit` flag (Docker spelling, guarded by a test in `run`, but free text in `system.rs` lagged behind) | OPEN |
| [DEF-002](defects/DEF-002-rm-cannot-target-namespaced-container-by-name.md) | P3 | `rm` does not accept `--namespace`; a container outside `default` is only removable by ID (asymmetry with `ls`) | OPEN |
| [DEF-003](defects/DEF-003-compose-up-does-not-restart-stopped-service.md) | **P2** | `compose up` does not reconcile a stopped service to Running (contradicts its own help and docker-compose) — it reconciles existence, not execution | OPEN |

Minor observations (not formalised as defects): `cluster delete <name>` gives a confusing error instead of
suggesting `destroy`; stopping a tunnel with `pkill` of the wrapper leaves `ssh`/`ngrok` children (the clean
route is `delete gateway`); SIGKILL to `cluster create` mid-way left an orphan SDN network (`dlx-relc-k8s`)
not auto-reclaimed (removed by hand).

## 7. Fixes and regressions executed

No code fix applied — by design for this round: preserve reproduction and **do not mix
change with measurement**. The 3 defects have minimal reproductions and closure criteria recorded.

## 8. Compatibility limitations

- **Docker/Podman side-by-side not measurable** (absent). Previously measured parity lives in `docs/comparacao-medida.md` (not re-measured live).
- `-o yaml` promised in `plano-folhas.md §8` but `ps -o` only offers `table|json` (contract DISCREPANCY — low).
- `rmi` is single-image (docker is variadic); `compose` has no global `-f` (uses directory discovery).
- **cloudflare edge** not reliable in this sandbox (525/timeout) — ngrok/pinggy reliable.

## 9. Performance results

**NOT EXECUTED** (`make stress`/`bench.sh` with SLO). Only incidental measurement: kind control-plane Ready
in ~1m07s (startup), libvirt VM to IP in ~34s, nginx pull ~6s. These do **not** represent absolute capacity.

## 10. Cleanup state

**Clean and verified.** 0 `relc-*` resources (containers/networks/vms/clusters/tunnels/images/cgroup leaves);
0 pending tunnel/ssh/ngrok/cloudflared processes; ports released. **Preserved**: `kaeso-odoo`/`kaeso-db`
up, 5 pre-existing networks, 6 pre-existing tunnels, VM `opnsense` running, `delonix-engine prod` service.
Disk not full (kept at ~96%, never pushed). Cached images (`registry:2`, etc.) reusable — they are not orphans.

## 11. Risks still open (for the release decision)

1. **Incomplete IaC reconciliation (DEF-003, P2):** reapply does not recover stopped services — affects declarative recovery and compose parity.
2. **kubeadm offline HA (§13) unproven:** the `k8s-golden` image exists and boots, but the complete path (multi-VM, kube-vip, CNI profiles, CP failure, blocked egress) was **not exercised** — the product's most complex capability is left without live evidence.
3. **Security/supply-chain (§16) not measured this round:** `image scan/sbom/sign/verify`, SonarQube, `make sec` — not run. The scanner exists but its coverage/CVE base was not verified.
4. **Observability and load (§16) not measured:** Prometheus/Grafana/traces, `make stress`/SLO.
5. **API parity (§18) partial:** only MCP validated by the protocol; the SUT's REST/gRPC/docker-api not served this round.
6. **Robustness to mid-mutation interruption (§19) rests on the repo harness** (`chaos.sh`), not re-run here; SIGKILL to `cluster create` left an orphan network — the resilience of cluster creation to an abrupt kill warrants verification.

## 12. Recommendations for the release assessment

- **Address DEF-003 before release** (or reclassify `compose up` and fix the help/parity promise).
- **Run the §13 path (kubeadm offline) with disk headroom** — it is the highest-risk gap; the infrastructure (golden image, `cluster kubeadm`) is present.
- **Execute the security/supply-chain and observability battery** before any "production-ready" seal — today these are evidence gaps, not negatives.
- DEF-001/DEF-002 are P3 cosmetic/UX — fixable together, low risk.
- **Strong, demonstrated core:** rootless container/image/network/scope/GC, libvirt VM, kind cluster with traffic, pinggy tunnel, init — all with independent verification. The **single-node rootless path (containers + kind + libvirt VM)** has solid live evidence at this commit.

---
*Evidence: `reports/release-campaign-bca34f6c/` (`00`–`12` sheets, `defects/`, `evidence/` logs). Matrix: `01-traceability-matrix.md`.*
