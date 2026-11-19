# Runbook — upgrading and rolling back the Delonix engine

> **Scope:** replacing the engine binary (`delonix`) on a node with LIVE load,
> and rolling back, without losing containers or state. Closes point G3 of the
> NO-GO→GO plan. The behaviour described here is **measured**, not claimed —
> the executable proof is `scripts/e2e_upgrade_rollback.sh` (17 assertions,
> green on 2026-11-02). What is still unproven is stated in [Limits](#limits).

## The mental model: daemonless

Delonix has no central daemon. A `delonix run -d` leaves a **supervisor**
process alive, launched from the binary **of that instant**, and every
subsequent CLI invocation talks to it via state on disk and control sockets.
Direct consequences for an upgrade:

- A running supervisor keeps running the code it was **born** with, even after
  the binary on disk changes. The new file is a new inode; the old process
  stays attached to the old inode, still mapped into memory.
- There is no "restart the daemon" to upgrade. What changes is (1) the binary
  on disk and (2) the CLI that from then on manages the old containers.

## Upgrade procedure (with live load)

**Golden rule: replace the binary by an atomic RENAME, never by `cp` on top.**
Measured: a `cp` on top of an ELF mapped by a live supervisor fails with
**`ETXTBSY` (Text file busy)** — the kernel refuses to rewrite an executable in
use. A rename swaps the name for a new inode and does not touch the old inode,
so it never hits `ETXTBSY` and live supervisors survive.

Package managers do this on their own: `dpkg`, `rpm` and `install(1)` install
via rename. The trap is only a **manual** upgrade with `cp`/`cat >`.

```sh
# 1. Put the new binary NEXT TO the current one, on the SAME filesystem
#    (rename is only atomic within a filesystem).
install -m 0755 delonix-new /usr/local/bin/.delonix.new     # or cp to a .tmp alongside

# 2. Swap atomically. Live containers are NOT touched.
mv -f /usr/local/bin/.delonix.new /usr/local/bin/delonix

# 3. From here on, every CLI invocation is the new version. Confirm and reconcile.
delonix --version
delonix ps                 # containers born under the old version are still Up
```

What survives the swap, measured:

| Fact | Result |
|---|---|
| Live `-d` container | stays `Up`, **same id** (not recreated) |
| `exec`/`logs` via the new CLI over an old container | work |
| `stop` → `start` via the new CLI | reconciles; the writable layer (state) persists |
| Previously created networks | survive |

## Rollback

Symmetric: rename the old binary back. The supervisors remain the same
processes (they never died), so the binary rollback is instant and the old CLI
resumes management with intact state.

```sh
mv -f /usr/local/bin/delonix.v-previous /usr/local/bin/delonix
delonix --version
delonix ps                 # everything still Up, state preserved
```

Always keep the previous version alongside before an upgrade:

```sh
cp -a /usr/local/bin/delonix /usr/local/bin/delonix.v-previous
```

## Validating on a node before trusting it

```sh
# Mechanism (one version only): survival + management + reconciliation + rollback.
scripts/e2e_upgrade_rollback.sh

# CROSS-version compatibility (when there is an earlier tag):
scripts/e2e_upgrade_rollback.sh --old /path/to/delonix-vN
```

The on-disk state format between versions is covered, separately, by
`scripts/e2e_state_upgrade.sh`.

## Limits

What is **proven** (mechanism): replacing the binary by rename with live load,
container survival, management by the new binary, reconciliation via
stop/start, and rollback.

What **remains** to be proven, and should not be claimed until then:

- **Code compatibility across different versions** — the new binary READING
  state/sockets written by an old version. The harness already accepts `--old
  <previous-tag>` and starts measuring this with nothing else needed; what is
  missing is a compiled earlier tag to point at.
- **Hot-patching a live supervisor** — does NOT exist. A supervisor runs the
  code it was born with until it is recreated. A security fix in the
  supervision path only reaches an existing container when it is recreated
  (stop + start recreates the process from the current binary). Plan a
  critical node's upgrade as "rename the binary (containers survive) +
  phased recreation of whichever need the new supervisor code".
- **Rollback mid-`apply`** — a `kind:` apply that fails partway has no
  transactional rollback today (`docs/discovery/47_IAC_REVISAO.md:400`). Until
  it exists, treat a failed `apply` as state to reconcile manually, not to
  undo.
