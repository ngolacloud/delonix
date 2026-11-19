# ADR-0085 — `cluster create` (kind-mode) provisions a multi-control-plane HA setup

**Status:** Proposed (2026-11-03) — closes the gap that the G4 proof revealed: the
turnkey `delonix cluster create` (kind-mode, containerized nodes) refused
`--control-planes > 1` because it did not provision the stable endpoint (LB/VIP) that
kubeadm HA requires. **Implemented and proven live** on `feat/ipv6-gonogo`:
`cluster create --control-planes 3` brings up 3 CPs + an HAProxy container + stacked
etcd + kindnet, 3/3 Ready, and **survives one CP going down**. Awaiting the owner's GO.
**Relates to:** ADR-0084 (cross-node overlay), `reports/go-nogo-alta-criticidade-20261101/g4-ha-controlplane.md`.

## Context

G4 (via proof) called for an HA k8s control plane **on top of Delonix**. That was
proven through the **kubeadm-on-VMs** path (`cluster kubeadm`, which already provisions an
HAProxy on a VM). But the **turnkey, lightweight** path — `cluster create`, which runs
its nodes as **containers** (`kindest/node`) — refused HA:

```
DX-1000: --control-planes 3 is not supported yet: kubeadm HA needs a stable
endpoint (load-balancer/VIP) in front of the control-planes, which delonix does
not provision yet.
```

The refusal was honest (several CPs with `controlPlaneEndpoint` pointing at ONE
of them is not HA, it is a single point of failure dressed up as HA), but the
gap was a product one: the LB simply was not provisioned.

## Decision

In kind-mode, when `--control-planes > 1`, Delonix itself provisions the
stable endpoint — an **HAProxy container** on the cluster's network — and runs kubeadm's
HA bootstrap. Concretely (`kindmode::create_ha`):

1. **N control-plane containers** (`<name>-control-plane`, `-control-plane2`, …)
   on the cluster's network; NONE of them publishes the API port.
2. An **HAProxy container** (`<name>-external-load-balancer`, image
   `haproxy:lts-alpine`) with an `haproxy.cfg` that load-balances TCP `6443` across the CPs'
   IPs (reuses the VM path's `lb::build_haproxy_cfg`); publishes `api_port`
   to the host. Its IP on the cluster's network is the `controlPlaneEndpoint`.
3. `kubeadm init --upload-certs` on the leader, with `controlPlaneEndpoint = <lb>:6443`
   and the LB's IP in `certSANs`; the certificate key is read from
   `kubeadm init phase upload-certs`.
4. The other CPs run `kubeadm join` with a `JoinConfiguration` that carries the
   `controlPlane` block (its `localAPIEndpoint` + the `certificateKey`) —
   `cp_join_config_yaml`.
5. CNI (kindnet) once; **per-node PodCIDR authorization** (without this the
   bridge's anti-spoofing drops cross-node pod traffic — the same class as
   block #2B of the overlay); kubeconfig exported (host → LB → apiservers).

This is what upstream `kind` does with `kindest/haproxy`; here the LB is a plain
HAProxy container, from the same `build_haproxy_cfg` as the VM path.

## Proof (live, containers, no VMs)

`delonix cluster create --name g4k --control-planes 3 --workers 0 --cni default`
→ `cluster "g4k" ready (HA)`. 4 containers (`g4k-external-load-balancer` + 3 CPs),
`kubectl` via `127.0.0.1:6443` → the LB → 3/3 Ready, cross-node pod `Running`.
**Failover:** stop the leader CP → the API survives via the LB (routed to cp2/cp3),
keeps **writing** (configmap, 2/3 quorum) and **scheduling** (a new pod on cp2).

## Alternatives considered

- **keepalived/shared VIP** instead of a dedicated LB. Rejected for kind: the
  nodes are containers on a bridge; a container LB is simpler and mirrors kind.
- **Keep the refusal and point to the VM path.** Rejected: `cluster kubeadm`
  (VMs) already did HA; the gap was precisely that the lightweight turnkey path did not.

## Consequences

- `cluster create --control-planes N` (and the `mode: kind` manifest with
  `controlPlane.count > 1`) now give turnkey HA **without VMs**. The
  single-CP path stays unchanged.
- New dependency: the `haproxy:lts-alpine` image (pulled on-demand when HA is used).
- Network closure security (per-port authorized PodCIDRs) is the same as the
  multi-worker path; the LB does not filter (TCP passthrough), governance stays on the
  nodes.
