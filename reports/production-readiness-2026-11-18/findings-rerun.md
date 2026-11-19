# Delonix — live re-measurement after dependency review (2026-11-18)

> **Objective:** confirm that the dependency review integrated into develop
> (phases 1-3, `cargo audit` 8 vuln → 0) **did not regress the runtime** vs round 1
> of production readiness (`reports/production-readiness-2026-11-12`).
> **Doctrine:** only the verified effect counts.
>
> **Build:** `delonix 0.1.1`, develop @ `fbf324a0` (+17 commits since v0.1.1),
> `cargo build --release -p delonix-bin`, 2026-11-18. **No new tag/release.**
>
> **Environment:** i9-13900H (20 threads), 62 GiB, kernel 7.0.0-43, **rootless**,
> under light load (load1 ~3.7-4.2; publication threshold nproc/2 = 10 → host-net
> **publishable**). Comparison binaries: docker 29.10.0, podman 4.9.3.

## Side-by-side results

| Measurement | Round 1 (2026-11-12) | Re-measurement (2026-11-18, 0.1.1) | Verdict |
|---|---|---|---|
| **Host-net startup** (`run --rm`, median) | 106 ms | **71 ms** | ✅ no regression (faster) |
| — docker (bridge) | 3779 ms | 2343 ms | context |
| — podman (slirp) | 378 ms | 228 ms | context |
| **Isolated-network startup** (`run --rm --net iso`, median n=8) | 679 ms | **499 ms** | ✅ no regression |
| **Density** (attach p50, isolated SDN network) | flat up to N=100 | **147→138→143→139→140 ms** (N=1→100) | ✅ flat curve, 100/100 alive |
| **Crash recovery** `--restart always` + SIGKILL | restarts, Running | PID 1494231→1494555, **Running**, RESTARTS=1 | ✅ identical |
| **Crash recovery** `--restart no` + SIGKILL | does not restart | status **Crashed** | ✅ identical |

### Measurement notes
- **Host-net publishable** (`bench.sh --json`, load 4.24 < 10,
  `bench-hostnet.json`). The absolute differences vs round 1 (71 vs 106 ms) are
  due to *machine load* (round 1 ran at load 6.22), not code — what
  matters is **there being no regression** and the relative profile holding
  (delonix ≪ podman ≪ docker).
- **Density not publishable** due to 1 live owner container on the node
  (`bench-density.json` marks `publishable:false`), but the **shape of the curve is
  unambiguous**: flat attach p50 (~140 ms) from N=1 to N=100, 100/100 containers
  alive at each level, 0 failures. Confirms round 1 (no O(N) degradation).
- `nft_sets`/`nft_fwdeny` = `null` by design (rootless: the holder does not
  `nsenter` from the host) — same as round 1.
- The isolated measurements ran in their own `DELONIX_ROOT`/`DELONIX_NET_RUNTIME_DIR`
  (ephemeral sandbox), **without touching the owner's containers**.

## Verdict

**NO REGRESSION.** The clean supply chain (0 vulnerabilities) cost neither
performance nor correctness: startup, density and crash recovery hold
equal to or better than round 1. The 0.1.1 binary from develop @ `fbf324a0` is
fit to stamp the already-validated production scope (dev/CI, edge single-node, k8s
node via CRI, NgolaCloud platform).

**Outside this re-measurement** (unchanged since the readiness assessment): multi-node
HA (proven in other sessions, 2 paths), full CRI critest (needs
root/CNI/VM), and the cloud-native competitiveness gaps of waves 1-6
(`reports/cncf-acceptance-2026-11-12`).

## Artefacts
- `bench-hostnet.json` — host-net startup (publishable).
- `bench-density.json` — density curve 1→100.
