# ADR-0084 — The rootless overlay routes its WireGuard to the holder (hostfwd)

**Status:** Proposed (2026-11-02) — the "proven" path of G4 (inter-node overlay),
**PROVEN end-to-end on 2 real VMs from the golden k8s**: container-to-container
**0 % loss in both directions** (~0.7 ms), with the VXLAN decapsulating on the
peer (`VXLAN RX > 0`). Took THREE fixes to get there, each necessary and all
measured: **#1** this ADR's WireGuard hostfwd (without it nothing listened on
51820, the handshake never came up); **#2A** `create_overlay` honouring
`--subnet` (both nodes on the same `/16`+gateway); **#2B** the holder's firewall
(`dlxinput`) accepting inbound VXLAN (`iifname "wgo*" udp dport 4789 accept`) —
without it the frame reached `wgN` decrypted but the terminal `ct state new drop`
ate it (`VXLAN RX=0`). Implemented on `feat/ipv6-gonogo`. Awaiting the owner's GO.
**Relates to:** `reports/go-nogo-alta-criticidade-20261101/g4-overlay-forwarding.md`
(the measurement that motivated it), ADR-0013 (inter-network routes), ADR-0082
(dual-stack IPv6).

## Context

The G4 bench (`scripts/e2e_overlay_multinode.sh`) ran against two REAL nodes
(2 VMs, `virbr0`, routable IPs, exchanged WireGuard pubkeys) and measured: `network
create --driver overlay` creates on both nodes, containers come up, **but inter-node
forwarding gives 100% loss**.

Root cause nailed down both on the node and in the code. The encrypted overlay
happens INSIDE the rootless holder: the WireGuard interface (`wg-up`, port 51820)
and the VXLAN uplink (port 4789, with the FDB pointing at the peer's `wg_ip` — so
VXLAN **rides** the WireGuard tunnel) live in the holder's netns. That netns only
reaches the outside via **slirp4netns**, which does outbound NAT and **does not
forward inbound**. Result: in the node's MAIN netns, `ss -ulnp` shows nothing on
UDP 51820 — the WireGuard handshake the peer sends to `<node-ip>:51820` reaches
the machine and hits a **closed port**. With no tunnel, there is no VXLAN or
forwarding.

The engine already has the missing piece: `slirp_add_hostfwd` routes
`<host>:port → <holder>:port`, used to publish container ports — but never for
the overlay's WireGuard port.

## Decision

When bringing up an encrypted overlay (`realize_overlay`), right after raising
WireGuard and its peers, **add a hostfwd on the slirp for UDP 51820**, so that
`<node-ip>:51820` enters the holder where WireGuard listens. This port only: the
VXLAN (4789) travels INSIDE the tunnel and needs no direct reachability.

Three choices, and why:

- **Bound to `0.0.0.0`**, not loopback (unlike a published port, which by default
  only binds to `127.0.0.1`). An overlay is inter-node by definition and the peer
  connects to the node's real IP; the tunnel is authenticated+encrypted by
  WireGuard, so exposing the UDP is the goal, not an added risk.
- **Idempotent and node-scale.** There is ONE WireGuard interface per node (a
  single `node.key`), shared by every overlay. The second overlay finds 51820
  already bound (slirp refuses a duplicate at the same address) and skips — the
  busy-port guard turns that into success, instead of slirp's opaque
  `add_hostfwd failed` (which does not distinguish a duplicate from a real
  failure).
- **No explicit teardown.** The hostfwd belongs to the slirp process and dies
  with it when the holder is torn down (`net netns down` / last network removed),
  so it never outlives the overlays it serves. No port left hanging between
  reboots.

## Alternatives considered

- **ROOTFUL overlay with WireGuard on a real node interface** (no slirp).
  Removes the problem at the root, but is a different execution/privilege model;
  left for a successor ADR if rootful multi-node becomes a goal.
- **Don't forward, document the overlay as single-host.** Rejected: contradicts
  G4's scope decision (Delonix wants to BE HA) and `--driver overlay` already
  advertises inter-node peers.

## Consequences

- The inter-node overlay now **forwards container-to-container end-to-end** —
  G4's "proven" path is now PROVEN on 2 real VMs (0% loss ping in both
  directions, ~0.7 ms). It took THREE fixes, each necessary:
  - **#1 — WireGuard hostfwd (this ADR):** without it nothing listened on 51820
    and the tunnel never came up.
  - **#2A — `--subnet` honoured** (`create_overlay`, separately): both nodes land
    on the same `/16`+gateway. Necessary but not sufficient.
  - **#2B — the holder's firewall accepts inbound VXLAN.** The root cause,
    nailed down via conntrack+nstat: the VXLAN frame reached the peer's `wgN`
    DECRYPTED but the device did not decapsulate it (`RX=0`) because the
    `dlxinput` chain has a terminal `ct state new drop` and the inbound VXLAN
    (`iifname wgo*`, udp 4789) is a NEW connection that fell into it. Fix:
    `dlxinput` now accepts `iifname "wgo*" udp dport 4789 accept` (the same
    allowlist discipline as the other rules — WireGuard has already
    authenticated the peer). After the fix, `VXLAN RX` rises and the ping
    passes. It was NOT the `--subnet` nor the `local/dev` binding (that was
    tried and reverted). See `g4-overlay-forwarding.md`.
- 51820/udp is exposed on every interface of the node while an overlay exists.
  This is the contract of an overlay node (as is `:51820` for any WireGuard),
  and the tunnel refuses anyone without the key.
- The proof is re-runnable: `scripts/e2e_overlay_multinode.sh --via ssh A B`.
