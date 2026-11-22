# Finding — `HOME=/root` for a non-root user with no `/etc/passwd` entry (CRI)

**CNCF area:** H1 (Sonobuoy conformance / interop)
**Severity:** high — it breaks non-root client-go workloads (the Sonobuoy
aggregator dies with `EACCES` resolving `~/.kube/config`).
**State:** FIXED (crate green + `arch_fitness` delta-zero; live proof below).

## Measured symptom (before)

With `--mode quick`, the Sonobuoy aggregator runs non-root (the image's own uid)
and dies writing/reading `~/.kube/config`:

```
... open /root/.kube/config: permission denied (EACCES)
```

The workaround that masked the bug: forcing `HOME=/tmp` in the Sonobuoy pod.
This fix removes the need for that workaround.

## Root cause

[`apply_env`](../../../crates/adapters/delonix-linux/src/lib.rs) — the
container's init (PID 1 inside the namespaces) assembled a clean environment and
pinned **`HOME=/root` for ANY user**, whatever the `runAsUser`:

```rust
std::env::set_var("HOME", "/root");   // ← fixed, even for uid != 0
```

A non-root process (Kubernetes `runAsUser: 1000` against an image whose
`/etc/passwd` only knows `root`) therefore got `HOME=/root` — a `0700` directory
owned by `root` that it cannot write to. Every client-go tool that resolves
`~/.kube/config` blows up with `EACCES`. runc/OCI, by contrast, derive `HOME`
from the user's `/etc/passwd` entry and default to `/`.

## The fix

`crates/adapters/delonix-linux/src/lib.rs`:

1. A new `passwd_home(uid)` (plus the pure half `parse_passwd_home(text, uid)`):
   it reads the container's `/etc/passwd` (post-`pivot_root`) and returns the
   *home* field (6th column) of the uid's entry; `None` when the file is missing
   or the uid has no entry.
2. `apply_env(hostname, env, uid)` now resolves
   `HOME = passwd_home(uid).unwrap_or("/")` — **never a fixed `/root`** for a
   non-root user with no entry. An explicit `HOME` in the ENV
   (image/stack/CLI) still wins, because it is applied last.
3. Both call sites pass the effective uid: the `run` path
   (`run_uid.unwrap_or(0)`) and the `exec` path (the `-u` user, otherwise the
   container's).

runc parity: a uid present in `passwd` → its *home* (root keeps `/root`); a uid
with no entry → `/`.

## Measured proof

### Unit (crate `delonix-linux`)

`parse_passwd_home_honours_the_entry_and_defaults_to_root_dir` covers: a uid in
passwd → its *home*; **uid 65532 with no entry → `None` (⇒ `/`, not `/root`)**;
an empty passwd → `None`; malformed lines ignored.

```
$ cargo test -p delonix-linux
test result: ok. 258 passed; 0 failed; 0 ignored; ...
```

### The `arch_fitness` gate — delta-zero

Measured with and without the change to `lib.rs` (the rest of the tree equal):
**byte-for-byte identical output**. Every FAIL (`env_writes 25`,
`cli_context_lines`, `library_prints`, `raw_error_variant_matches`,
`cmd/distroless`, `cmd/help`) is pre-existing debt / debt from the owner's
concurrent edits, not from this fix. This change's `git diff` over
`env::set_var`/`remove_var` is 1:1 (it replaces the `HOME` line), so its
contribution to `env_writes` is **zero**.

### Direct runtime (binary `target/release/delonix` 0.1.2 with the fix)

Decisive proof of the EXACT `apply_env` path (identical in the CLI's `run` and in
the CRI — both derive `HOME` from `run_uid` through `apply_env`). `busybox:1.36`
has no entry for uid 1000 in `/etc/passwd` (`grep -c ":1000:" → 0`; `root` →
`/root`):

```
$ delonix container run --rm --user 1000 busybox:1.36 sh -c 'echo HOME=$HOME; id'
HOME=/                       # ← uid 1000 with no entry → "/" (BEFORE: /root)
uid=1000 gid=1000 groups=1000

$ delonix container run --rm busybox:1.36 sh -c 'echo HOME=$HOME'
HOME=/root                   # ← root WITH a passwd entry → /root (no regression)
```

**Why this fixes Sonobuoy:** the aggregator runs non-root and client-go tries
`~/.kube/config`. With `HOME=/root` (0700, owned by `root`) the non-root process
cannot even _traverse_ the directory → `stat` returns `EACCES` → fatal. With
`HOME=/` (0755, traversable) the `stat` of `/.kube/config` returns `ENOENT` —
which client-go treats as "no kubeconfig" and falls back, cleanly, to the
*in-cluster* config (service-account token). It is exactly runc's semantics.

### Live (Delonix-CRI cluster — kubeadm integration + Sonobuoy)

Measured 2026-11-20. Golden image `delonix-vm-k8s-homefix:1.36` rebuilt to embed
the fixed binaries (`delonix-cri` sha256 `7a5166fc…`); a 1-node cluster from
`delonix cluster kubeadm --name homefix --control-plane 1 --workers 0`.

**Criterion 1 — the HOME of a `runAsUser:1000` in-cluster:**

```
$ kubectl run h --image=busybox:1.36 --restart=Never --rm -i \
    --overrides='{"spec":{"securityContext":{"runAsUser":1000}}}' \
    --command -- sh -c 'echo $HOME'
/
```

**Criterion 2 — Sonobuoy `--mode quick` WITHOUT the `HOME=/tmp` workaround:**

The aggregator (`pod/sonobuoy`, which runs non-root) stayed `1/1 Running`,
**0 restarts** — it no longer blows up with `EACCES` on `/root/.kube/config`.
Result:

```
$ sonobuoy results <tarball>
Plugin: e2e
Status: passed
Total: 7586
Passed: 8
Failed: 0
Skipped: 7578

Plugin: systemd-logs
Status: passed
Passed: 1
Failed: 0
```

e2e **passed, Failed 0**, without the workaround. (The "Errors detected in files"
in the full output are informative occurrences in the components' logs, not test
failures — `Failed: 0` is the authoritative count.)

## Live run (to reproduce)

```sh
# the binary with the fix
cargo build -p delonix-bin --release
CRI=$(pwd)/target/release/delonix

# a 1-node cluster (comes up in a VM; no sudo on the host)
delonix cluster kubeadm --control-plane 1 --workers 0

# Criterion 1 — the HOME of a non-root user with no passwd entry → "/"
kubectl run h --image=busybox:1.36 --restart=Never --attach --rm \
  --overrides='{"spec":{"securityContext":{"runAsUser":1000}}}' \
  --command -- sh -c 'echo $HOME'
# expected: /

# Criterion 2 — Sonobuoy quick WITHOUT the HOME=/tmp workaround
sonobuoy run --mode quick --wait
sonobuoy status          # e2e: Passed, Failed 0
sonobuoy retrieve && sonobuoy results <tarball>
```
