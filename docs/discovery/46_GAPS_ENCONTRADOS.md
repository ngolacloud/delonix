# 46 — Discovery: network/namespace foundation (block A) and data (block B)

| Field | Value |
|---|---|
| Date | 2026-08-10 |
| Prompt baseline | `0.45.0` (commit `544060a`) |
| **Real baseline on `main`** | **`0.46.0`** (commit `801deee`) — one version ahead of the prompt |
| **Binary serving the live holder** | **`0.44.0`** (commit `663c9b6`, `/usr/local/bin/delonix`) |
| Method | Live measurement on a real rootless host + code reading |
| Production changes | None. This document is the only artefact. |

> **Reading warning.** Everything below was either measured or read. Where
> measurement was not possible, it says `NOT MEASURED` and why. No
> classification is inferred from documentation — there was at least one case
> (§4.1) where the documentation states the opposite of the actual behaviour.

---

## 0. Measurement conditions (what validates and what does not)

### 0.1 The live holder is two versions older than `main`

```
$ /usr/local/bin/delonix --version   → 0.44.0 (663c9b6)   ← started the live pin/control
$ grep version Cargo.toml            → 0.46.0
$ git diff --numstat v0.44.0..HEAD -- crates/adapters/delonix-net
21   7   crates/adapters/delonix-net/src/bpf.rs
172  41  crates/adapters/delonix-net/src/infra.rs
```

The dataplane **changed** between the live holder and `main`. The two commits are
`d5d6553` (second state root) and `801deee` (v0.46.0 audit). What changes in `infra.rs`:
`foreign_holder_message`, `antispoof_rule_args` (shared veth/tap), the antispoof on
`do_vmtap`, and the signature of `reap_orphan_hostfwds`.

**Consequence for this discovery**, applied path by path:

- the per-container namespace/firewall chains **did not change** → the §2 and §3
  measurements are valid for `main`;
- the **VM tap antispoof** exists on `main` and **not** on the live holder → a test here
  would measure a gap `main` has already closed. It is not reported as a gap.

The host has 5 production containers (Odoo/Postgres) alive. **The holder was not
respawned** — so no fix to the holder's code is validatable live in this session, only by
test. This is the same constraint already recorded in `AGENTS.md`.

### 0.2 Test bench

Four `alpine:latest` containers on the existing SDN (`kaeso-net`), across three
namespaces, plus one pod. Server `nc -l -p 8080`; client `nc`/`wget`. All removed at the
end (§6).

> **Method trap, cost two wrong measurements.** `nc` speaking HTTP with
> `tail -1` returns empty both for *blocked* and for a *404 with no body* — I ended up
> recording "blocked" for a path that actually **passes**. Only an explicit control (the
> same request from the host, which answered `WIN`) separated the two, and the
> conclusion reversed. Every reachability measurement in this document has a positive
> control; wherever the verdict is "blocked", there is a line next to it proving that the
> same apparatus reaches when it should.

---

## 1. Reachability matrix (A1)

Measured with real policy and namespaces. `REACHABLE`/`blocked` is command output, not
expectation.

| # | Data path | Measurement | Verdict |
|---|---|---|---|
| 1 | Primary IPv4, cross-namespace | teamA→teamB `blocked`; teamB→teamA `blocked`; default→teamA `blocked`; teamA→default `REACHABLE` | ✅ correct (asymmetry documented) |
| 2 | Multi-homing (extra network IP) | cross-ns `blocked` by the primary **and** by the extra one; `@fwmap` maps the 2 IPs to the **same** chain | ✅ covered |
| 3 | IPv6 / ULA | `disable_ipv6=1`, 0 `inet6` addresses; `table ip6 dlxing` `policy drop` with `counter 241 packets` | ✅ closed |
| 4 | Published port (`-p`) | `allow`→`OK`; `policy deny`→empty; `+allow 8080`→`OK` | ✅ governed |
| 5 | `proto: any` + bare port | `allow 9999` does **not** open 8080; `allow 8080` opens it | ✅ correct — **proven live** (was unit-test only) |
| 6 | A container's own egress under `policy deny` | `REACHABLE` (v0.37.x's `ct state`) | ✅ correct |
| 7 | `--net host` / `none` | `ingress ls` says `n/a (host net)` | ✅ honest |
| 8 | L7 proxy (`httproute` / `--expose`) | was: teamB→proxy→teamA `WIN` even with `policy deny`. Now: `timeout`, host→proxy `WIN` (§4.2, fixed) | ✅ closed |
| 9 | pod netns | cross-namespace `blocked` on the custom network; `spec.network` was being ignored (§4.4, fixed) | ✅ covered |
| 10 | VM `tap` | **NOT MEASURED** — no image on this host boots on Cloud Hypervisor (the golden is libvirt-only). The antispoof exists on `main` (`antispoof_rule_args`), absent on the live holder | — |
| 11 | `vm bridge` (privileged) | **NOT MEASURED** — requires `root` and a veth in the host's init-netns; not run on a host with live production | — |
| 12 | Network storage (NFS/CIFS) | **NOT MEASURED** — `mount -t nfs` requires `CAP_SYS_ADMIN`, unavailable in pure rootless | — |

