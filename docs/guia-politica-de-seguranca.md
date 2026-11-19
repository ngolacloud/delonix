# Guide — the node's security policy (`policy.json`)

What this node **refuses to run**, no matter who asks: the CLI, a `crictl` talking to the
socket, the Docker Engine API, or someone writing `delonix vm create` by hand.

The file lives at `$DELONIX_ROOT/policy.json` (by default `~/.delonix/policy.json`).
None exists by default, and that is deliberate — **no file means no ceiling**, and
the engine behaves exactly as it always has.

## The three rules that govern this file

1. **No file, no ceiling.** A node that has never written a policy refuses nothing.
2. **A file that cannot be READ is an error, not an absent policy.** A truncated
   file, or one with a misspelled field (`denyPriviledged`), stops the command
   instead of passing through as "no opinion". Someone's intent is unknown, and
   running anyway is exactly the silent degradation this engine refuses everywhere.
3. **A field nobody ever sets never starts refusing things.** This is what lets
   the engine be updated without an entire fleet stopping overnight.

## The fields

### Container path

| Field | What it refuses |
|---|---|
| `denyPrivileged` | `container run --privileged` |
| `denyHostNetwork` | `--net host` — which is this engine's **default** mode, so setting this forces every workload to name its network |
| `denyLatestTag` | an image with no tag, or with `:latest`. No tag counts too: `alpine` and `alpine:latest` are the same thing, and a rule that only refused the explicit form could be worked around by dropping four characters. An `@sha256:…` passes — it's the strongest form there is |
| `allowedRegistries` | any registry outside the list. Takes **hosts** (`ghcr.io`), not references. The comparison is the whole string: `ghcr.io` does not accept `evil-ghcr.io` or `ghcr.io.evil.com` |

### VM path

These fields are **new** and all off by default. Until they existed, the VM
path had no node policy at all.

| Field | What it refuses |
|---|---|
| `denyDevicePassthrough` | `vm create --device` (VFIO PCI passthrough). **It's the VM equivalent of `--privileged`, and then some**: a passed-through device gives the guest DMA to the host's hardware, which is a wider hole than any capability a privileged container gets |
| `denyLatestVmImage` | a VM disk image with no tag or with `:latest`. Deliberately separate from `denyLatestTag` — a `vm build` tags what it produces as `<name>:latest` by default, and merging the two would have refused a normal build for anyone who updated |
| `allowedImageUrlHosts` | a `vm create --url-img` from a host outside the list. Without a `.sha256` published alongside it, that download is trusted **on TLS alone** — and TLS proves you reached the host you named, not that the host deserved to be named |

### Network path — egress to private address space

The one field that does **not** decide at admission: it is enforced by the
network plane (the holder), packet by packet, for every workload connected to
one of the engine's networks.

| Field | What it does |
|---|---|
| `denyPrivateEgress` | refuses NEW connections from a workload on an engine network toward private space reachable only through the node's uplink: `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` (RFC 1918), and `100.64.0.0/10` (CGNAT). That's where the node's LAN, the host's management APIs, and its neighbors live — without this, a workload that reaches the uplink reaches everything the host reaches. Link-local and loopback were already always denied (`fwguard`) |
| `allowedEgressPrefixes` | the private destinations that stay reachable with the guard on (a resolver, a gateway, a registry the node depends on). Canonical `a.b.c.d/len` prefixes. It's a **grant**: empty = none, and only the node's policy grants it — the workload cannot reopen it on its own |

- **A workload's own network, and any network routed to it (`kind: NetworkRoute`),
  are never touched.** It's not an exception list: that traffic leaves the holder
  through a bridge, never through the uplink, and the guard only watches the
  uplink. Isolation between networks remains `@netpair`'s.
- **Replies pass through.** A LAN client that reached a published port gets the
  reply; only the NEW connection is refused.
- **Neither a network `allowlist:` nor a workload `egress allow` reopens this**:
  the chain (`fwprivate`, priority -15) runs before `fwdeny` (-10), and a `drop`
  between base chains is final.
- `mode: "warn"` counts and lets through (the rule's counter shows how much would
  have been refused).
- **Only covers workloads on an engine network.** A `--net host`, or a container
  with its own slirp (no `--net`, only `-p`), does not go through the holder's
  uplink — which is why lint `POLICY-EGRESS-HOST-NET-OPEN` asks for
  `denyHostNetwork: true` alongside it.
- Takes effect on a `RuntimePolicy`'s `apply`, on `policy unset`, and on the next
  `container run` after a hand edit of `policy.json`. A guard record the holder
  cannot read **fails closed**: every range refused, no grant.

