<!--
  Source of the "Limits" section of docs/compatibility.md.
  Edit here, then run `python3 scripts/gen_compatibility.py` to regenerate the doc.
  These limits are part of the engine's contract (mission H): they say where a
  compatibility claim deliberately stops, so a reader never mistakes "not built"
  for "broken".
-->

The limits below are part of the contract, not gaps to be closed. They say where a
compatibility claim deliberately stops.

- **Delonix is not a kube-apiserver.** Helm, Argo CD and Flux only work end-to-end in
  **Mode 1** (under a real Kubernetes). In **Mode 2** only the closed `kube play`
  subset applies. GitOps of the engine itself over Kubernetes is done by an external
  operator, never inside the engine.
- **No service mesh in Mode 2.** Istio and Linkerd only run over Kubernetes (Mode 1).
- **`kube play` has no controllers.** No rollout, no ReplicaSet, no HPA. What does not
  map to the closed subset (Deployment, Service, ConfigMap, Secret,
  PersistentVolumeClaim) is **refused by the field name**, never silently ignored.
- **Rootless refuses privileged workloads.** Cilium, Falco, Rook-Ceph, the Istio CNI and
  anything that needs privileged eBPF, `hostNetwork` or `privileged` run in **root mode
  only**; under rootless they are refused with "not implemented in rootless" — never
  "impossible".
- **The sandbox runtime class is not for privileged DaemonSets.** Privileged DaemonSets
  and `hostNetwork` workloads do not run inside a microVM; the request is refused.
- **Architectures:** only the ones a release publishes (today `amd64` and `arm64`).
- **Vitess, TiKV and a customer's etcd are workloads, not engine capabilities.** They run
  on the engine; the engine does not claim to operate them.
- **CNCF Sandbox admission depends on governance** (public repository, neutral
  governance), which is an owner's decision outside this campaign's scope — reported,
  not decided. See `reports/cncf-acceptance-2026-11-12/report.md` and
  `CNCF-SANDBOX-READINESS.md`.
