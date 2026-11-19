# Delonix Runtime networking guide

> From basics to advanced, for whoever operates with **only** `delonix` — no PaaS, no
> control plane, no daemon. Written against the real binary; every output here was
> obtained by running the command, and whatever does not run in this environment is flagged
> as such instead of being made up.

---

## 0. What this tool is — and what it is not

`delonix` is **a single host's mechanism**. It creates bridges, netns, veth, nft rules and
tunnels on that machine. It does not know what a tenant is, has no user database,
and does not talk to other nodes unless you explicitly tell it
which ones (`--peer`).

| | `delonix` (this guide) | `delonixctl` |
|---|---|---|
| where it runs | **on** the host being changed | from anywhere |
| how it talks | kernel calls, files under `$DELONIX_ROOT` | HTTP only, `/v2/*` |
| unit | container, VM, network, rule | app, addon, team, org |
| who authorizes | the Unix user running the command | Bearer token + RBAC |

If you need tenants, quotas, RBAC, or to operate ten machines at once, that's
`delonixctl` — this guide will not take you there. If you need to segment networks on one
machine and prove they are segmented, you're in the right place.

### Two properties that explain almost everything else

**Rootless.** An unprivileged process does not have `CAP_NET_ADMIN` in the
host's netns — it only has it inside its own user+network namespace. That is why
Delonix does not create the bridge "on the host": it creates it inside a namespace it
owns itself (the *holder*), and connects workloads to it via veth. That is also why
there is only **one** networking command that needs root (`network vlan`), and it
tells you so to your face.

**Daemonless.** There is no resident process reconciling anything. The holder comes up at
the first need and goes down when the last workload lets go (`refcount 0`). A
command that returned no error did what it said; there is no queue where an intent
sits marinating.

### Declared ≠ realized

`network create` **declares** a network: it writes the record, chooses the `/16` and the
bridge name. The bridge only comes into being in the holder's netns on the first `attach`. This
is deliberate — declaring should not require live infrastructure — but it means that
`network ls` showing a network does **not** prove the bridge exists. For that, use
`net netns status`.

---

## 1. First steps

```bash
delonix network create app
delonix network ls
```

```
NAME      DRIVER   BRIDGE         SUBNET
app       bridge   dlxn1f6a832c   10.207.0.0/16
backend   bridge   dlxn79b4a5e1   10.220.0.0/16
```

The bridge name (`dlxn` + a hash of the name) is derived, not chosen: the same
network name always yields the same bridge, on any node. That is what lets two
nodes of an overlay agree on the device without negotiating it.

Three views of the same network, in increasing order of verbosity:

```bash
delonix network inspect app              # compact
delonix network inspect app -o json      # for automation
delonix network describe app backend     # readable block, kubectl-describe style
```

```
name:     app
driver:   bridge
bridge:   dlxn1f6a832c
subnet:   10.207.0.0/16
gateway:  10.207.0.1
```

Removing:

```bash
delonix network rm app
```

