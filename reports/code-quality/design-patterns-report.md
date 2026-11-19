# Patterns catalogue — Delonix Runtime (`cebf895`)

For `docs/architecture/design-patterns.md` (§71), if it is to be promoted.

## Ports & Adapters — `VmBackend` — **GOOD**

**Port:** `crates/delonix-vm/src/lib.rs:601`, 16 methods.
**Adapters:** `CloudHypervisorBackend` and `LibvirtBackend` (`delonix-vm`),
`ProxmoxBackend` (`delonix-proxmox`).
**Fakes:** `FakeBackend`, `FailingRemote`, `Counting`, `OnlyStop`, `Resumable`.

**Reason:** normalises the lifecycle of a VM over three very different
hypervisors.
**Direction:** adapter → port (`delonix-proxmox` depends on `delonix-vm`). ✔
**Proof the port is real:** five different fakes in the tests. A port nobody can
fake is not a port; this one can be faked.
**Recommendation:** keep it. Do not fatten the trait — 16 methods is already at
the §32 limit.

## Strategy — `ComputeDriver` — **GOOD**

`crates/delonix-runtime-bin/src/cmd/workload.rs:244`, 5 methods,
implemented by `ContainerDriver` and `VmDriver` (+ `FakeDriver`).
Small, cohesive contract — the healthy counter-example to the god trait of §32.

## Adapter — k8s schema translation — **GOOD**

`httproute.rs::ingress_to_httproute` converts `networking.k8s.io/v1 Ingress`
into the internal `HttpRouteSpec`. A well-placed boundary: the external shape does
not contaminate the internal model. Caveat in ARCH-0003 (fields accepted and
ignored).

## Newtype (§42) — **MISSING**

There is no `struct ContainerId(String)`, `struct NetworkId(String)`,
`struct TenantId(_)`. Identities travel as raw `String` across 138k lines.
**I do not recommend** a newtype campaign now: the concrete gain appears when two
ids of the same primitive type can be swapped in the same call, and the place
where that hurts is already identified — it is the 37-parameter signature of
ARCH-0002. Do newtypes there first; generalise only if it pays off.

## State machine (§40) — **REVIEW**

`Container` (`delonix-runtime-core/src/lib.rs:528`) carries 8 `bool` among 71
fields instead of a state `enum`. Contradictory combinations are
representable. Mitigating factor: it is the **persisted** record — changing this
is a state migration (§80), not cleanup. Requires an ADR.

## God Object (§31) — `Container` — **REVIEW**

71 fields in one type. Documented field by field, which makes it legible despite
its size. See above: do not touch it without an ADR.

## Reconciliation / Desired State — out of scope for this measurement

It exists (`cmd/reconcile.rs`, `stack apply`), but it was not audited in this pass.
