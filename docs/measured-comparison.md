# Docker × Podman × Delonix — measured comparison

> **What this document is.** A table where **every line was run**, on all three
> tools, on the same machine, on the same day. It does not decide who is better: it
> shows what each one does, including where Delonix is **worse**.
>
> **What it is not.** A feature analysis read off `--help`. That already exists in
> [`paridade-docker-podman.md`](paridade-docker-podman.md), which in fact starts by saying it
> measured nothing. Here, a cell with no measurement says `not verified` — never a claim.

| Field | Value |
|---|---|
| Date | **2026-08-13** |
| Where | libvirt VM `pari053`, 4 vCPU / 4 GiB, Ubuntu 24.04.4, kernel 6.8.0-136-generic |
| Why a VM | The development host **has no docker daemon**, and installing one there would mean touching a machine with production running on it. Without all three in the same place, two columns would be `not verified` and the comparison would be worthless. The VM was created by the engine itself (`delonix vm create`). |
| Docker | 29.1.3 (Ubuntu's `docker.io`, active `systemd` daemon, user in the `docker` group) |
| Podman | 4.9.3 (rootless) |
| Delonix | **0.1.0** (rootless), official release binary from the tag, at `/usr/local/bin/delonix` |

> **Setup note, and it matters for the fairness of the comparison.** The Delonix binary has
> to sit at `/usr/local/bin/delonix`: the golden image applies an AppArmor profile to it, and with
> `kernel.apparmor_restrict_unprivileged_userns=1` (active, measured) `unshare` is denied at
> any other path. An earlier battery unknowingly measured a `run` that was blowing up.
> Every tool has to be measured configured the way it is meant to be, or the table lies in favour of
> whoever happens to be well installed.
>
> The images (`alpine:latest`, `nginx:alpine`) were local on all three before the first
> stopwatch, and each engine runs with **its own** network default.

---

## The table

| # | Capability | Docker 29.1.3 | Podman 4.9.3 | Delonix 0.1.0 | How it was measured |
|---|---|---|---|---|---|
| 1 | Resident daemon | **Yes** — `systemctl is-active docker` → `active` | No — 0 processes | No — 0 processes | `systemctl is-active docker`; `pgrep -c podman`; `pgrep -cf 'delonix (netns\|serve)'` |
| 2 | Runs without privilege | No without the `docker` group/daemon: `permission denied … unix:///var/run/docker.sock` | Yes (userns, `id -u` → 0) | Yes (userns, `id -u` → 0) | `sudo -u nobody docker ps`; `<eng> run --rm alpine id -u` |
| 3 | Basic `run` | `ok` | `ok` | `ok` | `<eng> run --rm alpine echo ok` |
| 4a | **`run --rm` latency, each one's default** (median of **10**) | **208 ms** (bridge) | **268 ms** (slirp) | **91 ms** (host) | `date +%s%N` around `<eng> run --rm alpine true` |
| 4b | **Latency with isolated per-container network, network plane ALREADY up** (median of 10) | **208 ms** (the same — the bridge is ALREADY isolated) | **268 ms** (the same) | **216 ms** (`--net <network>`) | same as above, with a sentinel container on the network keeping the refcount ≥ 1 |
| 4c | **Same, with the network plane coming up FROM SCRATCH on each run** | 208 ms (the daemon and `docker0` are permanent — it doesn't pay the cost) | 268 ms (no shared plane — it doesn't pay the cost) | **344 ms** | no sentinel: the last container's `--rm` drops the refcount to 0 and `release` runs `teardown()` of the pin/control/slirp |
| 5 | Exit code of a `-d` container | `42` | `42` | `42` (`Exited (42)`) | `run -d … sh -c 'exit 42'`, then `inspect -f '{{.State.ExitCode}}'` / `container ls -a` |
| 6 | **Changing ports hot, without recreating** | **No** — `unknown flag: --publish-add` | **No** — `Error: unknown flag: --publish-add` | **Yes** — `port 19312->80/tcp hot-published`, **PID 4118 → 4118**, `container port` lists both | `<eng> update --publish-add 19312:80 <c>`; PID read via `describe` before and after |
| 7 | **stdin from a pipe, NO flags** | **Doesn't arrive** — empty output | **Doesn't arrive** — empty output | **Arrives** — `oi` | `echo oi \| <eng> run --rm alpine cat` |
| 7-ctl | same, **with `-i`** | `oi` | `oi` | (has no such flag; already arrives without it) | `echo oi \| <eng> run -i --rm alpine cat` |
| 7b | **TTY when the CALLER has one, no flags** | **No** — `NOTTY-IN`/`NOTTY-OUT` | **No** — `NOTTY-IN`/`NOTTY-OUT` | **Yes** — `TTY-IN`/`TTY-OUT` | all three under `script -qec … /dev/null`, with `test -t 0`/`test -t 1` inside the container |
| 7c | **Forcing a TTY when the caller does NOT have one** | **Yes** (`-t`) — `TTY-OUT` | **Yes** (`-t`) — `TTY-OUT` | **No, and there is no flag** — `NOTTY-OUT`; `-t` → `error: unexpected argument '-t' found` | `echo x \| <eng> run -t --rm alpine sh -c 'test -t 1 …'` |
| 8 | Declarative microVMs | No (`docker vm` does not exist) | `podman machine` — «Manage a virtual machine», a VM **for** running containers, not a workload | `delonix vm` — «Declarative microVMs: create/ls/stop/rm/status» | `<eng> vm --help` / `podman machine --help` |
| 9 | Native `docker-compose.yml` | `not verified` — `unknown command: docker compose` (the plugin is not installed; it exists as an official plugin) | `not verified` — `looking up compose provider failed` (`podman-compose` is not installed) | Yes, built in — «Native `docker-compose.yml` support (up/down/ps/logs/config)» | `<eng> compose version` / `delonix compose --help` |

---

## Where Delonix wins, and why

**Default latency: 91 ms versus 208 and 268** (line 4a) — 2.3× faster than docker,
2.9× than podman. It is not tuning, it is architecture: there is no daemon to contact and no
service to wake up. `run` is a process that does `clone()` and exits. **But read line 4b before
quoting this number** — part of the margin is the default being `host`, not an isolated
network.

**Hot reconfiguration (line 6) remains the fundamental difference.** On docker and
podman, changing a port forces recreating the container; here the dataplane does not belong to the
process lifecycle, and the PID proves it — **4118 before and 4118 after**, with both ports
active. It is the only line where the other two answer `unknown flag`.

**stdin and terminal arrive by default** (lines 7 and 7b). Docker and podman **detach**
stdin: without `-i` `cat` receives nothing, and without `-t` there is no terminal even when the caller
has one. Delonix inherits both, which is why it has no flags for them — the case they serve
is already the normal behaviour.

## Where Delonix loses

**The SAFE configuration is the slow one, and the cost is in the cold start of the network plane.** The
fair comparison is line 4b, not 4a: docker's `bridge` **already** gives the container its
own netns and its own IP, and Delonix's `host` default does not. There, Delonix does
**216 ms** versus docker's 208 — a technical tie, not the wide loss a first
reading of this bench suggested (see the method note below). But line 4c shows where it hurts:
when the network plane has to come up from scratch, **344 ms**, and docker/podman never
pay that cost because the daemon and `docker0` are permanent. On a node that bursts containers,
the cost dilutes; on an idle node that runs a container every now and then, it is always paid.

Broken down across the three lines: **91 ms** base, **+125 ms** of real attach to the SDN (veth,
IPAM, ruleset, and the `nsenter … ip netns exec` re-exec that rootless requires), **+128 ms** of
rebuilding the pin/control/slirp when the refcount hit zero.

**Method note, and it is the same trap this page calls out further below.** The first
run of this bench measured 333 ms and concluded «the worst of the three, 60% worse than docker». The
number was right and the conclusion wrong: with `--rm` and one container at a time, the network
refcount reaches **zero** between iterations and `release` runs `teardown()` of the whole
network plane — each run was rebuilding it. It was not the attach being measured, it was
attach **plus** bring-up. A measured number is not an understood number, and the difference
between the two was 128 ms attributed to the wrong place.

**Cannot force a TTY** (line 7c). When the caller has no terminal — CI, a pipe,
a cron — `docker run -t`/`podman run -t` still give the container a pty, and Delonix has
no way to (`-t` is refused as an unknown argument). It matters for whatever changes
behaviour based on `isatty`: colours, progress bars, a `read` that expects a terminal.
Narrow, but real, and **fixable** — the engine already has the pty machinery from console mode.

## Why the 2026-08-10 battery was WITHDRAWN

The earlier version of this table was measured on 2026-08-10, and its
latency numbers were: docker **1,406 ms**, podman **1,351 ms**, delonix **640 ms**. In this run,
with **the same docker 29.1.3 and podman 4.9.3**, on the same distro and kernel, all three
gave **208 / 268 / 89 ms** — all roughly **6× faster**.

Three engines all speeding up sixfold at once is not an improvement in any of them: it is the
bench. Those numbers were measuring the VM's contention at that moment, not the tools. The
qualitative ratio survived (Delonix wins on default), the absolute values were worthless,
and line 4b — the comparison that matters — never got run.

Two claims from that battery also did not survive:

1. **«interactive stdin does NOT exist»** was the **opposite** of the real behaviour, and is today
   line 7. The defect was method: that cell was not run like the others, it was a
   `run --help | grep -E '^\s+-i,'`. It measured the absence of a **flag** and concluded the absence
   of the **capability** — in the very document that promises every line was run.
2. **The «two findings» were one.** The first said `--net host` with `-p` fails because the
   combination is contradictory; it isn't — `--net host` is the **default** mode and `-p` over
   it is the normal, supported slirp-per-container path
   ([`container.rs:3466`](../crates/delonix-runtime-bin/src/cmd/container.rs)). What made
   `add_hostfwd` fail was the second finding: an orphaned `slirp4netns` holding the port. The
   symptom was filed as an independent bug, with reasoning the code contradicts.

## A finding caught while setting up this bench

**`delonix vm create` has no way to size the disk** — there is no `--disk-size` or
equivalent, and the overlay inherits the golden image's 2.4 GiB root. Installing `docker.io` and `podman`
inside filled it to 100% and `apt` died midway with `No space left on device`, leaving
`dpkg` unconfigured. The workaround was to stop the VM, `qemu-img resize +14G` by hand and boot it
back up (cloud-init's `growpart` does the rest on the next boot). It is a real gap, with no
new privilege boundary involved.

Verified in passing, and it is **not** a bug: the cgroup delegation warning goes to **stderr**, not
polluting stdout. A `run … | cat` returns exactly the container's output.

## Recount of 2026-08-25 — done, and what delayed it

**Refused on the first attempt**, and the refusal counts as a record. The host
had a `load average` of **55 to 72** on **32 threads** and the raw runs gave
docker **5080 ms** and podman **4149 ms**. That is not anyone's regression — it is
the bench, and it is what withdrew the 2026-08-10 battery. Hours later, with the host at
**load 3.3**, the same bench passed and the numbers below are from it.

| Field | Value |
|---|---|
| Date | **2026-08-25** |
| Where | development host — AMD Ryzen 9 8940HX (32 threads), 31 GiB, kernel 7.0.0-30 |
| Delonix | rootless, isolated root and runtime-dir |
| Docker | 29.7.2 · Podman | 4.9.3 |
| Harness | `scripts/bench.sh` (new) |

| # | line | docker | podman | delonix |
|---|---|---|---|---|
| 4a | `run --rm`, each one's default (median of 10) | **299 ms** (min 267, max 351) | **277 ms** (min 270, max 296) | **80 ms** (min 69, max 92) |
| 4b | isolated per-container network, plane ALREADY up | — | — | **~300 ms** (297 / 297 / 392) |
| 4c | same, with the plane coming up FROM SCRATCH | — | — | **~520 ms** (498 / 545) |

**The ratio survived, which is what these tables teach you to look at.** On default,
Delonix remains ~3.5× faster; with isolated networking it remains a technical tie
with docker (300 against 299), exactly as the 2026-08-13 battery
described with different absolute values (216 against 208).

**What was NOT attributed:** lines 4b/4c are larger than on 2026-08-13
(300 versus 216, 520 versus 344). The hardware is faster and the bench's root is
isolated, so the production node's density is not a factor. The cause was not measured, and
so it is not named.

### Two method errors caught while doing this recount

1. **The harness was comparing one cold engine against two warm ones.** Delonix's root
   is isolated and starts EMPTY, so the first run was paying for the pull and
   extraction of the image — 6,275 / 7,408 / 7,359 ms on a virgin root, and a sample of
   **16,782 ms** in the first version of this bench. The median absorbed it, which is
   precisely the danger: with `--runs 1` the published number would be sixteen
   seconds. `bench.sh` now explicitly warms up all three.
2. **The first measurement of line 4c did not measure line 4c.** It was done with
   `--net host`, which is the default and **raises no network plane at all** — it gave
   74 ms against 63-78 for the control, i.e. it measured 4a three times. 4c needs
   `--net <custom-network>`, which is what forces the holder to come up.

## The refused attempt, in detail

This table is from **2026-08-13**. A performance table that has aged
weeks deserves to be re-measured, and it was tried — the **2026-08-25** recount
above.

**It was refused, and the refusal is the result.** The development host had, at the
moment of the attempt, a `load average` of **55 to 72** on **32 threads** — more than
double capacity — with 189 containers running and Rust compiles happening in
parallel. The first raw runs gave docker **5080 ms** and podman
**4149 ms**, against this table's 208/268.

That is not a regression of anyone's: it is the bench, and it is **exactly** what
withdrew the 2026-08-10 battery. The proof is in the dispersion, which a median would
hide — three forced runs gave:

| engine | median | min | max |
|---|---|---|---|
| docker | 540 ms | 509 | **5,660** |
| podman | 628 ms | 584 | 709 |
| delonix | 169 ms | 144 | **7,922** |

A factor of **11×** between docker's fastest and slowest run, and
**55×** on delonix. Publishing the median of this would be publishing noise with a number
in front of it.

**The lesson became code.** `scripts/bench.sh` characterizes the bench
BEFORE measuring anything and **refuses to run** (exit 3) when the
`load average` passes half the threads, saying why. `--force` still runs
and marks the result as NOT PUBLISHABLE. It always publishes the dispersion next
to the median, and the **node density** — which is part of the bench, not a
detail: two calls in the attach path are `nft` dumps that grow with the
number of containers, and the battery below ran on an **empty** node.

Re-measuring properly needs the same thing the 2026-08-13 battery used: an
idle, dedicated machine. It remains **to be done**, not turned into a number.

## What remains unmeasured

- `docker compose` and `podman-compose` (line 9): the plugins are not installed on this VM.
  Marked `not verified` rather than turned into a claim about the tools.
- `pull` and `build` performance, and anything involving volumes or
  inter-container traffic: out of scope for this pass, which was deliberately short and
  runnable in one sitting.
- The FINE breakdown of line 4b's 125 ms of attach (how much is the binary re-exec, how much is
  the round trip to the holder, how much is veth/nft/IPAM). It is known that ~20-25 `fork`+`exec`
  calls of `ip`/`sysctl`/`nft` run per attach, counted in the code, but not measured here.
- Whether these numbers change with node DENSITY. Two calls in the attach path
  (`nft -a list chain fwdeny`, `nft list sets`) are text dumps that grow with the number of
  containers and namespaces — measuring on an empty node underestimates them, and this node was empty.

---

## IPv6 — recount of 2026-11-01

> **Why now.** The tables above never measured IPv6 — none of the three columns had
> even a v6 line. Delonix started governing dual-stack (ADR-0082), so the gap carried
> weight, and it was measured. All three on the **same machine, same day**, as this document
> requires.

| Field | Value |
|---|---|
| Date | **2026-11-01** |
| Where | development host — AMD Ryzen 9 8940HX (32 threads), kernel 7.0.0-38-generic |
| Docker | **29.9.2** — ROOTFUL (`systemd` daemon, `nvidia` runtime). Not rootless; the honest rootless-against-rootless axis is Podman × Delonix |
| Podman | **4.9.3** rootless |
| Delonix | **`feat/ipv6-phase1` branch (Phase 4), rootless** — a debug binary from the branch, NOT the release. **The published `0.1.0` has v6 OFF and would fail these lines** |
| Host with no v6 uplink | `ip -6 route show default` empty → `v6 → Internet` **is not measurable for ANY of the three** here; the cell stays `not verified` for all three |

| # | v6 capability | Docker 29.9.2 (rootful) | Podman 4.9.3 (rootless) | Delonix Phase 4 (rootless) | How it was measured |
|---|---|---|---|---|---|
| v1 | Container gets a v6 address | **Not by default** (`bridge` with `EnableIPv6=false`); with `--ipv6 --subnet <v6>`: Yes (`fd00:dddd::2`) | **Not by default**; with `--subnet <v6>`: Yes (`fd00:9999::2`) | **Yes, by default** — deterministic ULA `fd00:<o2>::<o3>:<o4>` (`fd00:254::143:123`) | `<eng> run … ; ip -6 addr show scope global` |
| v2 | v6 east-west, SAME network | **Yes** — 0% loss | **Yes** — 0% loss | **Yes** — 0% loss | `ping -6 -c2` between two containers on the same network |
| v3 | v6 isolation between DIFFERENT networks | **Yes, by default** — 100% loss | **Not by default** — 0% loss (passes through!); **only** with `--opt isolate=true` does it block (100% loss) | **Yes, by default** — 100% loss | two containers on two networks, cross `ping -6` |
| v4 | Per-workload v6 policy WITHIN a network | **No** — no concept; containers on the same network always talk | **No** — same | **Yes** — logical namespaces: on the SAME network, cross-namespace v6 is **dropped** by the isolation rule (`ip6 … ip6 saddr @dlxall6 ct state new … drop`, counter +2) | two containers, same network, different namespaces, `ping -6` + holder counter |
| v5 | Rootless | **No** (root daemon) | **Yes** | **Yes** | `docker info` / `podman info` / userns |
| v6 | v6 → Internet | `not verified` (host has no v6 route) | `not verified` | `not verified` — code ready (`slirp4netns --enable-ipv6` + masquerade), **pending v6 uplink on the host** | `ping -6` to `2606:4700:4700::1111` fails on all three |

### Where Delonix wins, and why

**v6 comes connected and governed by default** (v1, v3, v4). Docker needs an explicit
`--ipv6` network and Podman a v6 `--subnet`; Delonix gives the ULA on its own. And the line
the other two **have no way to fill** is v4: per-workload policy WITHIN a
network. On Docker and Podman, two containers on the same network always talk — the
concept does not exist; on Delonix, two on the same network but in different namespaces cannot reach
each other over v6, under the same isolation rule that already governed v4 (the counter on the
holder proves it).

**A finding measured about Podman, not about Delonix:** different networks are **not**
isolated by default (v3) — `pm_a` on one network reached `pm_c` on another, over both v6 and v4. It closes with
`--opt isolate=true`, but the default is surprising and worth recording.

### Where Delonix loses, or has not yet proven itself

- **It is not in a release.** This was measured on a branch binary. The published `0.1.0` has v6
  OFF and would fail v1–v4. Until there is a release, the Delonix column is a dated promise, not
  a product anyone can install.
- **v6 → Internet was not proven** (v6). The code exists, but the host has no v6 uplink — and
  on that point rootful Docker, on a host WITH a v6 route, is ahead today. A tie in `not
  verified` only because the bench has no v6 egress; it is not a merit of Delonix.
- **Firewall rules with an explicit v6 peer do not yet exist.** A rule naming an
  address (`ingress from 10.x`) governs only v4; that peer's v6 falls to the default plus isolation.
  Docker/Podman have no per-container firewall at all, so the comparison does not apply —
  but it is a real limit of Delonix against itself on v4.

### What remains unmeasured (v6)

- Performance (attach latency) with v6 on versus the v4 bench above — not re-measured here.
- Podman's `--opt isolate=true` under load / with many networks.
- v6 → Internet on any of the three: needs a host with a v6 uplink.