Removes the record, the VXLAN uplink (if it's an overlay), **and** tells the holder to
delete the bridge.

> **It does not check whether workloads are attached.** There is no "network is in use"
> guard stopping you: the bridge disappears and the containers that were on it are left without
> networking, without being stopped. If that is undesirable in your environment, the gate has
> to be yours — a `delonix net netns status` beforehand, or a rule in the
> procedure. Worth knowing before scripting an `rm`.

---

## 2. Address space

There are **two** regimes, and confusing them is the source of half the surprises.

### 2.1 Without `--subnet`: the engine picks, and picks `10.<200-254>.0.0/16`

```bash
delonix network create app
```

The on-disk record stores **a single octet**; the bridge, the gateway, and the IPAM
range are all derived from it. This is the default path and the cheapest to reason about:
one network, one `/16`, no possible overlap.

### 2.2 With `--subnet`: any private range, `/8` to `/28`

```bash
delonix network create backend --subnet 10.220.0.0/16
delonix network create small   --subnet 10.221.0.0/24     # /24 is accepted
delonix network create legacy  --subnet 192.168.5.0/24    # 192.168 too
```

Here the record stores the whole CIDR, and the rules differ: it must be
**private (RFC 1918)** space — `10.0.0.0/8`, `172.16.0.0/12` or `192.168.0.0/16` — with a
prefix between `/8` and `/28`, and it may not **overlap** an existing network.
The two refusals, verbatim:

```
error invalid argument: subnet '8.8.8.0/24': outside the private address space
(RFC 1918). Use a private range — `10.0.0.0/8`, `172.16.0.0/12` or
`192.168.0.0/16` — with a prefix between /8 and /28, or omit --subnet to let the
engine pick a free one

error conflict: subnet 10.220.0.0/24 overlaps network 'x' (10.220.0.0/24)
```

The `spec.subnet` of a `kind: Network` follows exactly the same rules — the
declarative and the imperative do not diverge here (verified by applying the three cases
above via manifest).

### 2.3 The ceiling: 55 auto-assigned networks

The automatic allocator has `10.200`–`10.254`, that is, **55** slots. At the 56th network
created **without** `--subnet`:

```
error conflict: no free /16 left for network 'one-too-many': the workload space
10.200.0.0-10.254.255.255 holds 55 networks and all are taken — remove one
(`delonix network rm <name>`) to free a subnet
```

A refusal, not a silent duplicate. The ceiling belongs to the allocator, not the engine:
networks with an explicit `--subnet` do not come out of those 55 slots, so an
organization needing more than that can keep using its own addressing. Still, it's
worth deciding this early — in a segmentation with one network per team and sixty teams, the
unit of isolation probably ends up being the *per-workload firewall rule* (section 5)
rather than the network.

---

## 3. The drivers

| driver | what it does | rootless |
|---|---|---|
| `bridge` | its own bridge + its own `/16`, filtered by the firewall | **yes**, it's the realized path |
| `macvlan` | the container's interface directly on the physical LAN | registered, **not realized** |
| `ipvlan` | same, sharing the parent's MAC | registered, **not realized** |
| `overlay` | VXLAN between nodes, optionally encrypted with WireGuard | yes, in the holder |

`macvlan`/`ipvlan` need `CAP_NET_ADMIN` in the host's netns. Without privilege,
`create` registers the network and **warns** that it did not realize it — it does not pretend it
realized it (that is what the command itself documents; it was not possible to confirm this
live here, since there was no physical NIC available for `--parent`). Two notes that tend to trip up
people coming from Docker:

- they are not filtered by Delonix's firewall; traffic goes straight out to the LAN;
- they require `--parent` and `--subnet`, and the `--parent` NIC has to **actually exist** —
  otherwise the refusal is immediate (`parent NIC 'eth0' does not exist on the host`).

```bash
delonix network create lan --driver macvlan --parent eno1 --subnet 192.168.1.0/24
```

---

## 4. Publishing ports

```bash
delonix net ingress publish web 8080:80      # host 8080 → container 80
delonix net ingress publish web 8443         # same number on both sides
delonix net ingress publish dns 5353:53/udp  # UDP
```

**Rootless does not bind host ports below 1024.** This is not a
Delonix limitation — it's the kernel's. Publish high and put a proxy in front to hold :443,
or grant the capability to the binary if the machine's policy allows it.

Listing and undoing:

```bash
delonix net ingress ls web
delonix net ingress unpublish web 8080
```

---

## 5. Segmentation — the part that matters to whoever has to prove it

### 5.1 What is already isolated without doing anything

Two distinct `bridge` networks **do not talk to each other**. It is not a rule that gets
written; it's the absence of a route between the two segments. It's the initial state and the
state you return to when everything is wiped.

### 5.2 Opening a path — and only in one direction

```bash
delonix network route web db          # web CAN initiate towards db
delonix network route web db --rm     # close it
```

`route` is **directed**: opening `web → db` does not open `db → web`. And, read
strictly, a `route` says the packet *can cross*, not that it is *allowed* — the
per-workload firewall still gets to decide next. These are two layers, on purpose: the
route is topology, the rule is policy.

> Needs the holder up. If nothing is attached yet, run `delonix net netns
> up` first.

### 5.3 Per-workload firewall: ingress

The pattern for an internal service is *default-deny* and then naming what comes in:

```bash
delonix net ingress policy db deny
delonix net ingress allow db tcp/5432 --from 10.200.0.0/16
delonix net ingress ls db
```

The rule is evaluated **after** DNAT, so the port you name is the one the
container listens on (5432), not the one published on the host. Getting this wrong is the
most common source of "I opened the rule and it still doesn't get through".

Accepted forms for `<PORT>`: `tcp/5432`, `udp/53`, `5432` (any protocol),
`tcp/*` (all ports).

`--note` stores a text alongside the rule. In an audit a year from now, it's the
difference between a justified rule and a rule nobody dares to delete.

### 5.4 Per-workload firewall: egress

```bash
delonix net egress policy app deny
delonix net egress allow app udp/53                        # otherwise nothing resolves
delonix net egress allow app tcp/5432 --to 10.200.0.20/32
delonix net egress allow app tcp/443
```

`udp/53` is not decoration: an egress *default-deny* takes DNS out, and the
symptom that reaches the operator is "the application is slow", not "the firewall blocked it".

**A policy change does not tear down already-established flows** — it only decides the
new ones. To verify a change, restart the workload or wait for the connection to
drop.

**Both ingress and egress decide.** A flow from `app` to `db` only passes if
`app`'s egress lets it **and** `db`'s ingress lets it; an *accept* on one side
does not skip the other side's decision. Before the NaaS audit's P0-1 fix, an
`egress policy deny` had no effect for any destination that accepted the
source — those in the same namespace, for example.

**A rule the engine cannot enforce as written is refused, and the entire
spec with it**: the previous rules remain in effect. There is no
partial application — a silently dropped *deny* would amount to an *allow*.

### 5.5 Network-level egress

Beyond per-container, an entire network also has egress policy:

```bash
delonix net egress net app deny                              # no Internet
delonix net egress net app allowlist --to 10.0.0.0/8,1.1.1.1/32
delonix net egress show app
```

And, for what CIDRs cannot express, a list of names:

```bash
delonix net egress host app github.com          # catches github.com and *.github.com
delonix net egress host app registry.npmjs.org
```

Addresses are learned as DNS responses pass through, so a
CDN that renumbers keeps working — which is exactly the case where a
hand-written CIDR list rots silently.

### 5.6 A reference design

Three zones, ingress only where needed, egress closed by default:

```bash
# zones
delonix network create dmz
delonix network create app
delonix network create data

# paths, always one direction
delonix network route dmz app
delonix network route app data

# ingress
delonix net ingress policy api deny
delonix net ingress allow api tcp/8080 --from 10.201.0.0/16   # only from dmz
delonix net ingress policy pg deny
delonix net ingress allow pg tcp/5432 --from 10.202.0.0/16    # only from app

# egress
delonix net egress net data deny                               # the database doesn't go out
delonix net egress policy api deny
delonix net egress allow api udp/53
delonix net egress allow api tcp/5432 --to 10.203.0.0/16
```

Notice that `data` has no route **from** anywhere except `app`, and has no
egress at all. If someone ever needs to export it, they will have to write a
command — and that command stays in the history.

---

## 6. Multi-node: encrypted VXLAN overlay

Each node has its own WireGuard identity. The private key sits at `0600` under
`<root>/wg/node.key`; the public one is what gets distributed.

```bash
delonix network node init     # idempotent; prints the public key
delonix network node key      # just the public one, to compose into a script
```

On each node, the same network with the same VNI:

```bash
# node A (10.0.0.1)
delonix network create mesh --driver overlay --vni 42 \
  --peer 10.0.0.2=<pub_B>=10.42.0.2 --wg-ip 10.42.0.1

# node B (10.0.0.2)
delonix network create mesh --driver overlay --vni 42 \
  --peer 10.0.0.1=<pub_A>=10.42.0.1 --wg-ip 10.42.0.2
```

`--peer` accepts `<ip>` (plaintext VXLAN) or `<ip>=<pubkey>=<wg_ip>` (encrypted), and is
repeatable.

Two things worth knowing before putting this into production:

- **encrypted requires `wg` on the host.** If you declare `--wg-ip` and
  `wireguard-tools`/the module are not there, the command **fails before** bringing up the VXLAN. It
  is deliberate: without that the FDB would point at addresses only reachable through the tunnel
  that would never come up, and you'd have an overlay pretending to be up.
- **MTU.** VXLAN eats 50 bytes, WireGuard another 60. An encrypted overlay over a
  1500 MTU leaves ~1390 usable. If the applications exchange large payloads with
  `DF` set, this shows up as unexplained slowness rather than an error.

---

## 7. Declarative

For network state versioned in git, instead of a sequence of commands:

```bash
delonix network apply -f delonix-manifest.yaml
```

Applies **only** the `kind: Network` documents in the manifest, leaving the other
Kinds alone, and it's idempotent by name. For the whole manifest (all
Kinds, in dependency order) use `delonix stack apply`.

