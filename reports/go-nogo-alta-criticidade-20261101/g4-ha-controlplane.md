# G4 — HA k8s control-plane on Delonix: PROVEN

> **Date:** 2026-11-03. **Scope:** the second half of the G4 "via proof" path
> (the first, the inter-node overlay, is in `g4-overlay-forwarding.md`). Criterion:
> an **HA (≥3 CP, stacked etcd, VIP) k8s control-plane that survives the loss of a
> CP, all on Delonix**. **Measured live**, not asserted.

## Setup (turnkey, one command)

```
delonix cluster kubeadm --name g4ha --control-plane 3 --workers 0 --cni default
```

Provisioned, on VMs of the golden `delonix-vm-k8s:1.36`, on a dedicated network:

- **3 control-planes** (`g4ha-cp1/2/3`), **stacked** (co-located) etcd;
- an **HAProxy load balancer** (`g4ha-lb`) as the control-plane endpoint (kubeadm's
  `--control-plane-endpoint`) — the HA VIP;
- **Cilium** as the CNI (the HA cross-node default), installed and awaited;
- **`delonix-cri` as each node's CRI** (the kubelet talks to it instead of
  containerd/CRI-O): `delonix-cri: next to delonix` + `delonix-cri re-execs it to
  spawn every container`. Confirmed in `kubectl get nodes`: `CONTAINER-RUNTIME
  delonix://0.1.0`.

Bootstrap result: `cilium agent Ready on 3/3 node(s); 3/3 node(s) Ready`.

## Baseline state (measured)

| Fact | Evidence |
|---|---|
| kubectl talks via the **HAProxy VIP** | `server: https://10.243.122.126:6443` (the CPs are `10.243.179.x`; the LB is `.122.126`) |
| 3 CP Ready, delonix runtime | `g4ha-cp1/2/3 Ready control-plane v1.36.5 … delonix://0.1.0` |
| stacked etcd + Cilium | `etcd-g4ha-cp{1,2,3}`, `cilium-*` all `Running` |
| **CNI works (real pod)** | `g4test` **Running**, Cilium IP `10.244.0.70`, `eth0` + ping-self ok — the path that the CRI entrypoint bug blocked, now live |

## Failover — loss of one control-plane (the criterion)

`delonix vm stop g4ha-cp1` (a CP with etcd), and right after, via the API through
the VIP:

| Observation | Result |
|---|---|
| kubectl via the HAProxy VIP | a momentary `TLS handshake timeout`, then **responds** (HAProxy forwards to cp2/cp3) |
| **etcd write during the failure** | `configmap/g4ha-failover created`; re-read afterwards: `survived=yes` — **2/3 quorum held** |
| **scheduling during the failure** | `kubectl run g4test2` → **Running** on `g4ha-cp3`, Cilium IP `10.244.2.80` |
| failure detection | after the grace period, `g4ha-cp1 NotReady`, `cp2/cp3 Ready` |

## Self-heal — the CP comes back

`delonix vm start g4ha-cp1` → **3/3 Ready again in ~20 s** (the etcd member
re-syncs, the kubelet re-reports). HA not only survives the loss but recovers when
the node returns.

## Measured bonus

The entire cluster was **stopped and cold-restarted** (all 4 VMs `shut off` →
`vm start`) and **reassembled itself in ~20 s** (etcd re-formed quorum, the
apiserver came up): the kubeadm + etcd state persists on disk.

## Verdict

The G4-HA criterion is **PROVEN on Delonix**: ≥3 CP + stacked etcd + VIP
(HAProxy) + **survives the loss of a CP** (live API, writes and scheduling
working, 2/3 quorum) + self-heal, with `delonix-cri` as the CRI and Cilium as the
CNI. Together with the inter-node overlay (`g4-overlay-forwarding.md`), **both
halves of the G4 "via proof" are proven**.

## Product note — candidate CLOSED (2026-11-03, ADR-0085)

At the time of this proof, HA could only be brought up via the **kubeadm-on-VMs**
path (`cluster kubeadm`); the turnkey `cluster create` (kind-mode, containers)
refused `--control-planes > 1`. **That candidate was closed next:** `cluster
create` now provisions an **HAProxy container** as the endpoint and performs the HA
bootstrap — `cluster create --control-planes 3` yields 3 CP + LB + stacked etcd
**in containers, without VMs**, and survives the loss of a CP (API via the LB,
writes with 2/3 quorum, scheduling). See ADR-0085.

## Reproducible

`delonix cluster kubeadm --name g4ha --control-plane 3 --workers 0 --cni default`,
`delonix vm stop g4ha-cp1`, `kubectl --kubeconfig <…> …`, `delonix cluster destroy g4ha`.
