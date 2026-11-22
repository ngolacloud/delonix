# G4 — inter-node overlay: proof on 2 real nodes + root cause

> **Date:** 2026-11-02. **Scope:** the "via proof" path of G4 (decided by the
> owner: Delonix wants to BE HA). Question: does `--driver overlay` (VXLAN+WireGuard)
> forward traffic between two nodes? **Measured**, not asserted.

## Setup

Two real VMs of the golden `delonix-vm-k8s:1.36` (ubuntu 24.04), on the `virbr0`
libvirt bridge, **routable to each other** (`192.168.122.175` ↔ `.210`, host-to-host
ping 0% loss). On each VM I ran the **0.1.0 binary of the current line** (copied
via scp; `delonix --version` = 0.1.0 on both), with an isolated `DELONIX_ROOT=/tmp/dlxr`.
Dependencies the golden did not ship and which were installed: `wireguard-tools`
(`wg`), `slirp4netns`; and `sysctl kernel.apparmor_restrict_unprivileged_userns=0`
(the binary in `/tmp` does not have the AppArmor profile that `install.sh` writes).

Flow: `network node init` on both → pubkey exchange → `network create mesh
--driver overlay --vni 42 --wg-ip … --peer <ip>=<pub>=<wgip>` on both (rc0,
peer with the correct pubkey confirmed in `network inspect`) → `run -d --net mesh`
a busybox on each → ping between the overlay IPs.

## Result: it does NOT forward

| Direction | Result |
|---|---|
| container A → container B (overlay IP) | **100% packet loss** |
| container B → container A | **100% packet loss** |

Identical to what the 2-netns simulation had already given. Two REAL routable nodes,
pubkeys exchanged, slirp present — and still zero forwarding.

## Root cause (pinned on the node and in the code)

On the node, `net netns status` shows `ingress UP — … slirp <pid>`: the rootless
holder is behind **slirp4netns**. And in the VM's MAIN netns:

```
$ ss -ulnp | grep -E '51820|4789'
(nothing)
```

**Nothing listens on UDP 51820 (WireGuard) or 4789 (VXLAN) on the node's real interface.**
WG and VXLAN are configured INSIDE the holder's netns (infra.rs: `wg-up`
port 51820 around ~3274, VXLAN port 4789 around ~4710), but `slirp_add_hostfwd`
(lib.rs ~2101) is only used for **published container ports** — never for
51820/4789. So the WireGuard handshake the peer sends to `<node-ip>:51820`
reaches the VM and hits a **closed port**: the tunnel never establishes, and without a
tunnel there is no VXLAN and no forwarding.

