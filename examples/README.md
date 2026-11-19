# Examples

Manifests are grouped by **API group** (each kind's `apiVersion`) — one
sub-directory per group, and **one file per kind**. Each file gathers that kind's
scenarios as `---`-separated YAML documents under a numbered bilingual header
(`── N. … ──`). Apply a file with `delonix stack apply -f examples/<group>/<kind>.yaml`.

## Convention

- Bilingual comments: the header block uses `PT:`/`EN:` lines; field comments
  use `# PT: … | EN: …` inline. Each document opens with a `── N. PT title ──` /
  `EN title` separator.
- Each scenario is COMPLETE and VALID (passes `delonix stack validate`), with no
  `...`, no invented fields, no real secrets. Important defaults are made explicit.
- The reference exemplar for the style is [`storage/objectstorage.yaml`](storage/objectstorage.yaml).

## Groups

| Group | `apiVersion` | Kinds (files) |
|---|---|---|
| `compute/` | `compute.delonix.io/v1beta1` | container, pod, virtualmachine, systemcontainer, workload |
| `networking/` | `networking.delonix.io/v1beta1` | network (includes Proxmox/SDN provider), service, networkpolicy, networkroute, networkaccessrule, networkgateway, ippool, dependency |
| `gateway/` | `networking.delonix.io/v1beta1` | httproute, ingress, gateway |
| `storage/` | `storage.delonix.io/v1beta1` | volume (local/raw/nfs/cifs/webdav/share/provision/pool), storagepool, objectstorage |
| `artifact/` | `core.delonix.io/v1beta1` | image, app |
| `core/` | `core.delonix.io/v1beta1` | secret, stack |
| `security/` | `policy.delonix.io/v1beta1` | runtimepolicy |
| `gitops/` | `core.delonix.io/v1beta1` | gitopssource |
| `cluster/` | `cluster.delonix.io/v1beta1` | kubernetescluster (kind/ssh/vm modes + CNI profile + external etcd) |
| `_labs/` | — | lab & helper apps (lab-rede, compose-webapp, delonix-temp) — not manifests of this engine |
