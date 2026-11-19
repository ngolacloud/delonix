# Delonix — critical-production readiness: Round 1 (rootless host)

> Objective: find where Delonix **breaks** for critical production —
> performance, runtime, failure recovery, density — compared
> against Docker and Podman. Doctrine: only the **verified effect** counts; `exit 0` is
> not enough.
>
> **Environment:** shared host i9-13900H (20 threads), 62 GiB, kernel 7.0.0-42,
> **rootless** (cgroup2 cpu/memory/pids delegated; io/cpuset not). **Under load**
> (load 6.22, owner's containers running) — high variance; the medians are
> directional, not clean-bench numbers. Binaries: delonix 0.1.0 (release),
> docker 29.9.3, podman 4.9.3.

## Findings (ordered by production severity)

### P1 — Isolated networking is the dominant cost (scale bottleneck)
Container startup `run --rm alpine true` (median, n≈8-10):

| Engine | Config | Median |
|---|---|---|
| docker | bridge (netns+veth+NAT) | **3779 ms** |
| podman | slirp4netns | **378 ms** |
| **delonix** | **`--net host`** (default) | **106 ms** |
| **delonix** | **isolated network `benchnet`** (SDN netns) | **679 ms** |

- Delonix's "fast" number (106ms) comes from **not isolating the network** (`--net host`
  is the default). With real isolated networking, startup is **~6× slower** (679ms) and
  **slower than podman slirp** (378ms), though well ahead of docker.
- Real `pod create` = **~1.6s** (1.0s to set up the pod's SDN netns + 0.6s the
  container). **Setting up the SDN netns is the dominant cost.**
- **Production impact — MEASURED, and better than feared:** workloads with isolated
  networking pay ~300ms-1s/pod, BUT the SDN path **parallelises and is robust**:
  - N=20 concurrent isolated → **20/20 success, 0 failures**;
  - N=50 concurrent isolated → **50/50 success, 0 failures** (subnet /16, huge
    IP pool);
  - sequential scales linearly (~300ms/container).
  So it is **not a correctness break — it is a *throughput* cost** (per-pod startup
  latency). It is the #1 **optimisation** target (reduce the SDN netns setup),
  not a production blocker.

### P2 — WITHDRAWN: it was a measurement error, NOT a Delonix bug
The initial hypothesis (cleanup leak under churn) **was not confirmed**. What looked like
a leak/concurrent failure was **the wrong cleanup verb in my tests**:
`delete container <name>` is the **declarative** surface and **correctly rejects**
imperative containers (`container run`) with `DX-1000` ("Container is the
imperative surface — use `container ps`; declarative containers live in
`kind: Pod`"). The correct way for imperatives is **`container rm -f`** (force if
running). Because my tests suppressed the error, the containers were left behind →
name collisions → the "concurrent failures" and the "leak". **Delonix's
imperative/declarative separation works as designed.** Doctrine lesson:
never suppress cleanup errors.

### P3 — Host-net density: linear scaling, no collapse ✅
| N containers | total | ms/container |
|---|---|---|
| 1 | 220 ms | 220 |
| 20 | 1418 ms | ~70 |
| 50 | 4016 ms | ~80 |
Steady-state ~80 ms/container up to N=50, **with no degradation**. Healthy for host-net.
(Density with isolated networking still to be measured — see P1.)

### P4 — Runtime recovery: WORKS ✅
- **Crash detection:** SIGKILL of the container PID → state **"Dead"** captured.
- **Restart policy:** `--restart always` → after SIGKILL it restarted (PID 928650→929001,
  restart-count 1), via **per-container detached supervisor (daemonless)**.
  `no|on-failure[:max]|always|unless-stopped` supported.

### P5 — Post-reboot recovery: PROVED in a VM ✅ (but with a bug)
Proof in a libvirt VM (golden `delonix-vm-k8s:1.36`, delonix 5.0.1, root):
1. `container run -d --restart unless-stopped survivor` + `system boot enable` →
   generates the system unit `delonix-boot-survivor.service` (enabled, in
   `multi-user.target.wants`);
