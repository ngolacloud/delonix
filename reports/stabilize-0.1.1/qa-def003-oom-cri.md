# QA re-verification v0.1.1 — DEF-003 / OOM / CRI

- Date: 2026-10-31
- Binary under test: `/home/walter/dlx-wt/stabilize-0.1.1/target/debug/delonix` → `delonix 0.1.0`, commit `b9925e06e (+52 commits since v0.1.0)`, built 2026-10-31. (NOTHING was built.)
- Isolated root: `DELONIX_ROOT=/home/walter/dlx-wt/qa-def` (created for the test; the owner's containers in the default root untouched).
- Host: Linux 7.0, cgroup2 with memory/cpu/pids delegated (io/cpuset not). alpine:3.20 pulled into the isolated root (pull ~1s, 3.5 MiB).

## Verdict summary

| Defect | Repro (exact command) | Observed | Verdict |
|---|---|---|---|
| **DEF-003** `compose up` does not reconcile a stopped service | `delonix compose up -d` (2 alpine services, project bridge network) | `compose up -d` **hangs** after "network ... already exists" and **never creates the containers** (120s timeout, exit 124). Same failure with `container run --net <bridge>` (exit 124). `--net host` works. The "stop + re-up" step is never reachable. | **BLOCKED-BY-ENVIRONMENT** (rootless bridge/ingress networking congested on the host) |
| **OOM** effective memory limit + kill | `delonix container run -d --memory 32M alpine sh -c 'tail /dev/zero'` | Container `Dead (OOMKilled)`, `Crash reason: oom_killed`, `Memory: 32M`. Proof of the limit's VALUE: 50M→tmpfs under 32M → **killed (exit 137)**; same workload under 128M → **success (RC=0)**. | **DOES NOT REPRODUCE / WORKS** (limit truly enforced by cgroup v2) |
| **CRI** basic sanity | `delonix serve cri --addr unix://.../cri.sock` | Refuses with `DX-6000`: `delonix-cri` is not next to `delonix` in the v0.1.1 tree; the copy on the PATH (`~/.local/bin/delonix-cri`) is **5.1.1** and, forced with `DELONIX_SERVER_FROM_PATH=1`, aborts on version mismatch (`delonix-cri 5.1.1 vs delonix 0.1.0`). | **BLOCKED-BY-ENVIRONMENT** (`delonix-cri` binary of the v0.1.1 line not built; building forbidden) |

---

## 1. DEF-003 — `compose up` does not reconcile a stopped service

### compose.yaml used
```yaml
name: qa-def003
services:
  qa-web:   { image: alpine:3.20, command: ["sleep","100000"] }
  qa-cache: { image: alpine:3.20, command: ["sleep","100000"] }
```

### Commands and output
```
$ DELONIX_ROOT=/home/walter/dlx-wt/qa-def delonix compose up -d
network/qa-def003_default: already exists, nothing to do
<hangs>                       # timeout 120s -> UPEXIT=124
$ delonix compose ps
SERVICE   NAME   IMAGE   STATUS   # empty, no container created
```

`compose up --dry-run` resolves the project correctly (Network bridge `qa-def003_default` 10.209.0.0/16 + 2 services). The dry-run is instant; the real `up` hangs.

### Isolating the cause
- `delonix container run -d --net qa-def003_default alpine sleep ...` → **hangs, exit 124** (it never even launches its own `slirp4netns`). So the hang is in the attach to the bridge/ingress network, not in compose itself.
- `delonix container run -d --net host alpine sleep ...` (qa-smoke) → **starts, `Up` 5+ min**. Host-net containers work.
- `network_mode: host` in compose is **refused** (`DX-1000`), so compose always uses the project bridge network → path always blocked on this host.
- During the hang, the **owner's own commands** are stuck on the host: `delonix container start travel-pg` (pids 205966/205990) and `delonix container restart stage-demo-ui` (pids 2723932/2723937) persist across all samples; the host also runs kind+ceph+kubelet. → the rootless networking layer is congested at the host level.

### Conclusion
The central step of DEF-003 (stopping a service and checking whether `up` reconciles it) is **not exercisable** because `compose up` can never bring the services up on this host (rootless bridge/ingress network congested; `slirp4netns` exists in `/usr/bin`). **BLOCKED-BY-ENVIRONMENT.** It needs a rootless host with a free ingress layer (or a host-net test) to re-verify DEF-003 at HEAD.

## 2. OOM — effective memory limit + kill

```
$ delonix container run -d --name qa-oom --memory 32M alpine:3.20 sh -c 'tail /dev/zero'
45cff32ea18f1ed6
$ delonix container describe qa-oom
Status:         Dead (OOMKilled)
Crash reason:   oom_killed
  Memory:       32M

# Proof that the limit's VALUE is enforced (not just recorded):
$ delonix container run --rm --memory 32M  alpine sh -c 'dd if=/dev/zero of=/dev/shm/x bs=1M count=50' -> exit 137 (SIGKILL/OOM)
$ delonix container run --rm --memory 128M alpine sh -c 'dd if=/dev/zero of=/dev/shm/x bs=1M count=50' -> RC=0 (success)
```

- Retained allocation (`tail /dev/zero`) under 32M → OOMKilled, state `Dead (OOMKilled)`, `oom_killed`.
- 50 MiB to tmpfs (accounted in cgroup v2) **dies at 32M** and **survives at 128M** → the limit is truly applied by the kernel, with the configured value.

**Verdict: DOES NOT REPRODUCE / WORKS.** memory delegated on this host; the limit is enforced and the OOM-kill occurs.

## 3. CRI — basic sanity

```
$ delonix serve cri --addr unix:///home/walter/dlx-wt/qa-def/cri.sock
error[DX-6000] unavailable: 'delonix-cri' is not next to delonix in .../target/debug, and a copy
found on the PATH would be from another install — install it with `install.sh --with-cri`, or set
DELONIX_SERVER_FROM_PATH=1 ...

$ DELONIX_SERVER_FROM_PATH=1 delonix serve cri --addr unix://.../cri2.sock
warning: serving with '/home/walter/.local/bin/delonix-cri' (DELONIX_SERVER_FROM_PATH=1)
ERROR delonix_cri: this delonix-cri is version 5.1.1 but delonix is 0.1.0 — install the delonix-cri
of the same release next to delonix   (exit 2)
$ crictl --runtime-endpoint unix://.../cri2.sock version  -> fatal: no such file or directory (socket never created)
```

- Host infra present: `crictl`, `critest`, `containerd`, `slirp4netns`.
- Missing: the **`delonix-cri` binary of the v0.1.1 line** next to `delonix` in `target/debug` (does not exist; building forbidden). The only copy on the PATH is **5.1.1** and the version guardrail refuses it even with `DELONIX_SERVER_FROM_PATH=1`.

**Verdict: BLOCKED-BY-ENVIRONMENT.** What is missing exactly: build/install `delonix-cri` of the same release (v0.1.1) next to `delonix` (`install.sh --with-cri`). Only then would `crictl version/info` and a subset of `critest` make sense.

---

### Environment notes
- No owner resource was touched; all work in `DELONIX_ROOT=/home/walter/dlx-wt/qa-def` with `qa-`-prefixed resources.
- qa-* resources and the isolated root cleaned up at the end of the session.
