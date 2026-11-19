# HA multi-node — `delonix cluster create --control-planes 3` (0.1.2)

Part (c) of the campaign: proof of **multi-node high availability** in rootless mode, without
VMs and without Docker — kind-mode (the nodes are containers in the Delonix engine itself).

## Environment
- Engine `delonix 0.1.2` (release), rootless host, isolated root (`/tmp/dlxha`).
- Command: `delonix cluster create --name ha012 --control-planes 3`.
- Images: `kindest/node:v1.37.0` (nodes) + `haproxy:lts-alpine` (load balancer).

## What came up (reproducible)
```
✓ Provisioning the HAProxy load balancer ⚖️ 4.8s
✓ Starting control-plane (kubeadm init) 🕹️ 1m30s
✓ Joining 2 control-plane(s) 🎮 2m49s
✓ Waiting for all 3 node(s) to be Ready ⏳ 56.5s
cluster "ha012" ready (HA)
```

## Verification (from inside the cluster)

**3 control-planes Ready:**
```
NAME                   STATUS   ROLES           VERSION
ha012-control-plane    Ready    control-plane   v1.37.0
ha012-control-plane2   Ready    control-plane   v1.37.0
ha012-control-plane3   Ready    control-plane   v1.37.0
```

**stacked etcd with a 3-member quorum** (`etcdctl member list`):
```
240d31b144fc1d03, started, ha012-control-plane3, ...:2380, ...:2379
3e868f92e46e4a94, started, ha012-control-plane2, ...:2380, ...:2379
9cf0ef4b1f69551a, started, ha012-control-plane,  ...:2380, ...:2379
```

**HAProxy VIP** (stable endpoint in front of the 3 CPs):
```
haproxy:lts-alpine  Up  127.0.0.1:6443->6443/tcp  ha012-external-load-balancer
```

4 containers in total (3 nodes + 1 load balancer).

## Teardown
`cluster destroy ha012 --force` → `cluster 'ha012' removed (4 node(s))`; 0 clusters,
0 containers, no residue.

## Verdict
**Multi-node HA PROVEN in rootless mode.** `cluster create --control-planes 3` delivers a
Kubernetes control plane of **3 Ready replicas**, with **stacked etcd in a 3-member
quorum** and an **HAProxy as VIP** — all in kind-mode (containers in the engine), without VMs
or Docker, on this host's rootless profile. It complements the single-node case (Part 2) and
the multi-node HA already proven in other sessions via kubeadm/VMs.

## Left for follow-up (require a rootful VM)
`cluster kubeadm` (VM provisioning + bootstrap), CRIU (`container checkpoint`/
`restore`/`clone`), and `bucket inspect`/`rm` (live S3).

## Artefacts
- `ha-create.log` — the HA bring-up run (milestones).
- `evidence.jsonl` — lab-ephemeral record.
