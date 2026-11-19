# Traceability matrix — Campaign dlxcamp-20261024

**Artefact under test**
- Repo: `/home/walter/workspaces/NgolaCloud/projects/delonix-runtime`
- The tested state started at `fix/stabilize-develop-after-merges` @ `257a0ee4` (diff `c062d290`); the owner checked out to `develop` @ **`558e5f20`** mid-campaign — final binary recompiled clean from develop: digest **`350a6320…`**, `--version` = `558e5f201 (+258)`, built 2026-10-24.
- Delta initial binary↔develop = only `pt.po` (i18n) + `image.rs` in `mod tests` (does not enter the release) → pre-checkout results valid.
- Host: Zorin OS 18.1, kernel 7.0.0-38, x86_64, 20 vCPU, 62 Gi RAM; rootless uid 1000; cgroup2 delegated: cpu/memory/pids (NOT cpuset/io).
- Providers: docker/podman/buildah/kind/kubectl/kubeadm/virsh/qemu/cloud-hypervisor/mkcert/cloudflared/ngrok; libvirt (opnsense, pve92).
- Disk: 5 GB at startup → +35 GB via a reversible `cargo clean` of delonix-paas → 27 GB at the end.

**States:** PASSED · FAILED · BLOCKED · NOT EXECUTED · NOT APPLICABLE

