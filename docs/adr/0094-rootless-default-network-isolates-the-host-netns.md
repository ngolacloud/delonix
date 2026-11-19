# ADR-0094 — In rootless, the default network isolates the host's netns (align with Docker/Podman)

**Status:** Accepted (2026-11-18) — owner's decision in the platform hardening
campaign (in-depth evaluation, `reports/e2e/0.1.1/PLATFORM_EVAL_2026-11-18.md`,
finding **R2**). **Gap measured live** in this session; **design validated**;
**implementation is its own e2e-gated slice** (touches the network contract at 32+
call sites — see «Implementation»). **Relates to:** ADR-0082 (IPv6 dataplane),
ADR-0084 (overlay/wireguard/hostfwd), ADR-0078 (pod network profiles).

> **The decision, in one line:** the **rootless** `--net` default moves from `host`
> (shares the host's netns — no isolation) to an **isolated NAT network** (own
> netns + `slirp4netns` with egress SNAT), as `docker`/`podman` rootless do.
> `--net host` remains available, **explicit**.

## Context

### The gap, measured live (rootless host, 2026-11-18)

Today `--net` has `default_value = "host"`
(`bins/delonix/src/cmd/container.rs:1166`). In rootless, `host` makes the container
**share the engine process's network namespace**, which is the host's:
`Launch::new_netns()` (`delonix-compute/src/domain/launch.rs:61`) only creates its
own netns with `--net none` or with `-p`, and `clone_flags`
(`delonix-linux/src/lib.rs`) only adds `CLONE_NEWNET` when `new_netns` — the test
`the_clone_flags_isolate_by_default_and_waive_what_is_asked` fixes precisely
`!CLONE_NEWNET` for the default.

Live proof (`0.1.1` binary from this worktree, rootless user):

```
$ python3 -m http.server 45999 --bind 127.0.0.1 &      # a service ONLY on the host's loopback
$ delonix run --rm alpine:3.24 -- wget -qO- http://127.0.0.1:45999/
<!DOCTYPE HTML> …                                         # ← the container REACHED it
$ delonix run --rm alpine:3.24 -- ip -o addr
… wlp0s20f3  inet 192.168.1.108/24 …                      # the host's WiFi
… br-lan  inet 192.168.10.1/24 …  br-wan … virbr0 …       # ALL the host's bridges/VMs
```

A tenant container sees and reaches **the whole** of the host's network: services on
`127.0.0.1` (local databases, the *CRI streaming server* itself), every interface,
other networks' and VMs' bridges. On a multi-tenant platform (NgolaCloud) this is an
isolation regression compared to Docker/Podman rootless, which give each container
its own netns with egress via `slirp4netns`/`pasta`.

### Why the current model supports the fix

The rootless datapath already has the whole mechanism: a **netns holder** with a
**single `slirp4netns`** (`delonix-sdn/src/infra.rs` `start_slirp`), which runs with
`--configure` (egress SNAT) and an `--api-socket` for Phase 4's `add_hostfwd`.
**Egress does not depend on `-p`:** the `hostfwd` are only for *ingress*; the
outbound SNAT exists the moment the container enters the holder's netns. So a
container **without** `-p` can have isolation **and** egress by joining the holder —
exactly the Podman model.

## Decision

1. **Rootless default of `--net` = isolated NAT network** (own netns +
   holder/slirp with egress), not `host`.
2. **`--net host` becomes explicit** — whoever wants to share the host's netns asks
   for it. `delonix run --net host` is the escape hatch (same as `docker run
   --network host`).
3. **In root mode (not rootless)** the default does not change in this ADR (root
   already has its own bridge/CNI paths; the measured gap is rootless).
4. **Admission:** with `workloadSecurity: baseline`, `--net host` becomes refused
   (ADR in the same batch, finding S1) — consistent: host-net is opt-in **and**
   subject to PSS policy.

## Consequences

- **Isolation by default**: a tenant container no longer sees/reaches the host's
  network; `127.0.0.1` becomes the container's own.
- **Egress preserved**: outbound internet access via slirp SNAT (as today with
  `-p`).
- **Cost**: every rootless container now carries a netns + (a share of) slirp —
  a cost already paid by containers with `-p`; for those without `-p` it is new.
  Measure the startup overhead vs. the current default.
- **Migration / breakage**: scripts that today rely on the container seeing
  `127.0.0.1` or the host's interfaces **break** — resolved with explicit
  `--net host`. Document in `MIGRATION.md` and in the release notes; emit a single
  warning the first time an implicit `--net host` would have been the old
  behaviour (a transition release), before the default actually changes.

## Implementation (its own e2e-gated slice — NOT in this commit)

Changing the network default touches the **contract everywhere** — 32+ measured call
sites assuming `net == "host"` / `net.is_empty()` as «shares the host»
(`container.rs:3428`, `compute/app/run.rs:178`, `gitops.rs:1396`, pods, compose,
stack, CRI). So the swap is a governed slice, gated by the full network e2e suite,
not a surgical edit.

Steps:

1. Introduce the isolated mode as a `--net` value (e.g. `nat`/`private`): own netns
   + holder/slirp egress, `-p` adds `hostfwd` on top (reuses the `-p` path, which
   already does this).
2. `Launch::new_netns()` becomes `true` for the isolated mode; empty `slirp_ports`
   ⇒ egress-only slirp (no `hostfwd`).
3. Swap `default_value` from `host` to the isolated mode; audit the 32+ call sites
   and the pod/compose/stack/gitops/CRI defaults for the new default.
4. **E2e gate (the «nothing fails» criterion)**, in rootless: (a) isolation — the
   host loopback test above starts to **fail** the connection; (b) egress — the
   container reaches the internet; (c) `-p` keeps publishing ingress; (d) internal
   DNS and container-to-container; (e) pods (shared pod netns) intact; (f) CRI
   conformance intact. Startup benchmark vs. baseline.
5. Update the `launch`/`clone_flags` unit tests (incl.
   `the_clone_flags_isolate_by_default_and_waive_what_is_asked`) for the new
   default, and `MIGRATION.md`.

Until this slice lands, the gap stays **documented and measured**; `--net host` is
the current behaviour and the isolated mode is the default's target.
