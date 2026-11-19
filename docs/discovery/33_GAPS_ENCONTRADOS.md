# 33 — Discovery: hardening the Ingress and Egress datapath

| Field | Value |
|---|---|
| Date | 2026-07-28 |
| Prompt baseline | `0.35.1` (commit `60efdd6a7`) |
| **Real baseline on `main`** | **`0.37.0`** (commit `55db34d`) — two versions ahead of the prompt |
| Method | Code reading + live exploratory tests on a real rootless host |
| Production changes | None. This document is the only artefact. |

> **Reading notice.** Everything that follows was measured or read. Where it was not
> possible to measure, it is written `INDETERMINATE` and the reason is given. No
> classification is inferred from the documentation.

## Execution status

| Block | Requirements | Status |
|---|---|---|
| **0 — Immediate security** | RF-NET-11 (mitigation), RF-NET-02 | ✅ **DONE** — see `docs/releases/archive/v0.md`, section v0.37.1 |
| 1 — A′ | RF-NET-03, RF-NET-05 (reframed) | to do |
| 2 — A″ | RF-NET-01 (reframed) | to do |
| 3 — B | RF-NET-07 → RF-NET-06 | to do |
| 4 — C | RF-NET-12, 13, 14(a) | to do |
| 5 — D | RF-NET-04 | to do |
| 6 — E | RF-NET-11 (full parity), 10, 09 (reduced) | to do |
| — | RF-NET-08, RF-NET-14(b) | out, pending an ADR |

---

## 0. Step 0 — version delta

### 0.1 The prompt's baseline is already out of date

The prompt declares `0.35.1` and the spec source `0.32.2`. `main` is at
**`0.37.0`**. Between `v0.32.2` and HEAD there are **6 commits** touching `crates/adapters/delonix-net/`,
two of them substantial:

| Commit | What it did |
|---|---|
| `3c0d1a4` | 5 bugs in the `-p` ↔ firewall flow, **2 of them security**: port silently dropped when `proto: any`; multi-homing bypassed the firewall **and** namespace isolation |
| `f8ab32b` | L4 hardening: `ct state` in the default policy, atomic application, verdict map, `counter` on every rule, refusal of IPv6 at validation, port ranges |

`f8ab32b` was produced in a work session immediately before this discovery and
**closes or partially changes 4 of the 14 requirements** — see the "Evidence"
column for RF-NET-05, 09, 11 and 13.

### 0.2 The announced version drift — confirmed

- `README.rst:18` → `:Version: 0.32.2`
- `Cargo.toml` → `version = "0.37.0"`

Five minor versions of drift. Deliverable item 10 (CI verification) is,
therefore, **justified and confirmed**.

### 0.3 `--help` vs cheatsheet divergence

`delonix net --help` on `0.37.0` exposes `netns`, `flow`, `ingress`, `egress`,
`httproute`, `tunnel`, `boot`. The reorganisation that created the `net` group (v0.30.0) was
**breaking with no aliases**, so any cheatsheet predating that version
documents CLI paths that no longer exist (`delonix ingress …` instead of
`delonix net ingress …`). The `0.32.2` cheatsheet postdates that change, so
the main drift is content (new commands missing), not path.

### 0.4 Process note — work carried into someone else's commit

The work in `f8ab32b` was written in this tree without being committed, and was **picked
up and committed by a parallel session** that was closing out `v0.37.0`. It is already in
`origin/main`. The content is correct and tested, but it was not reviewed by whoever
committed it. It is this repository's known shared-worktree pitfall, and it is worth
confirming `f8ab32b` in a dedicated review.

---

## 1. Inventory

### 1.1 Map of the nftables chains

A single table, in the holder's **ephemeral** netns:

```
table ip dlxing
├── set  dlxall              todos os IPs de container da SDN
├── set  dlxns<hash>         um por namespace lógico
├── set  dlxfq<hash>         um por bridge — FQDNs aprendidos
├── map  fwmap               ipv4_addr : verdict   (dispatch por container)
├── chain pre      nat    prerouting  -100   DNAT das portas publicadas
├── chain post     nat    postrouting  100   masquerade oifname tap0
├── chain fwdeny   filter forward      -10   egress global/por-rede, l4guard
├── chain fwcont   filter forward       -5   ip {daddr,saddr} vmap @fwmap
├── chain forward  filter forward        0   policy drop + established/related
└── chain fw<hash>                            uma por container (alvo do vmap)
```

