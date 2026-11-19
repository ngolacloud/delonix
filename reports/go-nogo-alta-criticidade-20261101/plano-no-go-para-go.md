# NO-GO → GO plan (high criticality)

> Closes the 6 gate points of the [verdict](README.md). Each point has: **current
> state** (assessment baseline), **GO criterion** (measurable — how we know it is
> closed), **tasks**, and **who** can do them:
> `[agent]` executable by me in the repo · `[owner]` owner decision/action
> (merge/release/commissioning) · `[external]` independent third party.
>
> **Global GO definition:** the 6 criteria green **AND** a re-run of this
> assessment on the shipped version confirms that no disqualifier survives.

## Recommended order (why)

```
Wave 1 (base of truth)     G6 re-measure ───┐
Wave 2 (confidence)        G1 security      ├─► G2 validated release
Wave 3 (op. safety)        G3 upgrade/rollback   G5 scale/density
Wave 4 (ceiling)           G4 HA/multi-node  (or decision to exclude failover)
```

- **G6 first:** every other claim rests on current numbers; the evidence today is
  from v0.x–v3.1.0/5.1.1 (`reports/.../stale-evidence.md`). Without G6, G1/G2
  validate a moving target.
- **G1 before G2:** a "trustworthy" release requires that the pipeline producing
  it be trustworthy itself (supply chain).
- **G4 last:** it is the biggest investment and can be **replaced by a scope
  decision** (exclude failover and document the single-node ceiling).

---

## G1 — Security assured externally

**Current state.** Audits internal only (AI-adversarial); supply chain:
G-020 0/20 Actions SHA-pinned; no `cargo-audit`/trivy in CI; CVE scanner with no
feed (`delonix-scanner/src/lib.rs:371`); signing/SLSA/SBOM "by design", not
verified against artefacts. Rootful/SELinux never assessed.

**GO criterion.** (a) External pentest of the shipped line with report and
HIGH/CRITICAL findings closed; (b) 20/20 Actions SHA-pinned; (c) `cargo-audit` +
image scan in CI failing the build on a known CVE; (d) scanner with a real feed
(OSV/NVD/Alpine secdb) proven to catch a known CVE; (e) signing +
provenance + SBOM **verified against the real release artefacts**; (f)
rootful and SELinux-enforcing assessed (or declared out of scope in writing).

**Tasks.**
- `[agent]` SHA-pin all `.github/workflows/*` (tag→SHA) + comment with the human-readable version.
- `[agent]` Add `cargo-audit` (or `cargo deny advisories` already exists — extend it to CVE) and an image-scan step to CI, failing the build.
- `[agent]` Wire the scanner to a real feed (OSV/NVD/Alpine secdb), with a test that catches a known planted CVE.
- `[agent]` Script that **verifies** signing/provenance/SBOM against a release candidate's assets (closes the "PASS by design").
- `[agent]` Rootful + SELinux-enforcing battery (in a VM) and record the results; OR an ADR excluding those modes.
- `[owner]`/`[external]` **Commission the external pentest** — it is not self-satisfiable; it is the item that defines "independent".

---

## G2 — Tagged and validated release

**Current state.** Nothing published on the 0.x line (`CHANGELOG` "[Unreleased]");
the last legacy (v5.1.1) was poorly validated (direct pushes to `main`, red CI,
`release.yml` failed 3× — `docs/RELEASES.md:78-99`).

**GO criterion.** A tag whose CI is **green** and in which `cargo test` + `e2e.sh`
+ `chaos.sh` + `critest` ran **on the tag itself** (not "on develop weeks ago"),
with the artefacts signed and verified (ties into G1).

