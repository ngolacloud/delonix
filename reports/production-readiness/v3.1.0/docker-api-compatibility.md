# Docker Engine API compatibility — measured on `fa8c6297`

## Measurement

| Field | Value |
|---|---|
| Server | `delonix serve docker-api --addr unix:///tmp/dxd.sock` (binary from `fa8c6297`) |
| Client | real Docker CLI **29.8.0**, `DOCKER_HOST` to the socket, isolated `DOCKER_CONFIG` |
| State | `DELONIX_ROOT`/`DELONIX_NET_RUNTIME_DIR` isolated |
| Announced API | 1.43 (minimum 1.24) |
| Socket | `srw-------` (measured), SO_PEERCRED same euid |
| Matrix published by the binary | 14 served / 12 refused with a reason (`raw/docker-api-matrix.txt`) |
| Logs | `raw/docker-cli-live.txt`, `raw/dockerapi-server.log` |

## Real Docker CLI → result

| Command | rc | State | Observation |
|---|---|---|---|
| `docker version` | 0 | PASS | API 1.43 |
| `docker info` | 0 | PARTIAL | `Name` empty |
| `docker images` | 0 | PASS | |
| `docker pull busybox` | 1 | UNSUPPORTED | `POST /images/create` refused with a reason |
| `docker create` | 0 | PARTIAL | several `HostConfig.*` ignored **with a warning** |
| `docker start` / `ps` / `inspect` | 0 | PASS | |
| `docker logs` | 1 | UNSUPPORTED | refused with a reason |
| `docker exec` | 1 | UNSUPPORTED | refused with a reason |
| `docker stop` / `start` / `kill` | 0 | PASS | |
| `docker wait` (after kill) | 0 | PASS | returns 137 |
| `docker restart` / `rename` / `rm -f` | 0 | PASS | |
| **`docker run -d`** | **124** | **FAIL** | **hangs** (timeout 60 s); the container is created |
| **`docker run --rm alpine echo hi`** | **1** | **FAIL** | `unable to upgrade to tcp, received 404`; `AutoRemove` ignored → **orphan container remains** |
| `docker run -d --restart always` | 125 | DELIBERATE DIVERGENCE | refused with a written reason |
| `docker network ls` / `volume ls` / `stats` | 1 | UNSUPPORTED | refused with a reason |
| `docker compose *` | — | NOT TESTED | depends on `/networks` and `/volumes`, refused |

## Defects found live

| ID | Sev | Defect | Evidence |
|---|---|---|---|
| G-027 | P1 | `docker run -d` does not return control | rc=124 in 60 s, reproduced twice (`dr1`, `dr3`); documented in AGENTS.md as a "limitation", but it is the most-used command |
| G-028 | P2 | `docker run --rm` fails and leaves orphan containers (`AutoRemove` ignored) | 2 `echo hi` containers were left in the store |
| G-029 | P1 | **Divergent state**: `docker ps` reports `Up About a minute` for an `echo hi` that has already terminated; `delonix container ls` says `Exited (unknown)` for the same id `b8d87aee9b8e`, and the other appears `Dead` | `raw/docker-cli-live.txt` + `ps -a` output in the session |

## Verdict per use case

| Consumer | Verdict |
|---|---|
| Docker CLI for the manual lifecycle (create/start/stop/rm) | PARTIAL — works |
| `docker run` (any form) | **FAIL** |
| Docker Compose | **UNSUPPORTED** (no `/networks`, `/volumes`, `/images/create`) |
| Testcontainers / Dev Containers / GitLab Runner | **UNSUPPORTED** (no pull, exec, logs, attach) |
| kind with the docker provider | NOT TESTED |

Documentation that says otherwise and is out of date: see `stale-evidence.md`.