Confirmed live: `nft list tables` inside the holder returns **exactly**
`table ip dlxing`. Nothing else.

**Exception found**: `bins/delonix-runtime-bin/src/cmd/vmbridge.rs:116-119,161-164`
writes `iptables -I FORWARD` on the **host's** table. This is the `vm bridge` command
(privileged, opt-in, root). It violates restriction 5 of the prompt's section 5.

### 1.2 Where policy is applied in the lifecycle

`cmd_start` (`bins/delonix-runtime-bin/src/cmd/container.rs`):

| Line | Instruction |
|---|---|
| ~3196 | `infra::attach_container(&c.id, &n, &c.namespace)?` — creates the netns, the veth, joins the namespace sets |
| ~3264 | `apply_firewall_everywhere(&c, fw)` — programs the per-container chain |
| ~3316 | `RunSpec { … }` — the process spec is only built after this |

**The order is correct**: rules are programmed before the process exists. There is no
window of unfiltered traffic at startup. What fails is error handling —
see RF-NET-03.

### 1.3 DNS learning path

`handle_dns_query` (`infra.rs:~3690`) → `forward_dns(q)` → **`snoop_fqdn(&name, &resp)`**
(`infra.rs:3701`). Learning happens **exclusively** on responses that the
holder's own resolver forwarded. There is no promiscuous sniffing.

### 1.4 Precedence — measured, not assumed

Three scenarios, real container, `nginx:alpine` on a custom network:

| Scenario | Measured result |
|---|---|
| `allow tcp/80` → `deny tcp/80` (same match) | **DENY** — the last command replaces it |
| `deny tcp/80` → `allow tcp/80` (same match) | **ALLOW** — same, with a note `replaces the previous deny rule` |
| `deny 80` → `allow tcp/80` (distinct, overlapping matches) | **DENY** — **insertion order** wins, with a shadowing warning that names the rule and the removal command |

Effective semantics: **replacement for an identical match (ufw), insertion order for
overlapping matches**. There is no `priority` field anywhere.

### 1.5 IPv6 — measured

| Measurement | Result |
|---|---|
| nft tables in the holder | `table ip dlxing` — **v4 only** |
| `slirp4netns` argv | no `--enable-ipv6` |
| v6 egress to the Internet | `Network is unreachable` |
| Container's v6 addresses | **4** — link-local **and global ULA** |
| ULA prefix | `fd00:<2nd octet>::/64` per network, **deliberately assigned** (`infra.rs:1184-1200`) |
| Container's ULA address | `fd00:<o2>::<o3>:<o4>` — **derivable from the IPv4** |

See RF-NET-11 — this is the most serious finding of this discovery.

### 1.6 Policy data model

There is no policy store. The policy lives **on the container itself**:
`Container.firewall: Option<ContainerFw>` (`delonix-runtime-core/src/lib.rs:92`), with
`enabled`, `policy_in`, `policy_out`, `rules: Vec<FwRule>`, `namespace`.
`FwRule { dir, proto, port, src, action, note }`.

The `FirewallPolicy` manifest (`FwDocSpec`, `cmd/firewall.rs:913`) has `target: String`
— a name — and **has no** `selector` or `priority`. It compiles to the named
container's `ContainerFw`.

`Container.labels: BTreeMap<String,String>` **already exists** (`runtime-core:375`), so
the data for RF-NET-06 is available; the mechanism is missing.

### 1.7 Metrics

`delonix-runtime-core/src/metrics.rs`: a shared Prometheus registry, plain `Counter` and
`Gauge`. **Zero `Family<…>`** — there is no metric with labels today, which is
exactly the shape RF-NET-13 requires (`{container, direction, rule_id}`).

### 1.8 `system events`

`delonix-runtime-core/src/events.rs`: an append-only `events.jsonl`, `Event { ts, kind,
action, id, name, … }`. The design is **deliberately lock-free**, relying on the
atomicity of an `O_APPEND` `write` below `PIPE_BUF` (4 KiB) — this is documented
in the module header. Opportunistic rotation to `.1`, a single generation.

