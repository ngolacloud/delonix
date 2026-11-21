# F-1 — Layer cache poisoning on upgrade (OCI whiteouts)

- **Severity:** P1 (image integrity / upgrade path). Bounded: only
  layers unpacked by a *pre-fix* binary; new pulls are already
  correct; workaround = prune the layer cache.
- **Domain:** §6 (data integrity), §7 (images, safe extraction),
  §12 (binary rollback ≠ state rollback).
- **Type:** product defect (cache reuse without format validation).
- **Candidate:** `v0.1.1`. Environment: `delonix 0.1.0` @ `8ab783dd`, rootless.
- **Status:** FIXED + unit test + **live end-to-end re-proof (closed)**.

## What was observed (forensics, on the test host)

The historical OCI whiteout bug (`.wh.<name>` unpacked as a regular
file instead of an overlay char device — the removal never reaches the lower
layer) **is already fixed in code**: the layer store has the exact
Apache MPM case as char device `0:0`:

```
layers/c4d2115b…/etc/apache2/mods-enabled/mpm_event.conf   # char dev 0:0  (OK)
layers/c4d2115b…/etc/apache2/mods-enabled/mpm_event.load   # char dev 0:0  (OK)
```

**But** layers with `.wh.*` leaked as regular files persist:

```
layers/ba1ebfcb…/etc/rabbitmq/conf.d/.wh.20-management_agent.disable_metrics_collector.conf
layers/25619be2…/tmp/.wh.pgvector
layers/42109c5c…/var/lib/apt/lists/.wh.auxfiles
layers/fa0f0019…/home/.wh.ubuntu
```

The chronology of the layer mtimes confirms the cause: the leaked ones were extracted
on **2026-10-21 19:52** and **2026-10-22 17:44**; the fixed ones (char device) on
**2026-10-22 18:04**. The whiteout fix landed between 17:44 and
18:04 on 2026-10-22.

## Root cause

`ImageStore::ensure_layers` / `extract_one` (`crates/adapters/delonix-oci/src/
overlay.rs`) reuse a cached layer **solely by the presence of the
`.extracted` marker** (contents `b"ok"`), without validating the on-disk format. Since
layers are addressed by digest and the blob/digest never change, an extraction
done by a buggy binary **survives forever** — upgrading the binary
fixes the *code* but not the *cache*. Any image whose layers were
pulled before the fix still has the broken whiteout at runtime
(e.g.: `rabbitmq` with the metrics plugin that should be disabled coming back,
`wordpress`/Apache with two MPMs).

## Fix

A **format epoch** was introduced into the `.extracted` marker (`LAYER_EPOCH`,
currently `b"2"`). A layer is only reused if the marker records the
current epoch; a missing marker (never finished) or an old token (`ok`, pre-whiteout-fix)
→ **not trustworthy** → the layer is re-extracted from the immutable blob on the
next use. Upgrading the binary now **heals** the cache lazily and
per-layer (one-off I/O cost on the first use of each image; to be documented
in migration notes).

- `overlay.rs`: `const LAYER_EPOCH` + `fn layer_is_current(dir)`; wired into the three
  reuse points (`ensure_layers`, entry guard and the race check in
  `extract_one`). `publish_layer_dir` already replaces a non-empty `dir`, so
  re-extraction over a poisoned layer works without any further change.

## Test

- `overlay::tests::a_layer_from_an_older_epoch_is_reextracted_not_reused` —
  a layer with a `b"ok"` marker + a leaked `.wh.poison` + stale content is
  re-extracted: the blob file appears, the planted junk disappears, the marker
  becomes `LAYER_EPOCH`. Green.
- Full `delonix-oci --lib` suite: **192/192** green (no regression in the
  existing tests that assert the marker).

## Live end-to-end re-proof (2026-10-29) — CLOSED

Fixed binary built from `stabilize/0.1.1` (`target/debug/delonix`, 0.1.0).
Rootless A/B in an isolated `DELONIX_ROOT`, using the **authentic poisoned layer
from the host** (`ba1ebfcb…` from `rabbitmq:4.0-management-alpine`: marker `ok`, the
whiteout `.wh.20-management_agent.disable_metrics_collector.conf` as a regular
file 000, zero char-devs) copied over a healthy extraction.

**A) OLD binary (without the F-1 fix)** — `run … -- ls -la /etc/rabbitmq/conf.d/`:

```
----------  1 root     root       0  .wh.20-management_agent.disable_metrics_collector.conf
-rw-r--r--  1 rabbitmq rabbitmq 491  10-defaults.conf
-rw-r--r--  1 rabbitmq rabbitmq 151  20-management_agent.disable_metrics_collector.conf   <-- NOT deleted
```

Reuses the layer (marker stays `ok`, 0 char-devs). The whiteout is NOT honoured:
the `.conf` from the lower layer appears + the literal `.wh.` marker 000 (which
rabbitmq's `*.conf` parser cannot read). Bug reproduced exactly.

**B) FIXED binary (F-1), SAME store** — `run … -- ls -la /etc/rabbitmq/conf.d/`:

```
 ✓ unpacking the image 1.6s        <-- re-extracted (old epoch != current)
-rw-r--r--  1 rabbitmq rabbitmq 491  10-defaults.conf
```

`conf.d` clean (only `10-defaults.conf`); the layer moves to marker `2`, 1 char-dev
(correct whiteout), 0 leaked `.wh.` files. **Healed automatically on the
first use after the upgrade**, with no operator action. P1 closed.
