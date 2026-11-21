# `delonix` network / volume / pod — parity defects and fix prompts

**Source:** live functional test, parameter by parameter, of the network, volume
and pod domains — `network create/connect/disconnect/inspect/ls/prune`, `volume
create/ls/inspect/rm`, `pod create/ps/inspect/logs/stop/start` — against
`docker`/`podman`.
**Environment:** delonix 0.1.2 (commit `8a795077`), rootless host · daemonless,
`alpine:3.19`. Docker 29.10.0 / Podman 4.9.3. Date: 2026-11-19.
**Method:** observed effect — IP assigned within the subnet, name resolution
(`nslookup`/`getent`), 2nd IP after hot `connect`, file persistence between two
containers on the same volume, shared pod netns (identical IP among members).

**Verdict:** the three domains are in **strong functional parity** — network (subnet,
gateway, DNS, hot multi-network), volume (persistence, safe in-use refusal) and pod
(shared netns/IPC/UTS, full Podman verbs) work. 4 defects, of flag parity; the
**declarative** pod model is a deliberate divergence (ADR-0006).

Attack order: security → functionality → parity → DX.

---

## Per-parameter matrix (observed)

| Domain / parameter | Observed effect | Verdict |
|---|---|---|
| `network create --subnet --gateway` | `inspect` confirms subnet/gateway; container → IP `10.251.0.153` within the subnet | ✅ parity |
| network: name resolution | `nslookup nsrv` → `10.251.0.2` | ✅ parity |
| `network connect` (hot, 2nd network) | `nmulti` gains `10.252.0.139` on `eth1`; keeps the 1st | ✅ parity |
| **`network connect --ip` / `--alias`** | `unexpected argument '--ip'` | ❌ **N1** |
| **`network create --internal`/`--ipv6`/`--label`/`-o`/`--ip-range`** | absent (has overlay extras: wg/vni/peer) | ❌ **N2** |
| `network prune` | requires `--force` without a terminal (safe) | ✅ (safer) |
| `network ls`/`inspect`/`rm` | listing/detail/removal correct | ✅ parity |
| `volume create` + `-v vol:/path` | persistence: `file.txt` written in one container, read in another | ✅ parity |
| `volume inspect`/`ls` | show real mountpoint + disk usage | ✅ (extra) |
| `volume rm` in use | refuses with clear DX-1000 + suggests `--force` | ✅ (safer) |
| **`volume create --label`** | `unexpected argument` (`--opt` only accepts NFS keys) | ❌ **V1** |
| `pod create -f` (`delonix.io/v1`) | 2 containers share netns+IPC+UTS, IP `10.200.0.2` | ✅ parity |
| pod: members share netns | `ppod-c1` and `ppod-c2` → both `10.200.0.2` | ✅ parity |
| `pod ps`/`inspect`/`logs`/`stop`/`start` | all Podman verbs present and correct | ✅ parity |
| **`pod create -f` with `apiVersion: v1`** (k8s) | rejected — requires `delonix.io/v1`/`compute.delonix.io/v1beta1` | ⚠️ **P1** |
| **`pod create --name`** imperative | `unexpected argument` (declarative model) | ⚠️ deliberate (ADR-0006) |

---

## Cross-cutting notes
- **Rootless host, no sudo**; `/tmp` ephemeral. Tests: `cargo test -p delonix-bin --bins`.
- New flag → **pt.po** (3 texts) + flag order (`delonix-new-code-and-flag-gates`).
- The `run --pod` BUG (does not join the netns) is **another** session (owner, `fix/run-pod-join-netns`) — here the **declarative** pod (`pod create -f`) was tested, which joins the netns correctly.

---

## N1 — `network connect` without `--ip`/`--alias`  _(Medium · parity)_

**Symptom.** `network connect --ip 10.252.0.50 pnet2 nmulti` → `unexpected argument
'--ip'`. Hot connect works, but without a fixed IP or network alias — Docker allows
`connect --ip <addr>` (stable address) and `--alias <name>` (extra name resolvable only
on that network).

**Root cause.** The `NetworkCmd::Connect { network, container }` variant
(`bins/delonix/src/cmd/network.rs:507`, handler `:648`) only accepts network + container.

**Fix.** Add `--ip <addr>` (reserves the address in the network's IPAM before
attaching) and `--alias <name>` (registers the alias in the DNS resolver, network-scoped).