`grep events:: cmd/firewall.rs` → **empty**: network policy changes **don't
emit events today**.

---

## 2. Requirement classification

### RF-NET-01 — Non-spoofable FQDN learning

- **Status: PARTIAL** (central premise refuted, three sub-requirements confirmed)
- **Evidence:**
  - *Refutes* the prompt's scenario: `infra.rs:3701` — `snoop_fqdn` is called **only**
    from `handle_dns_query`, on the response the holder's resolver just
    forwarded. A container running its own resolver, or forging a
    response, **never feeds the set**. There is no promiscuous traffic observation.
  - *Confirms* sub-requirement 1: `infra.rs:2580-2581` — `egress_specs` unconditionally
    emits `udp dport 53 accept` and `tcp dport 53 accept` for **any
    destination** whenever there is an allowlist. There is no DNAT of port 53 to the
    internal resolver. Consequences: (a) the internal resolver is bypassable, (b) under
    the most restrictive egress policy the engine can express, an **exfiltration
    channel via DNS tunnelling** to a hostile resolver remains.
  - *Confirms* sub-requirement 4: `infra.rs:~1943` — `timeout 1h` **hardcoded**. The TTL
    of the response is entirely ignored. There is no `--ttl-floor` or `--ttl-ceiling`.
  - *Confirms* sub-requirement 5: only the address enters the set. No FQDN,
    timestamp, expiry or source container is recorded; `egress show` lists IPs
    (`egress_set_members`).
  - **Unanticipated gap**: `snoop_fqdn(name, resp)` **does not receive the client**. It
    matches the name against the suffixes of **every** registered network and injects into
    the corresponding sets. A container on network A can cause entries to be
    injected into network B's set.
- **Real impact:** smaller than the prompt assumes on the spoofing vector, bigger on the
  exfiltration one. The egress allowlist holds no data: DNS always gets out.
- **Effort:** M
- **Dependencies:** none
- **Recommendation: REFRAME** — rewrite the requirement around (1) DNAT of :53,
  (4) TTL ceiling/floor, (5) entry metadata, and (6) per-client scoping. Point
  2 of the original requirement is already true today.

### RF-NET-02 — Default-deny for sensitive destinations

- **Status: CONFIRMED**
- **Evidence:** `grep -rn "169.254|link-local"` over `crates/` → **zero occurrences**
  outside VM code. There is no default denial whatsoever of link-local, of
  metadata, of the host's loopback or of management sockets.
- **Real impact:** high and **made worse** by RF-NET-11: `fe80::/10` is not merely
  "not denied" — it is the path by which the entire policy is bypassable today.
- **Effort:** S
- **Dependencies:** must be done **together** with RF-NET-11, otherwise v4
  denial is decorative.
- **Recommendation: IMPLEMENT**

### RF-NET-03 — Fail-closed enforcement at startup

- **Status: PARTIAL** (order correct, error handling incorrect)
- **Evidence:**
  - *Order, refuted as a problem*: `container.rs:~3196` (attach) and `~3264`
    (`apply_firewall_everywhere`) run **before** the `RunSpec` is built at
    `~3316`. There is no window.
  - *Silent failure, confirmed*: `container.rs:3264-3272` — on error,
    `eprintln!("warning: firewall/isolation of '{name}' not reapplied on start: {e}")`
    and **the container starts anyway**, with no policy. The same pattern at `~4300` and
    `~4373` (`update` paths). It is literally anti-pattern No. 3 from section 5.
- **Real impact:** high. A transient holder failure produces a container
  running without isolation, with a warning on `stderr` that nobody reads in a pipeline.
- **Effort:** S for `run`/`start`; M for the atomicity of `update` (the prompt
  requires "the transition is atomic or it rolls back", and there is no rollback today).
- **Dependencies:** none
- **Recommendation: IMPLEMENT**

### RF-NET-04 — Reconciliation and drift detection

