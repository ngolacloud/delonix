# `delonix container run` — parity defects and fix prompts

**Source:** live functional test, flag by flag, of `delonix container run`
against Docker/Podman behaviour.
**Environment:** delonix 0.1.1 (commit `64ec5a498`), rootless host · daemonless,
delegated cgroup2, images `alpine:3.19` + `busybox`. Date: 2026-11-19.
**Method:** observed effect (not just `--help`) — reading host cgroups via
`/proc/<pid>/cgroup`, inodes `/proc/self/ns/*`, `/proc/self/status`
(CapEff/Seccomp/NoNewPrivs), native HTTP/TCP probes and end-to-end `curl`/`wget`.

**Verdict:** ~95 flags exercised; core in strong parity (and sometimes more
secure than Docker). 4 defects where the flag is accepted but inoperative/broken.

Order of attack (severity): security → functionality → parity → DX.

---

## Cross-cutting notes (for any of these sessions)

- **Rootless and sudo-less** host; validate with the rootless harnesses.
- `/tmp` and the scratchpad are **ephemeral between turns** — store artefacts outside `/tmp`.
- Binary tests: `cargo test -p delonix-bin --bins` (the `delonix-bin` package has no lib).
- **develop-GitFlow** process; the `push`/`merge` of a PR is the owner's.
- Do not weaken security defaults when fixing (1, 3 and 4 touch hardening).

---

## BUG-1 — `--knows` / `--knows-none` do not apply DNS isolation  _(High · security)_

**Symptom.** `run --knows-none` and `run --knows X` are accepted and persisted but do not
isolate: the container keeps resolving and connecting to all peers on the network.

**Evidence.** On a dedicated network:
```
run --net N --knows-none alpine nslookup srv  → Address 10.239.102.249 (resolved)
                                   wget http://srv/ → SRVBODY (connected)
run --net N --knows srv ...  → srv=OK  but  srv2 also responded (K2LEAK leak)
```

**Root cause.** The `dns_knows` field IS written to the record
(`crates/contexts/delonix-compute/src/app/run.rs:583-586`,
`crates/contexts/delonix-compute/src/app/record.rs:647`) but **no resolver
consumes it**. The container name resolver
(`crates/contexts/delonix-networking/src/domain/dns.rs` and
`crates/adapters/delonix-sdn/src/`) does not filter responses by the allowlist.

**Fix.** When answering A/AAAA, filter by the `dns_knows` of the container making the
query: `None` = resolve everything (default); `Some([])` = resolve nothing (`--knows-none`);
`Some([...])` = only those names + itself.

**Done when.**
1. `--knows-none` → `nslookup <peer>` returns NXDOMAIN and `wget` fails.
2. `--knows A` resolves `A` but returns NXDOMAIN for `B`.
3. An e2e/regression test that PROVES the isolation (not a blind test — see the
   `nft ip-vs-inet` trap, where harnesses counted in the wrong family and did not see the leak).

**Gates.** No new flag (no new pt.po). Do not regress normal resolution (without `--knows`).
Rootless gates. **Agent:** general-purpose, high tier (resolver/SDN).

---

## BUG-2 — `--pod <name>` does not join the pod's netns  _(High · functionality)_

**Symptom.** `run --pod <name>` gives the container its own IP instead of sharing the
pod's, and a non-existent pod is accepted without error.

**Evidence.** Pod `vp` with IP `10.239.0.2` serving on `:8080`:
```
run --net N --pod vp busybox …  → hostname -i = 10.239.9.136  (≠ 10.239.0.2)
                                   wget 127.0.0.1:8080 → Connection refused (SEPARATE-NS)
run --pod naoexistexyz busybox echo joined  → rc=0  (should fail)
```

**Root cause.** Imperative join in `bins/delonix/src/cmd/container.rs:3416-3424`;
netns/error handling in `container.rs:3931-4058`. The "pod does not exist" guard
(`:4056`, `joined.is_err() && !pod::exists`) does not fire — the join "succeeds" without
connecting to the netns. The neighbouring `--net container:<name>` path (`container.rs:3931`)
works and does the correct setns.

**Fix.** Align `--pod` to reuse the same setns mechanism as
`--net container:` (enter the netns of the pod infra/member), and make the
non-existent pod guard abort with a clear error.

