# ADR-0013: Routed topologies — external gateway/DNS, subnets and VLANs, without leaving rootless

**Status: Accepted (2026-08-12).** Nothing here is implemented yet. It exists so the first line of
code is written against a decided boundary rather than discovered halfway. Tier B's GO/NO-GO spike
has been run — see "Spike result" — and its answer changed what tier B is.

## Decision taken

Split the request into three tiers by the PRIVILEGE each one needs, and ship them in that order,
because the first is free, the second turned out to be an exemption rather than a dataplane (the
spike below), and the third is not rootless at all:

| Tier | What it buys | Privilege | Verdict |
|---|---|---|---|
| **A — the address space becomes real** | arbitrary CIDRs, several subnets, a declared gateway/DNS | none (rootless) | **GO**, unblocked |
| **B — routing between networks** | subnet ↔ subnet, namespace ↔ namespace through a declared path | none (all inside the holder's netns) | **GO** — spike done, see below |
| **C — 802.1Q VLANs on a physical NIC** | trunk to a real switch, `macvlan`/`ipvlan` realized | `CAP_NET_ADMIN` in the **host's** init-netns | **opt-in privileged**, never the default |

Tier C is the one that cannot be made rootless, and saying so up front is the point of this ADR.

## Context — five things measured, not assumed (2026-08-12)

1. **A network's registry entry is ONE OCTET.** `~/.local/share/delonix/networks/kaeso-net`
   contains the single line `210`. Everything else — the bridge name, the `10.<n>.0.0/16` space,
   the `.0.1` gateway, the IPAM range — is DERIVED from it. There is no CIDR stored anywhere.
2. **`--subnet` only started meaning anything in v0.47.0**, and only to pick that octet: the
   accepted space is `10.<200-254>.0.0/16` and nothing else. `172.20.0.0/16` or a `/20` are
   refused, naming the form that works.
3. **`--gateway` is already fail-closed**, and correctly: a bridge network's gateway is the first
   address of its subnet, and passing a different one is refused rather than ignored. So
   "point this network at MY gateway" is **new semantics**, not the relaxation of a check.
4. **DNS is a thread inside the holder** (`infra::dns_server_main`), answering A records for
   `<name>.<ns>.delonix.internal` and forwarding the rest. There is no per-network resolver, no
   forwarder list, and no way to say "this subnet resolves against 10.0.0.53".
5. **There is no 802.1Q anywhere in the tree.** `macvlan`/`ipvlan` exist in the declarative store
   and `network create` WARNS that they were not realized (`Realized=False`,
   `reason=DriverNotImplemented`) rather than pretending. The reason is privilege, not missing
   code, and that distinction is why tier C is separate.

And one fact about the layer above: cross-namespace isolation today is **nftables chains
per workload** (`fwcont`, verdict map by IP). It decides whether a packet is allowed. It does not
route, and it has no notion of a path between two subnets.

## Decision

### Tier A — the address space becomes real

The registry stops being an octet and becomes a record with a CIDR. This is the change everything
else waits on, and it is the one with no privilege question at all: the holder already owns its
netns, and giving its bridge `172.20.4.0/22` instead of `10.210.0.0/16` needs nothing it does not
already have.

* `spec.subnet` accepts any private CIDR that does not overlap an existing network, `/16` to `/28`.
* `spec.gateway` may name an address inside that subnet that is NOT the derived `.1` — which is
  what "an external gateway" means for a rootless SDN: the holder keeps routing, and the named
  address is where it forwards to.
* `spec.dnsServers: []` — the resolvers handed to workloads on this network, replacing the
  holder's forwarder for them. The internal `.delonix.internal` zone keeps being answered locally;
  the ADR's rule is that a declared external resolver **adds** a forwarder and never removes the
  internal zone, because losing service discovery is not a networking option.
* **Migration is the hard part, and it is decided:** a record holding a bare octet is read as
  `10.<octet>.0.0/16` — which is exactly what it always meant — and rewritten on the next write.
  No flag day. This is the same promotion the `base=<n>` line already does.

### Tier B — routing between networks

A `kind: Network` gains no routing field. Instead a new document type describes the PATH, because
a route is a relationship and belongs to neither end:

```yaml
kind: NetworkRoute        # name provisional
spec:
  from: frontend          # network or namespace
  to: backend
  via: gateway-vm         # optional: a workload that forwards (a firewall appliance)
```

Rationale: putting `routes:` inside a `kind: Network` makes the same relationship expressible from
both sides, and two documents that disagree about one route is the class of bug this repo already
paid for with `FirewallPolicy` (two policies for the same target/direction are REFUSED for exactly
this reason).

**Composition with what exists is the whole risk**, and the rule is: routing decides where a packet
MAY go, the existing `fwcont` chains decide whether it goes. A route never implies an allow. A
namespace boundary crossed by a route still needs a `kind: Dependency` or an explicit policy — the
same way a container on a shared bridge is still isolated by namespace today.

**The spike this gated on has been run** — see "Spike result" below. Its answer is that the
forwarding ALREADY exists and an explicit pairwise drop is what closes it, so tier B never needed
the dataplane this section was written to justify. What is left of the design is the document shape
above and the composition rule; the mechanism it compiles to is one line of `nft`.

### Tier C — 802.1Q and physical NICs

`kind: Network` gains `vlan: <1-4094>` and `parent: <nic>`, realized as `ip link add link <nic>
name <nic>.<vlan> type vlan`. **This needs `CAP_NET_ADMIN` in the host's init-netns and there is no
rootless path to it** — the same wall `macvlan`/`ipvlan` already hit, and the same one `vm bridge`
already crossed deliberately.

So it follows the `vm bridge` precedent exactly, and that precedent is the decision:

* a separate, explicitly privileged command — never a flag that silently escalates;
* **dry-run by default**, printing the plan; `--apply` to execute;
* refuses under an unprivileged uid with the reason, rather than degrading;
* a `delonix-runtime-sec` pass before merge, because it puts the host's physical NIC in reach.

## Spike result (2026-08-12) — tier B is a **GO**, and smaller than this ADR assumed

Run rootless on a live host, against two throwaway bridge networks (`10.252.0.0/16` and
`10.244.0.0/16`) with one container on each, alongside the host's production workloads.

**Isolation between networks is not the absence of a route. It is an explicit pairwise drop.**
Inside the holder's netns:

* `ip_forward` is **1**;
* the routing table already has `10.252.0.0/16 dev dlxn…` and `10.244.0.0/16 dev dlxn…`;
* every `forward` chain (`fwguard` -20, `fwdeny` -10, `fwcont` -5, policy 0) is `policy accept`;
* and there is one `iifname "<bridgeA>" oifname "<bridgeB>" drop` per ORDERED PAIR of bridges.

The blocker was isolated without mutating anything, by comparing two destinations from the same
container: A → **B's gateway** (`10.244.0.1`, which is the holder itself, so an `input` path)
answered with **0% loss**, while A → **a container on B** (a `forward` path between the two
bridges) lost **100%**. Routing works; the pair is dropped.

**Consequence for tier B: opening a path is EXEMPTING a pair, not building a dataplane.** No new
forwarding, no privilege, no second mechanism — which is why tier B stays rootless and does not
descend to tier C. The `kind: NetworkRoute` document compiles to «do not install (or bypass) the
drop for this ordered pair», and the composition rule already stated holds unchanged: the
`fwcont` chains still decide whether the packet is allowed.

**Two defects found on the way, both worth their own fix and neither blocking:**

1. **The pairwise drops carry no `counter`.** Every other rule this engine emits does — that was
   the point of `fw_rule_tail` and of the PACKETS/BYTES columns in `ingress ls`. Here the one
   question worth asking, «did this pair ever try to talk», cannot be answered at all. It is also
   why the spike had to prove the blocker by comparing two destinations instead of just reading a
   number.
2. **The rules are O(n²) and unmanaged.** Measured on this host: **8 bridges, 73 rules**. Every new
   network rewrites a mesh against every existing one. This is the same shape the per-container
   dispatch had before it became a verdict map (`@fwmap`, 2 rules regardless of container count),
   and the same fix applies — an `nft` set/map keyed on the interface pair. Tier B should land on
   top of that, not on top of the mesh, or it will be adding exceptions to a structure that is
   already the wrong one.

## Alternatives considered

**Give the holder `CAP_NET_ADMIN` on the host and do everything in one tier.** Rejected: it makes
the engine privileged by default to serve the minority of installs that need a trunk port, and
the daemonless/rootless default is the product.

**Model VLANs as another `driver:` value on `kind: Network`.** Rejected: `driver` already means
"how the dataplane is built" (bridge/macvlan/ipvlan/overlay), and a VLAN is orthogonal — an
802.1Q tag on a bridge network is a coherent thing to want. `vlan:` is a property, not a driver.

**Keep the octet and add a CIDR beside it.** Rejected for the reason this repo keeps rediscovering:
two fields that must agree eventually disagree. The octet becomes derived from the CIDR, or it goes.

## Consequences

* The IPAM stops being arithmetic on one octet and becomes allocation inside an arbitrary prefix —
  including overlap detection between networks, which does not exist today.
* `network ls`/`inspect` gain CIDR, gateway and resolvers; the JSON contract only ever ADDS fields
  (ADR-0005), so this is additive.
* The reconciler already compares `subnet` (`RECONCILED_NETWORK_FIELDS`), so a widened subnet field
  is comparable on day one — but changing a live network's CIDR is a COLD field and must land in
  the recreate path, never a silent in-place edit.
* Tier C splits the product's promise: most of `delonix network` stays rootless, one command is not.
  That must be visible in `--help` and in `docs/cli-stability.md`, not discovered at runtime.

## What this ADR does NOT decide

The `via:` forwarding appliance. Sending a subnet's traffic through a workload (an OPNsense VM, a
firewall container) changes where masquerade happens — today everything leaves under one
`oifname "tap0"` masquerade, and a gateway appliance that sees every packet with a single source
address is a gateway whose own per-source rules are useless. That is its own ADR, and it depends
on tier B being real first.

