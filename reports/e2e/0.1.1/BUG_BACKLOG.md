# BUG_BACKLOG.md — 0.1.1 (@596663c6)

Defects in something that should already work. Reproduced live on a rootless host,
2026-11-17 (see `report.md`). Numbering continues from 0.1.0 (last was BUG-0006).

## Status (2026-11-17) — all 3 RESOLVED (not committed; hand over to the owner)

| Bug | Files | Live proof | Regression test |
|-----|-----------|---------------|--------------------|
| **BUG-0007** ✅ | `dockerapi.rs` (resolves `Image.id`) | `curl /containers/json` → `ImageID: sha256:…`, `Image` keeps `repo:tag` | assert in `tests/compat/docker_api_smoke.py` (runs in CI; docker-py absent on the local host) |
| **BUG-0008** ✅ | `delonix-linux/src/lib.rs` (exec + init, unconditional `setgroups`) | `exec … id` → `uid=0(root) gid=0(root)`, no 65534 | assert in `scripts/e2e.sh` (exec section) |
| **BUG-0009** ✅ | ROOT fix: `canonical_restart_policy` in `delonix-compute/domain/pod.rs` + stamping in `bins/.../pod.rs` (reverted the band-aid in `reconcile.rs`) | `apply`+`drift` with `Always`/`OnFailure`/`Never` → zero drift | unit test `restart_policy_maps_k8s_to_canonical_and_passes_delonix_through` (green) |

Build green; **178 tests** pass across `delonix-compute` (124) + `delonix-stack` (54), zero regressions. Working branch: `feat/init-templates-prod-hardening` tree (edits isolated from the owner's files), **not committed**.

## P1

### BUG-0007 — Docker API `ImageID` returns the `repo:tag` name instead of the digest
- **Component:** `bins/delonix/src/cmd/dockerapi.rs:2777` (`"ImageID": c.image`, inside `containers_json()` :2765-2796). The real digest is in `delonix_oci::Image.id` (cf. `bins/delonix/src/cmd/image.rs:122`), resolvable via `state.images` (already used at `dockerapi.rs:2898` for `image_size`).
- **Owner:** unassigned
- **Impact:** in `GET /containers/json`, `ImageID` comes out equal to `Image` (e.g. `"alpine:3.24"`), whereas Docker always returns a `sha256:…` digest. The official `docker` client renders a truncated IMAGE column (`3.24`, `16`); tools that correlate containers↔images by ImageID (Testcontainers, dashboards) break.
- **Reproduction:** `delonix serve docker-api --addr unix:///tmp/d.sock &`; `curl -s --unix-socket /tmp/d.sock http://d/containers/json | jq '.[].ImageID'` → returns `"alpine:3.24"` instead of `sha256:…`.
- **Acceptance criteria:** `.ImageID` starts with `sha256:`; `Image` keeps `repo:tag`; `docker ps` via `DOCKER_HOST` shows the full IMAGE column. Regression test in the docker-api smoke (`tests/compat/`).
- **Related tests:** `tests/compat/docker_api_smoke.py` (extend to assert the ImageID format).
- **Dependencies:** none. **Complexity:** XS. **Tier:** B (rootless).

### BUG-0008 — `exec`/init inherit the host's supplementary groups → repeated `groups=65534(nobody)`
- **Component:** `crates/adapters/delonix-linux/src/lib.rs:10325-10332` (exec: `setgroups` only inside `if !container.group_add.is_empty()`) and `:4885-4893` (container init, same gap). Contrast: the `--user` branch always resets (`:10333-10342`, `:4803-4806`). gid_map at `write_userns_maps` `:3024-3083`.
- **Owner:** unassigned
- **Impact:** when there is no `--group-add`, the container process **inherits the ~13 supplementary groups of the `delonix` process** (host), unmapped in the userns, which the kernel projects all to the overflow gid 65534. `id` inside the container shows `groups=65534(nobody),65534(nobody),…` ×13. It is correctness noise **and** a leak of host group context into the container (defence-in-depth).
- **Reproduction:** `delonix run -d --name t alpine:3.24 sleep 300; delonix exec t id` → a row of `65534(nobody)`.
- **Acceptance criteria:** `exec … id` without `--group-add` shows a clean `groups=` (only the primary gid, or empty); no inherited 65534. Regression test that runs `id` and asserts the absence of a repeated 65534.
- **Related tests:** add to `scripts/e2e.sh` (exec section).
- **Dependencies:** none. **Complexity:** S. **Tier:** B (rootless).

### BUG-0009 — `drift` reports a false positive due to `restartPolicy` casing (`Always` vs `always`)
- **Component:** lowercase normalization in `crates/contexts/delonix-compute/src/domain/pod.rs` / `launch.rs:189` (`should_restart`) vs literal comparison in `crates/contexts/delonix-stack/src/app/reconcile.rs:731` (`same_value`), consumed by `bins/delonix/src/cmd/drift.rs:113` (`rows_for_with`) and `diff.rs:34`.
- **Owner:** unassigned
- **Impact:** a manifest with `restartPolicy: Always` (the **canonical Kubernetes** form) produces permanent, spurious drift: `LAST-APPLIED=Always` vs `OBSERVED=always`. It undermines trust in `drift`/`diff` and would make `drift --correct`/GitOps re-apply unnecessarily.
- **Reproduction:** `apply` a Pod with `restartPolicy: Always`, then `delonix drift -f manifesto.yaml` → reports drift in `restartPolicy` with no change having occurred.
- **Acceptance criteria:** `apply` followed by `drift` over an unchanged manifest with `restartPolicy: Always` reports **zero** drift. Normalize enum values on all three sides (desired/last-applied/observed) before stamping/comparing. Regression test in `drift.rs` (casing case table).
- **Related tests:** `drift`/`reconcile` unit tests (cf. `drift.rs:449`).
- **Dependencies:** none. **Complexity:** S. **Tier:** B (rootless).

## Parity note (not a bug, document or align)
- State after SIGKILL is `Dead` (Docker: `Exited (137)`). The normal exit code **is** exposed (`describe` → `Status: Exited (N)`); only the death-by-signal case diverges. See GAP_BACKLOG (minor).