| ID | Requirement | Expected result | State | Evidence |
|----|-----------|--------------------|--------|-----------|
| CLI-01 | `--version` | consistent with state | **PASSED** | `558e5f201 (+258)` exit 0 |
| CLI-02 | main help + per-resource | exit 0 | **PASSED** | help 84L + 8 resources exit 0 |
| CLI-03 | invalid → error+exit≠0 | useful stderr | **PASSED** | subcmd/flag → exit 2 |
| CLI-04 | missing required arg | clear error | **PASSED** | `run` without IMAGE → exit 2 |
| CLI-05 | root↔grouped verb equivalence | identical | **PASSED** | ps≡container ls≡container ps; images≡image ls (identical JSON) |
| CLI-06 | JSON output | valid | **PASSED** | `ps -o json`→`[]` exit 0 |
| CONTAINER-01 | complete life cycle | observable in the OS | **PASSED** | process+cgroup rootless; stop 10.3s (SIGTERM ignored PID1→SIGKILL); rm with no orphan |
| CONTAINER-02 | port + external HTTP | HTTP 200 origin | **PASSED** | `127.0.0.1:18090->80`; curl→`dlxcamp-20261024-OK` |
| CONTAINER-03 | bind mount R/W | data/permissions | **PASSED** | `:ro` blocks writes; content visible |
| CONTAINER-04 | effective cpu/mem limits | in the hierarchy | **PASSED** | `memory.max=67108864`, `cpu.max=50000 100000` |
| CONTAINER-04b | `--cpuset` (controller not delegated) | fail-closed refusal | **PASSED** | DX-6000 exit 69, does not create; message "not implemented, not impossible" |
| CONTAINER-04c | PIDs limit | apply | **FAILED (gap)** | no `--pids-limit` flag (ACHADO-1) |
| CONTAINER-05 | `--rm`, detached, exit via wait | consistent | **PASSED** | `wait` prints exit 3 (= docker wait) |
| CONTAINER-06 | negative errors | diagnostic+exit≠0 | **PASSED** | nonexistent img exit 4; name conflict DX-5000 exit 5; port conflict DX-1000 exit 1 |
| IMAGE-01 | pull by digest + inspect | stable digest | **PASSED** | pull busybox (linux/amd64); inspect tags/id/size |
| IMAGE-02 | build (cache/cancel/partial failure) | correct artefact | **NOT EXECUTED** | time; the generated template's `go build`/Dockerfile OK as a partial proxy |
| IMAGE-03 | push→pull round-trip by digest | recovers | **NOT EXECUTED** | required a local registry; time |
| NETWORK-01 | topology incl. `delonix0` | consistent | **PASSED** | SDN bridges in a dedicated netns (rootless); the owner's networks preserved |
| NETWORK-02 | DNS + egress + communication | resolves/connects | **PASSED** | nslookup+wget by name→origin; egress 1.1.1.1 OK |
| NETWORK-03 | hot port swap without restarting proc | new one serves, same pid | **PASSED** | `+18090 -18080`: new one serves, old one dies, **httpd pid unchanged** |
| NETWORK-04 | persistence after engine restart | rules restored | **NOT EXECUTED** | daemonless; explicit restart not exercised |
| SCOPE-01 | isolation between scopes | blocking | **PASSED** | alpha↔beta: wget timeout + ping 100% loss; `default` shared (ADR-0011, by design) |
| SCOPE-02 | identical names in different scopes | resolves correctly | **PASSED** | `web` in alpha+beta coexist; engine requires `ns/name` qualification |
| MANIFEST-01 | validate/plan/apply/reapply/drift/rm | idempotent+convergent | **PARTIAL (1 defect)** | validate/plan/apply/idempotency/destroy OK; **DEF-002**: changing `command` → "unchanged" (neither converges nor signals) |
| VM-01 | libvirt cycle + no orphans | VM lives and cleans up | **PASSED** | guest alive (ssh, cloud-init done, 2vcpu); real virsh domain; destroy 0 orphans; the owner's VMs intact |
| VM-02 | Cloud Hypervisor native cycle | VM lives and cleans up | **PASSED** | guest alive (hostname/nproc/mem effective); real CH process; stop/start/restart/destroy with no orphans |
| VM-03 | Proxmox create/snapshot/destroy | no orphans | **BLOCKED** | appliance `pve92` shut off; would require booting + DELONIX_PROXMOX_* credentials |
| VM-04 | OpenStack | — | **NOT APPLICABLE** | not an engine provider (refused on load; ADR-0039 Proposed with no backend) |
| CLUSTER-01 | kind: create/kubeconfig/DNS/Service/workload+traffic/remove | app serves traffic | **PASSED** | control-plane Ready v1.34.0; nginx Deployment+Service; pod→svc by **DNS name** and **ClusterIP** serve nginx; clean destroy |
| CLUSTER-02 | load local image into kind | pod runs local image | **NOT EXECUTED** | time (CLUSTER-01 covered traffic) |
| KUBEADM-01 | kubeadm offline golden+kube-vip+CNI+HA | Ready offline | **NOT EXECUTED** | very heavy (VMs+blocked egress+HA); see separate audit in memory |
| CLUSTER-03 | multiple clusters + kubeconfig preserved | distinct contexts | **NOT EXECUTED** | time |
| TUNNEL-01 | pinggy HTTP(S) + origin responds | public endpoint serves | **PASSED** | `https://…free.pinggy.net`→`dlxcamp-20261024-OK`; "origin ready"; no software on the visitor |
| TUNNEL-02 | invalid credential/reconnection/stop | correct handling | **PARTIAL (1 defect)** | **DEF-003**: killing the parent leaves `ssh -R` orphaned (exposure persists until SIGKILL) |
| TUNNEL-cf/ngrok | cloudflare/ngrok | serves | **NOT EXECUTED** | cloudflare edge unreliable in the sandbox (memory); ngrok without a token |
| API-01 | CLI/REST/gRPC/MCP parity | consistent+authoriz. | **NOT EXECUTED** | required booting node-api; time |
| RECOVERY-01 | controlled failures → resume | consistent state | **NOT EXECUTED** | supervisor kill/resume not exercised explicitly |
| RECOVERY-02 | GC removes neither in-use nor data | preserves in-use | **PASSED** | `system prune --dry-run`: only stopped/debris; live ones preserved; DECLARED≠DEBRIS; dry-run destroyed nothing of the owner's |
| INIT-01 | init non-interactive/name/destination | consistent project | **PASSED** | template `go` generated complete (git, CI, compose, Delonixfile) |
| INIT-02 | generated project: build/test | app compiles/tests | **PASSED** | `go build` exit 0; `go test ./...` all ok |
| TEMPLATE-01 | structure/no placeholders/secrets/compose | valid | **PASSED** | 0 placeholders; `.env` secrets generated + in .gitignore; `compose config` exit 0 |

## Findings / defects
- **ACHADO-1 (CONTAINER-04c, P3):** `run` does not expose a PIDs limit (`--pids-limit`). Docker parity gap (§7).
- **DEF-001 (SCOPE, P3):** `--help` of `--namespace` does not mention the shared `default` exception (ADR-0011). Doc, not behaviour.
- **DEF-002 (MANIFEST, P2):** `stack plan`/`apply` reports "unchanged" when changing an existing Pod's `command` — it neither converges nor signals (violates declarative convergence; `--replace` resolves it).
- **DEF-003 (TUNNEL, P2):** killing the `tunnel expose` process leaves `ssh -R` orphaned — public exposure persists until SIGKILL.
