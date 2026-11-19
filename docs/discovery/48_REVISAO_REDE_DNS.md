# Network, DNS and isolation review (2026-08-11)

Audit requested on: bugs, latencies, gaps, DNS collision and cloud-native
networking and isolation best practices (namespace, stack, pod, default).

**Method**: a complete Delonix node mounted on the side (`DELONIX_ROOT=/tmp/dxa/root`,
`DELONIX_NET_RUNTIME_DIR=/tmp/dxa/run`), with its own pin/control/slirp. This
host's real node has live work (`kaeso-odoo18-multi`, `kaeso-db18`) and was not
touched — the PIDs were confirmed unchanged at the end. **Everything that
follows was measured, not deduced**; where there was no measurement, it is said.

---

> **Status (2026-08-12).** Closed and validated live: **A1, A2, A4, A5**
> (1st pass) and **A3, A6, A7, A8, A10** (2nd pass, under ADR-0011). **A9
> closes by a conscious decision** — `default` remains open, and the reason is
> in ADR §5. A **new finding (A11)** appeared while validating, is PRE-EXISTING,
> and stays open with the measurement done. This note is kept alongside the
> purpose-built corrections — a findings table that does not track what was
> closed lies in both directions, as `AUDITORIA-E2E.md` did for weeks.

## A1 — CRITICAL. FIXED. Service discovery is broken for real applications

A container **cannot talk to another by name** if it uses `getaddrinfo()`
— the default path for Go, Java, Node, Python, curl, wget, nc. Measured:

```
nslookup -type=a weba   → 10.250.209.43     ✓
getent hosts weba       → 10.250.209.43     ✓
nc -w2 weba 9           → nc: bad address 'weba'      ✗
wget -O- http://weba:8080/ → wget: bad address 'weba:8080'   ✗
```

**Cause**, isolated by querying the server directly:

```
weba A     → NOERROR  10.250.209.43   (ours)
weba AAAA  → SERVFAIL                 (came from the UPSTREAM)
```

`handle_dns` only handles `qtype == 1` (A). Everything else — including the
AAAA that `getaddrinfo` **always** emits, in parallel with the A — falls into
`forward_dns` and goes to the external resolver, which knows nothing of a
single-label name and returns SERVFAIL. musl and glibc treat SERVFAIL on
either half as a failure of the whole resolution.