The schema comes straight from the binary itself, not from a `.md` that ages:

```bash
delonix explain Network       # field reference, kubectl-explain style
delonix manifest schema       # JSON Schema generated from the code
```

The manifest needs `apiVersion: delonix.io/v1` — without it the refusal is
`missing field \`apiVersion\``, which is clear but catches whoever pasted a
truncated example:

```yaml
apiVersion: delonix.io/v1
kind: Network
metadata:
  name: data
spec:
  subnet: 10.221.0.0/24
```

---

## 8. Observing

```bash
delonix net netns status          # the FIRST thing to read when there's no network
delonix net netns status --json
```

```
ingress DOWN — pin — · control in-pin · slirp — · bridge delonix0 (10.200.0.1) · refcount 0
```

Reads as: infra down, no slirp, `refcount 0` (no workload holding it). With
`refcount > 0` and `ingress UP`, the bridge really does exist.

```bash
delonix net flow             # RX/TX per container, right now
delonix net flow -w          # redrawn every 2s
delonix dashboard --scope network         # network TUI
delonix dashboard --scope network --once  # a single snapshot, for piping
delonix dashboard --scope network --json
```

`net flow` uses eBPF when available and **degrades to veth
counters** when it's not — telling you which of the two it's using, rather than giving
numbers with no provenance.

