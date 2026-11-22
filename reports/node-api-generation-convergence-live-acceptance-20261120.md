# A4 — `generation`/`observedGeneration` on the node API: live acceptance (2026-11-20)

ADR-0099. Branch `feat/node-api-mtls-transport` (A1 committed 64f01d72; A2+A3+A4 on top, not
committed). Sub-step A4 of the fleet-operability Phase A.

## What was built

The node-API mutation path now stamps the ADR-0090 record-metadata counters, with the
desired/observed split ADR-0090 deferred and ADR-0099 decides:

- The pure versioning primitives (`next_generation`, the `delonix.io/{generation,observed-
  generation,stored-version,last-applied}` keys, `encode/decode_last_applied`) moved from
  `delonix-stack::reconcile` to `delonix-model::versioning` (the crate both the CLI reconciler and
  the node API depend on; no interface→context edge). `delonix-stack::reconcile` re-exports them, so
  every CLI call site compiles unchanged.
- New pure helpers for the two-phase node-API stamp: `desired_annotations` (bump `generation`, leave
  `observedGeneration` behind) and `observed_generation_entry` (advance it on success).
- `Container` create (`container_launch::create_work`) stamps the **desired** generation before the
  reconcile (the launch) and advances **observed** after it succeeds; `UpdateContainer`
  (`container_update::update_in`) stamps both in its single synchronous save.
- `ResourceMeta` (common.proto) carries `generation`/`observed_generation`/`stored_version`
  (fields 9/10/11), filled from the record's annotations; the engine's own `delonix.io/*` keys are
  kept off the contract's annotations map.

## Proof 1 — full cycle over the real gRPC service (in-process, host, rootless)

`cargo test -p delonix-node-api --test live_container_update -- --ignored` (busybox, a real
container create+start+resize over gRPC on a unix socket):

```
create → generation=1 observed_generation=1 stored_version=compute.delonix.io/v1beta1
update (memory 128→64 MiB, changed spec) → generation=2 observed_generation=2
re-apply (same memory, unchanged spec)   → generation=2   (kept)
```

- A create stamps `generation 1` and, once the launch succeeds, converges `observedGeneration` to it.
- A changed desired spec bumps `generation` to 2; the synchronous resize converges `observedGeneration`.
- An unchanged spec keeps `generation` at 2 (the applied-spec fingerprint is unchanged).

## Proof 2 — cross-machine over mutual TLS (VM delonix → host)

Node API served over TCP+mTLS on the host at `192.168.122.1:29443` (ADR-0096 transport, ADR-0097
CA, same `DELONIX_ROOT` holding busybox). Client = a freshly-created delonix VM `a4node`
(`192.168.122.96`, `delonix-vm-k8s:1.36`), running `curl` with the control-plane client cert.

```
[VM] GET /v1                → api_version=delonix.node.v1   (mTLS handshake, cross-machine)
[VM] CREATE d4 (start:false)   op=SUCCEEDED
     GET d4    → generation=1 observed_generation=1 stored_version=compute.delonix.io/v1beta1   (realized)
[VM] CREATE d4fail (start:true, entrypoint /nao-existe)   op=FAILED
     GET d4fail→ generation=1 observed_generation=0   (asked for, NOT realized)
```

- A converged create reads `generation == observedGeneration`: the control plane sees the change realized.
- A create whose **reconcile failed** reads `generation=1 observed_generation=0` — the pending/failed
  state ADR-0090's post-success-only stamp could not express, now representable and readable remotely.

## Gates (delta ZERO vs the pre-A4 / A3 reference)

- `cargo fmt --check` on the touched crates (delonix-model/stack/node-api): clean. (Pre-existing
  unrelated fmt debt in `delonix-node-api-bin/src/main.rs` authz loader is A3's, untouched here.)
- `cargo clippy` on the touched crates: no new warnings.
- `scripts/arch_fitness.py`: the four FAIL metrics read **cli_context_lines 126957 / env_writes 25 /
  library_prints 119 / raw_error 1** — byte-identical to the A3 reference (pre-existing A2/A3 debt vs
  the script's A1 baseline). A4 adds zero. (A stray +3 `library_prints` from test `eprintln!` was
  removed to hold the line; the asserts carry the proof.)
- `scripts/lang_ratchet.py`: docs 382 = the A3 reference; ADR-0099 is English and appears nowhere in
  `--list --only docs` — A4 contributes zero PT. (The 382>380 FAIL is pre-existing A2/A3 debt.)
- `cargo test -p delonix-node-api`: all pass (incl. 5 new `delonix-model::versioning` unit tests and
  the extended live update test). `cargo test -p delonix-bin`: 1733 pass, only the 3 pre-existing red
  (provider×2, schema×1 — schema/matrix drift on the base, unrelated to node/v1).

## Finding (not A4's to fix)

The REST transcode does not bind a `FieldMask` from the query string, so `PATCH
/v1/namespaces/{ns}/containers/{name}` cannot carry `update_mask` over REST (`unknown query
parameter`). The full changed-spec→bump cycle is therefore shown over gRPC (Proof 1); the
cross-machine REST leg shows create convergence and the failed-reconcile divergence (Proof 2). A
query-bound FieldMask is a separate transcode improvement.