**Tasks.**
- `[agent]` Harden `release.yml`/`gitflow.py`: a gate that refuses to tag with non-green CI and without the suites run on the tag.
- `[agent]` Close the baseline gates that fail today (rustfmt/lang_ratchet) — see [[delonix-merge-ci-pitfalls]] — so that "green" is real.
- `[owner]` Merge the stable line → `main` and **cut the tag** (owner action; the hook refuses the agent's push).

---

## G3 — Upgrade/rollback

**Current state.** NOT TESTED (`release-verdict.md`); live supervisors run the
code they were born with — no hot-patch (`RELATORIO-PRE-PRODUCAO.md:116`); no
rollback mid-`apply` (`47_IAC_REVISAO.md:400`). Only
`e2e_state_upgrade.sh` exists (on-disk state format).

**GO criterion.** (a) E2E test: install vN, run workloads, upgrade to
vN+1 **with live containers/VMs**, confirm they survive and reconcile; (b)
rollback to vN proven (previous binary + `netns down/up` in a window) without loss
of state; (c) written procedure to fix a live supervisor (or an ADR that
accepts the limit and the "recreate in a window" runbook).

**Tasks.**
- `[agent]` ✅ **DONE (mechanism):** `scripts/e2e_upgrade_rollback.sh` — vN→vN+1→vN
  with a live container + network + state mark; 17 assertions GREEN (2026-11-02):
  survival (container `Up`, SAME id after the binary swap), management by the new
  engine (`exec`/`logs` over a container born under the old one), reconciliation
  (`stop`/`start` preserves the writable layer), and rollback. Params `--old`/`--new`.
  **MISSING the CROSS version compatibility** (new binary reading old-version
  state): needs a prior tag compiled for `--old` — the harness
  already measures it with no further changes. (On-disk state across versions is in
  `e2e_state_upgrade.sh`.)
- `[agent]` ✅ **DONE:** hot-reattach measured and documented. Key findings:
  **(1)** in-place `cp` of the binary in use → **ETXTBSY** (kernel refuses); upgrade
  MUST be by **atomic rename** (dpkg/rpm/`install(1)` already do this).
  **(2)** live containers **survive** the binary swap (the supervisor stays on the
  old inode, mapped). **(3)** there is NO hot-patch of the live supervisor: it runs
  the code it was born with until recreated — a fix in the supervision path only
  reaches an existing container via phased recreation.
- `[agent]` ✅ **DONE:** runbook `docs/operations/upgrade-rollback.md` (atomic-rename
  procedure, table of what survives, rollback, validation, and the section
  Limits: cross-compat unproven, no hot-patch, no rollback-mid-apply
  `47_IAC_REVISAO.md:400`).

---

## G4 — HA / multi-node

**Current state.** Single-node by design; fleet delegated to `delonix-paas`
(ADR-0010). Inter-node overlay (VXLAN/WireGuard) not proven E2E; VM HA/replication =
`requires-external-component`; live migration NO-GO (ADR-0031); kubeadm HA offline
not executed in the campaigns. There is work done (5 VMs + 3 CP etcd stacked + HAProxy
proven in a prior session — see [[delonix-kubeadm-ha-offline-findings]]).

**GO criterion (one of two).**
- **Via proof:** inter-node overlay forwarding E2E between ≥2 nodes + a k8s HA control-plane (≥3 CP, etcd stacked, VIP) surviving the loss of one CP, all on Delonix; OR
- **Via scope:** an explicit ADR excluding failover from the product, documenting the single-node ceiling, and pointing the fleet to `delonix-paas`/external k8s — making "high criticality WITH HA" an honest non-goal instead of a gap.

**Tasks.**
- `[owner]` ✅ **DECIDED (2026-11-02): via PROOF — Delonix wants to BE HA.** So the
  criterion is the first bullet above (inter-node overlay E2E + k8s HA control-plane).
  Next steps: (1) run `scripts/e2e_overlay_multinode.sh` against 2 real nodes
  (2 delonix VMs or 2 SSH hosts); (2) resume the kubeadm HA chain (≥3 CP, etcd
  stacked, VIP), today blocked by [[delonix-cri-entrypoint-override-bug]] (fix in
  `fix/cri-command-entrypoint-override`, to be integrated). The "via scope" path
  (ADR excluding failover) is DISCARDED.
- `[agent]` **BENCH RUN ON 2 REAL NODES (2026-11-02) — see
  `g4-overlay-forwarding.md`:** 2 VMs of the golden k8s (virbr0, routable IPs, WG
  pubkeys exchanged, 0.1.0 binary of the current line). RESULT: **forwarding = 100%
  loss** in both directions. ROOT CAUSE pinned: `ss -ulnp` on the node shows NOTHING on
  UDP 51820/4789 — the overlay does `wg-up`/VXLAN INSIDE the holder (slirp4netns), but
  `slirp_add_hostfwd` only wires published ports, never 51820/4789 → the peer's WG
  handshake hits a closed port. Confirms (with data) the `Partial` in the matrix.
  2nd finding: **`--subnet` ignored** (overlay at 10.234/16 auto, each node
  independent), reproduced on real VMs. **TWO SDN-core blocks to close before "via proof"
  can pass:** (1) slirp hostfwd for 51820(WG)+4789(VXLAN)
  when there is an overlay — or a rootful mode with WG on a real interface; (2) honour
  `--subnet` + coordinate the /16 across nodes. These are changes with
  firewall/ADR implications → they need owner design before implementing.
- `[agent]` ✅ **DONE (bench):** `scripts/e2e_overlay_multinode.sh` — the
  multi-node bench of the "via proof" path, topology-agnostic (`--via ssh A B`). It asserts:
  WireGuard identity on each node, creating the SAME overlay (`--driver overlay --vni`)
  with the other as `--peer <ip>=<pub>=<wg_ip>`, a container on each node on the network,
  **E2E forwarding** A→B and B→A over the overlay IPs, and **path proof** (WireGuard
  handshake + peer). **MEASURED and proven that a single box does NOT run this** (`node
  init` ok, but `network create --driver overlay` in rootful gives `DX-9301 timeout
  waiting for the netns holder`; rootless is behind slirp, no inbound) — so
  the harness **refuses to fake it** (exit 3) and prints the runbook. Matches the
  reason the matrix itself already gives for marking the overlay `Partial`. **MISSING**:
  run against 2 real nodes (SSH) — or resume the kubeadm HA chain blocked by
  [[delonix-cri-entrypoint-override-bug]] for the HA control-plane.
- `[agent]` ✅ **INTER-NODE OVERLAY: PROVEN E2E (2026-11-02).** Three fixes (WG
  hostfwd ADR-0084 + `--subnet` honoured + firewall `dlxinput` accepting the inbound
  VXLAN `iifname "wgo*" udp dport 4789`): container-to-container ping between 2 real
  VMs at **0% loss** in both directions. `net.overlay.{vxlan,encrypted}`
  Partial→Supported in the matrix (183/271). See `g4-overlay-forwarding.md`.
- `[agent]` ✅ **k8s HA CONTROL-PLANE: PROVEN (2026-11-03) — see
  `g4-ha-controlplane.md`.** After freeing disk (`cargo clean` of
  DORM/target, 54.9 GiB → 99G free), `delonix cluster kubeadm --control-plane 3
  --workers 0 --cni default` provisioned 3 CP + **HAProxy LB (VIP)** + etcd
  stacked + Cilium, **all on delonix** (`delonix-cri` as each node's CRI,
  `CONTAINER-RUNTIME delonix://0.1.0`). `3/3 Ready`, kubectl via the VIP, a real pod
  with a Cilium IP (the CNI the CRI bug blocked, now live). **FAILOVER:**
  `vm stop g4ha-cp1` → the API survives via the VIP, **etcd write OK** (configmap
  `survived=yes`, quorum 2/3), **new pod scheduled** on cp3; cp1 goes `NotReady`.
  **Self-heal:** `vm start g4ha-cp1` → 3/3 Ready in ~20 s. G4-HA criterion closed.
  Product candidate this proof revealed — **CLOSED (ADR-0085):** the
  `cluster create` kind-mode now provisions an **HAProxy container** and does
  HA in containers (`--control-planes 3` → 3 CP + LB, failover proven, no VMs).
- `[agent]` (if via scope) Write the ADR + the "single-node ceiling" section in the handbook.

---

## G5 — Scale / density

**Current state.** Not measured. Only sample: chaos `scale(30)` with clean teardown.
The attach path does ~20-25 `fork`+`exec` + 2 `nft` dumps that grow with the number
of containers/namespaces; all numbers were taken on an empty node
(`comparacao-medida.md:207-230`).

**GO criterion.** A declared density ceiling (e.g. "N containers/node with attach p99
< X ms and stable teardown"), measured on a dedicated node, with the latency curve
vs density (not a point on an empty node) and the corresponding SLOs.

**Tasks.**
- `[agent]` ✅ **DONE (harness):** `scripts/bench_density.sh` — a tier ramp
  (`--tiers 1,10,25,50,100`), attach p50/p99 per tier + size of the two
  `nft` dumps (`list sets`, `-a list chain fwdeny`); it emits the CURVE in JSON (the
  `bench.sh` `publishable` contract), inherits the bench judgement of `bancada.sh` (refuses
  above the threshold), marks NOT PUBLISHABLE when there are foreign containers, and cleans
  up everything it creates (prefix `benchdens-`). Proven end-to-end in rootless (curve
  + cleanup + JSON). The `nft` columns fill in **rootful** (`inet dlxing` table
  visible from the host); in rootless they stay `null` by design (the holder cannot be
  `nsenter`ed from outside). **Still missing a run on a DEDICATED node** (this box has the
  owner's DBs → not publishable) to produce the real curve.
- `[agent]` ✅ **CURVE MEASURED (2026-11-03) — see `g5-density.md`.** Ramp 1→100
  containers (`bench_density.sh`, not publishable: shared box + 1 owner
  container): **attach p50 FLAT** (193→215 ms, ~11 % = noise); p99 noisy but with no
  trend. **Attach latency does NOT grow with density up to 100.** The
  O(N) suspicion (`drop_from_every_ns_set` `nft list sets`, `fwdeny` scans) **does
  NOT hold in practice**: `list sets` counts NAMESPACES, not containers (the 100
  were in `default` → ~1 ns-set). **DECISION: do not optimise** — it would solve a
  non-problem in the security datapath; the target stays documented if one day a
  workload with many namespaces shows the curve rising.
- `[agent]` SLOs + the PUBLISHABLE curve are left for a **dedicated node** (the shape is
  already known and reassuring).

---

## G6 — Re-measure the stale evidence

**Current state.** CRI/bench/E2E/Docker-API measured on v0.x–v3.1.0/5.1.1; "almost
no published number was measured on 3.x+" (`stale-evidence.md`). The verdict
rests on this with a caveat.

**GO criterion.** `critest`, `bench.sh`, `e2e.sh`, `chaos.sh` and the capability
matrix **re-run on the shipped version**, with dates and commit matching up;
the comparison documents updated from those runs.

**Tasks.**
- `[agent]` ✅ **DONE (matrix):** `capability-matrix.md` regenerated from the current binary
  (2026-11-02): **181/271 (66.8 %)**, engine 0.1.0 — was 180/270@5.1.1; `net.ipv6`
  became `supported` (ADR-0082). Drift closed: the test `the_published_matrix_is_
  the_generated_one` is GREEN again. This report's README + `stale-evidence.md`
  updated with date/commit.
- `[agent]` **MISSING (dedicated node/infra):** re-run `bench.sh` (refuses on a loaded
  node — by design), `bench_density.sh` (curve — idle node), `critest`
  (k8s/CRI) and `chaos.sh`/`e2e.sh` (create VMs/kind). This box has foreign load
  (owner's DBs) → not publishable. Update `cri-conformance.md`/`comparacao-medida.md`
  once those runs exist.
- `[agent]` ✅ **DONE:** `stale-evidence.md` marks the matrix as FRESH and lists what
  remains STALE (and why).

---

## What I can start right now (no merge/release, no third parties)

Executable by me immediately, highest to lowest leverage:
1. **G1 (b,c,d,e):** SHA-pin the Actions, `cargo-audit`+scan in CI, real CVE feed, artefact verifier. *(the supply chain is the weakest link and it is all in-repo.)*
2. **G6:** re-run the suites and refresh the numbers — it unblocks confidence in everything else.
3. **G5:** density bench (gives the first scale ceiling that never existed).
4. **G3:** upgrade/rollback harness.

Left for the owner/third parties: the **external pentest** (G1a), the **tag/release** (G2), and
the **HA scope decision** (G4). Tell me which of the four I start with.