---

## 9. The only command that needs root

```bash
delonix network vlan eth0 100              # DRY-RUN, changes nothing
sudo delonix network vlan eth0 100 --apply
sudo delonix network vlan eth0 100 --rm --apply
```

An 802.1Q interface on a host NIC needs `CAP_NET_ADMIN` in the host's
netns, which no unprivileged user has. That's why this command is
dry-run by default: it prints the plan and only touches the machine with `--apply`.

---

## 10. Diagnosis

### "I created the network and the container has no connectivity"

Reading order:

1. `delonix net netns status` — is the infra up? Does the `refcount` make sense?
2. `delonix network inspect <network>` — are the bridge and the `/16` the ones you expect?
3. `delonix net ingress ls <container>` / `net egress ls <container>` — is there a
   *deny* policy without the matching rule?
4. `delonix net flow` — is anything passing?

### "Another `DELONIX_ROOT` already holds the infra"

```
error system call `control socket` failed: another delonix state root on this
user already owns the network infra: `/tmp/delonix-net-1000/control.sock` has a
live listener, but there is no pidfile under `<...>/ingress`. The sockets are
per-USER while the pidfiles are per-ROOT, so rebuilding from here would delete
that infra and unplug every workload on it. Either use that root (unset/point
`DELONIX_ROOT` at it), or stop it deliberately with `delonix net netns down`
from the root that owns it.
```