- **Status: PARTIAL**
- **Evidence:**
  - *Sub-requirement 1, already satisfied with caveats*: a dedicated table, confirmed
    live (`nft list tables` → only `table ip dlxing`). Caveats: the family is `ip`, not
    `inet` (RF-NET-11), and `vmbridge.rs:116` writes to the host's `FORWARD`.
  - *Sub-requirements 2, 3, 5*: `policy verify` doesn't exist; there is no comparison
    at all between declared and effective; no drift metric.
  - *Sub-requirement 4*: `net boot enable` **exists** and installs systemd units — the
    extension point for the timer is available.
  - **Unanticipated gap, bigger than the prompt's**: the nft state lives in the holder's
    **ephemeral** netns. It doesn't take an `nft flush ruleset` to lose it — the
    holder simply dying is enough. `AGENTS.md` already records that namespace isolation
    **is not rebuilt on a holder respawn**. Drift from a holder restart is the common
    case; the prompt's (an external agent writing to the table) is the rare case, because
    the table lives in a netns nobody else sees.
- **Real impact:** high, but for a different reason than the prompt points to.
- **Effort:** L
- **Dependencies:** RF-NET-13 (metrics) to instrument it; RF-NET-14 (events).
- **Recommendation: IMPLEMENT**, reframing the target: reconciliation **against
  holder respawn** first, against external writes second.

### RF-NET-05 — Deterministic precedence

- **Status: CONFIRMED** (no `priority`) — **but the proposed ordering is breaking**
- **Evidence:** section 1.4 above, three scenarios measured live. The current semantics
  are replacement for an identical match + insertion order for overlaps, with an explicit
  shadowing warning. No `priority` field exists in either `FwRule` or `FwDocSpec`.
- **Real impact:** the absence of `priority` is a real expressiveness limitation.
  But adopting "all `deny`s before all `allow`s" **silently inverts**
  policies already in production: today an `allow` inserted before an overlapping `deny`
  wins; under the proposal it would lose. It's exactly anti-pattern No. 10.
- **Effort:** M
- **Dependencies:** should precede RF-NET-06 and 07 (which multiply the rules per
  container and make the order critical).
- **Recommendation: REFRAME** — add `priority` as an **optional** field, with
  insertion order preserved as the tie-breaker and as the default behaviour. That
  gives explicit determinism without breaking anything. If the user
  insists on the global reordering, it's `breaking` and needs a migration guide and a major.

### RF-NET-06 — Label-based selection

- **Status: CONFIRMED** (absent)
- **Evidence:** `FwDocSpec.target: String` (`cmd/firewall.rs:925`), with no `selector`.
  `Container.labels: BTreeMap<String,String>` already exists (`runtime-core:375`), and the CLI
  already has `container run --label` — the data is there.
- **Real impact:** high for multi-tenancy. Without this, every container created forces
  a manual policy re-apply, and the requirement "a container created after the policy
  inherits it" is impossible to express.
- **Effort:** L
- **Dependencies:** **RF-NET-07** (compiling into named sets is the mechanism that
  makes the selector viable without reprogramming rules on every event) and RF-NET-05.
- **Recommendation: IMPLEMENT**, after 07.

### RF-NET-07 — Source and destination by identity

- **Status: CONFIRMED** (absent)
- **Evidence:** `FwRule.src: String` validated by `fw_src_ok` — **IPv4 CIDR only**. The
  rules are generated with **literal addresses** (`fw_rule_tail`, `infra.rs:~2148`),
  which is anti-pattern No. 4, described by the prompt as the current state.
  *In favour of viability*: the engine **already uses named sets** in three places
  (`@dlxall`, `@dlxns<hash>`, `@dlxfq<hash>`) and a verdict map (`@fwmap`). The pattern
  is proven in this codebase; what's missing is generalising it.
- **Real impact:** high. It's the requirement that unblocks 06 and the one that makes
  policy expressible in tenant terms.
- **Effort:** L
- **Dependencies:** RF-NET-05
- **Recommendation: IMPLEMENT** — it's the centrepiece of block B and should come before 06.

### RF-NET-08 — Stable per-network egress address

- **Status: CONFIRMED** (absent) — **with serious doubt about rootless viability**
- **Evidence:** a single shared `slirp4netns`; the `post` chain does
  `oifname "tap0" masquerade` with no per-network distinction. `network create` has no
  `--egress-ip`.