Also undecided: IPv6. The engine currently DISABLES it per container by design (v0.37.1, it was a
complete bypass of the policy model). A routed topology is where v6 stops being avoidable, and
re-enabling it is a security decision, not a networking one.


## Where this stands (2026-08-12) and what's left for "fully functional"

Implemented in this series, with live validation in isolated root:

| | status |
|---|---|
| isolation between networks (set + verdict map, with counter) | **done** — it was an O(n²) mesh with no counters; measured, 8 bridges → 73 rules → **2** |
| tier B — `network route` + `kind: NetworkRoute` | **done** — directed, and the return flows via `established`; **reversible** as of the next series, see below |
| tier C — `network vlan` (802.1Q) | **done**, privileged and contained; a real run with `sudo` is still needed |
| tier A — the `Cidr` type and its arithmetic | **foundation done**, IPAM still to be wired in |

Exactly this, and nothing more, is missing for what the user called "fully
functional at the networking level":

### 0. Tier B became reversible (closed)

Tier B delivered the path and not its reverse: `kind: NetworkRoute` was applied and validated but
stayed outside `CONVERGING_KINDS`, with no ownership and no teardown. Removing it from the manifest and running
`stack apply --prune` **did not close it**, neither did `stack destroy`, and `stack ls` answered
`?  unsupported kind`. An exception to isolation that the manifest opened and didn't know how to close.

