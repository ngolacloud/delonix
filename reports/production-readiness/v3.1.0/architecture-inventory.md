# Technical inventory — `fa8c6297`

Source: reading the code of this tree (grep/wc/awk). Pattern-based numbers are approximate
and the method is indicated. Nothing here was inferred from documentation.

## 1. Workspace

15 crates, all present in the root `Cargo.toml`. **None declares `[features]`.**

| Metric | Value | Method |
|---|---|---|
| LOC `src/**/*.rs` | 156 185 | `wc -l` |
| LOC `tests/*.rs` | 1 926 (11 files) | `wc -l` |
| `#[test]` / `#[tokio::test]` | 1 598 / 24 | grep (does not count `proptest!`) |
| `unsafe` in code | 291 | grep without comment lines |
| `#![forbid(unsafe_code)]` | 0 crates | grep |
| `pub fn` / `pub struct` / `pub enum` | 1 349 / 242 / 93 | grep (includes `pub(crate)`) |

| crate | LOC | unsafe | #[test] | internal deps | privilege / external commands |
|---|---|---|---|---|---|
| runtime-core | 5 287 | 11 | 71 | — | flock, `kill(pid,0)`, SO_PEERCRED |
| **runtime** | 11 742 | **150** | 95 | core | clone, pivot_root, ~36 mount/umount, setns, unshare, seccomp, cgroup v2, `newuidmap/newgidmap`, `ldconfig`, `iptables-nft` |
| runtime-bin | 90 618 (58%) | 57 | 866 | 13 crates | ~22 external programs (virsh, qemu-img, ssh, systemctl, tcpdump, …) |
| image | 6 487 | 0 | 81 | core | overlay mount; reqwest; signature |
| net | 15 434 | 44 | 191 | net-rules, core | `ip`, `nft`, `nsenter`, `unshare`, `slirp4netns`, `wg`, `tc`, `bpftool`, CNI |
| net-rules | 607 | 0 | 15 | — | pure, zero deps |
| vm | 7 307 | 5 | 96 | core, net | `virsh`, `qemu-img`, `cloud-localds` |
| volume | 1 894 | 0 | 25 | core | `mount`, `losetup`, `mkfs.ext4`, `resize2fs` |
| cri | 6 331 | 21 | 40+2 | core, runtime, image, net | setns(net) in port-forward; re-exec of `delonix` |
| mgmt | 3 094 | 1 | 11+18 | 6 crates | re-exec of `delonix` |
| scan | 939 | 0 | 12 | core, image | — |
| security-runtime | 2 369 | 0 | 55 | core | no I/O |
| truenas | 843 | 0 | 8 | core | HTTP |
| proxmox | 1 897 | 0 | 22 | core, vm | HTTP |
| mcp | 1 336 | 2 | 10+4 | 6 crates | re-exec of `delonix`, stdio |

Functions > 300 lines on the privileged path: `spawn` (607, `delonix-runtime/src/lib.rs:5197`),
`exec_with` (349, `:6155`), `container_init` (334, `:2850`); in the CLI `cmd_run` (1 042,
`cmd/container.rs:2955`).

## 2. Public surfaces

| Surface | Measured |
|---|---|
| CLI | 245 leaves (`scripts/cli_baseline.tsv`), 34 top-level groups |
| Docker Engine API | announces 1.43 (min 1.24); **14 routes served**, **12 refused with a reason** (`exec`, `attach`, `logs`, `events`, `build`, `networks`×3, `images/create`, `images/{n}/json`, `stats`, `volumes`) |
| CRI RuntimeService | implemented except `UpdateContainerResources`, `CheckpointContainer`, `GetContainerEvents` (Unimplemented); `Attach` output only; `UpdateRuntimeConfig` only logs |
| CRI ImageService | 5/5 implemented |
| Kinds | 19 (all `*.delonix.io/v1alpha1`); 13 converge |
| Compose | 1-level allowlist (7 top, 28 service); **nested sub-keys not checked** (see gaps) |
| Management API | unix socket, 32 paths / 40 method+path pairs, SO_PEERCRED same euid |
| MCP | stdio only; 12 READ, 1 SAFE_WRITE, 1 DISRUPTIVE |
| Env vars | ~60 runtime `DELONIX_*` (list in §8 of the raw inventory) |

## 3. Privilege boundaries

| Process | Namespaces / privilege |
|---|---|
| CLI | host namespaces, real uid |
| container init | mnt+pid+uts+ipc+net+user (rootless); 14 caps by default; `--privileged` = all + no seccomp; `no_new_privs` by default |
| netns pin | `unshare --user --map-root-user --net --mount`; full caps only in its own userns |
| netns control | `nsenter` into the pin; control socket, DNS, DHCP, nft |
| slirp4netns | host, engine uid |
| supervisor / `--rm` watcher / log shim | `fork`+`setsid` on the host |
| mapped re-execs (`__rmtree`, `__ovlhold`, …) | new userns with the full subuid map |
| CRI / mgmt / MCP | same uid; mutations via re-exec of the CLI |
| `vm bridge` | requires real root; **no uid check in the code** (`cmd/vmbridge.rs:275`) |

| Socket | Mode | Check |
|---|---|---|
| CRI `/run/delonix-cri.sock` | 0600 **after** bind, chmod error ignored | SO_PEERCRED euid |
| mgmt / docker-api | same | SO_PEERCRED euid |
| holder `control.sock` | 0600, dir 0700 | SO_PEERCRED euid |
| per-container slirp `/tmp/delonix-slirp-<pid>.sock` | **measured `srwxr-xr-x` with umask 022**, predictable path in shared `/tmp` | none |

## 4. Relevant OCI Runtime support

| Requirement | State read in the code |
|---|---|
| OCI hooks | **not executed** (CDI hooks are read and ignored with a warning) |
| seccomp from file | supported; parse failure → exit 126 |
| AppArmor | supported; an unloaded profile is refused |
| masked/readonly paths | applied; **mount failure only warns** |
| rlimits | 8 names; **unknown name and `setrlimit` error ignored** |
| mount propagation | rslave/rshared/private per mount |
| recursive read-only (rro) | **not supported** on the creation path; the CRI `recursive_read_only` is not read |