- **Real impact:** real as a commercial requirement, but in **rootless the engine has no
  access to the host's routing tables** — prompt requirement 2 (policy
  routing with a dedicated table) is not satisfiable without privilege. The viable
  escape routes are: a `slirp4netns` **per network** with `--outbound-addr` (the flag
  exists, marked `experimental` upstream), or an explicit privileged mode.
- **Effort:** L (per-network via slirp) to XL (privileged policy routing)
- **Dependencies:** none
- **Recommendation: REFRAME** — decide first, in an ADR, between "one slirp per
  network" and "privileged mode", and write the requirement against the decision. As it
  stands, requirement 2 is unachievable under restriction 2 of section 5.

### RF-NET-09 — Real client address

- **Status: PARTIAL** — the main premise is **REFUTED by measurement**
- **Evidence:** measured in this tree with three clients against a published `nginx`,
  reading the container's own access log:

  | Client | Origin seen by the container |
  |---|---|
  | `127.0.0.1` | `10.0.2.2` (slirp gateway) |
  | `172.16.31.103` (LAN) | **`172.16.31.103`** |
  | `192.168.122.1` (libvirt gateway) | **`192.168.122.1`** |

  libslirp only substitutes the origin when it **has no route back** to it, which is the
  case for loopback. Any routable origin passes through intact. Documented in
  `delonix_net::SLIRP_GW` since `f8ab32b`.
  *Verified corollary*: `net ingress allow <c> <port> --from <cidr>`
  **works** against a real remote client (allowed origin 200, another origin 000).
  So sub-requirements 2 (L4 PROXY protocol) and 3 (`net flow` post-DNAT) **have no
  problem to solve**.
  *Confirms* sub-requirement 1: `grep -i x-forwarded cmd/ingress_proxy.rs` → **empty**.
  The L7 proxy does not propagate `X-Forwarded-For`/`-Proto`/`-Host`.
- **Real impact:** low, and confined to the L7 proxy.
- **Effort:** S
- **Dependencies:** none
- **Recommendation: REFRAME** — reduce to sub-requirement 1 (`X-Forwarded-*`
  headers + `--trusted-proxies` on `httproute`). Drop 2 and 3.

### RF-NET-10 — Ingress limits

- **Status: PARTIAL**
- **Evidence:** `do_l4guard` (`infra.rs:~1998`) already emits
  `meter dlx_conn_rate { ip saddr limit rate over N/second burst … } counter drop` and
  `meter dlx_conn_count { ip saddr ct count over N } counter drop`, with a pre-flight
  `nft -c` and a clean degradation if the kernel doesn't support `meter`. Limitations: it's
  **global** to ingress (not per network, not per rule, not per port), and only reachable
  via `kind: FirewallPolicy` with `rateLimit` — `grep set_l4_guard` outside the crate shows
  **zero CLI commands**.
- **Real impact:** medium. The mechanism exists and works; granularity and
  surface are missing.
- **Effort:** M
- **Dependencies:** RF-NET-07 (for `ct count` by identity instead of by address)
- **Recommendation: IMPLEMENT**, reusing `do_l4guard` instead of rewriting it.

### RF-NET-11 — IPv4/IPv6 parity

- **Status: CONFIRMED — CRITICAL. Bypass reproduced live.**
- **Evidence (full reproduction, two containers on the same network):**

  ```
  # firewall a NEGAR em IPv4
  $ delonix container exec cli wget -T3 -O/dev/null http://10.216.133.231/
  wget: download timed out                                   → V4_DENY

  # o mesmo destino, o mesmo porto, por IPv6
  $ delonix container exec cli wget -T3 -O/dev/null 'http://[fd00:216::5081:c3ff:fe63:8bd1]:80/'
                                                             → ULA_BYPASS
  ```

  Chain of facts that produces it:
  1. `infra.rs:1184-1200` — the SDN **deliberately assigns** a per-network IPv6 ULA
     (`fd00:<2nd octet>::/64`), with a per-container address **derivable from the IPv4**
     (`fd00:<o2>::<o3>:<o4>`). It's not a kernel accident: it's code.
  2. The entire firewall is `table ip` — **v4 only**. Confirmed: `nft list tables`
     in the holder returns only `table ip dlxing`.
  3. `nginx:alpine` listens on `:::80` by default (the image's own entrypoint
     enables `listen [::]:80`), like the vast majority of modern images.
  4. Discovery is trivial: a `ping -6 ff02::1%eth0` enumerates **every** neighbour on the
     bridge in a single pass (measured: 3 replies, including the target).
