# RPC traceability matrix — CRI `runtime.v1` (34 RPCs)

State legend: **IV** implemented and validated (critest/crictl/kubelet) ·
**IN** implemented, validated only by gRPC round-trip/unit · **P** partial ·
**U** unimplemented (returns `UNIMPLEMENTED`, does not fake success) · **N/A**
not applicable with justification.

Source: `crates/interfaces/delonix-cri/src/{lib.rs,runtime_svc.rs,runtime_svc/lifecycle.rs}`.

## RuntimeService

| RPC | State | Validation / note |
|---|---|---|
| Version | IV | `runtime_api_version=v1`, name `delonix`; `grpc_status.rs` + crictl |
| RunPodSandbox | IV | rootless netns (holder/slirp); critest sandbox lifecycle |
| StopPodSandbox | IV | idempotent; forces `-t 0` (contract: grace already given); fixes `kubeadm reset` orphans |
| RemovePodSandbox | IV | only deletes the record if the engine's `rm -f` wins (RemoveContainer rule) |
| PodSandboxStatus | IV | critest |
| ListPodSandbox | IV | filter by id/state (unit test) |
| CreateContainer | IV | full security context; node cap-ceiling applied here |
| StartContainer | IV | critest |
| StopContainer | IV | critest; timeout honoured |
| RemoveContainer | IV | idempotent |
| ListContainers | IV | filter by sandbox (unit test) |
| ContainerStatus | IV | exit code/137 OOM, stable timestamps, real writable_layer |
| UpdateContainerResources | IV | honour-or-refuse cpuset/unified/hugepages (ADR 0038) |
| ReopenContainerLog | IV | recreates the file + shim follows the inode (rotation) |
| ExecSync | IV | separate stdout/stderr, exit, timeout; enters all ns incl. IPC |
| Exec | IV | SPDY/WebSocket; token with entropy (fail-closed) |
| Attach | IV | `logs -f`; stdin to PID 1 not supported (by design) |
| PortForward | **P** | server prepares URL; 2 portforward specs fail (real gap #12/#17) |
| ContainerStats | IV | cgroup v2 cpu/mem; eviction manager fed |
| ListContainerStats | IV | filter |
| PodSandboxStats | IV | cgroup v2 |
| ListPodSandboxStats | IV | filter |
| UpdateRuntimeConfig | IV | pod CIDR: honours CNI, warns on the native SDN (never a blind `Ok`) |
| Status | IV | real `RuntimeReady`+`NetworkReady` (ref-count); `capabilityCeiling` in verbose |
| CheckpointContainer | **U** | requires CRIU; optional; does not fake success |
| GetContainerEvents | **U** | Evented PLEG (optional); kubelet falls back to the generic PLEG |
| ListMetricDescriptors | IN | real (cgroup v2); critest v1.36.0 does not exercise it — `grpc_status.rs` |
| ListPodSandboxMetrics | IN | same |
| RuntimeConfig | IV | advertises `cgroupfs` (test `the_kubelet_is_told_cgroupfs`) |

## ImageService

| RPC | State | Validation / note |
|---|---|---|
| ListImages | IV | `repo_tags`/`repo_digests` split by reference type (**fixed** #18); `uid`/`username` (**fixed** #19) |
| ImageStatus | IV | same; shared `to_cri_image` mapping |
| PullImage | IV | creds from `imagePullSecrets` (user+pass); by tag and by digest |
| RemoveImage | IV | idempotent (kubelet GC) |
| ImageFsInfo | **P** | real `used_bytes`; `inodes_used` = 0 (follow-up); returns image+container fs |

## Coverage

- **IV**: 27 · **IN**: 2 · **P**: 3 (PortForward, ImageFsInfo, and the rootless
  network set) · **U**: 2 (optional).
- No unimplemented operation returns `Ok`. The `U`s are explicit `UNIMPLEMENTED`
  — the kubelet treats them as a missing feature.