### Mode

| Field | Effect |
|---|---|
| `mode: "enforce"` | refuses. It's the default, and the behavior that always existed |
| `mode: "warn"` | lets through and says what would have been refused. For rolling out a rule across a fleet before it bites |

## Full example

```json
{
  "denyPrivileged": true,
  "denyHostNetwork": true,
  "denyLatestTag": true,
  "allowedRegistries": ["ghcr.io"],

  "denyDevicePassthrough": true,
  "denyLatestVmImage": true,
  "allowedImageUrlHosts": ["cloud.debian.org"],

  "denyPrivateEgress": true,
  "allowedEgressPrefixes": ["192.168.50.10/32"]
}
```

## The warnings the node gives you about yourself

When you locked half the house and left the other half open, the node tells
you — by name, with a stable identifier, and **on the path where you can act**
(a warning about VMs on every `container run` is noise you learn to ignore):

```
warning: runtime policy [POLICY-VM-PASSTHROUGH-OPEN] this node refuses
`--privileged` containers but allows `vm create --device` (VFIO PCI passthrough)…
```

| Identifier | What it's telling you |
|---|---|
| `POLICY-VM-PASSTHROUGH-OPEN` | you refuse privileged containers and allow VFIO passthrough |
| `POLICY-VM-LATEST-OPEN` | you refuse `:latest` in containers and not in VM images |
| `POLICY-VM-URL-OPEN` | you restrict registries and `--url-img` still goes to any host |
| `POLICY-SILENT` | the file exists and refuses nothing — almost always a mistake |
| `POLICY-REGISTRY-NOT-A-HOST` | you put a reference (`ghcr.io/org/app`) where a host is expected |
| `POLICY-EGRESS-HOST-NET-OPEN` | you refuse private egress but allow workloads outside any engine network, which don't pass through the guard |
| `POLICY-EGRESS-GRANT-UNUSED` | you granted destinations in `allowedEgressPrefixes` without `denyPrivateEgress` — the grant changes nothing |

They appear **once per command** (this engine has no daemon: every command is a
process that is born, works, and dies). To silence them, once you've read them
and decided: `DELONIX_POLICY_LINT=0`.

## The trail

Every refusal leaves a line in `$DELONIX_ROOT/events.jsonl`, with `kind: "security"` and
a stable rule identifier:

```
delonix system events | grep security
{"ts":…,"kind":"security","action":"ADM-DEVICE-PASSTHROUGH","name":"db-01","detail":"0000:01:00.0"}
```

The identifiers (`ADM-PRIVILEGED`, `ADM-HOST-NETWORK`, `ADM-LATEST-TAG`,
`ADM-REGISTRY`, `ADM-DEVICE-PASSTHROUGH`, `ADM-LATEST-VM-IMAGE`, `ADM-IMAGE-URL-HOST`)
are stable: the message's wording may change, the identifier does not. Alert on
them, not on the text.

**This log is operational signal, not proof.** It is *best-effort* by design — a
logging error never fails the operation that triggered it —, it detects nothing
about its own failures, and anyone with write access to the state root can edit
it. The tamper-proof trail is the Ed25519-anchored hash chain, and that lives in
`delonix-paas`. Calling this proof when it is a warning would be false
advertising.

## What this policy does NOT do

Written so no one confuses the name with the scope:

- **It watches nothing at runtime.** It decides at admission, and nothing else —
  with ONE exception, stated above: `denyPrivateEgress` is a dataplane rule, and
  what it does is refuse packets, not observe them. There are no
  eBPF sensors, no file-integrity monitoring, no malware detection, no
  ransomware behavioral scoring, and no network anomaly detection. All of those
  are resident processes, and this engine is *daemonless* by design — opening
  that door is matter for its own ADR, with a spike measuring what such a
  sensor can observe **in rootless**, where this engine normally runs.
- **It does not act.** It does not freeze, isolate, quarantine, roll back, or restore.
- **It knows nothing about tenants.** There is no `tenant`, `project`, or
  `environment` anywhere, and there is a test that fails the build if anyone
  adds one. That lives in `delonix-paas`, by recorded decision (ADR-0010, ADR-0025).
- **It does not replace cluster admission.** It complements it. This is the
  **local** answer, and it holds even with a misconfigured Pod Security policy
  and with a `crictl` talking directly to the socket — because a policy that
  only lives in one cluster's admission chain runs in another process, on
  another machine, that this node cannot see or verify.

See [ADR-0026](adr/0026-security-runtime-decision-crate.md) for why each of
these boundaries exists.
