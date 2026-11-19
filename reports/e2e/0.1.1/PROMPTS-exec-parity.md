# `delonix container exec` — parity defects and fix prompts

**Source:** live functional test, parameter by parameter, of `delonix container exec`
against the behaviour of `docker exec` and `podman exec`.
**Environment:** delonix 0.1.2 (commit `8a795077`, +40 since v0.1.2), rootless host ·
daemonless, delegated cgroup2, image `alpine:3.19`. Docker **29.10.0**,
Podman **4.9.3**, kernel `7.0.0-43-generic`/`x86_64`. Date: 2026-11-19.
**Method:** observed effect (not just `--help`) — real PTY via `pty.fork`/`pty.spawn`
(with `TIOCSWINSZ`), `test -t 0`, `stty size`, `id`/`id -G`, `pwd`, `env`,
`grep CapEff /proc/self/status`, propagation of `exit code` and `stdin` through a pipe.

**Verdict:** 13 parameters/behaviours exercised; the core is in **strong
parity** — PTY and `winsize` identical to Docker, **typed** errors (DX-3101/DX-4101)
clearer than Docker's, `CapEff` inherited and reduced (safe). **5 defects**:
2 behaviour divergences and 3 missing parity flags.

Order of attack (severity): security → functionality → parity → DX.

---

## Per-parameter matrix (observed)

| Parameter | Observed effect | Verdict |
|---|---|---|
| `-i` / `--interactive` | `printf … \| exec -i` → `read` reads the line | ✅ parity |
| `-t` / `--tty` | `test -t 0` = TTY_YES, even with stdout in a pipe (== docker/podman) | ✅ parity |
| `-t` winsize | `TIOCSWINSZ 40×120` → `stty size` = `40 120` (== docker) | ✅ parity |
| `-it` | interactive shell via PTY, command runs, clean exit | ✅ parity |
| `-e K=V` (repeatable) | `-e FOO=bar -e BAZ=qux env` → both present | ✅ parity |
| `-w <existing>` | `-w /tmp pwd` → `/tmp` | ✅ parity |
| **`-w <nonexistent>`** | delonix → `/` **rc=0**; docker/podman → **rc=127** | ❌ **BUG-1** |
| `-u <uid>` / `<uid:gid>` / `<name>` | resolves against `/etc/passwd`; effective uid/gid correct | ✅ parity |
| **`-u` supplementary groups** | delonix → empty `groups=`; docker/podman → 12 groups from `/etc/group` | ❌ **BUG-2** |
| inherited user (`run --user` → `exec`) | exec without `-u` inherits the container's configured uid | ✅ parity |
| `COMMAND …` + exit code | `echo a b` → `a b`; `exit 37` → rc=37 | ✅ parity |
| no command (fallback) | tries `bash`, falls back to `sh` (alpine) → runs | ✅ parity |
| inherited caps | `CapEff=00000000a00425fb` (reduced, not full) | ✅ safe |
| error: stopped container | `DX-3101 container is not running` rc=3 (docker rc=1) | ✅ typed |
| error: nonexistent container | `DX-4101 no such container` rc=4 (docker rc=1) | ✅ typed |
| **`-d` / `--detach`** | `unexpected argument '-d'` | ❌ **BUG-3** (missing) |
| **`--env-file`** | `unexpected argument '--env-file'` | ❌ **BUG-4** (missing) |
| **`--privileged`** / `--detach-keys` | `unexpected argument` | ❌ **BUG-5** (missing) |

Current surface of `exec` (`ExecArgs`, `bins/delonix/src/cmd/container.rs:1623`):
`-i`, `-t`, `-e`, `-w`, `-u`. `docker exec` adds `-d/--detach`,
`--detach-keys`, `--env-file`, `--privileged`.

---

## Cross-cutting notes (for any of these sessions)

- Host is **rootless and without sudo**; validate with the rootless harnesses.
- `/tmp` and the scratchpad are **ephemeral between turns** — keep artefacts outside `/tmp`.
- Binary tests: `cargo test -p delonix-bin --bins` (the `delonix-bin` package has no lib).
- **develop-GitFlow** process; the `push`/`merge` of a PR belongs to the owner.
- Do not weaken security defaults when fixing (BUG-2 and BUG-5 touch credentials/caps).
- The verb is also exposed at the top level as `delonix exec` (alias) — the fix applies to both.