**Done when.**
1. `connect --ip 10.252.0.50 pnet2 nmulti` → `nmulti` gets `10.252.0.50` on `eth1`.
2. `connect --alias db pnet2 X` → another container on pnet2 resolves `db`.
3. Test of the fixed IP + alias.

**Gates.** New flags → **pt.po** + order. Must not collide with the IPAM. Rootless gates.
**Agent:** general-purpose, high tier (IPAM/DNS/SDN).

---

## N2 — `network create` without `--internal`/`--ipv6`/`--label`/`-o`/`--ip-range`  _(Low-medium · parity)_

**Symptom.** The `Create` variant (`bins/delonix/src/cmd/network.rs:560`, handler `:700`)
has `--driver/--parent/--subnet/--gateway/--vni/--peer/--wg-ip` (includes WireGuard
overlay extras), but lacks what Docker brings: `--internal` (no route out),
`--ipv6`, `--label`, `-o/--opt` (driver options), `--ip-range` (IPAM sub-range).

**Fix.** Add, by priority: `--internal` (security — isolate the network),
`--ipv6` (pair of IPv4; there is already IPv6 support on other fronts), `--label`,
`--ip-range`, `-o/--opt`.

**Done when.**
1. `network create --internal X` → container on X cannot reach the Internet; reaches peers.
2. `network create --label a=b X` → `inspect X` shows the label.
3. Test of `--internal` (observed isolation).

**Gates.** New flags → **pt.po** + order. `--internal` must not break the isolation.
Rootless gates. **Agent:** general-purpose, high tier (dataplane/nft).

---

## V1 — `volume create` without `--label`  _(Low · parity)_

**Symptom.** `volume create --label k=v X` → `unexpected argument`. There is `--opt`,
but it only accepts NFS keys (`server`, `share`, `username`, …), not free labels nor
Docker's free `-o`.

**Root cause.** `Create` variant in `bins/delonix/src/cmd/volume.rs:805` (handler
`:837`).

**Fix.** Add `--label <k=v>` (repeatable → volume metadata, visible in
`inspect`). Consider accepting `-o`/`--opt` with free keys for third-party drivers.

**Done when.** `volume create --label a=b X` → `volume inspect X` shows `a=b`; test covered.
**Gates.** New flag → **pt.po** + order. **Agent:** general-purpose, low tier.

---

## P1 — `pod create -f` rejects `apiVersion: v1` (native k8s)  _(DX · ecosystem parity)_

**Symptom.** A Kubernetes Pod manifest (`apiVersion: v1`, `kind: Pod`) is
rejected: `unknown apiVersion 'v1' (expected 'compute.delonix.io/v1beta1' or
'delonix.io/v1')`. A user who pastes an existing k8s Pod has to rewrite it.

**Root cause.** The set of accepted apiVersions is fixed
(`crates/contexts/delonix-stack/src/app/reconcile.rs:1096`,
`Some("compute.delonix.io/v1beta1")`; + the `kind: Pod` schema).

**Decision / fix (requires the owner).** Consider accepting k8s `apiVersion: v1 / kind: Pod`
as an equivalent input (map the supported fields, warn about the ignored ones),
to paste k8s manifests without friction — **without** abandoning the native
apiVersion. If the owner prefers to keep the boundary, document it in the release menu.

**Done when.** `pod create -f <pod-k8s.yaml>` (apiVersion v1) creates the pod, with a warning
about ignored fields; the native apiVersion still works.
**Gates.** Manifest parser change → tests of both apiVersions. Rootless gates.
**Agent:** general-purpose, medium tier. **Note:** imperative `pod create --name`
**stays** out (deliberate ADR-0006 divergence — pods are declarative).

---

## What is NOT a defect (parity confirmed live)
- **Network**: `create --subnet/--gateway` (IP in subnet + DNS verified), hot `connect`
  to a 2nd network (2 IPs), `ls`/`inspect`/`rm`; `prune` **requires `--force`** without
  a terminal (safer than Docker).
- **Volume**: real persistence between containers; `inspect`/`ls` show mountpoint and
  disk usage; `rm` in use **refuses** with a clear error (safer).
- **Pod**: **declarative** model (`kind: Pod`, ADR-0006) joins netns+IPC+UTS
  (members with identical IP); all Podman verbs (`ps`/`inspect`/`logs`/`stop`/
  `start`/`kill`/`pause`/`prune`/`exists`) present.