2. **REAL reboot** of the VM (`systemctl reboot`);
3. after boot: `container inspect survivor` → **`"status": "Running"`**, PID of
   `sleep 3600` alive, same id — the unit restarted it (`journalctl`: "Started
   delonix-boot-survivor.service"). **Post-reboot recovery WORKS.**

### BUG (confirmed on develop) — systemd unit with invalid `Restart=`
`boot.rs:264` writes `Restart={policy}` with the Delonix policy **verbatim**:
```
[Service]
Type=forking
Restart=unless-stopped   ← systemd: "Failed to parse service restart specifier, ignoring"
```
`unless-stopped` and `on-failure:N` **are not valid systemd specifiers**
(only `no|always|on-failure|on-success|on-abnormal|on-abort|on-watchdog`) → systemd
ignores them. **Impact:** boot recovery still works (the `ExecStart`
runs `delonix container start`, whose supervisor applies the real policy), **but
systemd does not restart the supervisor if IT dies midway** for `unless-stopped`
containers. **Fix:** map policy→systemd (`unless-stopped`/`always`→
`Restart=always`, `on-failure[:max]`→`Restart=on-failure`).

### NON-bug (caught while verifying) — `ps` missing on the VM
`delonix ps` gave "unrecognized subcommand" on the VM — but that is the **old
version** of the golden image (5.0.1, before the root `ps` alias from #930), **not**
a listing bug. `container inspect` confirms the container is Running.

### Quality notes (measurement pitfalls caught by the doctrine)
- `--net bridge` **does not exist** (error `DX-4303`) — it produced times of 25-81ms that
  were **fast-error**, not startups. Always verify the functional result.
- `pod create` at "21ms" was a **name conflict** (fast-error); the real figure is ~1.6s.
- **Polish bug:** the `DX-4303` message has a double negative — "no such ingress
  network 'bridge' does not exist".

### P6 — CNI parity (root + CNI): CONFORMANT ✅
Via root+CNI (real bridge + veth + IPAM, routable pod IP), through
`delonix-cri` running root in a `--privileged` container (no VM):

| CRI/CNI specs (critest v1.36.0) | Result |
|---|---|
| `PortForward` + `port mapping` | **2/2 PASS** |
| `networking` + `PortForward` + `port mapping` + `DNS` + `HostNetwork` | **6/6 PASS, 0 failures** |

Matches the **103/103** of the full critest (previous conformance campaign):
the CNI path — bridge/veth/IPAM, port-forward, port-mapping, DNS, host-network —
is conformant. Plugins in `/opt/cni/bin` (bridge, dhcp, firewall, bandwidth…).
**CNI parity with Docker/Podman: yes** on the k8s conformance plane.

### BUG — CNI harness broken by ADR-0086 (same class as BUG-0001)
`scripts/critest-root-cni.sh` does `cargo build -p delonix-cri` but the binary
moved to `delonix-cri-bin` (ADR-0086) → **the script does not start** without a patch.
Same defect as the `Makefile` (BUG-0001 of the 0.1.0 e2e). To be fixed in the repo.

## To do (needs a root VM — real reboot)
- **R2-reboot:** prove `system boot enable` after a REAL reboot (the only test
  that requires a VM; the shared host does not reboot). Mechanism present (P5).
- **R1b:** density with **isolated networking** (the P1 bottleneck at scale — ~1s×N).
- **R2b:** dedicated churn test to quantify the P2 leak.

## Round 1 verdict (host + VM) — corrected after rigorous testing
- **Where it is strong (measured):** fast host-net startup (106ms), linear density
  (up to 50, no collapse), **robust and parallel isolated networking** (50/50 concurrent,
  0 failures), restart policies, **recovery from crash AND from real reboot**,
  **CNI conformance 6/6 + 103/103**, build ≈ docker. Correct imperative
  (`container rm`) / declarative (`delete …`/`kind: Pod`) separation.
- **Where it breaks / to resolve (only TWO, both small):**
  1. **systemd `Restart=` BUG (medium):** the `system boot enable` unit uses the
     Delonix policy as an invalid systemd specifier (`unless-stopped`,
     `on-failure:N`). Boot recovery works; the mapping is missing.
  2. **Harness BUG (low):** `critest-root-cni.sh` references `delonix-cri`
     (ADR-0086 → `delonix-cri-bin`).
- **Cost (not a break):** isolated networking ~3-6× host-net per pod — a
  **throughput optimisation** target, not a blocker.
- **Parity:** ahead of docker in latency; philosophically on par with podman
  (rootless/daemonless/pods) and with proven post-reboot recovery; conformant in
  CNI/k8s.
- **Hypotheses WITHDRAWN as measurement error (doctrine "verify, don't
  suppress"):** the "cleanup leak" (P2) and the "concurrent failures" were the wrong
  delete verb in the tests, not engine defects.

## Conclusion
For critical production, Delonix proved **surprisingly solid**:
it recovers from crash and from real reboot, isolated networking scales without failures, and CNI is
conformant. The two real defects are *polish* (systemd `Restart=`, package name
in the harness). The only **optimisation** axis with weight is the startup
latency with isolated networking (throughput), not a correctness break.