Control sockets are per **user**; pidfiles are per **state root**. Running two
different `DELONIX_ROOT`s under the same uid is the recipe for
this. The refusal is intentional: rebuilding from there would disconnect the workloads from
the other infra.

### "I upgraded the engine and an `egress deny` still lets traffic through to another container"

The firewall dispatch (the `fwout` and `fwcont` *base chains* in the `dlxing`
table) lives in the holder's netns. A **control plane started by an old binary**
has the old shape, where the destination's *accept* skipped the origin's *egress*.
The new binary **migrates it by itself** on the first network operation after the
upgrade (a `run`, a `start`, a `net ingress`/`egress`, a `net netns up`):
it enters the pin's namespaces and swaps the dispatch in an atomic transaction, without
touching the pin, the per-container chains, or `@fwmap`, and without restarting anything.
If the migration fails, the command **fails** with the reason, instead of applying rules
that would not be enforced.

Only if that migration fails is the manual step needed, which restarts **only
the control**:

```bash
delonix net netns status                  # note the "control" pid
kill <control-pid>
delonix net netns up                      # the new control reattaches and migrates
```

Confirm with `nsenter -t <pin-pid> -U -m -n -- nft list chain ip dlxing fwout`:
it must exist and have a single rule, `ip saddr vmap @fwmap`. The refusal of
invalid rules is also done by the CLI, so it already protects a node with an old holder.

### "The rule is there and it's not getting through"

- The port in the *ingress* rule is the **container's**, not the host's.
- A policy change does not cut already-established connections.
- `macvlan`/`ipvlan` **do not pass through** Delonix's firewall.
- Outside the `default` namespace, deleting the last rule does **not** remove
  namespace isolation: the chain stays, with only the *accept* for the same namespace and the
  *drop* for others.
- An explicit *ingress* rule (even a `deny` on a port) **also does not** remove
  namespace isolation: new connections coming from another namespace are still cut off.
  Only an explicit `allow` — which is what a `kind: Dependency` writes — lets through
  the pair it names.

---

## 11. Reference

### `delonix network` — user networks

| command | what it does |
|---|---|
| `create <name>` | creates; `--driver`, `--subnet`, `--parent`, `--gateway`, `--vni`, `--peer`, `--wg-ip` |
| `ls` | lists; `-o table\|json` |
| `inspect <name>` | detail; `-o table\|json` |
| `describe <names>...` | readable block, several networks |
| `rm <name>` | removes record + bridge |
| `route <from> <to>` | **directed** path; `--rm` closes it |
| `vlan <nic> <id>` | 802.1Q; **root**, dry-run without `--apply`; `--rm` |
| `apply -f <file>` | applies the `kind: Network` documents from a manifest |
| `dash` | TUI; `--once`, `--json` |
| `node init\|key` | this node's WireGuard identity |

### `delonix net` — plumbing

| group | commands |
|---|---|
| `netns` | `up`, `down`, `status`, `attach`, `detach`, `exec`, `publish`, `unpublish`, `firewall` |
| `ingress` | `ls`, `allow`, `deny`, `policy`, `publish`, `unpublish`, `clear`, `rm` |
| `egress` | `ls`, `show`, `allow`, `deny`, `host`, `net`, `policy`, `clear`, `rm` |
| `flow` | per-container traffic (`--iface`, `-w`) |
| `httproute` | `apply`, `ls`, `rm` — embedded L7 proxy |
| `tunnel` | `expose`, `ls`, `describe`, `apply`, `rm` — public exposure |
| `boot` | `enable`, `disable`, `status` — post-reboot systemd units |