---

## BUG-1 — `-w <nonexistent>` silently falls back to `/` instead of failing  _(Medium · functionality)_

**Symptom.** `exec -w <dir-that-does-not-exist>` is accepted, the command runs in `/` and returns
**rc=0**. Docker and Podman **fail** (`chdir to cwd … no such file or directory`,
**rc=127**). A script launched in the wrong directory goes unnoticed.

**Evidence.**
```
delonix exec -w /naoexiste exectgt pwd   → /        rc=0
docker  exec -w /naoexiste dexec   pwd   → (error)  rc=127
podman  exec -w /naoexiste pexec   pwd   → (error)  rc=127
```

**Root cause.** In the `exec` path, the workdir `chdir` **ignores the error**:
`crates/adapters/delonix-linux/src/lib.rs:10699`
```rust
let workdir = overrides.workdir.or(container.workdir.as_deref())
    .filter(|w| !w.is_empty()).unwrap_or("/");
let _ = chdir(workdir);   // error discarded → stays in "/"
```
Contrast with the **startup** path (`run`), at `lib.rs:5258-5262`, which applies the
Dockerfile WORKDIR semantics (`create_dir_all` + warning). In `exec` neither of the
two actions should happen: Docker **does not create** the dir on `exec` — it **aborts**.

**Fix.** In `exec`, when `overrides.workdir` (the explicit `-w`) points to a
nonexistent directory, **abort** with a clear error and `rc≠0` (docker/podman parity),
instead of `let _ = chdir`. Preserve the current fallback only for the workdir **inherited
from the container** (it must not regress a container whose WORKDIR was removed at runtime).

**Done when.**
1. `exec -w /naoexiste <ctr> pwd` → error naming the directory, `rc≠0`.
2. `exec -w /tmp <ctr> pwd` → `/tmp`, `rc=0` (no regression).
3. Test covering nonexistent workdir (error) + existing (ok).

**Gates.** No new flag. Do not regress the inherited workdir. Rootless gates.
**Agent:** general-purpose, medium tier (`delonix-linux` exec path).

---

## BUG-2 — `-u` does not load supplementary groups from `/etc/group`  _(Medium · correctness/permissions)_

**Symptom.** `exec -u <user>` gives the process **only** the primary gid; the
supplementary groups that the rootfs `/etc/group` assigns to that user are **not**
applied. Docker and Podman load them. A process that depends on belonging to
`wheel`/`disk`/`video`/… to access a resource behaves differently.

**Evidence.** (`alpine`, root belongs to 12 groups in `/etc/group`)
```
delonix exec -u 0 exectgt id → uid=0(root) gid=0(root)                 (no groups)
docker  exec -u 0 dexec   id → uid=0(root) gid=0(root) groups=0,1(bin),2(daemon),
                               3(sys),4(adm),6(disk),10(wheel),11,20,26,27
podman  exec -u 0 pexec   id → same (12 groups)
```

**Root cause.** `resolve_run_user` returns only `(uid, Option<gid>)` —
`bins/delonix/src/cmd/container.rs:2748` → `delonix_oci::rootfs_user::resolve_user`.
`getgrouplist`/a scan of the rootfs `/etc/group` for the target user is never called,
so the list passed to `setgroups`
(`crates/adapters/delonix-linux/src/lib.rs:1105`) does not contain the supplementary
ones. The startup path already has group logic for the case **root without uid switch**
(`lib.rs:10704+`, `if !user_switched`), but `exec -u` (with a switch) does not reach it.

**Fix.** When resolving `-u`, derive the supplementary groups by reading the **rootfs**
`/etc/group` (equivalent to `getgrouplist` of the target user) and pass them to
`setgroups`, filtering to the range mapped in the userns (the `setgroups` is already best-effort:
`lib.rs:1105-1107`). Do not regress the root-without-switch case already covered.

**Done when.**
1. `exec -u 0 <ctr> id -G` lists the same groups as `docker exec -u 0`.
2. `exec -u nobody <ctr> id` → gid/groups consistent with the rootfs `/etc/group`.
3. Test comparing the exec's `id -G` with the list derived from `/etc/group`.

