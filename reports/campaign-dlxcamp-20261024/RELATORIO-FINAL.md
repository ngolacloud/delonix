# Final report — Real-test campaign dlxcamp-20261024

Integration/reliability campaign that **precedes** the release evaluation. It did not publish anything.

## 1. Artefact and exact state tested
- Binary **compiled from the code itself** (not the installed one, which was 45 commits behind and without Docker verbs at the root).
- Initial state `fix/stabilize-develop-after-merges` @ `257a0ee4` (dirty working tree, diff `c062d290`). The owner checked out to **`develop` @ `558e5f20`** mid-campaign (concurrent operation — known memory).
- Final binary recompiled **clean from develop `558e5f20`**: digest `350a6320e67cb0f80bb37e74fff4ebe24abd6976ff64afdabe16d9c77bc80d50`, `--version` = `delonix 5.1.1 · 558e5f201 (+258 commits) · built 2026-10-24`.
- Delta from the initial state to develop in compilable code = only `pt.po` (i18n) and `image.rs` inside `mod tests` (does not enter the release) → **the results obtained before the checkout remain valid** (identical runtime).

## 2. Environments and versions
- Zorin OS 18.1 (Ubuntu noble), kernel 7.0.0-38-generic, x86_64, 20 vCPU, 62 GiB RAM.
- Rootless (uid 1000); cgroup2 **delegated: cpu/memory/pids** (not cpuset/io).
- Tools: docker, podman, buildah, kind v-node 1.34.0, kubectl, kubeadm, virsh/qemu, cloud-hypervisor, mkcert, cloudflared, ngrok.
- VM images: `delonix-vm-base:ubuntu-24.04`, `k8s-golden`, proxmox appliance.
- **Disk: initial 5 GB blocker resolved** with a reversible `cargo clean` of delonix-paas (+35 GB) after authorization; 27 GB at the end.

## 3. Requirements coverage (SEPARATE from the success rate)
37 cases in the matrix, across 13 families.
- **Executed coverage (PASSED+FAILED+PARTIAL): 24/37 = 65%.**
- **Not demonstrated: 13/37 = 35%** (9 NOT EXECUTED, 1 BLOCKED, 1 NOT APPLICABLE, + 2 partials already counted in the execution).

## 4. Success rate of the EXECUTED cases
Of the 24 executed: **20 PASSED · 1 FAILED (gap) · 3 PARTIAL/defective.**
- Clean success rate = 20/24 = **83%**; counting partials as non-full, 20/24.
- (Not inflated: the 13 not demonstrated are explicitly listed, not excluded.)

## 5. Complete end-to-end flows demonstrated
- **Rootless container**: run→ps→exec→logs→stop(graceful)→start→rm, with process and cgroup verified in the OS; published port + real external HTTP; bind mount `:ro`; effective cpu/mem limits in the hierarchy.
- **Hot port swap** (distinctive): port swapped without restarting the process (pid unchanged).
- **Network/scope**: internal DNS, communication by name, egress; isolation between named namespaces (ping 100% loss); `default` shared by design.
- **IaC**: validate/plan/apply/idempotency/destroy of a Pod.
- **VM on both backends** (libvirt + Cloud Hypervisor): create→guest alive over SSH (cloud-init done, vcpu/mem effective)→stop/start/restart→destroy with no orphans; the owner's VMs intact.
- **Kubernetes cluster (kind-mode, no Docker)**: control-plane Ready v1.34.0 → nginx Deployment + Service → **pod→service traffic by DNS name and by ClusterIP** → clean destroy.
- **Public pinggy tunnel**: URL `https://…free.pinggy.net` served the synthetic origin over the internet.
- **init+template `go`**: project generated → `go build` + `go test` green; secrets generated; compose valid offline.
- **Safe GC**: dry-run distinguishes in-use/declared/debris without destroying anything of the owner's.

## 6. Defects and severity
| ID | Sev | Summary |
|----|-----|--------|
| DEF-002 | P2 | `stack plan`/`apply` says "unchanged" when changing a Pod's `command` — it neither converges nor signals (incomplete declarative convergence; `--replace` resolves it) |
| DEF-003 | P2 | `tunnel expose` leaves `ssh -R` orphaned when the parent is killed — public exposure persists until SIGKILL |
| ACHADO-1 | P3 | `run` without a PIDs-limit flag (Docker parity gap, §7) |
| DEF-001 | P3 | `--help` of `--namespace` does not document the shared `default` exception (ADR-0011) |

No P0/P1 (blocking) defect found in the executed flows.

## 7. Fixes and regressions
- No code fix applied (the scope prioritized execution/evidence; the defects are left with minimal repro for a targeted fix).
- False positives discarded through investigation (avoided non-existent defects): `default` isolation (ADR-0011), exit codes masked by `| tail`/`| head` (cpuset exit 69, strict exit 1), poorly-designed OOM proof, state-vs-config drift, VM `permission denied` due to missing `-i`.

## 8. Compatibility limitations
- `--cpuset` and IO cgroup: controllers not delegated on this host (rootless) → explicit and correct refusal (fail-closed), with a documented fix.
- Direct SSH host→guest does not route in CH rootless (slirp topology); the way is `delonix vm ssh` (confirmed in the audit in memory).

## 9. Performance (point measurements, NOT absolute capacity)
- VM Cloud Hypervisor create→running: **7.8 s**; libvirt: **14 s**.
- kind single-node cluster to control-plane Ready: **~2 min** (consistent with the memory ~2m45s).
- Graceful container stop of PID1 with no handler: 10.3 s (grace→SIGKILL, = docker).
- Incremental release build of the state: 7m16s (initial) / 4m40s (develop).

## 10. Cleanup state
- **0** `dlxcamp-*` containers/networks/clusters/VMs remaining.
- **0** orphan processes (dlxcamp, ssh pinggy, cloud-hypervisor), **0** cgroup leaves, **0** VM files on disk.
- Owner resources **intact**: 10 images, 5 networks, VMs opnsense/pve92.
- The delonix-paas `cargo clean` is reversible (it recompiles when the owner returns to paas).

## 11. Risks still open (not demonstrated in this campaign)
- **KUBEADM offline + HA + kube-vip + CNI** (§13) — the heaviest flow, NOT executed.
- **API/MCP REST/gRPC parity** (§18), **observability/HTTPS/stress/SonarQube** (§16), **IMAGE build+push/pull by digest** (§7), **multiple clusters** (§14), **recovery via supervisor kill** (§19), **NETWORK-04 persistence post-restart**, **cloudflare/ngrok tunnels** — all NOT EXECUTED.
- **Proxmox** (§10) BLOCKED (appliance powered off).

## 12. Recommendations for the release evaluation
1. Address **DEF-002** before the release: silently incomplete declarative convergence is a real operational risk (the operator trusts a false "unchanged").
2. Address **DEF-003**: public exposure that survives the stop is a security risk; tie the `ssh -R` lifecycle to that of the parent process and add `tunnel stop`/`ls`.
3. Fix the P3s (`--namespace` text; evaluate a PIDs flag) — low cost.
4. Schedule a second round focused on the 35% not demonstrated, prioritizing **kubeadm offline** and **API/MCP**, which are load-bearing for the value proposition.
5. The core (container/image/network/scope/VM/cluster/init/tunnel/GC) proved **solid and without blockers** in the executed flows — an acceptable base to move forward, conditioned on the two P2s.