Global: `--l18n en|pt` (or `$DELONIX_L18N`) changes the output language, before
any subcommand.

---

## 12. Known limits

Written here so they are not discovered in production:

- **55 `bridge` networks** per host (`10.200`–`10.254`), with an explicit refusal
  once past the ceiling.
- **`macvlan`/`ipvlan` are not realized without privilege** — they stay registered,
  with a warning.
- **Host ports < 1024 are not published** under rootless.
- **`route` and network-level egress policies need the holder up.**
- **`network rm` does not check for attached workloads** — it deletes the bridge regardless.
- **No IPv6** on bridge networks.
- **Private space only** for `--subnet` (RFC 1918, `/8`–`/28`).
- **One `DELONIX_ROOT` per user** at a time, for the network infra.
- **`ls` shows what's declared.** To know what's realized, use `net netns
  status`.

---

## 12.1. The PODS' network is not this network

This guide covers the network of **containers and VMs on a host**. A
Kubernetes cluster created by this engine has, on top of that, its own network — and
confusing the two is the source of a class of problems that gets diagnosed in the
wrong place. There are **four distinct planes**, with distinct owners:

| plane | who owns it | where it's configured | where it's diagnosed |
|---|---|---|---|
| **pods network** (pod CIDR, Service CIDR, inter-node routing) | the CNI profile chosen in `spec.cni` | `spec.podSubnet`/`serviceSubnet`/`spec.network` of `kind: KubernetesCluster` | `delonix cluster network status`, `docs/network-compatibility.md` |
| **network of the VMs that are the nodes** (`--network` / `spec.vm.network`, DHCP, the VM's firewall) | this engine, via `kind: Network` | this guide | `delonix net netns status`, `delonix network ls` |
| **apiserver VIP** | the HAProxy that `cluster kubeadm` provisions with `--control-plane > 1` | `spec.controlPlaneEndpoint` | `cmd/lb.rs`, the `<name>-lb` VM |
| **`LoadBalancer` Services** | **nobody — not delivered** | — | ADR-0066, accepted and not built |

Three practical consequences:

1. **The pod CIDR must not collide with the VMs' network.** If it collides, the cluster
   comes up and a host (or an entire network) becomes unreachable from inside the pods —
   and that is not an error at creation time, it's a symptom afterwards. That's why it is
   **refused** before creating any machine (DX-1704), comparing the cluster's CIDRs
   against the networks this host already declares.
2. **MTU gets paid for twice if nobody accounts for it.** The VMs' network may already be
   on an overlay (VXLAN = 50 bytes, +60 if encrypted with WireGuard — see §6), and
   the cluster profile adds its own encapsulation on top. Hence
   `spec.network.nodeMtu`: declares the nodes' **real** MTU, and the pods' MTU is
   calculated from it.
3. **This engine's firewall does not filter the pods' network.** The `nft` rules in the
   `delonix`/`dlxing` tables govern containers and VMs on this host. *Pod-to-pod*
   isolation is `NetworkPolicy`, and it's the CNI profile that enforces it
   — and **Flannel enforces none** (see `docs/network-compatibility.md`).

And one that is not practical but is important: **nothing here enables BGP, L2
announcements, or an external address controller**. A second owner of the same address
pool is a conflict that manifests as traffic that sometimes gets through, and
this engine does not create one without explicit configuration of the network the cluster is born into.

---

## 13. Where to go next

- `delonix explain Network` — the fields of `kind: Network`, straight from the code.
- `docs/commands/` — generated reference, one file per command.
- `docs/adr/` — the decisions and the why (networking is mostly ADR-0013; a cluster's
  pod network is ADR-0078).
- `docs/network-compatibility.md` — cluster network profiles: pinned
  versions, what was measured, and what wasn't.
- `docs/cli-stability.md` — what can break on an upgrade and what can't.
