# ADR-0107: `delonix.node.v1` is a served contract with a written promise — a stability tier per RPC, the release tag as its version, and a compatibility rule a client can pin to

- **Status:** Proposed (2026-11-27) — design only, nothing implemented; awaiting the owner's
  decision.
- **Date:** 2026-11-27
- **Deciders:** Walter Angolar (owner)
- **Relates to:** ADR-0041 D4 (the promise and when it starts — accepted, never delivered; this
  ADR delivers it and supersedes its `v1alpha1` clause), ADR-0042 D1 (one API, one number:
  `v1`; `buf breaking` says what is breaking), ADR-0040 D4 and P1 (the contract and its gate),
  ADR-0096 (the contract reachable over TCP + mTLS, so clients outside the node exist),
  ADR-0106 (an additive change made under this promise).

## Context

An external client that integrates the node contract over TCP + mTLS wants to pin its client
to a version of `delonix.node.v1` and to know what may change under it. On 2026-11-27 it could
only copy the `.proto` files at a commit and test for divergence. Measured:

- **The contract says it is a draft that nothing serves.** `proto/README.md:3`: «Status:
  draft — ADR-0040. Nothing serves or consumes these files yet.» and `:8`: «Served — once phase
  P5 lands».
- **It is served, all of it.** `delonix-node-api` mounts every service of the package — Node,
  Network, Volume, Container, Operation, VirtualMachine, Storage, Image, Pod and Stack
  (`crates/interfaces/delonix-node-api/src/service.rs:1536-1575`); some RPCs still answer
  `UNIMPLEMENTED` with a reason, for example `VirtualMachineService.Console`
  (`service.rs:1112-1121`), and REST answers those 501 / DX-6001
  (`crates/interfaces/delonix-node-api/src/lib.rs:55`). Since ADR-0096 it is reachable from
  other machines.
- **The promise that was decided was never written.** ADR-0041 D4 (accepted 2026-10-06) asked
  for the package to be `delonix.node.v1alpha1` until wave W1, then for a «Node contract»
  section in `docs/cli-stability.md` promising, per service: additive growth within the major
  (`buf breaking` against the last tag), a removal only with `v2` served alongside `v1` for at
  least one minor, no promise for `UNIMPLEMENTED` RPCs beyond their shape. The package is
  `delonix.node.v1` in every file (`proto/delonix/node/v1/node.proto:6`, and the five others),
  and `docs/cli-stability.md:505-513` still lists `serve node-api` under «NOT stable — may change
  in any version», saying ADR-0040 and ADR-0041 are «both *Proposed*» — both are accepted.
- **The compatibility check already runs.** `scripts/contract_gate.py:11-13`, `:294-305` runs
  `buf breaking` (FILE rules, `buf.yaml`) against the newest release tag containing `proto/`;
  `v0.2.1` contains it and the CI job checks out every tag (`.github/workflows/ci.yml`, job
  `contract`, `fetch-depth: 0`). So every wire break is already refused in CI — but no document
  tells a client so, and nothing says which RPCs a client may rely on.
- **The product's 0.x rule is looser than the gate.** `VERSIONING.md:18-24`: in 0.x a minor
  «may break CLI/API/JSON/schema contracts», each break noted `BREAKING` in the CHANGELOG.
- **A client can read the server's version, not its contract level.** `ApiRoot.api_version`
  and `NodeInfo.api_version` say «delonix.node.v1»; `NodeInfo.engine_version` gives the release
  (`proto/delonix/node/v1/node.proto:76-88`).