What the fix decided, and is worth keeping because it reversed the obvious design: **the plan's `actual` comes
from a REGISTRY (`infra::RouteDef`) and not from `@netpair`**. The map lives in the holder's ephemeral netns,
which is born on demand and dies with the last container — on an idle node the probe returns empty, the plan reads
«the route disappeared», and `--detailed-exitcode` stays at 2 forever, with the CI drift gate
red every day. It's the same problem `EgressState` already had, and the answer is the same:
persist it and restore it in `ensure_net_bridge` when the bridge is reborn.

A route's identity is the **pair**, not `metadata.name` — with the document's name, renaming it
would give `Create`+`Delete` for the same nft element, and `--prune` (which runs last) would close
the path the apply itself had just opened. Hence the refusal of two documents for the same pair.

What this ADR already flagged is still undone: **the exemptions carry no `counter`**, so «did this
route carry any traffic?» still has no answer.

### 1. Wire IPAM to `Cidr` (tier A, 2nd slice) — rootless

The type exists and is proven; what still receives a `"10.X"` string and assumes a
`/16` are three functions (`derive_ip_in`, `valid_ip_in_subnet`, `probe_free`) and the
registry, which still stores an octet. Until that changes, `--subnet`
only picks WHICH `10.<200-254>.0.0/16`, and `--gateway` only accepts the derived one.

