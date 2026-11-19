# ADR-0082 — Dual-stack IPv6 (phased plan)

**Status:** Proposed (Phase 1 — design). **Date:** 2026-11-01.
**Relates to:** the current deliberate refusal in `delonix-sdn/src/infra.rs`
(`ipv6_sdn_enabled`, `disable_ipv6_argv`, `ingress_v6_refusal_ruleset`).

## Context

Today IPv6 is **deliberately disabled**, in two layers: (1) `disable_ipv6`
(sysctl) in each container netns; (2) `table ip6 <ingress> forward drop` in the
holder. `DELONIX_ENABLE_IPV6=1` is an **unsafe and noisy** escape hatch.

The reason is a **security invariant**, not missing work: **the whole
firewall is `table ip` (IPv4 only)** — `ingress`, `egress`, `policy deny`,
namespace isolation, `kind: Dependency`, and the L4 guard are all IPv4. A
v6 address (a ULA derivable from v4) would bypass **all** of these policies.

### Finding that fixes the phasing (2026-11-01)

There is no shortcut to "turn on just one slice":
- **Not even host-mode via slirp is safe to enable in isolation**: **egress**
  policy is also `table ip` (v4), so v6 egress (even via slirp NAT)
  **would bypass `egress: deny`** — exfiltration over v6.
- The **v6 firewall needs v6 addresses** to write its rules (`ip6 saddr/
  daddr`), and those addresses come from IPAM (today v4 only).

**INVARIANT (non-negotiable):** no v6 path (ingress OR egress, host-
mode OR bridge) is enabled before the dual-family firewall covers v6. The
default stays v6-OFF in every phase until Phase 4.

## Decision — target and phases

**Target** (already named by the code itself): migrate the firewall from `table ip` to
**`table inet`** (dual-family v4+v6), with **v6 IPAM (ULA `fd00::/8`)** and **v6
sets** alongside the `ipv4_addr` ones. Execute in phases, each with its own commit and
tests, and **v6 is only really enabled in Phase 4**, with live proof.

| Phase | Scope | Files | Proof | Enables v6? |
|---|---|---|---|---|
| **1 — Design** (this one) | Invariant + plan + `table inet` target | `docs/adr/0082` | review | no |
| **2 — v6 IPAM (ULA)** | Deterministic derivation of a ULA per `(network,id)`, alongside the v4 `derive_ip_in`; dual-stack record per network | `delonix-sdn/src/{lib.rs,ipam.rs}` | unit (stable ULA, no collision, within the /64) — **inert** (nothing consumes it yet) | no |
| **3 — Dual-family firewall** | `table ip`→`table inet`; every rule gains the `ip6 saddr/daddr` pair; `ipv6_addr` sets alongside `ipv4_addr`; egress+ingress+policy+namespace+Dependency cover v6; remove `ingress_v6_refusal` (replaced by real filtering) | `delonix-sdn/src/{policy_nft.rs,infra.rs}` | unit (dual ruleset renders golden verdicts for v4 **and** v6); v4 byte-identical to current (no regression) | no (v6 sets empty until phase 4) |
| **4 — Datapath + enable** | Stop `disable_ipv6` when the network is dual-stack; assign the ULA in the netns; v6 gateway/RA on the bridge; slirp `--enable-ipv6` (host-mode); opt-in per network (`network create --ipv6`) | `delonix-sdn/src/{infra.rs,lib.rs}`, `bins/.../cmd/{network.rs,container.rs}` | **live**: a container on a dual-stack network gets v4+v6, `egress: deny` blocks v6, namespace isolation holds for v6 | **YES** |
| **5 — Contracts** | v6 `--ip`, v6 port publishing, internal AAAA DNS, `provider matrix` `net.ipv6`→supported (with evidence) | `delonix-sdn`, `delonix-node-api`, docs | live + capability gate | yes |

## Consequences

- Through Phase 4, observable behavior **does not change** (v6 OFF); Phases 2–3
  are inert, tested foundation → zero security regression risk.
- The `net.ipv6` capability **only** turns `supported` in Phase 4/5, with live proof
  that egress/ingress/isolation hold for v6 — never before (otherwise it would be
  today's unsafe escape hatch with another name).
- `DELONIX_ENABLE_IPV6=1` (the unsafe escape hatch) is **removed** once Phase 4
  delivers filtered v6.
- Cost: Phase 3 (`table inet`) touches the firewall's hot path — it requires the
  v4 output to stay byte-identical (the `policy_nft` golden tests are the guard).

## Why no more was done in this session

Phase 1 is design, for integrity's sake: enabling v6 in any slice before Phase 3/4
would reintroduce exactly the hole the current refusal closes (v6 with no egress
policy). The first safe code is Phase 2 (v6 IPAM, inert). "IPv6 supported" is not
faked with a partial, unfiltered change.
