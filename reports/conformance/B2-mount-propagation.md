# B2 — Mount propagation (Track B · also closes 3 CRI failures)

**Date:** 2026-10-24 · **Binary:** `delonix 5.1.1 @ ee615f3b` · rootless.

## Result: ALREADY IMPLEMENTED and verified effective at the engine level

The dated documents (`paridade-docker-podman.md` "only `:ro`" and `cri-conformance.md` "3 mount-propagation
failures") are from **v0.63.1** and are **stale**. In the current binary propagation works.

### Live verification (container mountinfo)

| Option | Expected | Observed | Status |
|---|---|---|---|
| `-v H:/mnt:rslave` | `master:` tag | `… /mnt rw,nosuid,nodev,relatime master:1` | **PASSED** |
| `-v H:/mnt:rshared` | `shared:` tag | `… /mnt … shared:1187 master:1` | **PASSED** |
| `-v H:/mnt:rprivate` | no tags | `… /mnt rw,nosuid,nodev,relatime` (no shared/master) | **PASSED** |
| `-v H:/mnt:zzz` (invalid) | explicit error | `error[DX-1210] unsupported bind option ':zzz' — supported: …` | **PASSED** (fail-closed) |

### Code (current)

- `delonix-volume/src/lib.rs:1075-1083` parses `rprivate`/`rslave`/`rshared` **and** the `ro,<prop>` combos.
- `delonix-linux/src/lib.rs:1439-1455` applies `MS_SLAVE`/`MS_SHARED`/`MS_PRIVATE` with `MS_REC` as its
  own `mount()` call (correct order per kernel rule — propagation in a separate call).
- `delonix-cri/.../lifecycle.rs:3052-3056` translates CRI: `PropagationHostToContainer→rslave`,
  `Bidirectional→rshared`, default→private (safe direction).

## CRI (the 3 specs): confirmation DEFERRED — collision with concurrent session

Confirmation via `critest` (the 3 "Container Mount Propagation" specs) **was not run here**:
the host has a **concurrent owner session** in a `cri-image-repodigests-and-user` worktree with
`delonix-cri` + `critest` active (sharing `/tmp/delonix-cri-critest-root.*` and socket). A
`critest FOCUS=propagation` from here collided (300 s timeout at setup) and risked interfering with his
run. **No owner process was killed.** Since the engine applies propagation (proof
above) and the CRI path translates to the SAME propagation, the 3 specs should pass — but the number is
left for the **Track C critest run**, coordinated, without collision.

> Note: the owner's worktree (`cri-image-repodigests-and-user`) attacks precisely the plan's **C2**
> (Image Manager: repodigests + Uid/Username) — Track C is already partly under way by him.

## B2 verdict

- **Done at the engine level**, verified live. The Track C **C4** dependency (mount-propagation) is
  satisfied.
- Pending: the critest number for the 3 specs, to be obtained in a coordinated run (the owner is
  running critest now).
