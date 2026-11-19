# GAP_BACKLOG.md — 0.1.1 (@596663c6)

Missing/divergent functionality — not a defect of something that already worked.
Numbering continues from 0.1.0 (last was GAP-0017). Lenses: Docker 29.x, Podman, k8s.

---

## Section A — Docker/CLI parity (P1)

### GAP-0018 — Non-variadic removal (`network/volume/secret/rmi` accept only 1 name)
- **Current behaviour:** `network rm` (`bins/delonix/src/cmd/network.rs:608`), `volume rm` (`volume.rs:704`), `secret rm` (`secret.rs:104`), `rmi`/`image rm` (`image.rs:244`) use `name: String`. Only container `rm` (`container.rs:1607`, `ids: Vec<String>`) is variadic.
- **Expected:** Docker/Podman accept N names in all of them (`docker volume rm a b c`).
- **Why it matters:** daily friction and breakage of scripts ported from Docker.
- **Acceptance criteria:** the 4 commands accept `Vec<String>` and remove all; a partial error does not abort the rest. (The pattern already exists in `image.rs:291`.) Differential test in `tests/cli-parity/`.
- **Dependencies:** none. **Complexity:** S. **Tier:** B.

### GAP-0019 — `vm ls` accepts only `-A`, not `-a` (inconsistent with `ps -a`)
- **Current behaviour:** `bins/delonix/src/cmd/vm.rs:1097` — `#[arg(short = 'A', long)]`; `ps` uses `#[arg(short, long)]` (`container.rs:1552`) → `-a`.
- **Expected:** uniform `-a`/`--all` (Docker/Podman).
- **Acceptance criteria:** `delonix vm ls -a` works; `--all` is kept. Help/parse test.
- **Dependencies:** none. **Complexity:** XS. **Tier:** B.

### GAP-0020 — Docker API does not serve the `/networks` family (blocks Testcontainers/kind)
- **Current behaviour:** `delonix serve docker-api --matrix` = 21/28 served; the 7 refused are `GET/POST/DELETE /networks`, `/networks/create`, `/networks/{id}` (`bins/delonix/src/cmd/dockerapi.rs`). The functionality exists in `delonix network`.
- **Expected:** ecosystem clients (Testcontainers, kind, SDKs) call `/networks`.
- **Why it matters:** this is what keeps the Docker API "experimental" (cf. `system features`). It unblocks real portability.
- **Acceptance criteria:** the matrix goes to 28/28; a Testcontainers smoke test creates/inspects/removes a network via the API. (Relates to the old GAP-0004.)
- **Dependencies:** none. **Complexity:** L. **Tier:** B.

### GAP-0021 — `stats` has neither streaming nor `--no-stream` (inverted semantics vs Docker)
- **Current behaviour:** `container.rs:1701` — `stats` is always one-shot; neither `--no-stream` nor `--stream` exists. `docker stats` streams by default and `--no-stream` is the one-shot.
- **Acceptance criteria:** accept `--no-stream` (no-op, habit compatibility) and, ideally, `--stream` for continuous sampling. Parse test.
- **Dependencies:** none. **Complexity:** S (no-op) / M (real streaming). **Tier:** B.

### GAP-0022 — `logs --tail/--since/--timestamps` require the container to have run with `--log-cri`
- **Current behaviour:** runtime check (`container.rs:7924-7929`, triggered at `:7964`), not clap. Without `--log-cri` (`:1538`), the 3 flags fail with DX-1000.
- **Expected:** in Docker they always work.
- **Why it matters:** major friction — the user has to anticipate at `run` time that they will want `--tail` later.
- **Acceptance criteria:** make the per-line timestamp format the capture default (or always record the sidecar timestamp), so that `--tail/--since/--timestamps` work on any container. e2e test on logs.
- **Dependencies:** default log format decision. **Complexity:** M. **Tier:** B.

### GAP-0023 — `Pod.restartPolicy` default is `no` (Kubernetes uses `Always`)
- **Current behaviour:** `crates/contexts/delonix-compute/src/domain/pod.rs:16` (`default_restart() -> "no"`). A Pod declared without a policy does not auto-restart; a k8s Pod does.
- **Expected:** k8s parity (default `Always`), or an explicitly documented divergence.
- **Why it matters:** expectation of those coming from k8s; ties into GAP-0025 (declarative liveness).
- **Acceptance criteria:** decision recorded (ADR) + default aligned or documented. Test that pins the chosen default.
- **Dependencies:** couples to GAP-0025. **Complexity:** S (+ decision). **Tier:** B.

### GAP-0024 — Minor parity items (grouped)
- `compose -f` only per subcommand (`compose.rs:456-532`), not global like `docker compose -f x up`.
- `config set` requires `KEY VALUE`; accept `key=value` (`config.rs:81`).
- `pod create` is only declarative (`pod.rs:120`); the imperative form `--image/--name` (podman) is missing.
- `volume inspect` shows `created: unix=<epoch>` — human-readable date.
- `Dead` state after SIGKILL → map to `Exited (137)` (parity; see BUG_BACKLOG note).
- **Complexity:** XS each. **Tier:** B.