Chain precedence, read from the kernel (`nft list ruleset` inside the pin's netns), all
in the `forward` hook — priority comparison is only valid within the same hook:

```
fwguard  priority -20   ip daddr 169.254.0.0/16 drop ; ip daddr 127.0.0.0/8 drop
fwdeny   priority -10   antispoof per veth (8 veths, 8 rules — 1:1)
fwcont   priority  -5   ip daddr vmap @fwmap ; ip saddr vmap @fwmap
forward  priority   0   policy drop; ct established,related accept; …
```

---

## 2. What is correct and confirmed (leave alone)

Recorded because half the value of an audit is knowing what does **not** need work:

- **The v0.37.0 CRITICAL (`proto: any` dropping the port) is closed, now proven
  live.** `AGENTS.md` listed it as "proven by unit test, NOT live" because the holder
  could not be respawned. The current holder already has the code; the §1/#5
  measurement closes that documentation debt.
- **Multi-homing is covered in practice**, not just by reading: `@fwmap` has
  `10.210.51.70 : jump fw237dd714` **and** `10.233.51.70 : jump fw237dd714` — the same
  destination for the two IPs of the same container.
- **`pod rm` without `-f` is exemplary fail-closed**: refuses, names the member still
  running, and prints the recovery command.
- **`net ingress ls`** already distinguishes `n/a (host net)` from `allow (default)`.

---

## 3. Findings — block A

### 4.1 HIGH — `container rm` never deregisters the L7 proxy's auto-route

> The documentation states the opposite: *"auto-registered … removed on `container rm`"*
> (`AGENTS.md`, reverse-proxy section).

**Reproduction:**

```
$ delonix container run -d --name d46-x --net kaeso-net --expose 8080 alpine:latest sleep 300
$ grep -o '"expose":[^,]*' ~/.local/share/delonix/containers/*.json | grep -v null
(nothing)                                   ← the field is NEVER persisted
$ delonix container rm -f d46-x
$ cat ~/.local/share/delonix/httproute/auto.json
[ { "name": "d46-a", "namespace": "teamA", "ip": "10.210.23.139", "port": 8080 },
  { "name": "d46-x", "namespace": "default", "ip": "10.210.15.167", "port": 8080 } ]
```

Both containers **no longer exist** (`container ls -a | grep -c d46-a` → `0`).

**Root cause.** The removal path exists and is correct —
`ingress_proxy::auto_deregister` (`ingress_proxy.rs:751`), called from
`container.rs:4521`. What fails is the **guard**:

```rust
// container.rs:4520
if c.expose.is_some() {
    super::ingress_proxy::auto_deregister(&c.name);
}
```

`c.expose` is assigned at `container.rs:2977`, but that branch's `store.save` never
actually writes the field to disk — no container record has a non-null `expose`. The
guard is therefore **always false** and `auto_deregister` is, in practice, dead code.

**This is the 5th occurrence of the already four-times-documented trap** ("state
needed to RECONSTRUCT the resource must be persisted, not just used at creation time" —
`-v`, `-p` on a custom network, extra networks, `Container.pod`). Here it does not cost a
reconstruction: it costs a route that outlives its owner.

**Security consequence.** The route keeps pointing at an SDN IP that the IPAM
**reassigns**. The next container to receive `10.210.23.139` silently starts receiving
traffic addressed to `d46-a.teamA.delonix.internal` — from another namespace, owned by
someone else. Combined with §4.2 (any container reaches the proxy), it is a cross-tenant
delivery path.

**Operational aggravating factor:** there is no command to clean this up. `httproute rm`
only touches the MANUAL part — by design — and `auto_deregister` has no CLI surface. The
recovery in this session was editing `auto.json` by hand.

**State: consequence FIXED, root cause STILL OPEN.**

The guard was removed — `auto_deregister(&c.name)` now always runs on `rm`. It is safe
and cheap: `with_auto_locked` exits without writing (and without rebuilding the proxy)
when the list does not change, and the function's own contract is already "if it was not
registered, do nothing." Cleanup now depends on the **name**, which we always have,
instead of a registry field.

Validated live, same command before and after:

```
before (with the guard)  rm of d46-a/d46-x/d46-y → auto.json left with the 3 orphan routes
after (without the guard) rm of d46-fix           → auto.json empty, proxy stopped
```

No unit test at the call site, for a concrete reason: `with_auto_locked` calls
`rebuild()`, which talks to the holder and starts/stops the proxy — a `cargo test` would
have side effects on the node. The proof is the live validation above.

**Still to resolve: why `expose` never reaches disk.** Ruled out in this session, so it
is not repeated:

- the re-exec spec **does** carry the value — caught in flight:
  `.reexec-<id>.json` → `expose = 8080`;
- `run_from_spec` (`container.rs:3813`) deserializes `RunOpts` and passes it **whole** to
  `cmd_run` — there is no field-by-field reconstruction losing it;
- `Container::expose` has no `skip_serializing` (only `#[serde(default)]`), so it would
  be written if present;
- there is no `store.save` in `container.rs` after line 2979 on the `run` path;
- the 2nd pass **does** reach the block that writes it — proved by `c.ip` at line 2952,
  which ends up correct in the record.

What remains is explaining how the local variable `expose` ends up `None` on the 2nd
pass despite the spec carrying it. The symptom persists with the fix above applied
(which does not touch it) and remains observable in one line:
`delonix container run -d --net <network> --expose 8080 …` followed by checking `expose`
in the record.

---

### 4.2 HIGH — the L7 proxy breaches namespace isolation **and** per-container policy

**Reproduction** (backend `d46-a` in `teamA`, client `d46-b` in `teamB`):

```
teamB → teamA direct ................. wget: download timed out   [blocked ✓]
teamB → proxy → teamA ................ WIN                        [PASSES ✗]
  same, with `ingress policy deny` on the backend ... WIN          [PASSES ✗]
```

Positive control: the same request from the host returns `WIN`; the backend answers
itself; `nc` to the gateway's ports 9 and 53/TCP gives `rc=1` (refused) and port 8080
gives `rc=0` (accepted) — containers **do reach** the proxy listener.

**Root cause** — two facts that compose, neither a standalone bug:

1. The proxy runs **inside the pin's netns**. Proxy→backend traffic **originates from
   the holder**, so it crosses `output`/`postrouting`, **never the `forward` hook** — and
   every policy chain (`fwguard`, `fwdeny`, `fwcont`, `forward`) is in `forward`.
   Per-container firewalling does not see this traffic, by construction.
2. The holder **has no `input` chain** (already recorded in `AGENTS.md`, there as the
   reason publish does not need DNAT). So any container reaches the proxy's listener on
   its bridge gateway IP.

The result is an ungoverned relay: whoever reaches the gateway reaches **any** registered
backend, in **any** namespace. The FQDN does not even need guessing — it is
`<name>.<namespace>.delonix.internal`, and internal DNS resolves names.

#### GO/NO-GO spike of the two containment options (measured, without changing code)

**Option A — an `input` chain in the pin's netns.** Prototyped **by hand** inside the
netns, with `nft -f`, then removed — without touching code or respawning the holder:

```
chain dlxinput { type filter hook input priority filter; policy accept;
                 iifname "tap0" accept ; iifname "lo" accept ; tcp dport 8080 counter drop }
```

| Measurement | Before | After |
|---|---|---|
| host → proxy → backend (ingress's purpose) | `WIN` | `WIN` |
| **teamB → proxy → backend (the leak)** | `WIN` | **`download timed out`** |
| Internal FQDN DNS from a container | 1 A record | 1 A record |
| teamA → teamA direct (non-regression) | — | `WIN` |
| drop rule counter | — | `6 packets / 360 B` |

**GO.** Closes the leak, preserves ingress, and the counter proves it was the rule. Two
checks that mattered and passed: service discovery does **not** depend on the proxy —
`s46-be.teamA.delonix.internal` resolves to `10.210.243.29`, the container's own IP, so
container→container by name goes direct and remains governed by isolation; and
legitimate traffic within the same namespace was unaffected.

Scope: it is structural — covers **any** listener resident on the holder, present or
future, not just this proxy. Cost per packet: 3 rules in a hook that today has no chain
at all. Limitation: it is a coarse boundary — a container loses the ability to use the
proxy **even within its own namespace** (for instance for TLS termination, or to reach a
manual `kind: HTTPRoute` backend).

**Option B — the proxy decides at L7.** Viable and finer-grained, confirmed by reading:
`ingress_proxy.rs:319` is already `let (stream, _peer) = listener.accept().await` — **the
client's address exists and is being discarded**; and the process already resolves
`state_root()` (line 545), so it can map IP→container→namespace. It requires deciding the
"client with no identity" case (a normal request comes from outside the node with no
namespace — presumably allow), registry cache invalidation, and it only governs **this**
proxy. It cannot be validated by a kernel counter: it requires rebuilding and respawning
the proxy.

**They are not mutually exclusive.** A is the structural, cheap, verifiable containment;
B is finer policy on top. The recommendation is A first (closes the leak now, with
proof), and B only if a use case appears for a container wanting to use its own
namespace's ingress.

#### State: A implemented and validated live with the holder respawned

The `dlxinput` chain entered the base ruleset (`ingress_table_ruleset`), with a noisy
escape hatch `DELONIX_ALLOW_HOLDER_INGRESS=1`. Confirmed in the kernel after the respawn:

```
chain dlxinput { type filter hook input priority filter; policy accept;
  ct state established,related accept ; iifname "lo" accept ; iifname "tap0" accept
  udp dport { 53, 67, 68 } accept ; tcp dport 53 accept ; meta l4proto icmp accept
  ct state new counter packets 7 bytes 420 drop }
```

| Measurement with the code (not the prototype) | Result |
|---|---|
| host → proxy → backend | `WIN` |
| teamB → proxy → backend | `download timed out` |
| drop rule counter | `7 packets / 420 B` |
| internal DNS from inside a container | `kaeso-db.default.delonix.internal` → `10.210.58.41` |
| external DNS (forwarding) | `github.com` resolves |
| a container's egress to the internet | `rc=0` |
| production throughout all this | Odoo `HTTP 303`, Postgres accepts connections |

The `SERVFAIL` for a **bare** name (`nslookup kaeso-db`) appears both before and after the
change — it was measured at the start of the session, with the old holder, and is not a
regression from this chain.

The regression test asserts **order** (the drop has to be the last rule, otherwise the
allowlist after it is dead code) and was demonstrated to fail with the protection reverted.

---

### 4.3 MEDIUM — the `@dlxall` set only grows — **FIXED**

```
@dlxall entries : 49        live veths : 8        registered containers : 5
```

`ns_set_join` (`infra.rs:1863`) inserts into `@dlxall` and into `@dlxns_<ns>`; **there is
no `ns_set_leave`** and `do_detach` removes nothing. The function's comment reasons
correctly about a namespace change (the join takes the IP out of the previous `@dlxns`),
but nothing handles permanent departure.

It is not a policy leak — `@dlxall` is only read to *drop* (`ip saddr @dlxall ct state
new drop`), so one extra entry never opens anything. It is unbounded kernel-state growth
and diagnostic noise: an `@dlxall` with 49 addresses on a node with 8 veths does not help
answer any operational question. During this very session it rose from **49 to 74** — the
leak measured actually happening.

**Fixed** with `ns_set_leave`, on its own control line (`nsleave <ip>`) and not inside
`detach`: `detach` carries no address, and `unfirewall` — which does — is also sent by
`clear_firewall` to a **live** container, so hanging the removal there would evict a live
peer from the sets. Sent best-effort, so an old holder simply refuses it and behaves as
before, instead of failing the teardown.

Validated live with the holder respawned: **5 → 4 → 3 → 2** elements, one per removal, and
the two left over are exactly the IPs of the two production containers.

---

### 4.4 MEDIUM — `kind: Pod` ignored `spec.network`, and the reported IP was recomputed

**FIXED.** A `kind: Pod` with `spec.network: kaeso-net` booted on `10.200.0.2` — the
**default** bridge, not `kaeso-net` (`10.210.0.0/16`). It was not the manifest:
`create_pod` passed a **hardcoded `"ingress"`** to `attach_container` (`pod.rs:148`), and
the word `network` never appeared anywhere else in the module. The field was parsed,
documented as a delonix extension, and had no effect at all — a pod ended up unreachable
from the network it asked for and reachable by everything on the one it got instead.

**The obvious fix alone would have put the engine in a position to lie**, and that is the
part worth recording. `ls`/`describe`/`rm` never **read** the address: they recomputed it
with `infra::container_ip`, which fixes the default prefix. It happened to be accidentally
correct while every pod landed on the default bridge; as soon as the network is actually
honoured, all three started reporting — and `rm` **detaching** — an address the pod never
had. Measured: `pod create` said `10.210.0.2` and `pod ls`, right next to it, said
`10.200.0.2`.

The real address is now recorded in a label at attach time
(`delonix.io/pod-ip`, the same "membership from labels" idiom as `POD_LABEL`, no new
store), falling back to the old recomputation for pods created before the label existed.

Validated live: `create`/`ls`/`describe` all say `10.210.0.2`, matching what the kernel
has in the netns (`ip netns exec pod-d46-pod` → `10.210.0.2/16`); the pod's
cross-namespace isolation still blocks on the custom network; and `rm` cleans up the netns
with no leftovers. **Row 9 of the matrix becomes ✅.**

> **Lesson, the same as §4.1 by another path:** the value was not being read from where
> it was decided — it was being *re-derived* by a formula that only happened to match
> reality while there was a single case. A formula that reproduces state instead of
> reading it is a second source of truth waiting to diverge.

---

### 4.5 LOW — `net ingress clear` does not warn that the policy survives

`clear` does exactly what it promises (*"Remove all inbound rules"*) and
`firewall.rs:849` only discards the entire firewall when **both** policies are also at
default — that is design, not a defect, and `ingress ls` shows the `deny` that remains.
What is missing is one line: after `d46-c: removed 0 inbound rule(s)` the container
remains fully closed and the output does not say so. A note naming
`net ingress policy <c> allow` closes it.

*(Also recorded because this finding's initial characterization was wrong — it was read
as a bug before the code was read, and the code disproved it.)*

---

## 5. Findings — block A2 (namespace)

Real surface, read from the code:

```
$ grep -rln 'pub namespace' crates/*/src/*.rs
crates/foundation/delonix-runtime-core/src/lib.rs      (Container)
crates/adapters/delonix-vm/src/lib.rs                (Vm)
```

**Only `Container` and `Vm` have a namespace.** `Network`, `Volume`, `Storage`,
`ShareVolume`, `Secret`, `Image`, `HTTPRoute`/`Ingress`, `Dependency` and the `compose`
project **have no field at all** — so `metadata.namespace` on them is accepted by the
parser and has no effect, which is exactly the kind of silent failure cycle rule 2
forbids. (Not confirmed command by command: it is a reading of the registry, not a
per-resource measurement.)

**`container describe` never prints the namespace** — `vm describe` does
(`vm.rs:2032`, `d.sub("Namespace", …)`), `container describe` has not a single
occurrence. A2's acceptance criterion ("`describe` always shows the effective
namespace") fails today on the most-used of the two resources.

### 5.1 What was done, and the underlying decision that remains

**`container describe` now always prints the namespace** — `default` included. A field
that disappears when it has the default value forces the reader to guess whether the
resource has no namespace or whether nobody assigned it one.

**The silence of the other Kinds became a warning.** `manifest::load` warns when
`metadata.namespace` is written on a Kind that does not honour it, naming the ones that
do. The warning fires **before `Stack` expansion**, on purpose: a namespaced Stack
propagates the namespace to all its children, and warning afterwards would fire one line
per child for a field the user never wrote there. Confirmed live: a top-level
`kind: Network` with a namespace warns (EN and PT), a namespaced Stack with three
children warns **zero** times and the namespace reaches the three.

**Decision recorded — namespace is network isolation, not a naming scope.** Only
`Container`/`Pod`/`Vm` honour it because only a workload with an address participates in
the rules that implement it (`@dlxns_<ns>` + the cross-namespace drop). Giving a namespace
to `Volume`/`Storage`/`Secret` would mean **NAMING scope** — two namespaces being able to
have a volume both named `db` — and that is a store key-change with its own migration
question. It is exactly the `Storage`/`ShareVolume` cycle item marked as a decision for
you to make; it was not pre-empted here.

**Already satisfied, verified in code:** the three Kinds that honour it persist it
(`Container.namespace` and `Vm.namespace` with `#[serde(default)]`; the pod derives it
from its members' records) and reconstruct it on `start` and on post-respawn recovery.

---

## 6. Host state at the end

Test bench fully removed: 4 containers, 1 pod, 1 network, 2 orphan routes (these by hand,
see §4.1) and the proxy stopped. Confirmed: `container ls -a` shows only the 5 production
ones (`kaeso-odoo18-multi`, `kaeso-odoo18`, `kaeso-odoo-8016`, `kaeso-db`, `kaeso-db18`),
the holder was not touched and the 5 stayed running throughout the session.

---

## 7. Suggested order of work

1. **§4.1** — persist `expose` + make `auto_deregister` unconditional. Small, testable
   without a holder, and closes the 5th occurrence of a recurring trap.
2. **§4.2** — L7 proxy containment. It is the highest-reach finding and the only one
   requiring a behaviour decision; should not go in hastily.
3. **§5** — `container describe` printing the namespace (trivial), then the underlying
   decision about which resources actually get a namespace.
4. **§4.3 / §4.4 / §4.5** — in this order.

---

## 8. Block B1 — volumes without data loss

Measured before changing anything, with a full inventory of metadata writes per crate.

| B1 requirement | Measured state | Action |
|---|---|---|
| Atomic metadata writes (temp + `fsync` + mode at creation + rename) | `Store`/`JsonStore`/`delonix-volume`/`ipam.rs` already atomic; **`NetworkStore` not** (7 writes with raw `fs::write`), `delonix-vm` with 2 | ✅ fixed |
| `fs::remove_dir_all` does not erase accounting first | Already correct: the volume's `remove` erases "everything EXCEPT the metadata" and only then `meta.json`, with the EACCES/subuid reason written in the code | ✅ nothing to do |
| An unreadable directory ≠ empty | Already correct: `Usage { bytes, unreadable }` and `QuotaState { measured }` — an incomplete measurement is *unknown*, never zero | ✅ nothing to do |
| Network volumes: what happens to an in-flight write when the NAS disappears | Read from the emitted options: only `credentials=`/`ro`/extras — **no `soft`/`timeo`/`retrans`**, so NFS's `hard` default applies | ⚠️ documented, default kept |

### 8.1 `NetworkStore` was the only store in its own crate without atomic writes

`ipam.rs`, in the same crate, has used `write_atomic` all along. `NetworkStore` wrote its
seven records with `fs::write`, which **truncates** the destination and only then fills
it. Two failure modes, and the second is the bad one:

- a torn multi-line body loses lines, and `get()` deliberately ignores missing keys (it
  is what lets an older binary read a newer record), so the network comes back
  **degraded** instead of failing;
- the base octet is worse: `get()` parses a bare number as the old format, so `"142"`
  truncated to `"14"` is still a **perfectly valid** octet. The network's `/16` changes
  silently and every container on the old prefix becomes unreachable.

It is not hypothetical. With the fix reverted, the new test catches concurrent readers
getting `network 'ov' is corrupted` — repeatedly, across several threads.

**Two raw `fs::write` calls remain, for a reason stated in the code**: the two `/proc`
sysctls. The kernel takes the value at write time, there is no file to publish and
nothing to tear, and `write_atomic`'s `rename` is not even accepted there.

### 8.2 NFS: a NAS that disappears blocks forever — and that is the correct behaviour

Without `soft`/`timeo`, an in-flight write does not fail: it **blocks indefinitely** in
uninterruptible sleep, and the process cannot be killed until the server answers. The
default stays as is on purpose — `soft` turns the same failure into an `EIO` mid-write,
which for a database is silent corruption instead of a halt, exactly what this block
exists to prevent.

Documented because the symptom the operator sees ("the container is stuck and won't die")
does not point at the NAS. The escape hatch already exists and needs no new flag: pass
`soft,timeo=50,retrans=2` in the extra mount options.

**NOT measured** — this host does not mount NFS/CIFS at all (`mount -t` requires
`CAP_SYS_ADMIN`, unavailable in rootless). The above is read from the emitted options and
from `nfs(5)`, not observed.

---

## 9. Block B2 — per-namespace shared storage

Decisions made by the user before any code: *context* **is the namespace**, and migration
is via an **explicit command** (old records keep working).

### 9.1 The defect, and the second layer the statement did not anticipate

The data path was `<storage>/shares/<name>`, **with no namespace**: two tenants naming
their share `db` pointed at the SAME directory, and one's `--purge-data` took the other's
data with it.

Scoping only the record and the path would have made the acceptance criterion pass **to
the letter** while leaving a bug of the same family behind: the share is also registered
as a **volume** (`register_external`), and it is by that name that a container consumes
it (`-v db:/data`). The two would still register a volume `db`, the second on top of the
first — and `-v db` would resolve to the other namespace's data.

And there was no safe separator to invent: `VolumeStore::valid_name` accepts
alphanumerics, `-`, `_` and `.`, so any `<ns><sep><name>` is ambiguous — the same
collision class compose project names already had to solve. The way out was a
**directory boundary**: `volumes/.ns/<namespace>/`, and `.ns` cannot collide because
`valid_name` rejects names starting with a dot.

### 9.2 Resolution, and why it REFUSES instead of choosing

`resolve_spec_in(spec, namespace)` looks first for the namespace's own volume and then the
global one — the "two read paths" the migration decision called for, so nothing that
exists today stops resolving. If both exist, it **refuses**: two different volumes answer
the same `-v db`, and silently picking one is exactly how a workload ends up writing
into another tenant's data. The error names `migrate`.

`Storage` stays **global on purpose**: it is the NAS mount, node infrastructure. Scoping
it would mean one mount per namespace of the same export.

### 9.3 Validated live, with real NFS

Impossible on this host (`mount -t nfs` requires `CAP_SYS_ADMIN`), so validation ran
**inside a VM** (`delonix-vm-base:ubuntu-24.04`, libvirt), where there is root — with an
`nfs-kernel-server` exporting `/srv/nas` and `delonix storage create` mounting it for real.

```
$ delonix sharevolume ls
NAMESPACE   NAME   STORAGE   MOUNTPOINT
teamA       db     nas1      …/volumes/nas1/_data/shares/teamA/db
teamB       db     nas1      …/volumes/nas1/_data/shares/teamB/db

$ find /srv/nas -type d          # on the real NAS
/srv/nas/shares/teamA/db
/srv/nas/shares/teamB/db

BEFORE: teamA/db/mark:data-from-teamA
        teamB/db/mark:data-from-teamB
$ delonix sharevolume rm db --namespace teamA --purge-data
sharevolume/db: removed (data deleted)
AFTER:  teamB/db/mark:data-from-teamB          ← intact
```

**Migration, also live.** A record in the PRE-scope format (flat under
`sharevolumes/`, **no** `namespace` field) is read and reported as `default`;
`migrate --dry-run` says what it would do without changing anything; `migrate` moves
`sharevolumes/old.json` to `sharevolumes/default/old.json` — and the legacy data stays
where it was (`legacy-data`, read after migration). It moves **records, not data**: each
record carries the path it was created with, and touching a tenant's bytes is not
something a migration should do behind their back.

### 9.4 Consequence for A2

`kind_honors_namespace` now includes `ShareVolume`, and it is the **only** Kind where
the namespace scopes a NAME rather than reachability. A2's comment said the opposite and
was corrected — leaving it would have had the code lying about itself.