Guardrails touched: **no silent failure** (#6) — a contract that claims to be an unserved draft
while clients depend on it misleads in both directions; **no consumer in the contract**
(`proto/README.md` rule 3) — the promise is to any client. No daemon, tenancy, dependency or
privilege boundary.

## Decision

**D1. The package stays `delonix.node.v1`.** Renaming it to `v1alpha1` now, as ADR-0041 D4
asked before anything was served, would break every existing client for no gain; this clause of
ADR-0041 D4 is superseded. The rest of ADR-0041 D4 is delivered by D2–D5.

**D2. Every RPC has a stability tier, written once and checked.**
- **`stable`** — changes only additively within `v1`: new RPCs, new fields, new enum values; a
  field or RPC is never renumbered, retyped or removed. A removal needs `v2` served alongside
  `v1` for at least one minor release, and a `deprecated = true` option for at least one minor
  before it.
- **`preview`** — served and tested, but its shape may change in a minor release, each change
  listed as `BREAKING` in the CHANGELOG and made through an explicit, named exception in the
  `buf breaking` configuration (never by moving the baseline).
- **`unimplemented`** — answers `UNIMPLEMENTED` (REST 501, DX-6001); only its shape is promised.

The tier of every RPC lives in one table, the «Node contract» section of
`docs/cli-stability.md`, and `scripts/contract_gate.py` gains a check that every RPC of the
package appears in it exactly once, and that an RPC marked `unimplemented` is the one the server
answers `UNIMPLEMENTED`. The initial assignment is part of the implementing change, RPC by RPC,
from what the conformance tests cover (ADR-0042 D4): no tier is granted on assertion.

**D3. The version a client pins to is the release tag.** The contract has no version number of
its own besides the package's major: a client pins to the `.proto` files of an engine release
`vX.Y.Z`. The release publishes them as an asset (`delonix-node-v1-proto-X.Y.Z.tar.gz`) covered
by the release's signed `SHA256SUMS`, so a client vendors a verifiable artefact instead of a
commit. A server reports its release in `NodeInfo.engine_version` (exists).

**D4. The compatibility rule, stated.** For `stable` RPCs, a client built against release `T`
works against any server `S ≥ T` with the same major (fields it does not know are skipped, as
proto3 and the generated REST encoding already do). Against a server `S < T`, an RPC or field
added after `S` is answered `UNIMPLEMENTED` or absent — never silently misread. For `stable`
RPCs this promise holds **during 0.x as well**: the node contract is exempt from
`VERSIONING.md`'s «a minor may break» rule, which keeps applying to `preview` RPCs and to the
rest of the product.

**D5. The documents say what is true, in the same change.** `proto/README.md` drops «draft —
nothing serves» and points at the tier table; `docs/cli-stability.md` moves `serve node-api`
out of «NOT stable» into the new section and corrects the ADR states; `VERSIONING.md` names
the exemption of D4.

## Alternatives considered

- **Do nothing; clients pin a commit.** Rejected: the gate already refuses breaks, so the
  engine pays the cost of a promise without giving one; every client re-derives what is safe.
- **Rename to `v1alpha1`/`v1beta1` and promise later (ADR-0041 D4 as written).** Rejected: the
  contract is served and consumed; a package rename is itself the largest possible break.
- **One tier for the whole package.** Rejected: «all stable» would promise RPCs that answer
  `UNIMPLEMENTED`; «all preview» would deny clients the guarantee the gate already enforces.
- **A separate semantic version for the contract.** Rejected: a second number beside the
  release tag and the package major is one more thing to keep aligned
  (`scripts/version_gate.py` exists because one number already drifted); ADR-0042 D1 chose one.
- **A custom proto option carrying the tier.** Deferred: machine-readable inside the files, but
  it adds an extension to the contract and to every client's codegen. The table plus the gate
  gives the same check; an option can replace the table later without changing the promise.

## Consequences

- A client can vendor a signed set of `.proto` files from a release and know, per RPC, what it
  may rely on across upgrades.
- The engine commits to keeping `stable` RPCs additive even in 0.x; a needed break in one of
  them becomes a `v2` decision with its own ADR, served alongside `v1`.
- Assigning tiers costs one pass over the 69 RPCs (`compute` 29, `infra` 20, `node` 10,
  `storage` 6, `operations` 4) and keeping the table current is a gate, not a habit.
- `preview` breaks need an explicit exception in `buf.yaml`, which makes each one visible in
  review.