- **Real impact:** **the entire policy model is bypassable**, not just the
  explicit rules: `policy deny`, namespace isolation, `kind: Dependency` and the
  RF-NET-10 limits are all `table ip`. One tenant reaches another tenant's containers
  via ULA, without privilege, with a single command. RF-NET-07's `--from` would inherit the same hole.
- **Effort:** M for the explicit refusal (disable ULA and drop v6 on the bridge);
  L for real parity (migrate to `table inet`, v6 IPAM, dual sets).
- **Dependencies:** none. **Blocks the usefulness of 02, 05, 06, 07 and 10.**
- **Recommendation: IMPLEMENT FIRST** — raise it from P1 to P0, ahead of everything
  else. The short mitigation is recommended (disable ULA + `ip6tables`/a v6
  drop rule on the bridge, or `table inet` with a full v6 drop) in an immediate security
  release, with full parity as follow-up work.

### RF-NET-12 — `delonix net policy explain`

- **Status: CONFIRMED** (absent)
- **Evidence:** there is no `net policy` group. The closest thing is the verdict for a
  published port (`published_reach`, `cmd/firewall.rs:781`, introduced in `f8ab32b`),
  which already answers "does this port respond, and to whom" — it's the reusable core.
- **Real impact:** high support cost, zero security impact.
- **Effort:** M
- **Dependencies:** RF-NET-05 (the explanation is only correct if precedence is
  deterministic) and RF-NET-07.
- **Recommendation: IMPLEMENT**, extending `published_reach` instead of starting from
  scratch.

### RF-NET-13 — Drop metrics

- **Status: PARTIAL**
- **Evidence:** the Prometheus registry exists (`runtime-core/src/metrics.rs`) and is
  exposed at `/metrics` by `delonix-mgmt` and `delonix-cri`. **There is no metric with
  labels at all** — zero `Family<…>` in the file; every current metric is a scalar
  `Counter`/`Gauge`. The `{container, direction, rule_id}` shape requires
  new plumbing.
  *In favour*: since `f8ab32b` **every rule emits a `counter`**, and there is already a
  channel to read them from the holder (`fw_counters` → `fwstats`). The data source is
  ready; exposing it is what's missing.
- **Real impact:** medium-to-high (diagnostics), zero on security.
- **Effort:** M
- **Dependencies:** none for the first two counters; RF-NET-04 for
  `drift_total`; RF-NET-01 for `dns_learned_entries`.
- **Recommendation: IMPLEMENT**

### RF-NET-14 — Chained audit

- **Status: PARTIAL — with a design conflict to resolve**
- **Evidence:**
  - `events.jsonl` exists and is append-only (`runtime-core/src/events.rs`).
  - Network policy changes **don't emit events**: `grep events:: cmd/firewall.rs`
    → empty. The requirement is missing before chaining even comes into play.
  - **Conflict**: the module is explicitly **lock-free**, and the header documents
    why — atomicity comes from an `O_APPEND` `write` below `PIPE_BUF`, which
    lets N ephemeral processes write concurrently without `flock`. Chaining by
    hash **requires reading the last line before writing**, which reintroduces a
    critical section between processes. Either a `flock` is accepted (losing the property
    the design protects), or a scheme is used that doesn't depend on the immediate
    predecessor (e.g. a per-entry signature, or epoch-based chaining with periodic
    sealing).
  - The rotation to `.1` with a single generation also **breaks the chain** by design.
- **Real impact:** medium. Relevant for audit presentation, not for the
  datapath's security posture.
- **Effort:** M to emit events; L for chaining once the conflict is resolved.
- **Dependencies:** none
- **Recommendation: REFRAME** — split in two: (a) emit network policy events,
  which is pure gain with no conflict; (b) chaining, which needs its own ADR
  to decide the trade-off with the lock-free design and with rotation.

---

## 3. Unanticipated gaps

1. **Parallel, unfiltered IPv6 data path** — see RF-NET-11. It's not an
   omission of v6: it's a v6 SDN **built on purpose** and left without policy.
   It's the finding that reorders the entire plan.