**It's the only piece that unlocks free choice of CIDR and gateway**, and it's the most dangerous of
what remains: a mask error here doesn't give an error, it gives containers with overlapping
addresses. It deserves a whole session, with the isolated lab this series left working.

### 2. `macvlan` belongs to a BOUNDARY, not a tenant (tier C) — privileged

**Corrected decision (2026-08-12), and the previous version of this section was worse.**
It said "realize the `macvlan` driver" and left unresolved the warning the
code itself emits: a macvlan network puts containers DIRECTLY on the physical
LAN, OUTSIDE this engine's firewall, anti-spoofing, and isolation. Realizing it
as it stood would mean shipping a feature whose own message says it
turns off the security model — and a loud warning is still a warning about
something that shouldn't be the normal path.

**`macvlan` becomes the EXTERNAL leg of a boundary workload**, not a
network a tenant connects to. The shape:

* a boundary workload (firewall/gateway) with TWO legs — a `macvlan` on the
  physical NIC, which gets an address from the host LAN's DHCP like any other
  machine there, and one on the internal `kind: Network`;
* containers and VMs stay ONLY on the internal network, filtered, isolated, and with
  `fwcont` deciding as always;
* their egress to the LAN/internet goes through the boundary, which is where egress
  policy now lives.

What this buys, and is the reason for it: privileged capability stays confined to
ONE declared workload, instead of being an attribute of any network anyone
connects to. It's the same containment principle as `network vlan` — the line
is crossed at one named place, not across the whole surface.

**What this requires and doesn't yet exist**: masquerade. Today EVERYTHING leaves under a
single `oifname "tap0" masquerade` on the holder, so the boundary would see every
packet with the same source address — and a gateway whose per-source rules don't
distinguish anyone isn't a gateway. Moving masquerade to the boundary is the
`via:` this ADR had already left undecided, and is now a prerequisite for this
piece rather than an extra.

**Known limit, measured in this series**: a VM boundary only works if it's
Cloud Hypervisor, because a libvirt VM lives on the host's `virbr0`, not on the SDN.
And OPNsense — the obvious appliance for the role — does NOT boot on Cloud Hypervisor
(measured: firmware fails before the kernel, with the golden Linux booting on the same
firmware as a control). So the first realizable boundary is a
CONTAINER, not a VM appliance.

### 2b. What realizing the driver still needs

"A network that gets an IP from the host network via DHCP and serves as a gateway for VMs and
containers" has a name in this stack: it's the `macvlan` driver. Today it is REGISTERED and
`network create` loudly warns that it **was not realized**
(`Realized=False reason=DriverNotImplemented`) instead of pretending — measured on
2026-08-12. It hits the same wall as the VLAN: `CAP_NET_ADMIN` in the host's init-netns.