Only tools that ask for A explicitly work (busybox's `ping`, `getent`) —
which is exactly why this can go unnoticed in manual tests and fail in every
application.

**Fix**: for a name the index resolves, answer the AAAA with **authoritative
NODATA** (rcode 0, ANCOUNT 0). That is the correct answer — IPv6 has been off
by design since v0.37.1 — and it is what CoreDNS does.

## A2 — CRITICAL. FIXED. Internal names go outward, and there is no negative answer of our own

The server **never generates a negative answer**. Confirmed by scanning: there
is no NXDOMAIN/SERVFAIL construction anywhere in the file. So every
`.delonix.internal` that does not resolve is forwarded:

```
weba.teamA.delonix.internal AAAA → NXDOMAIN, coming from upstream
client.teamB.delonix.internal A  → NXDOMAIN, coming from upstream
```

Two consequences:

1. **Information leak.** The names of each tenant's workloads and namespaces
   are sent to the external resolver (`SLIRP_DNS`, then `1.1.1.1`). An internal
   domain should never leave the node.
2. **Latency and DoS.** Every internal name that fails to resolve costs up to
   **6 s** (2 upstreams × 3 s) and holds a thread from the 64-thread ceiling.
   Measured with the upstream unreachable: an AAAA query from a **live**
   container cost **9.03 s**, and `nslookup weba` (which emits A+AAAA) took
   **5.05 s**, ending in `*** Can't find weba: No answer`.

AGENTS.md states that a wrong namespace «gives NXDOMAIN» — it does, but the
NXDOMAIN is the upstream's, not ours. The observable behaviour matches; the
cost and the leak do not.

**Fix**: authoritative NXDOMAIN for any `.delonix.internal` the index does
not know, without ever forwarding that suffix.

## A3 — CRITICAL. FIXED (ADR-0011). DNS ignores the namespace of the requester

The dataplane isolates (measured: `client`@teamA → `webb`@teamB = 100% loss),
but the name plane isolates nothing:

```
client(teamA) → nslookup webb                        → 10.250.198.79
client(teamA) → nslookup webb.teamB.delonix.internal → 10.250.198.79
```

A tenant can enumerate the existence and exact address of every other
tenant's workloads. `dns_server_main` **has** the source IP (`peer`) and
throws it away — `handle_dns(&q)` never receives it.

This also makes the tie-break for bare names (`idx.entry(name).or_insert(ip)`,
«first wins») structurally undecidable: the right answer depends on who is
asking.

**Fix**: pass `peer.ip()` into `handle_dns`, map source → namespace through
the index that already exists, and resolve the bare name **first inside the
requester's own namespace**. This is the model of k8s's `search
<ns>.svc.cluster.local`.

## A4 — SERIOUS. FIXED. A pod member on a custom network registers the WRONG IP → real collision

Measured, with two pods:

| pod | network | real IP | registered `ip` |
|---|---|---|---|
| `p1` | `audit` (10.250/16) | **10.250.0.2** | `10.200.0.2` |
| `pdef` | default (10.200/16) | 10.200.0.2 | `10.200.0.2` |

Both pods end up with **the same address in the registry**, and that address
actually belongs to `pdef`. Measured consequence: `nslookup p1-a` returns
`10.200.0.2` — **the other pod**. Traffic addressed by name to one pod is
delivered to another, which may belong to a different tenant. Checking by the
real IP confirms the workloads themselves are fine (`nc 10.250.0.2 8080` →
`SOU-O-P1`); what is wrong is the name.

**Root cause** (`cmd/container.rs:3330`):

```rust
let ip = infra::container_ip(pn);   // "IP on the DEFAULT ingress network (10.200.A.B)"
```

`container_ip()` always computes on the default prefix, and the pod path
calls it without looking at the pod's own network. **The right function
exists and is not called**: `container_ip_on(prefix, id)`. Sixth occurrence
of a pattern already catalogued in this repo (`mount_live`, `set_net_rate`,
`update_limits`, `create_with_base`, `JsonStore::update`).

`fwmap` is correct (10.250.0.2 present) because `apply_pod_namespace_isolation`
uses the netns name and not the member's `c.ip` — that is why isolation holds
and the damage is confined to DNS. Verified: `spy`@teamB → pod@teamA = 100%
loss; `client`@teamA → pod = 0% loss.

## A5 — MEDIUM (security). FIXED. `forward_dns` accepts a response from any source

```rust
let sock = UdpSocket::bind("0.0.0.0:0")?;   // no connect()
sock.send_to(q, "10.0.2.3:53");
if let Ok((n, _)) = sock.recv_from(&mut buf) { return Some(buf[..n].to_vec()) }
```

The source is discarded (`_`), there is no `connect()`, and neither the txid
nor the question is checked against what was sent. The first UDP response to
reach the ephemeral port is delivered to the client. This is the classic
Kaminsky vector. The bridge's anti-spoofing makes it harder but does not close
it, and the fix is one line: `connect()` on the socket makes the kernel refuse
anything not coming from the upstream.

## A6 — SERIOUS. FIXED (ADR-0011). Container names are GLOBAL, not per-namespace

```
run --name web --namespace teamA   → ok
run --name web --namespace teamB   → error: the name 'web' is already in use
```

A namespace that is not a namespace contradicts its own name and the k8s
model, where uniqueness is *within* the namespace. It prevents two teams or
two stacks from both having a `db`/`web`/`api` — precisely the names everyone
uses — and forces manual prefixes (`loja-api`, `banco-api`) that hand-reinvent
the namespace already declared.

## A7 — GAP. CLOSED (warning; see ADR-0011 §4 for what was NOT done). `stack` is no isolation boundary at all

Two distinct stacks, with no `namespace` declared, both fall into `default`
and can reach and resolve each other with no barrier (measured: 0% loss, name
resolved):

```
loja-api   ns=default  {'delonix.io/stack': 'loja'}
banco-api  ns=default  {'delonix.io/stack': 'banco'}
```

The `delonix.io/stack` label is ownership for the reconciler, not network
policy. It is not a bug — nothing promises otherwise — but it is the gap that
most surprises someone coming from compose (where a project has its own
network by default). The options are to derive the namespace from the stack
name when none is declared, or to explicitly document that isolating means
declaring `namespace`.

## A8 — GAP. FIXED. A Pod has no DNS name

`nslookup p1` → SERVFAIL. Only the members (`p1-a`, `p1-b`) resolve — and
they resolve to the wrong IP (A4). k8s gives the Pod a name and, above all, a
**Service** with a stable name in front of N replicas. Without that there is
no stable service discovery: the consumer has to know the name of one
specific member.

## A9 — MINOR. CLOSED BY DECISION (ADR-0011 §5). `default` is asymmetric, and it is the opposite of a secure default

Already documented as a v1 limitation: `default` is reachable from inside any
namespace but cannot reach into them. Cloud-native practice (k8s
NetworkPolicy, and what any security audit asks for) is **default-deny** with
explicit opening. Today, whoever does not declare `namespace` ends up on a
flat, open SDN, which is the default case — so the default state is the least
secure one. Changing this is breaking and deserves its own decision (ADR).

## A10 — MINOR. FIXED (stderr; the MTU remains). Observability and MTU

- The control process's stderr goes to `/dev/null` (confirmed via
  `/proc/<pid>/fd/2`). A panic in a DNS thread is invisible: the service
  degrades and there is a line nowhere. This is what made diagnosing A1/A2
  slower than it needed to be.
