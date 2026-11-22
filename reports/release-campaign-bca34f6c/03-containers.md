# 03 — Containers and images (rootless cycle)

SUT: `delonix 5.1.1` @ `bca34f6c3`. Mode: **rootless** (uid 1000). Image: `nginxinc/nginx-unprivileged:1.30`, `ubuntu:22.04`. Resources: `relc-` prefix.

## Full cycle verified (independent verification, not just Delonix's own response)

| # | Case | Command | Independent verification | Observed | State |
|---|---|---|---|---|---|
| 1 | pull + run detached + port + limits | `run -d --name relc-web -p 18080:8080 --memory 128m --cpus 0.5 nginx-unprivileged` | `ps`; kernel | live pull from registry-1.docker.io (platform amd64); `Up`, id `5ad98df8ac21` | **PASSED** |
| 2 | exec | `exec relc-web sh -c 'id -u; hostname'` | process output | `EXEC_OK=101:relc-web` (uid 101 nginx, UTS ns = name) | **PASSED** |
| 3 | published port + external HTTP | `curl 127.0.0.1:18080` | nginx log | `HTTP 200`; log shows `10.0.2.2 ... "GET / HTTP/1.1" 200` (request arrived via slirp gw) | **PASSED** |
| 4 | **limits effective in the kernel** | read `memory.max`/`cpu.max` from the hierarchy | `/sys/fs/cgroup/.../dlx-5ad98df8.../` | `memory.max=134217728` (**=128 MiB**), `cpu.max=50000 100000` (**=0.5 core**) — effective, not just config | **PASSED** |
| 5 | logs | `logs relc-web` | — | nginx lines + the curl GET | **PASSED** |
| 6 | RW bind mount | `run --rm -v $H:/data ubuntu ...` | read file on the host | container reads `host.txt`; host sees `cont.txt` written by the container | **PASSED** |
| 7 | wait + exit code | `run -d ... sh -c 'sleep 1; exit 42'; wait` | — | `wait` returns **`42`** (real code propagated) | **PASSED** |
| 8 | graceful stop | `stop relc-web` | `ps -a` | `Exited (0)` (clean SIGTERM) | **PASSED** |
| 9 | start + persistence | `start relc-web`; curl | HTTP | `HTTP 200` (port re-published, same container) | **PASSED** |
| 10 | restart | `restart relc-web`; curl | HTTP | `HTTP 200` | **PASSED** |
| 11 | `--rm` | mount case used `--rm` | `ps -a` | container removed on exit | **PASSED** |
| 12 | **rm + absence of orphans** | `rm -f relc-web relc-exit` | kernel/network/ps | ps=0; **cgroup dir gone**; **port 18080 released** (slirp listener gone); curl refused | **PASSED** |

## Input errors (correct refusal class, no effect)

- Invalid port `99999:80` → `error[DX-1307]`, exit 1, **0 containers created** (see `02-cli.md`).
- Invalid names (`../`, `/`, space, empty) → `error[DX-1000]`, exit 1, **0 leaks**.
- Missing image / exec in a non-existent container → `error[DX-4101] no such container`, exit 4.

## Limits: effective vs accepted-and-ignored (engine honesty)

`delonix system doctor` + `system resources` report exactly what is effective on this host:

- **Effective:** `--memory`, `--cpus` (cgroup v2 `cpu memory pids` delegated) — confirmed in the kernel above.
- **DLX-RES-001:** `--cpuset` is **accepted and silently ignored** (cpuset controller not delegated) — the engine says so.
- **DLX-RES-002:** `--io-weight`/`--device-*` **refused (exit 69)** — io not delegated.

This satisfies the §7/§8 requirement to "verify the effective limits, not just the stored configuration": the engine not only applies but declares what it cannot apply.

## Preservation

Pre-existing containers `kaeso-odoo` and `kaeso-db` remained `Up` during and after the whole container campaign (verified: `ps | grep -c kaeso = 2`).

## PIDs — requirement note

There is no `--pids-limit`/`--pids` flag on `run`/`update` (a deliberate decision: `main.rs:1689` tests its absence — it is the Docker spelling). The `pids` controller **is** delegated and used (default `pids.max=512` observed; compose honours `deploy.resources.limits.pids`). So the §7 "PIDs limits" requirement is covered by **compose/kind**, not by an ad-hoc `run` flag. → see DEF-001 for the contradiction in the documentation/UX.