**Gates.** Do not weaken: only groups within the mapped range; keep the
best-effort (never abort the exec because of `setgroups`). Rootless gates.
**Agent:** general-purpose, high tier (credentials/userns — see the
`setgroups=deny` pitfall in `pin_userns.rs`).

---

## BUG-3 — `-d` / `--detach` missing on `exec`  _(Medium · parity)_

**Symptom.** `exec -d <ctr> <cmd>` is rejected by the parser
(`error: unexpected argument '-d' found`). Docker runs the command in the background and
returns immediately.

**Root cause.** `ExecArgs` (`bins/delonix/src/cmd/container.rs:1623`) does not define
`detach`. `cmd_exec` (`container.rs:6495`) always assumes foreground.

**Fix.** Add `-d/--detach` to `ExecArgs`: launch the process in the container
and return without waiting (analogous to `run`'s detach), without wiring up stdio. Mutually
exclusive with `-i`/`-t` (as in Docker).

**Done when.**
1. `exec -d <ctr> sh -c 'sleep 2; touch /tmp/mark'` → returns immediately; `/tmp/mark` appears.
2. `exec -d -it …` → invalid combination error.
3. Test of the detached path.

**Gates.** New flag → **pt.po** (3 texts) + flag order (see
`delonix-new-code-and-flag-gates`). Rootless gates. **Agent:** general-purpose, medium tier.

---

## BUG-4 — `--env-file` missing on `exec`  _(Low · parity)_

**Symptom.** `exec --env-file <f> <ctr> env` rejected (`unexpected argument`). The repeatable
`-e` exists and works; only the bulk reading of a `KEY=VAL` file is missing.

**Fix.** Add `--env-file <PATH>` (repeatable) to `ExecArgs`, with the same
parsing semantics as `run --env-file` (reuse the existing parser; `-e` takes
precedence over the file, as in Docker).

**Done when.**
1. `--env-file` with `A=1\nB=2` → `env` shows `A=1` and `B=2`.
2. `-e A=override --env-file f` → `A=override` wins.
3. Test of the file + `-e` merge.

**Gates.** New flag → **pt.po** + order. Reuse the `run` parser. Rootless gates.
**Agent:** general-purpose, low tier.

---

## BUG-5 — `--privileged` and `--detach-keys` missing on `exec`  _(Low · parity/DX)_

**Symptom.** Both rejected by the parser. In Docker, `exec --privileged` elevates
capabilities **only for that command** (ad-hoc debugging in a non-privileged container);
`--detach-keys` redefines the PTY detach sequence.

**Security note.** Today `exec` inherits the container's reduced `CapEff`
(observed: `00000000a00425fb`) — **good default**. An opt-in `--privileged` must **not**
change that default; only, when explicitly requested, run the command with the
augmented cap set (without altering the container or other execs). `--detach-keys`
is purely DX.

**Fix.** Add `--privileged` (opt-in, caps augmented only for that exec) and,
if worthwhile, `--detach-keys`. Prioritise `--privileged`; `--detach-keys` is optional.

**Done when.**
1. `exec --privileged <non-priv-ctr> grep CapEff /proc/self/status` → full `CapEff`,
   **without** altering the container (a subsequent `exec` without the flag returns to the reduced one).
2. (Optional) `--detach-keys` redefines the sequence in the PTY path.
3. Test proving that the default (no flag) **remains** reduced.

**Gates.** New flag(s) → **pt.po** + order. Do **not** weaken the default (the reduced one
must remain without the flag). Rootless gates. **Agent:** general-purpose, high tier
(touches caps/seccomp).

---

## What is NOT a defect (parity confirmed live)

- **PTY**: `-t` synthesises a TTY even with stdout in a pipe — **same** as docker/podman; the
  `winsize` (`TIOCSWINSZ`) propagates (`stty size` = `40 120` == docker).
- **`-u`** resolves `uid`, `uid:gid` and **name** against the rootfs `/etc/passwd`; the user
  configured in `run --user` is **inherited** by `exec` without `-u`.
- **Typed errors** `DX-3101` (stopped) and `DX-4101` (nonexistent) — clearer than Docker's
  `Error response from daemon`, with dedicated rc values (3/4).
- **Security**: `exec` inherits the container's reduced `CapEff` (it does not escalate to full).
- **exit code** and **stdin** propagate correctly; the `bash`→`sh` fallback works.
