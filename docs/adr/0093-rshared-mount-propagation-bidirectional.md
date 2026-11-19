# ADR-0093 — `rshared` (bidirectional mount propagation) requires a peer group with the host, behind privilege

**Status:** Accepted (2026-11-08) — identified and measured during the CRI
conformance campaign (`critest v1.36.0`), and **implemented and validated on a
rootful+CNI golden VM**: the spec **mount with 'rshared' should support propagation from host
to container and vice versa** passes, closing the suite at **103/103**. **Relates to:**
ADR-0083 (rootful supported), `reports/cri-conformance-0.1.1/`.

> **What was needed (measured on a VM, not assumed — see "Investigation"):** a
> per-mount `MS_SHARED` alone is NOT enough, and making the root `MS_REC|MS_SHARED` makes the spec
> pass but **leaks** the container's mounts to the host (breaking the CRI server's
> cgroup). The volume only lands as a **peer** of the host's group (`shared:N`, which
> propagates a mount made in the container up to the host) if the host-side source
> **keeps** the peer group it was cloned with AND the bind's parent (the rootfs) is `shared`
> at the moment of the bind. Any RECURSIVE propagation change on `/` destroys the
> first condition (pulls the source out of the group, and a mount never rejoins a group it
> has left). Hence the final form, all behind the privilege gate:
>
> 1. **`/` is NOT touched recursively** — the source keeps the `shared:N` from the clone;
> 2. the mount that ENCLOSES the rootfs (e.g. `/run` in CRI) is set `MS_PRIVATE`,
>    confining the sharing to the rootfs's sub-path (nothing the container mounts
>    underneath escapes to the host);
> 3. the **rootfs** is made `MS_SHARED` before the volumes → the shared source's bind
>    lands as a peer of the host's group;
> 4. even before `pivot_root` (which refuses a shared current root), `/` becomes
>    `MS_PRIVATE` **non-recursively** (the volumes, its own mounts, keep `shared:N`);
> 5. the old root (`/.delonix_old`) is made `MS_REC|MS_PRIVATE` **before** the
>    `MNT_DETACH` — otherwise the detach, via the sharing, would unmount the host's
>    `/sys/fs/cgroup` where the engine runs (measured: `cgroup: could not create
>    delonix.slice`).
>
> An `rshared` from a non-privileged container or a rootless engine is **refused with a
> reason** (`Error::BidirectionalNeedsPrivileged`, DX-1117), never silently degraded
> to slave.

## Context

`critest` checks three mount propagation modes:

- **`rprivate`** — no propagation (neither side sees the other's mounts). Passes.
- **`rslave`** — unidirectional, **host→container** (a mount created on the host appears
  in the container, but not the reverse). Passes.
- **`rshared`** — **bidirectional** (host→container **and** container→host). Fails: the
  test creates a mount INSIDE the container and checks it appears on the HOST
  (`len(fileInfo) should not be zero`, `container_linux.go`) — and it does not.

The cause is in `container_init` (`delonix-linux/src/lib.rs` ~2504-2519): the container's
mount root is set to **`MS_REC | MS_SLAVE`** whenever some mount asks for
propagation (and `MS_PRIVATE` otherwise). The comment in the code is explicit and the
choice is deliberate:

```
MS_PRIVATE … makes rslave/rshared on any individual mount a no-op … MS_SLAVE keeps
the one-way link the propagating mounts need (and is what Docker and runc use for
ALL containers …).
```

`MS_SLAVE` gives the **host→container** direction (the slave receives from the
master), which is why `rslave` works. But a slave **does not send back** to the
master — so the **container→host** half of `rshared` never happens, and the
per-mount `MS_SHARED` applied to the target (lines ~1508/1718) is not enough while the
root remains slave: there is no peer group shared with the host to receive the event.

## Why it is not simply «swap SLAVE for SHARED»

Making the container's root `MS_SHARED` with the host would make **all** of the
container's mounts propagate to the host (and to other containers in the same peer group)
— a much bigger opening than requested and an escalation vector (a container
creating mounts visible on the host). That is why `MS_SLAVE` is the safe default
Docker/runc use. True `rshared` is, by nature, a **privilege**: in
Kubernetes, `mountPropagation: Bidirectional` **requires a privileged container**.

## Decision

> **Note (as-built):** this is the original *proposal*. Measuring on the VM showed
> that point 1 below (root stays `rec-slave`) **cannot** make the volume a peer of
> the host — the form actually implemented is in the summary box at the top and in
> the "Investigation" section. The goal (bidirectional only on the `rshared` sub-path,
> behind privilege) and point 3 (the gate) still hold; it is the root mechanics that
> changed.

Honour `rshared` **per-mount** (not on the root) and **only** when explicitly
requested, mounting that specific target into a **peer group shared with the host's
source mount**:

1. the container's root stays `MS_REC | MS_SLAVE` (the safe default, unchanged);
2. for a mount with `propagation: rshared`, ensure the **source on the host** is
   `shared` (in a peer group), bind it into the container and mark the
   target `MS_SHARED` — so source and destination end up in the SAME peer group and
   propagation is bidirectional **only on that sub-path**, not across the whole tree;
3. **privilege gate:** `rshared` is only accepted when the container is privileged
   (the same requirement k8s has for `Bidirectional`). An `rshared` requested by a
   non-privileged container is **refused with a reason** (not silently degraded
   to slave — the class of silent-failure this repo hunts for), with the
   message naming the privilege requirement and the alternative (`rslave`).

This aligns with runc (per-mount propagation; `rshared` requires the shared source) and
keeps isolation by default.

## Alternatives considered

1. **Root `MS_SHARED` (trivial).** Makes the spec pass but leaks ALL of the
   container's mounts to the host — escalation. Rejected.
2. **Silently degrade `rshared`→`rslave`.** Safe but a silent failure (the container
   asks for bidirectional and gets unidirectional with no warning). Rejected — against
   the repo's doctrine.
3. **Per-mount peer group behind privilege (this decision).** Conforms to the spec without
   opening the host's tree or misleading the caller.

## Consequences

- **+** Conforms `rshared`; keeps `rprivate`/`rslave` and isolation by default;
  the opening is limited to the requested sub-path and only to privileged containers.
- **−** Requires making the **source on the host** `shared` (a mount operation on the
  host, hence root — consistent with ADR-0083, rootful supported; in rootless `rshared`
  remains refused for lack of privilege over the host tree). New refusal path
  to test (non-privileged → clear error).
- **Compat:** no public contract changes shape; it is mount-path behaviour.
  `rshared` moves from «degrades to slave» to «supported-if-privileged /
  refused-otherwise».

## Validation (done — 2026-11-08, `delonix-vm-k8s:1.36` golden VM, rootful+CNI)

- **Behaviour:** `critest`'s `mount with 'rshared'` **passes** (a `mount --bind
  /etc` done INSIDE the container appears on the host), `rslave`/`rprivate` still
  pass; full suite at **103/103**.
- **Security:** `rshared` from a non-privileged container / rootless engine is
  refused with `Error::BidirectionalNeedsPrivileged` (DX-1117); `rprivate`/`rslave`
  gain no propagation to the host (every non-rshared mount is pinned `rprivate`).
- **Unit:** `per_mount_propagation`, `root_propagation`, `rshared_allowed` and
  `enclosing_mount_point` — pure, in `mount_propagation_tests`.

### Investigation (2026-11-08/09) — resolved on the VM, peer groups measured

The initial hypothesis («per-mount `MS_SHARED` is already enough, same as runc») was
**wrong for delonix**. Measured in `/proc/self/mountinfo` of container vs host,
iterating on a golden VM:

- With the root at `MS_REC|MS_SLAVE` (default), the volume came out `shared:X master:1` —
  **slave** of the host's group (receives host→container, never sends back). Suspect #2
  CONFIRMED.
- Setting `/` to `MS_REC|MS_SHARED` made the spec pass with `delonix run` (the host did
  see the submount), BUT it broke the CRI server's cgroup: the old root's
  `MNT_DETACH`, with `/` shared, PROPAGATED the unmount of `/sys/fs/cgroup` back
  to the host namespace where the engine runs (`cgroup: could not create
  delonix.slice/...`). Confirmed by direct comparison: the `develop` binary
  (rec-slave root) booted the same rshared pod fine; mine (rec-shared) did not. It is the
  «leaks ALL mounts» of Alternative 1, observed.
- The final form (see the box at the top) avoids the leak: `/` is **not** touched
  recursively (the source keeps `shared:N`), the mount enclosing the rootfs gets
  `MS_PRIVATE`, the rootfs gets `MS_SHARED` only so the volumes' bind lands as a peer, and
  the old root is `MS_REC|MS_PRIVATE` before `MNT_DETACH`. Recipe measured
  working end to end: a `mount --bind /etc` done INSIDE the container appears
  at `/tmp/.../src/sub` on the host, and the container's cgroup is created with no error.

Implemented in `setup_rootfs`/`detach_old_root`/`enclosing_mount_point`
(`delonix-linux/src/lib.rs`) and in the gate in `spawn` via
`rshared_allowed(privileged, rootless)`. Blast radius contained to the
`rshared` sub-path: the old root is isolated before the detach, the rootfs's enclosing
mount is private, and every non-rshared volume is pinned `rprivate` by `per_mount_propagation`.