2. **`vm bridge` writes to the host's `FORWARD`** (`cmd/vmbridge.rs:116-119,161-164`,
   `iptables -I FORWARD`). It violates restriction 5. It's privileged and opt-in, but it's shared
   state that the engine creates and that nobody reconciles.
3. **Anti-pattern No. 6 is the current state of the entire crate.** The prompt forbids
   "shelling out to `nft` when a netlink connection exists"; `delonix-net` invokes the
   `nft` binary on **every** path, and parses text to discover
   handles and map elements. Meeting the requirement means rewriting the datapath over
   netlink — an XL-sized piece of work, independent of the fourteen requirements. **It has to
   be negotiated explicitly**: either the existing code is exempted and the ban applies
   only to new code, or it enters as its own block with its own budget.
   Mitigating note: the invocations are `Command::new` with a vectorised argv (no shell),
   and the interpolated fields go through dedicated validators (`fw_src_ok`,
   `fw_port_ok`, `fw_proto_ok`, `validate_publish`), so the injection surface
   is smaller than "shelling out" suggests. The real problem is the lack of transactions and
   the fragility of the parsing, not injection.
4. **State in the holder's ephemeral netns** — see RF-NET-04. The entire policy disappears
   with the holder, and it is not rebuilt.
5. **Pods (CRI) and VMs outside namespace isolation** — already recorded in `AGENTS.md`
   as a known limitation; relevant because RF-NET-06 promises "every container
   matching the selector" and the CRI attaches via a different path.
6. **`l4guard` global and manifest-only** — see RF-NET-10.
7. **Announced version drift** — `README.rst` says `0.32.2`, `Cargo.toml` says
   `0.37.0`. Confirms deliverable 10.

---

## 4. Proposed implementation sequence

Diverges from section 7 of the prompt, and the reason is RF-NET-11.

| Order | Block | Content | Why here |
|---|---|---|---|
| **0** | **Immediate security** | **RF-NET-11 (mitigation) + RF-NET-02** | While the v6 path stays open, the entire policy is decorative and any work on the following blocks rests on a false premise. Publish as a security release. |
| 1 | A′ | RF-NET-03, RF-NET-05 (reframed) | Fail-closed and explicit precedence are the foundation the new model rests on. |
| 2 | A″ | RF-NET-01 (reframed) | DNAT of :53, TTL, metadata, per-client scoping. Independent. |
| 3 | B | RF-NET-07 → RF-NET-06 | Named sets first, selector after — the reverse order from the prompt, because 06 without 07 forces reprogramming rules on every event (anti-pattern No. 4). |
| 4 | C | RF-NET-12, RF-NET-13, RF-NET-14(a) | Pays for the support cost of the previous ones. `explain` extends `published_reach`. |
| 5 | D | RF-NET-04 | Reconciliation, with C's metrics already available. Target: holder respawn. |
| 6 | E | RF-NET-11 (full parity), RF-NET-10, RF-NET-09 (reduced) | Parallelisable. |
| — | Out | RF-NET-08, RF-NET-14(b) | Each needs its own ADR before code. |

---

## 5. Phase 0 acceptance criteria — verification

| Criterion | Status |
|---|---|
| Fourteen requirements classified | ✅ |
| No `INDETERMINATE` among the P0s | ✅ — 01, 02, 03, 04, 05 all with measured or read evidence |
| Precedence exploratory test run and recorded | ✅ section 1.4 |
| IPv6 exploratory test run and recorded | ✅ section 1.5 and RF-NET-11 |
| Implementation sequence proposed and justified | ✅ section 4 |

**Points that need a user decision before Phase 1:**

1. **RF-NET-11 first**, ahead of block A — confirm the reordering.
2. **Anti-pattern No. 6 (netlink)** — exempt the existing code, or budget the
   datapath rewrite?
3. **RF-NET-05** — additive `priority` with insertion order preserved
   (recommended), or a global reordering assumed as `breaking`?
4. **RF-NET-08** — which of the two rootless escape routes, or privileged mode?
5. **RF-NET-09** — accept the reduction to sub-requirement 1, given that the main
   premise was refuted by measurement?