**Done when.**
1. `run --pod vp` → `hostname -i` == pod IP and `wget 127.0.0.1:<pod-port>`
   responds (`JOINED-NS`).
2. `run --pod nonexistent` → clear error, `rc≠0`.
3. Test covering a successful join + a missing pod.

**Gates.** Podman `run --pod` parity. Do not regress `--net container:`. Rootless gates.
**Agent:** general-purpose, high tier.

---

## BUG-3 — `--pid host` / `--host-pid` fail in non-privileged rootless  _(Medium · parity)_

**Symptom.** They fail at startup; they work with `--privileged`. Docker allows
`--pid=host` without privilege.

**Evidence.**
```
run --pid host busybox …  → delonix: could not bind '/proc/sys' for read-only:
                              EINVAL: Invalid argument; aborting
run --pid host --privileged busybox …  → 905 host pids visible  ✓
```

**Root cause.** The hardening remounts `/proc/sys` read-only
(`crates/contexts/delonix-compute/src/app/run.rs:615`
`default_readonly_paths: ["/proc/sys"]`, applied in
`crates/adapters/delonix-linux/src/lib.rs` ~1625-1660 / 2034-2047). With the host
PID-ns shared (`host_pid` in `bins/delonix/src/cmd/container.rs:136/1369-1377`), the
`/proc` is the host's and the rootless user cannot remount `/proc/sys` ro → EINVAL.

**Fix.** When `host_pid`, omit `/proc/sys` from `default_readonly_paths` (or
make that remount best-effort with a warning — never abort).

**Done when.**
1. `run --pid host busybox sh -c 'ls /proc | grep -c "^[0-9]"'` → >50 in rootless without
   `--privileged`.
2. `--host-pid` likewise.
3. Normal container (without `--pid host`) keeps `/proc/sys` ro (no regression of the hardening).

**Gates.** Do not weaken the default. Rootless + security gates. **Agent:**
general-purpose, high tier (namespaces/mounts — consider the Rust reviewer panel).

---

## BUG-4 — `--wait` ignores the probe defined on the CLI  _(Medium · DX)_

**Symptom.** `--wait` + `--health-cmd`/`--health-http`/`--health-tcp` is rejected,
even when the probe (on its own) works and reaches `healthy`.

**Evidence.**
```
run -d --health-http :8080/ --wait busybox <srv>  →
  error[DX-1000]: --wait: image 'busybox' declares no HEALTHCHECK — nothing to wait for
(identical for --health-tcp and --health-cmd; the probes WITHOUT --wait → healthy)
```

**Root cause.** The `--wait` validation in
`bins/delonix/src/cmd/container.rs:4242-4252` only considers `--no-healthcheck` and the
image's HEALTHCHECK, ignoring the CLI probes. The path at
`container.rs:6687` already considers `--health-cmd`+image — align the condition at `:4252`
to accept any effective probe (CLI `--health-cmd/http/tcp` OR image) before
rejecting.

**Done when.**
1. `run -d --health-http :8080/ --wait --wait-timeout 40 busybox <srv>` blocks and
   returns `rc=0` with state `healthy`.
2. Likewise `--health-tcp` and `--health-cmd`.
3. `--wait` with no probe at all keeps the current clear error.

**Gates.** Consistent error message. No new pt.po (existing flags). Test gate.
**Agent:** general-purpose, medium tier (logic localized in the CLI).

---

## Safe divergences (NOT bugs — do not fix)

- **`-p` safe-by-default**: binds to `127.0.0.1`, not `0.0.0.0` (safer than Docker;
  `0.0.0.0:p:p` explicitly widens it). Intentional behaviour.
- **`--sysctl` refuses non-namespaced `net.*`** with "not namespaced; refused (affects
  the host)". Namespaced keys (`kernel.shmmax`) apply. Conservative, on the safe side.
- **`/sys/fs/cgroup` is not mounted inside the container** (rootless; Docker mounts it ro).
  The limits ARE applied — verified in the host cgroup.
- **Host SKIP:** `--gpus all` requires NVIDIA CDI (`--gpus dri` works);
  `--no-userns` is not possible in rootless (userns is mandatory for the mapping).
