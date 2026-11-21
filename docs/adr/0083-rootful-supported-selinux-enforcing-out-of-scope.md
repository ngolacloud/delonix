# ADR-0083 — Rootful is supported; SELinux-enforcing (with relabel) is out of scope for the 0.x line

**Status:** Proposed (2026-11-01) — evaluation of item (f) of gate G1 of the
NO-GO→GO plan (`reports/go-nogo-alta-criticidade-20261101/`). Rootful **measured**
on this host; SELinux-enforcing **evaluated by code reading** and declared out of
scope, with the support path described. Awaiting the owner's GO.
**Relates to:** `reports/go-nogo-alta-criticidade-20261101/g1f-rootful-selinux.md`
(the battery), ADR-0079 (group honoured in root mode), ADR-0077 (rootless sensor).

## Context

The GO/NO-GO report (2026-11-01) flagged «rootful/SELinux never evaluated» as one
of the points keeping the verdict at NO-GO for high-criticality. Gate G1 requires
both to be **evaluated**, or **declared out of scope in writing**. This ADR is
that written decision, grounded in the battery from the report above.

The product line is **rootless-first** by design (daemonless, userns, no startup
privilege). Rootful exists and has its own paths (ADR-0079: group in root mode, a
cgroup leaf for the VMM). SELinux was never exercised because no development host
has it (all are AppArmor/Ubuntu).

## Decision

### 1. Rootful — SUPPORTED (measured)

The rootful core is supported and was measured on this host (real uid 0, `/dev` via
`mknod` with real nodes, bridge network, `run -d`/`exec`/`rm`, its own store). It is
documented as a supported mode alongside rootless.

Two edges were **recorded as known debt**, neither blocking normal rootful use —
the first already **resolved** (see below):

- ~~**`build FROM scratch` fails rootful packaging** (`DX-1410 image has no
  layers`).~~ **RESOLVED.** The real cause predated packaging: `FROM
  scratch` in rootful hit `mount_rootfs`, which refuses a zero-layer image
  (overlay needs ≥1 lower), so the build aborted before the first `COPY`. It now
  mounts an empty base (`mount_scratch_rootfs`: an ephemeral empty lower +
  writable upper); `COPY`/`RUN` fill the upper, which `commit_upper` packages as
  the image's only layer (`build_image` over the zero-layer scratch base → 1
  layer). `image save`/`push` no longer refuse. Covered in e2e (`build FROM
  scratch … and packages`). It is distinct from the three `FROM scratch` bugs
  fixed earlier in this line (those were startup/rootless — `scratch`
  resolution, userns `/dev`, scaffolding on `COPY`); this one was in the rootful
  commit.
- **Rootful memory limit** was not conclusively confirmed (`memory.max` not
  visible from inside the container in the battery) — needs a dedicated
  host-side test.

### 2. SELinux-enforcing (with volume relabel) — OUT OF SCOPE for the 0.x line

What **already exists**: the container's **process** label is applied
(`apply_selinux` writes the context to `/proc/thread-self/attr/exec` when
`/sys/fs/selinux/enforce` exists).

What is **missing**, and is the reason for the exclusion: **volume relabel**
(`:z`/`:Z`) is deliberately not implemented and refused fail-closed
(`delonix-volume`), and there is no mount labelling (`context=`/`fscontext=`) nor
file labelling. On an enforcing host, a bind mount keeps the host's label and the
container's domain is **denied** — the typical failure mode on RHEL/Fedora.
Without this, a workload with volumes does not run under enforcing, however
correct the process label is.

That piece — recursive relabel of the volume tree to the container's domain (via
`libselinux`/`setfilecon`), per-mount `context=`, and the shared (`z`) vs private
(`Z`) semantics — is **its own work**, not a tweak, and needs a real enforcing
host to be honestly validated. Declaring it out of scope for the 0.x line is the
honest choice: the alternative would be pretending support that the first bind
mount under enforcing would disprove.

## Consequences

- Gate G1(f) closes: both **evaluated in writing**, rootful measured,
  SELinux-enforcing out of scope with justification and a path.
- `delonix` announces **rootful** and **rootless** support; it does **not**
  announce SELinux-enforcing support for workloads with volumes. The fail-closed
  refusal of `:z`/`:Z` stands — preferable to a silently absent relabel.
- The path to lifting the exclusion (when the owner decides): implement volume
  relabel + per-mount `context=`, and validate on a Fedora/RHEL enforcing VM (the
  rootful battery from this report, repeated under enforcing, plus a `:Z` volume
  case). Enters as a successor ADR.
- Known rootful debt: `build FROM scratch` **resolved** (see §1); the memory
  limit stays tracked here and in the report. Does not change the verdict of
  «rootful supported» for normal use.

## Alternatives considered

- **Implement volume relabel now.** Rejected for the 0.x line: without an
  enforcing host to validate against, it would be code exercised outside the
  exact condition it exists to serve — the same «PASS by design» trap that G1(e)
  closed.
- **Claim SELinux is supported because the process label is applied.**
  Rejected: it is a half-truth; it fails on the first bind mount under
  enforcing.
