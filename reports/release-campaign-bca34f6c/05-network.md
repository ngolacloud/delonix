# 05 — Network, scopes/cgroups and isolation

SUT: `delonix 5.1.1` @ `bca34f6c3`, **rootless**. Test network: `relc-net` (`dlxn93807333`, 10.206.0.0/16).

| # | Case | Independent verification | Observed | State |
|---|---|---|---|---|
| NETWORK-1 | topology | `ip`, `network ls` | network created, bridge `dlxn93807333`, subnet /16; bridge lives **inside the rootless netns** (slirp4netns), invisible in the host root netns — expected under rootless | **PASSED** |
| NETWORK-2 | published port + external HTTP | `curl` + nginx log | `HTTP 200`; request seen in the log coming from `10.0.2.2` (slirp gw) | **PASSED** (see 03) |
| NETWORK-3 | **re-publishing a port while running** | `describe` PID + `curl` | PID `1098119` **stable** across `--publish-add 18082` (new→200, old→200) and `--publish-rm 18081` (old→000, new→200). **Process never restarted.** | **PASSED** |
| NETWORK-4a | DNS by name | `getent`, wget | `relc-a` resolves `relc-b`→10.206.61.43; alpine wget fetches the nginx page by name | **PASSED** |
| NETWORK-4b | isolation between namespaces | wget by name **and by IP** | teamA↔teamA reachable (control); teamB→teamA **blocked by name AND by direct IP** (nft drop, not just DNS) | **PASSED** |
| SCOPE-1 | cgroups v2 delegate effective | `/sys/fs/cgroup` + `system resources` | `memory.max`/`cpu.max` effective (see 03); `--cpuset` accepted-and-ignored (DLX-RES-001), io refused exit 69 (DLX-RES-002) — the engine declares what it does not apply | **PASSED** |
| SCOPE-2 | same NAME in different namespaces | `ps`, `system namespace ls` | `relc-dup` coexists in teamA and teamB; `namespace ls` counts 1+1; correct resolution | **PASSED** |

## Design notes confirmed in the code

- **`default` is the shared namespace by design** (`firewall.rs:780-789,3172-3199`): isolation
  is enforced only **between non-`default` namespaces**. A first test (teamB→`default`)
  gave "reached"; after reading the code, I re-tested the correct pair (teamA↔teamB) — blocked.
  *Recorded to avoid it being read as a defect: it is not.*
- **Robust isolation:** the teamB→teamA block holds even **by direct IP**, so it does not
  depend on DNS scoping — it is `nft` dropping the new connection (consistent with `network.rs:81`
  "same-namespace accept, cross-namespace new connections dropped").
- **Correct conflict:** `network rm` of a network in use → `error[DX-5307] conflict ... in use by container` (exit 5).

## Daemonless model and "engine restart"

The §8 requirement "repeat after engine restart to verify persistence" is **N/A in the
Docker sense**: Delonix is **daemonless** — there is no engine to restart. The dataplane (nft DNAT + hostfwd
over slirp) persists independently of the process that created it; the relevant persistence
(surviving the exit of the control process) was proven in 03 (stop/start, republish).

## Cleanup

All `relc-*` containers and `relc-net` removed; teamA/teamB namespaces disappeared once
empty (documented behaviour). `kaeso-*` preserved.