---

## Section B — Architecture (P1/P2)

### GAP-0025 — Build/commit flatten layers in rootless: no base sharing, disk amplification
- **Current behaviour:** branching by `runtime::is_rootless()` in `bins/delonix/src/cmd/build.rs:1357` and `container.rs:~6425`. The rootless path → `commit_flat_rootfs` (`crates/adapters/delonix-oci/src/build.rs:1115-1180`, single-element `diff_ids` `:1143`, `layers: vec![layer]` `:1161`) packs the **entire** rootfs as 1 layer. The root/overlay path → `commit_upper` + `build_image` (`:868`, `base.layers.clone(); push(delta)` `:876`) stacks correctly. The internal format **already supports** multi-layer (`Image.layers: Vec<String>`, `image.rs:115`); CAS deduplicates by digest (`cas.rs:69`). A rootless build cache (snapshot-chain) exists but only for caching (`build.rs:616`).
- **Measured evidence:** 2 images `FROM alpine:3.24` → 3 distinct layer digests (`040a…`, `853b…`, base `881f…`): zero sharing. `system df`: `images 12.2 GiB / layers ≥20.3 GiB`.
- **Why it matters:** every image carries a complete, flattened copy of the base → disk amplification (critical with 3 GiB free), push/pull without reuse of common layers, inefficient registry caching. It is the no. 1 density/production risk.
- **Acceptance criteria:** in rootless, `history` of `FROM alpine + RUN` shows ≥2 layers; the base-layer digest **equals** that of `alpine:3.24`; two images of the same base share the base blob in the CAS. Suggested path: capture a diff per step from the already-computed snapshot-chain, or `fuse-overlayfs` as the snapshotter, instead of `pack_rootfs_tar` of the full rootfs (which today throws away the lineage, cf. `build.rs:1470`).
- **Dependencies:** none (the format already supports it). **Complexity:** XL. **Tier:** B.

### GAP-0026 — Declarative layer blind to runtime state (no liveness reconciliation)
- **Current behaviour:** `apply`/`plan`/`diff`/`drift` compare **only stamped configuration**. `Facts.running` (`crates/contexts/delonix-stack/src/app/reconcile.rs:663`) only modulates Reboot-vs-Update, never triggers a restart. Pod idempotent by *presence* (`bins/delonix/src/cmd/pod.rs:439-459`: "already exists, nothing to do" does not read Status); container likewise (`container.rs:2634`). `drift` compares stamp vs observed config (`drift.rs:113`, phrase `:302`); `diff` OBSERVED=`a.fields` of config (`diff.rs:34`). There is no declarative control loop. Self-healing exists but is disconnected: per-container supervisor (`crates/contexts/delonix-compute/src/domain/launch.rs:189` `should_restart`, `:149` `start`), `health_monitor_loop` (`container.rs:8553`), `rm_watcher` (`container.rs:4387`), read-only death detection (`reconcile_status`, `container.rs:4334`), boot persistence via systemd (`cmd/boot.rs`).
- **Measured evidence:** killing the container of a declared Pod → `0/1 Stopped`; `drift` says "no drift"; `diff` DESIRED=OBSERVED; re-`apply` "already exists, nothing to do" (does not restart). WITH `restartPolicy: Always` the supervisor recovers (Stopped→Running ~3 s) — so the gap is one of **declarative observation/reconciliation**, not an absence of self-healing.
- **Acceptance criteria:** (a) `drift`/`diff` start reporting a downed Pod/stack member (Running/Stopped as a comparable fact); (b) optionally, an "ensure-running" (generalisation of the `rm_watcher`/supervisor, independent of parentage) that, governed by policy, restarts what was declared. Test: kill the container of a declared Pod → `drift` reports it and reconciliation recovers it.
- **Dependencies:** couples to GAP-0023 (default restartPolicy). **Complexity:** XL. **Tier:** B.

### GAP-0027 — `build-cache` (17 GiB) with no GC; `system prune` does not touch it
- **Current behaviour:** `system prune` removes containers/images/CAS blobs/cgroups/orphan directories, but **not** the build-cache; the only way is to delete `~/.local/share/delonix/build-cache` by hand (confirmed by `system df`).
- **Why it matters:** it is the biggest disk consumer (17 GiB measured) and has no reclamation by command.
- **Acceptance criteria:** `delonix system prune --build-cache` (or `delonix build prune`) reclaims the cache by LRU/age, with `--auto`/`--threshold` for a timer. Test that creates a cache, runs the GC and measures the reclamation.
- **Dependencies:** none. **Complexity:** M. **Tier:** B.
