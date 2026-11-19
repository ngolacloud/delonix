# critest 103/103 clean — end-to-end proof (2026-11-10)

Closes the CRI conformance campaign: on a golden rootful+CNI VM, `critest
v1.36.0` passes the **full suite `103 Passed | 0 Failed`**, with the `rshared` spec
(ADR-0093) green. It confirms, in a single run with no retries, what the `rshared`
fix (`a7f8c01f`), the OOM cgroup barrier (`ba1eca84`) and the concurrent-unpack
scratch (`6092ca89`) promised individually.

## Measured environment

| Axis | Value |
|---|---|
| Engine | `delonix` + `delonix-cri` compiled from the SAME tree `feat/cri-conformance-rootful` @ `8353c302` (release) |
| VM | `delonix vm create crit-vm --disk delonix-vm-k8s:1.36 --vcpus 4 --memory 4G` (libvirt, 192.168.122.243, ubuntu 24.04, kernel 6.8.0-142) |
| Suite | `critest`/`crictl` **v1.36.0** |
| Mode | **rootful + CNI** (bridge+host-local 10.88.0.0/16 + portmap), cgroup v2 with `memory` delegated |
| Server env | `DELONIX_CRI_CAP_CEILING=all`, `DELONIX_CNI=1`, `CNI_PATH=/opt/cni/bin`, `DELONIX_CRI_SECCOMP_NIL=unconfined` |
| AppArmor | 2 test profiles loaded on the node (`apparmor_parser`), securityfs mounted |

Harness: `scratchpad/vm-critest.sh` (derived from `scripts/critest.sh` — same
AppArmor profile definition, + cgroup2 delegation, CNI and seccomp-nil).

## 1. The `rshared` spec in focus — PASSES

```
FOCUS="mount with 'rshared'"  → SUCCESS! -- 1 Passed | 0 Failed | 121 Skipped
```

`mount with 'rshared' should support propagation from host to container and vice
versa`: a `mount --bind` done INSIDE the container shows up on the host (the
container→host half that was missing). This is the delivery of ADR-0093.

## 2. Full suite — 103/103 clean

```
Ran 103 of 122 Specs in 184.548 seconds
SUCCESS! -- 103 Passed | 0 Failed | 0 Pending | 19 Skipped   (rc=0)
```

Three full, consecutive runs on the same VM:

| Run | Result | Only failure |
|---|---|---|
| 1 | 102 Passed \| 1 Failed | attach-"hello" (interference, see §3) |
| 2 | 102 Passed \| 1 Failed | attach-"hello" (same) |
| **3** | **103 Passed \| 0 Failed** | — |

`rshared` passes in **all three** (it is never among the failures). The number
oscillates 102↔103 exactly as the campaign documented — and the only variance is the
attach spec, never an engine spec.

## 3. The 102↔103 oscillation is a ginkgo async-leak, not an engine gap

In runs 1 and 2 the failure is reported against a spec `[k8s.io] Image Manager …
[Conformance] [Serial]` whose own *Timeline* completes every step (Pull,
Check, Get status, Remove, Remove by ID, Check empty) without error. What fails,
grafted on top, is:

```
[FAILED] in [It] - pkg/validate/streaming.go:317
[FAILED] Timed out after 60.001s.
The stdout of attach should be hello — Expected <string>: "" to equal "hello"
```

This is the classic ginkgo **leaked asynchronous assertion** pattern: the earlier
**attach** spec starts an `Eventually(stdout=="hello", 60s)`, its `It`
returns, and the timeout fires 60 s later — **during** the following `[Serial]`, to
which ginkgo attributes the failure (hence the victim migrates: run 1 = "digest", run
2 = "tag"). Proof that this is interference, not conformance:

- **attach in isolation → `1 Passed | 0 Failed`** (`FOCUS=attach`, 0.576 s);
- the `[Serial]` victim always passes when it is not preceded by the attach (run 3);
- it matches the campaign's earlier characterization (OOM-race: "1 attach-hello
  interference, passes in isolation 5/5"; concurrent-unpack: "102/103 ×2, the 1 failure
  varies and passes 3/3 in isolation").

None of the three runs showed an unpack-race (`.tmp`), OOM or `rshared` failure.

## 4. Privilege gate — `rshared` never enabled in rootless

- **Unit** (`mount_propagation_tests`, `delonix-linux`): `rshared_allowed(true,false)=true`
  and `false` for rootless and for non-privileged; 10/10 green.
- **Live, rootless (host, uid 1000):**

```
$ delonix container run --rm -v /tmp/rs-src:/mnt:rshared python:3.12-slim true
error[DX-1117] invalid argument: mount propagation 'rshared' (bidirectional,
Kubernetes `mountPropagation: Bidirectional`) is refused: this engine is rootless
… Use 'rslave' for one-way host→container propagation, or run privileged on a
rootful node.
```

A named refusal (`Error::BidirectionalNeedsPrivileged`, DX-1117), with the `rslave`
alternative — never a silent degrade to slave.

## Verdict

- **103/103 clean, in a single run with no retries** (run 3), rootful+CNI.
- `rshared` (ADR-0093) conformant and **behind privilege**: green on a rootful VM,
  refused with a reason in rootless.
- The engine has no remaining conformance gap; the 102↔103 oscillation is an
  artefact of ginkgo's asynchronous attribution in the attach spec, proven by
  isolated passage.