This explains — and confirms with data — the honest `Partial` in the matrix
(`provider_report.rs:113`: "inter-node forwarding needs a second node the battery
does not have"; "traffic across two nodes not measured"). Now it has been measured: **it does
not forward**, and we know exactly why.

## Secondary finding: `--subnet` ignored (bug)

`network create --driver overlay --subnet 10.244.0.0/16` → the overlay landed on
**`10.234.0.0/16`** (auto), on both nodes, each choosing independently.
For an L2 that is supposed to be shared across nodes, (a) the option was swallowed
silently (the class of bug the repo chases) and (b) nothing coordinates the /16 across
nodes → two nodes may assign the same IP. Reproduced in the netns sim AND the VMs.

## What this means for G4

The rootless overlay, as it stands, **is not fit for multi-node HA**: the data-plane
does not cross nodes. Two blocks to close before "via proof" can pass:

1. **Route the overlay to the holder.** When an `overlay` network exists,
   add hostfwds in slirp for **UDP 51820 (WG)** and **UDP 4789 (VXLAN)**
   (`slirp_add_hostfwd` already exists; it just needs wiring to these ports), so that
   `<node-ip>:51820/4789` enters the holder. It is SDN-core with
   firewall/security implications (which address to bind, idempotency, teardown, interaction
   with `fwdeny`/governance) — it deserves design/ADR, not a blind patch.
2. **Honour `--subnet`** in the overlay driver + coordinate the /16 across nodes.

Alternative to (1): a ROOTFUL overlay mode where WG/VXLAN live on a real
node interface (no slirp). Also a design decision.

## Fix for block #1 (ADR-0084) — measured AFTER, on the same VMs

`realize_overlay` now adds a slirp hostfwd for UDP 51820 bound to
`0.0.0.0` (`infra::set_overlay_wg_hostfwd`). Re-run on the 2 VMs with the binary
carrying the fix:

| | BEFORE | AFTER |
|---|---|---|
| `ss -ulnp` on the node, UDP 51820 | nothing | `slirp4netns … 0.0.0.0:51820` (on both nodes) |
| WireGuard handshake | never | **completes** on both (`wg show`: `latest handshake: …`) |
| transfer over the tunnel | 0 | node-a 10.27 KiB rx / 7.23 KiB tx; node-b 7.99 / 10.06 |
| VXLAN over the tunnel | — | `10.42.0.1 ↔ 10.42.0.2:4789` (container ARP encapsulated and sent) |

In other words: the **inter-node WireGuard+VXLAN transport works** — exactly what
was not happening. Two more fixes were still needed for container-to-container ping to pass.

## Block #2 — decomposed, measured and RESOLVED (2026-11-02)

**#2A — `--subnet` ignored: RESOLVED.** `create_overlay` now honours
`--subnet` (via `base_from_subnet`, like the bridge), with a unit test. Two nodes
with the same `--subnet` land on the same `/16` and gateway — confirmed live (both
`10.244.0.0/16`, gw `10.244.0.1`). **Necessary but NOT sufficient:** with both
nodes already on the same `/16`, forwarding was still 100 % loss.

**#2B — VXLAN decapsulation on the peer: RESOLVED.** The root cause, pinned by
`conntrack` + `nstat` + the nft chain:

| Observation | Measurement |
|---|---|
| VXLAN frame from ca reaches node-b's `wgN` (decrypted) | `tcpdump wgN`: `10.42.0.1 > 10.42.0.2:4789 VXLAN vni 42 ARP who-has …` ✓ |
| does the VXLAN device decapsulate (before the fix)? | **`ip -s link` RX = 0** ✗ |
| delivered to the local stack? | **`IpInDelivers` ≈ 0** ✗ |
| conntrack of the inbound flow | `src=10.42.0.1 … dport=4789` marked **NEW** |
| counter that MOVED | **`dlxinput`: `ct state new counter … drop`** (+14…23 packets) |
| what it was NOT | `--subnet`, nor rp_filter (I forced 0), nor the `local/dev` binding (tried and reverted — `RX=0` all the same) |

The `dlxinput` chain (the holder's input hook) is an allowlist with a terminal
`ct state new drop`; it accepts `lo`, `tap0` (WG comes in there, via slirp — which is why
the handshake passed), DNS/DHCP and ICMP. The inbound VXLAN arrives on the WG interface
(`wgo<vni>`) as UDP 4789, a **NEW** connection, was in no accept rule → it fell to the
drop. **Fix:** `dlxinput` now accepts `iifname "wgo*" udp dport 4789 accept`
(VXLAN only, only on the WireGuard interfaces — which only deliver traffic from authenticated
peers; the same allowlist discipline as the other rules). Test hardened
in `os_servicos_do_holder_nao_sao_alcancaveis_de_um_container`.

## FINAL PROOF — end-to-end on the 2 VMs (after the three fixes)

```
### FORWARDING A->B
64 bytes from 10.244.189.89: seq=0 ttl=64 time=0.736 ms
4 packets transmitted, 4 packets received, 0% packet loss
### FORWARDING B->A
4 packets transmitted, 4 packets received, 0% packet loss
### VXLAN device RX em node-b:  2684 bytes  31 packets   (era 0)
```

The inter-node overlay **forwards container-to-container between two real nodes**, 0 %
loss in both directions, ~0.7 ms. The "via proof" path of G4 (overlay part)
is proven. (The k8s HA control-plane is the other half of G4 via-proof.)

## Reproducible

`scripts/e2e_overlay_multinode.sh --via ssh <userA@nodeA> <userB@nodeB>` runs
this flow and the assertions against any two nodes. The script header carries
the procedure and these findings.