- MTU: `delonix0` and the veths at 1500, `tap0` at 65520. No problem today,
  but the VXLAN overlay encapsulates (−50 B) and there is no MTU adjustment
  anywhere — it is the classic cause of «TCP hangs on large transfers» once
  inter-node traffic kicks in.

---

## What is fine, and was measured

Worth recording, so the next review does not come in suspecting this again:

- **Cross-namespace isolation of containers and pods**: correct in both
  directions. `fwmap` with one entry per workload and the right IP.
- **Control socket concurrency**: 12 concurrent attaches in **1091 ms**
  (~90 ms each), 12/12 with no failure. The 30 s ceiling fix holds.
- **Creation latency** of a container on a custom network: 249–341 ms.
- **A resolution of an internal name**: 0 ms on the server, 19–38 ms end-to-end.
- **DNS does not hang on startup**: the server answered in 0 ms even with
  threads stuck on slow forwards.

## Suggested order

1. **A1 + A2 together** — they are the same fix (handle AAAA and the internal
   suffix locally, with an authoritative answer). They unblock service
   discovery, close the name leak and eliminate the DoS vector. It is the
   highest-return fix of the whole list.
2. **A4** — one line (`container_ip_on` with the pod's own network prefix)
   and a test requiring two pods on different networks.
3. **A5** — one line (`connect()`).
4. **A3** — needs design (the `peer` has to be threaded down into the
   resolver).
5. **A6/A7/A9** — these are model decisions, not bugs; they deserve an ADR
   before code.

---

## Validation of the fixes (A1, A2, A4, A5)

A new node mounted with the fixed binary (`/tmp/dxb`, its own pin/control/slirp),
the same scenario that produced each finding.

**A1 — service discovery.** The `getaddrinfo` path, which was the broken one:

```
before: wget -O- http://weba:8080/  → wget: bad address 'weba:8080'
after:  wget -O- http://weba:8080/  → Connecting to weba:8080 (10.250.9.124:8080)
before: nc -w2 weba 9               → nc: bad address 'weba'
after:  nc -w2 weba 9               → (connects)
```

**A1/A2 — in the protocol.** Both answers are now ours, with `aa`:

```
weba AAAA                        → NOERROR, aa, ANSWER: 0, Query time 0 msec   (was upstream SERVFAIL)
naoexiste.teamA.delonix.internal → NXDOMAIN, aa,           Query time 0 msec   (was upstream NXDOMAIN)
```

**A2 — the proof that it no longer leaves the node.** With the upstream
DROPPED by nftables (the air-gapped case, which is what exposed the cost):

| | before | after |
|---|---|---|
| `nslookup weba` (A+AAAA) | **5.047 s**, `*** Can't find weba: No answer` | **0.034 s**, address returned |
| `wget http://weba:8080/` | `bad address` | **0.040 s**, resolves and connects |
| AAAA from a live container | **9.03 s** | 0 ms |

~148× on the resolution path, and the leak closes by construction: a 0 ms
response with egress blocked cannot have gone anywhere.

**I did not break the internet** — `example.com` is still forwarded and
resolves (`172.66.147.243`), and `.delonix.io` remains deliberately outside
our zone.

**A4 — collision eliminated.** The same two pods as before:

```
before: p1-a 10.200.0.2 · p1-b 10.200.0.2 · pdef-a 10.200.0.2   (all three the same)
after:  p1-a 10.250.0.2 · p1-b 10.250.0.2 · pdef-a 10.200.0.2   (each its own)
```

And with real traffic, the two fixes together: `nc -w3 p1-a 8080` from
another container resolves by name **and** reaches the right pod —
`SOU-O-P1`. Before, it failed with `bad address`, and the name pointed to the
other pod.

**No isolation regression** (the thing that most mattered not to break):
`spy`@teamB → `weba`@teamA and → pod@teamA remain at 100% loss; `client`@teamA
→ `weba` at 0%. `cargo build`/`clippy`/`fmt` clean and the whole workspace
suite passing; the two DNS decision tests **fail with the fix reverted**
(explicitly verified, per the repo's rule).

---

## Validation of the 2nd pass (A3, A6, A7, A8, A10 — under ADR-0011)

A new node (`/tmp/dxd`), with the scenario the audit could not set up before:
**two tenants with the SAME name**, something the global uniqueness used to
forbid.

**A6 — two `db`s coexist** (the second used to be refused), and each tenant
resolves its own:

```
db  ns=teamA  ip=10.250.229.6
db  ns=teamB  ip=10.250.194.118

app(teamA) → nslookup db  → 10.250.229.6     (its own)
app(teamB) → nslookup db  → 10.250.194.118   (its own)
```

**A3 — the leak closed**, and closed with address precision rather than
namespace precision:

```
app(teamA)   → db.teamB.delonix.internal   → NXDOMAIN
  (after `ingress allow teamB/db 5432 --from <app's ip>`)
app(teamA)   → db.teamB.delonix.internal   → 10.250.194.118   ← the Dependency opens it
outro(teamA) → db.teamB.delonix.internal   → NXDOMAIN         ← and ONLY for the named address
```

The neighbour from the SAME namespace, not covered by the allow, still cannot
see it — that is the difference between mirroring the policy and opening the
boundary.

**A6 on the destructive path**, which is where guessing gets expensive:

```
net ingress allow db 5432 …        → refused: «exists in several namespaces
                                      (teamA/db, teamB/db) — qualify it»
net ingress allow teamB/db 5432 …  → applies, to the right db
```

This came from a gap the change itself created and that only live validation
revealed: `util::find` started refusing ambiguous names, but `Store::load` —
where the firewall and manifests resolve — still used «the most recent wins».
An `ingress deny db` would have silently picked one tenant. Fixed in
`Store::load`, with a test.

**A8 — the pod resolves by its own name**: `nslookup web` → `10.250.0.2`
(used to give SERVFAIL). **A10** — the control log exists and was USED in
this session to rule out a panic within seconds, which was exactly what was
missing in the 1st pass.

**No regression**: external names resolve (`example.com` →
104.20.23.154); `getaddrinfo` by name works (`wget http://db:5432/` →
connects to `10.250.229.6`); 20 consecutive resolutions in **68 ms**.

## A11 — OPEN, PRE-EXISTING. The 1st resolution after creating a pod sometimes fails

Found while validating. Right after a `pod create`, the first DNS query from
an **already existing** container can time out (5.04 s); subsequent ones
answer in ~30 ms. The network is not broken (ping to the gateway at
0.127 ms) and the server is alive (a `dig` from inside the holder answers in
1 ms in the same window) — what fails is the container's request.

**Measured on both binaries, with the SAME scenario**, because the first
reading was «this is my regression» from a sample of ONE, which is the
method error this file already documents elsewhere.

| binary | failures |
|---|---|
| with these fixes | **0 / 10** |
| `main` (without them) | **2 / 10** |

So it is pre-existing and intermittent, and there is no evidence that this
work makes it worse. See **D4** below for what was done next.

---

# 3rd pass: closing DNS for critical production (2026-08-12)

Request: «everything needs to resolve by name». A MEASURED inventory of what
resolved, done before touching anything — because until now «what's missing»
was guesswork.

| form | before | now |
|---|---|---|
| `api` (bare name) | ✓ | ✓ |
| `api.loja.delonix.internal` | ✓ | ✓ |
| `api.delonix.io` (legacy) | ✓ | ✓ |
| **network alias** (`backend`) | **✗** | ✓ |
| **`api.loja`** (short) | **✗** | ✓ |
| **DEAD workload** | **used to resolve** ✗ | NXDOMAIN ✓ |
| external (`example.com`) | ✓ | ✓ |

## D1 — a dead workload still resolved

Measured: a container that exits on its own becomes `status=Stopped` with
its address still in the registry, and `nslookup` kept delivering it. A
client would connect to a service that no longer exists and wait for a
timeout, instead of failing fast with NXDOMAIN. And the lease goes back to
IPAM, so a dead service's name can end up pointing to whatever workload
receives that address next.

**I could not trigger the reassignment** in 6 attempts — addresses derive
from the id and short-term collision is unlikely. It is stated anyway,
because that is the serious consequence and I did not demonstrate it.

The index now serves only `Running`/`Paused`/`Created`. `Failed(code)` and
the legacy `Exited` serialize as OBJECTS and arrive as `None`, same as a
stateless record — all three stop answering. Fail-closed, which is the right
side of the question «is this address still yours?». The same applies to
stopped VMs.

## D2 — network aliases were never consulted

`--network-alias`/`networkAlias:` had always been persisted in `net_aliases`
and **nothing read it** — the option was accepted and did nothing, the
failure shape this repo hunts for. They are now indexed as names, with the
same namespace scope and with `or_insert`, so that an alias never displaces a
container that genuinely owns that name. Validated: `backend` and `api-v2` →
the `api`'s address.

## D3 — the short form `<name>.<namespace>`

`api.loja` did not resolve; only the bare name and the full FQDN did. k8s
gives this via `search` in `resolv.conf`; here it is done in the resolver
itself on purpose — a `search` makes the client glue our suffix onto **every**
external failure, which is traffic and a default leak.

It is only attempted when the whole string does not resolve as a proper name,
and **the namespace has to exist on this node** for the name to be ours:
without that, an `api.loja` that failed to resolve would leak out with the
tenant's namespace attached. A name with more than one label after the head
(`api.github.com`) is never ours — and a container in a namespace called
`com` does not make us claim `github.com`. Validated: `naoexiste.loja` →
**authoritative NXDOMAIN (`aa`) in 1 ms**.

## D4 — A11, and what was done without fixing it blindly

It did not reproduce with the current code in the sequential form (0/10). It
reproduced with a CONTINUOUS probe from inside a container while pods were
being created: **2 in 300** (~0.7%), versus 0 in 90 with no creation at all.
I did not isolate the cause, and a fix without an isolated cause is a guess.

**Method note**: the first probe ran `dig` from the holder and gave 200/200
«failures» — the holder does not belong to the namespace, so scope correctly
refused it. That was isolation working, not a malfunction. A probe has to run
from where the real client runs.

What was done instead is a different thing and is defensible on its own:
**`options timeout:2 attempts:3` in `resolv.conf`**. DNS is UDP, and a lost
datagram while the node is reprogramming the dataplane is a NORMAL event; the
libc default is `timeout:5 attempts:2`, i.e. **five seconds** before the
first retry — that is what turns a transient loss into a visible stall.
Measured afterward, in the same scenario with 16 pods created during the
probes: **0 failures in 600**.

This does not claim the cause was fixed — it claims the client stopped
feeling it. The cause remains open, with its number.