So it enters through the SAME path `network vlan` already opened — a separate
privileged command, dry-run by default, a clear refusal without root — and the work
is the `ip link add … type macvlan` plan plus the routing that makes it the gateway
for the internal networks. `network vlan` is the ready-made precedent to copy; there's no
new privilege model to invent.

**Warning already in the code that must stay visible**: a macvlan network puts
containers DIRECTLY on the physical LAN, OUTSIDE this engine's firewall, anti-spoofing, and
isolation. Realizing it must not silence that warning.

### 3. What stays out, by decision and not by oversight

**IPv6.** Deliberately disabled since v0.37.1 — the SDN gave each
container a ULA and the whole firewall is `table ip`, i.e. a second data path
with no policy at all, bypassing `ingress`/`egress`, namespace isolation,
and `kind: Dependency`. It was measured at the time: with the firewall denying over IPv4, the
same target answered over the ULA. Re-enabling it is security work with its
own ADR, not the extension of a field.

**`via:`** (routing a subnet through an appliance) remains where this ADR already
left it: it depends on moving masquerade to the appliance, otherwise it sees all
packets with a single source address.

## Addendum (2026-10-08) — a route has to be usable from an ADDITIONAL network

**What the spike did not cover.** The tier B spike put one container on each
network as its PRIMARY network. Such a workload sends everything to that
network's gateway, so its packets enter the holder by the bridge the pair names.
A workload attached with `network connect` does not: `do_attach_extra` gives it
an address on the network and deliberately leaves its routing table alone, so a
packet for the routed network leaves by the default route, enters by the PRIMARY
network's bridge, and never matches `<bridge A> . <bridge B>`.

Measured on v4.5.0, rootless, three throwaway networks, both containers on a
base network and each connected to one of the routed networks:

| | `a → b` | `b → a` |
|---|---|---|
| no route | 100 % loss | 100 % loss |
| route `A → B` | **100 % loss** | 100 % loss |
| routes both ways | **100 % loss** | **100 % loss** |

`ip route get <b>` inside `a` answered `via <base gateway> dev eth0`. The route
was listed, the pair was in `@netpair`, and nothing crossed it.

**Decision.** A declared route must be usable by every workload attached to its
source network, on whichever interface. The holder keeps, in each multi-homed
workload, a route to the other network's prefix through THIS network's gateway
on the interface the workload has here — and the mirror on the far end, because
a workload attached to B as an additional network would otherwise send its
replies out of its own default route, with a source address the anti-spoofing of
that interface refuses.

* What to install is a pure function (`delonix_sdn::route_plan::plan`), tested
  without a holder. The holder only executes it (`sync_workload_routes`).
* It runs when a route opens or closes (every workload) and when an additional
  network is attached or detached (that workload).
* The routes carry their own routing-protocol id, so the holder removes exactly
  what it installed and never touches the default route, the connected routes
  or anything the workload set for itself.
* Left alone on purpose: the default interface (the default route already
  covers it), a destination the workload is directly connected to (`ip route
  replace` would replace the kernel's connected route), and a destination
  already routed through an earlier interface (one destination, one route,
  interfaces taken in name order).

**The composition rule is unchanged.** A route in a workload's table says where
a packet goes OUT. Whether it is forwarded is still the pair's decision, and
the `fwcont` chains' after it. The mirror route does not open the reverse
direction — measured, same setup, with the fix:

| | `a → b` | `b → a` |
|---|---|---|
| no route | 100 % loss | 100 % loss |
| route `A → B` | **0 % loss** | 100 % loss (directed, as declared) |
| routes both ways | 0 % loss | 0 % loss |
| a workload connected to A AFTER the route existed | 0 % loss | — |
| both routes closed | 100 % loss, and the routes are gone from both tables | |

**Not covered, and said.** A microVM's routing table is the guest's: the holder
cannot write it. A VM attached to a routed network as an additional interface
still needs the route from its own configuration (or from DHCP, option 121,
which this engine does not serve yet). IPv6 stays where this ADR left it.
