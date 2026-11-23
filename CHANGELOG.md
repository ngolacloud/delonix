# Changelog

All notable changes to **Delonix** (formerly `delonix-runtime`). The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project
the versioning policy described in [VERSIONING.md](VERSIONING.md).

The prose notes for each historical release live in `docs/releases/v<tag>.md`
(consolidated in `docs/RELEASES.md`); this file is the categorized summary.

> **Version lines.** The product was renamed from `delonix-runtime` to
> **Delonix** and **re-based the version number on purpose**: the
> `delonix-runtime` line (v1.0.0 … v5.1.1) is frozen and becomes **history**; the
> **Delonix** line opens at `0.1.0`. `0.1.0` being lower than `5.1.1` is not a
> regression — it is a new line. The capabilities inherited from the previous line
> are **not** new work in `0.1.0`; see the history section below.

---

## [0.2.0] - 2026-11-22 — the container network is isolated by default, and the node is operable from outside

First **minor** of the Delonix line: 102 commits since `0.1.2`. Under the
pre-1.0 policy a minor may break contracts; this one breaks exactly one, below.
Prose notes: [docs/releases/v0.2.0.md](docs/releases/v0.2.0.md).

### BREAKING

- **A bare `run` no longer shares the host's network namespace.** It gets its
  own, with an outbound uplink: `slirp4netns` when rootless, a shared `dlxnat0`
  bridge (`10.88.0.0/16`) with NAT egress when rootful. `--net host` opts back
  into sharing; `--net none`, `--net <network>` and pods are unchanged, and `-p`
  still adds host port forwards. A container created before this release keeps
  the host netns — `start` reconstructs the mode from the record.
- **And a `kind: Container` with no `network:` means the same thing**, which it
  did not when the change above first landed: the declarative default stayed
  `host` while `run`'s became `private`, so one field meant opposite things
  depending on which door read it. Measured: a hand-made `container run -d`
  against a manifest declaring it planned `network: private → host`, a REPLACE —
  destroy and recreate a container that was already running — so adoption was
  impossible without `--replace`. The declarative default is now `private`
  (`docs/schema/v1/delonix.json` carries it). **A manifest that does not name a
  network changes behaviour**: declare `network: host` to keep sharing. A `Pod`
  is unchanged. One welcome consequence: a `GitOpsSource` may now declare a
  container without naming a network — the guard exists to keep a repo off the
  HOST's network, and an unnamed one is no longer on it.

### Added

- **Node contract over TCP with mutual TLS** (ADR-0096), plus fleet operability
  A2–A6 (ADR-0099): enrollment, authorization, generation, watch and scheduling.
- **SPIFFE node identity** (ADR-0102, phase H0): the X.509-SVID chain, key and
  trust bundle as rustls material, so an external authority can be the issuer.
  Foundation only — no resident process, no rotation loop. `AttestWorkload` RPC.
- **Warm plane** (ADR-0095): pre-loaded runners behind an isolated path, with
  per-image CRI warming and asynchronous refill, after a per-phase profile of
  the cold start found `pre_spawn` dominant.
- **OCI runtime root conformance**: the 5-set capability model, seccomp and the
  stdio plumbing the validation suite reads — 19 → 32 passing assertions.
- **CNCF compatibility campaign foundation**: `tests/interop` and a generated
  `docs/compatibility` matrix.
- **`exec` parity**: `-d/--detach`, `--env-file`, and a `--privileged` scoped to
  the call.
- **Docker-style help**, grouped by command and by flag (#1020).
- **`vm logs`** with `--capture-serial`; **`vm create --distro`**; distroless by
  default with a labelled fallback.
- **TCP `/metrics` listener** for the management API (`DELONIX_MGMT_METRICS_ADDR`).

### Fixed

- `exec -u` loads the supplementary groups from `/etc/group`; `exec -w` on a
  missing directory aborts with rc 126 instead of landing the process in `/`.
- The positional split keeps container flags written after the image, and a
  mistyped flag is no longer consumed as the image or container name — which
  used to send a typo to a public registry.
- `--wait` recognizes the CLI's own probe (`--health-cmd`/`http`/`tcp`);
  `--pid host` starts rootless; `--pod` joins the pod's netns by name.
- `RemoveImage` is idempotent (CRI-F1) and `exec setgroups` tolerates EPERM
  (CRI-F2).
- Egress uses the `inet` family and a per-architecture BPF (#438); the cgroup
  leaf is removed under `--cgroup-parent` (#812); a repeated container name in
  another namespace is a conflict, not "already exists" (#824).
- **A `slirp4netns` with no published ports is no longer un-reapable.** The
  api-socket was passed only when there were ports, and it is not merely how
  ports are published — it is the only ownership token the process carries, so
  one of ours whose supervisor died could never be claimed, and the refusal was
  right (`slirp4netns` is not our binary; rootless Podman spawns it with the
  same argv). It leaked one process per container: measured, a chaos run left 19
  on a clean host while `net netns down` answered "torn down". The socket is now
  unconditional, `net netns down` reaps and says how many, and the `rm` of a
  port-less container reaps its own instead of waiting for the next teardown
  (#1035, #1036). With the bare `run` now getting a netns of its own, every
  container has such a slirp — which is what made this worth the release.
- **A `TMPDIR` long enough to overflow `sun_path` refuses `-p` instead of losing
  the publish in silence.** With a 134-byte socket path, `run -d -p 43225:80`
  answered rc 0, `container port` printed the mapping and `ss` showed nothing
  listening: the publishing loop sits behind `if let Some(sock)`, a guard that
  was safe only while "no socket" meant "no ports". It now refuses before
  starting anything, naming the length, the limit and the remedy; with nothing
  to publish the same path still starts, warning that the slirp carries no
  ownership token.

### Security / Hardened

- `CapInh`/`CapAmb` asserted, `recv_fd` hardened, and the children's credentials
  and `dup2` tightened (P3-1/P3-2).
- The anti-spoof `forget`+`add` became a single `nft` transaction (P3-3).
- The `--gpus dri` glob delivers device nodes only — `by-path` is a directory,
  and it used to be replaced by an empty regular file.

### Dependencies

- `url` 2.5.9 closes **RUSTSEC-2026-0592**; three `deny.toml` ignores that
  matched no crate were removed, each having met the exit condition its own
  comment described.

### Docs

- The gate drift on `develop` was paid rather than absorbed: 119 lines
  translated in three reports, and two counters corrected where they measured
  the sentence describing the rule instead of code.
- A `.mailmap` joins the owner's six identities into one, and a versioned
  `commit-msg` hook refuses AI attribution trailers.

## [0.1.2] - 2026-11-18 — zero-vuln supply-chain, VFIO/platform hardening and CLI parity

Patch on top of `0.1.1`: **additive** on the CLI/API/JSON/schema surface. The
exceptions are **deliberate security hardenings** (VFIO passthrough and the
platform base) that now **reject** what used to be permissive — more
restrictive, never less secure.

### Added

- **Docker/Podman CLI parity:** docker-style per-layer `pull` progress (#1009);
  log capture on *foreground* runs (#1008); forced TTY on `run` and `--disk-size`
  on `vm create` (#1007); *preflight* warning for `EXPOSE` < 1024 in rootless
  `--net host` (#1011, #1014).
- **Kernel-native axis (KRN-0001..0006):** nativization base and backlog (#1013).
- **Reproducible `init`:** lockfile on `init`, per-stack-class e2e and secret
  generation for the WordPress template (#1019).

### Security / Hardened

- **VFIO passthrough (V3-V5):** device allowlist, **non-silent** failures and a
  raw-XML *gate* on passthrough (#1016).
- **NgolaCloud platform base (W1-W4):** hardening + VM/storage audit (#1015).

### Dependencies

- **Dependency review (phases 1-3):** `cargo audit` **8 vulnerabilities →
  0**. `serde_yaml` → `serde_norway`; `ratatui` 0.29 → 0.30 (removes the
  *unsound* `lru` and `paste`); removal of unmaintained crates from the tree;
  maintenance within the *caret*. Zero observable behavior change.

### Docs

- **"Production readiness"** section in the README and on the site's *landing*
  page (#1018).
- Live re-measurement after the dependency review: no regression in startup,
  density or recovery (`reports/production-readiness-2026-11-18/`).

---

## [0.1.1] - 2026-11-12 — parity, live catalog, rootful CRI and supply-chain

Patch on top of `0.1.0`: everything **additive**, **without breaking any
contract** of CLI/API/JSON/schema. Prose notes in [docs/releases/v0.1.1.md](docs/releases/v0.1.1.md).

### Added

- **Docker/Podman CLI parity:** `volume export/import/mount/unmount` +
  `exists/prune`; `network reload/prune/exists`; `container export/mount/unmount/
  clone/checkpoint/restore`; `image build/diff/tree/search/import/history/untag`;
  `pod ps/exists/prune/kill/pause/unpause/stats/top/inspect`; root shortcuts
  `search/import/export/history/untag` and `buildx/builder/bake/checkpoint` shims.
- **Live Kinds catalog (ADR-0086):** `kind: Config` (the plaintext sibling of
  `Secret`), `kind: IPAddress` (reservation from an `IPPool`), `kind: Snapshot`
  (convergent snapshot of `VM`/`Volume`); catalog v1 and v2 on top of the
  restructured engine.
- **Supply-chain:** in-toto attestations (**SLSA v1** provenance + SPDX **SBOM**)
  published through the **OCI 1.1 Referrers API** with a `sha256-<hex>` tag
  fallback; `scan --policy`/`--compare-to` and CI gating; policy-based trust on
  signature admission.
- **Kubernetes interoperability:** `cluster kube play` and `kube generate`.
- **Compose:** `${VAR}` interpolation (with `.env` and braces).

### Changed

- **Rootful CRI conformance:** `rshared` propagation (ADR-0093), per-pod PID
  namespace anchored by `pause` (ADR-0092) and shared-PID on the root-CNI,
  `localhost/…` AppArmor normalization and seccomp with explicit opt-in.
- `perf(linux)`: `ldconfig` is skipped on **musl/scratch** rootfs (no
  `ld.so.cache`).

### Fixed

- **`boot.rs`: valid systemd `Restart=`** — the Delonix policy
  (`unless-stopped`/`always` → `always`, `on-failure[:N]` → `on-failure`) is no
  longer written as an invalid specifier that systemd ignored.
- **`cluster create`: load-aware node wait** — a live node gets an extension
  under contention; a dead node fails immediately (no longer aborts the cluster
  over one slow node).
- **OCI:** whiteout confined to the layer directory via shared `scoped_resolve`
  (symlink escape), own scratch per concurrent unpack.
- **linux/CRI:** cgroup applied before the workload (non-userns); VM
  `move_namespace` writes the record before moving the overlay; the CRI *Streamer*
  tolerates Mutex *poisoning*; OOM/cgroup, `clone3` and `cmsghdr` hardening.

---

## [0.1.0] - 2026-10-28 — Delonix 0.x line (identity transition)

First official release of the **Delonix** line. It opened the `0.x` line
re-basing from `5.1.1` (a new line, not a regression). The engine is the same as
the `delonix-runtime` line — see the history section below.

### Changed
- The product identity becomes **Delonix**; the repository migrates to
  `github.com/ngolacloud/delonix`. The CLI binary was already named `delonix`.
- **ADR-0040 R1 restructuring (internal renames, `publish = false`).** The four
  unprefixed providers are standardized — `delonix-{proxmox,truenas,opnsense,powerdns}`
  → `delonix-provider-*` — and the `delonix-security-runtime` context →
  `delonix-security`. The CLI package `delonix-runtime-bin` becomes `delonix-bin`
  and moves to `bins/delonix`; **the binary is still named `delonix`** (the only
  package in `bins/` that diverges from the directory name, A6a). These are
  internal **crate** names: binaries, sockets, services, `DELONIX_*`, schemas and
  CLI/API contracts stay the same. Moved with `git mv` (history preserved); the
  `scripts/arch_fitness.py` gate now requires the `delonix-provider-` prefix.
- `scripts/version_gate.py` becomes **version-line aware**: the legacy
  `delonix-runtime` line (majors 1..5) and the Delonix line (0.x) are compared
  separately, with a guard that no commit loses the shipped history (it must
  contain the most recent `v5.*` tag). Covered by
  `scripts/test_version_gate.py` (new).

### Added
- `CHANGELOG.md` and [VERSIONING.md](VERSIONING.md) (pre-1.0 compatibility policy
  and the line-transition mechanics).
- Addendum to ADR-0061 bringing the `rust` template (axum/tokio) into the
  `delonix init` contract, with measured verification (generate → compile → run →
  health 200).

### Preserved (DO NOT rename in the transition)
- State directories (`/var/lib/delonix`, `/etc/delonix`,
  `~/.local/share/delonix`), sockets (`/run/delonix-*.sock`), systemd services,
  `DELONIX_*` variables and `*.delonix.io` schema groups — operational contracts
  unchanged between the 5.x and 0.x lines.

---

# History — `delonix-runtime` line (v1.0.0 – v5.1.1)

Capabilities inherited by the Delonix line. Dates = actual tag date.

## [5.1.1] - 2026-10-15
### Changed
- The `--l18n` flag loses `global`, no longer being accepted after a subcommand (it becomes `rc=2` at the end of a normal command); it works before the subcommand or via `DELONIX_L18N`.
- `install.sh` prints a final `downloads and documentation:` line with the release address.
- `Cargo.toml` bumps to `5.1.1` and the `docs/comandos/` pages are regenerated.
### Added
- `build.rs` reads the `tls=` key from `template.meta`; `scaffold.rs` gains `template_supports_tls` and `template_has_version` (still with no caller at this tag).

## [5.1.0] - 2026-10-15
### Changed
- The CRI now has a default capabilities ceiling (`KEPT_CAPS` in `reject` mode), refusing `privileged: true` or `capabilities.add` above it in `CreateContainer`.
- `UpdateContainerResources` implemented and location-aware (kubelet scope vs leaf), with field-by-field merging.
- Hot CPU and memory hotplug for Cloud Hypervisor, libvirt and Proxmox, restarting instead of replacing the VM.
- The eleven `delonix init` templates are born with tests, two CIs, Conventional Commits, SonarQube and an architecture section, with a gate requiring it.
- The coverage of CLI leaves executed and judged by the battery rises from 126/272 to 258/270 (95.6%).
### Added
- Storage pools (ADR-0067) with a root helper per socket and `btrfs`, `zfs` and `lvm-thin` drivers.
- `VirtualMachineService` served on the node API (ADR-0040 P5), with socket activation and idle shutdown.
### Fixed
- `secret rotate-key` no longer rolls the version back to 1 (ACH-034).
- A network route becomes usable from an additional network (#733); a failed startup no longer tears down the network of a running container.
### Security
- The admission gate fails closed on a scan failure, not only on findings (#778).
- The optional `denyPrivateEgress` guard keeps workloads out of private space and CGNAT; layer extraction gains a ceiling on entries and bytes against decompression bombs.
### Removed
- `image vm …`, `image --vm …` and the flat forms `vm build|convert|pull|push|ls-remote` leave without an alias; `delonix vm image <verb>` is the only path (#772).

## [5.0.2] - 2026-10-11
### Fixed
- An open `NetworkRoute` now serves containers connected to more than one network via `network connect`, installing a route and a mirrored route per dedicated protocol (#733).

## [5.0.1] - 2026-10-10
### Security
- `--secret-files` aborts the container init (exit 126) when the tmpfs does not mount, instead of writing the secret to the persistent layer (`3626ae70`).
- Scan admission (`DELONIX_SCAN_ON_PULL`) refuses the pull and removes the image on any scan failure; only `DX-1401` (image without a package manager) is let through (#778).

## [5.0.0] - 2026-10-08
### Changed
- BREAKING: a container runs as the image's `USER` instead of uid 0 (ADR-0062); without `--user` the process is no longer root, recoverable with `--user 0`.
- BREAKING: `--user` no longer re-owns the entire filesystem, applying only the image's ownership index (odoo went from 16 min to 1.7 s).
- BREAKING: the `can_bind_host_port` and `host_port_busy` signatures of `delonix-sdn` now take a `Proto` (#718).
- The bind address of a published port is now recorded as `addr:hostPort:contPort/proto` and shown in `container ls` (#718).
- The `provider` block becomes typed inline `type` + `spec` (ADR-0070/0071); `kind: NetworkZone` becomes `kind: Network` with `spec.provider.proxmox`; the old spellings load as superseded.
### Added
- `kind: SystemContainer` for Proxmox LXC (ADR-0058) with complete day 2; `kind: StoragePool` with the `dir` driver (ADR-0067 P0).
- `delonix serve node-api` serves the node contract over gRPC and HTTP/JSON on a unix socket, with RFC 9457 errors and generated OpenAPI (ADR-0042).
- Versioned secrets with `secret apply` reporting `created`/`unchanged`/`rotated to version N`; network providers per role (`GatewayProvider`, `SegmentProvider`).
- `hostPort` published in root/CNI mode for tcp, udp and sctp (ADR-0074); FreePBX 17 + Asterisk 22 appliance.
### Security
- A named namespace on a host without `br_netfilter` is refused (`DX-6305`, exit 69), with a `DELONIX_ALLOW_UNENFORCED_ISOLATION=1` relief valve.
- Anti-spoofing moves to the `bridge` table where it actually cuts (MAC, IPv4 source and ARP sender per port); a policy-governed workload is born closed.
- Lab credentials (`opnsense-lab.env`) are now ignored by git (#716).
### Fixed
- An unknown field refuses the entire manifest before any effect (ADR-0069), including inside `spec.containers[]`; an unreadable store is an error and not an empty machine.
- An `rm -f` that gave up is not resurrected by the supervisor; a container's `slirp4netns` terminates with it (#646, #648, #651).

## [4.5.0] - 2026-09-28
### Changed
- The per-container firewall fails closed: a spec with an invalid rule, `dir`, `action` or policy is refused entirely instead of applied partially.
- A namespace isolation failure becomes an error (not a warning) in `container run`, `start` and `pod create`, undoing the attach.
- `--cpuset`, `--io-weight` and `--device-read/write-bps/iops` are refused (exit 69) when the cgroup lacks the controller, with a `DELONIX_ALLOW_UNENFORCED_LIMITS` relief valve.
- `container rm -f` only returns once the process has died; after 30 s it keeps the record and returns `DX-8101` (exit 124).
- Proxmox: measured API coverage rises from 16 (2.4%) to 161 routes (23.9%); errors typed by HTTP status and a task log per VM.
- Streaming image pull and push without the whole image in memory (RSS peak of the `node:22` pull: 214 MB → 27 MB).
### Added
- Verbs `vm resize`, `vm cloud-init`, `vm move`, `provider ls|describe|matrix|config`, `net netns gc`, `serve node-api`; Kinds `NetworkGateway` and `NetworkZone`.
- A `providers.yaml` file per node (ADR-0054); a versioned capability catalog (ADR-0050); the MCP tool `workload.usage`.
- `vm create --require <capability>`, `--allow-mac-spoofing` and booting an image from the engine store on Proxmox (ADR-0057).
### Fixed
- `network rm` refuses a network with something attached (`DX-5307`, exit 5) and names the consumers; transactional IPAM refuses to reserve an IP owned by someone else (`DX-5308`).
- The VMs' DHCP pool now lives inside the network, serving the correct CIDR; the binary no longer dies from SIGPIPE on a network socket write (#573).
- `system df` counts the entire store (190 GiB were previously reported as 105); the tests stop polluting `/tmp`.

## [4.4.0] - 2026-09-23
### Added
- `delonix drift` compares the last apply's stamp against the observed one, with an optional manifest (M05).
- `compatibility compose` (28 served, 12 refused, 49 missing of 89 keys) and `migrate assess -f docker-compose.yml` (M02).
- `kind: RuntimePolicy`, the admission ceiling in declarative form that `stack apply` can only raise or tighten (M04); Kinds in the manifest 19 → 20.
- `RUN --mount=type=cache` and automatic SPDX 2.3 SBOM per built image (M03); `POST /images/create` on the Docker API.
- `scripts/bench_gate.py` with three outputs, including `3` when it refuses to judge due to bench noise.
### Changed
- BREAKING for publishers: pushing a tag no longer publishes; the tag and the release become two decisions via `gh workflow run` (#472).
### Security
- A fuzz job in CI found and fixed an OOM from an expansion bomb in `parse_dockerfile`, with a 1 MiB ceiling in `substitute_vars` (#461, #463).

## [4.3.0] - 2026-09-21
### Added
- The `delonix hosts` group (not stable) with `hosts sync` to publish service names in `/etc/hosts` (ADR-0048 phase 2).
### Changed
- `install.sh` now accepts `aarch64`/`arm64`, composing the asset name from `uname -m`; it is the first release whose installer accepts it.
- Off x86_64, Cloud Hypervisor, EDK2 and `hypervisor-fw` are not downloaded, and the VM backend is libvirt.
### Fixed
- The v4.2.0 notes are corrected: the libvirt VM and `kind: IPPool` were already built (phases 2 and 3 of ADR-0046).

## [4.2.0] - 2026-09-21
### Added
- `delonix vm build` and the `vm.yaml` file (the "compose" of the `VMfile`), with routes per distro and per `appliance:` (#451).
- `VirtualMachine.spec.expose` publishes a VM's HTTP/S service by name; `kind: IPPool` for host address reservations (ADR-0046).
- Default service name `<name>.<ns>.svc.delonix.internal` and a `SVC` column in `container/vm/stack ls` (ADR-0048 phase 1).
- Monitoring, Carbonio CE, GLPI 11 and Wazuh appliances; `install.sh --performance`.
- aarch64 binaries (`delonix`, `-cri`, `-mcp`, `-mgmt`) published for the first time.
### Changed
- `delonix-cri`, `delonix-mcp` and `delonix-mgmt` respond to `--version`/`-V` and exit instead of starting the server.

## [4.1.1] - 2026-09-20
### Security
- `vm migrate`: shell injection and namespace isolation collision fixed (CRITICAL); `kubeadm join` validates the remote output before passing it to a shell.
- A cluster's `metadata.name` is validated before composing paths (a name with `..` wrote the etcd CA outside the root); the CRI's `ExecSync` gains a size ceiling.
### Added
- `vm destroy` (cross-provider teardown); `vm create --rm` / `--rm-after` for disposable VMs; `--cri-bin` in `cluster apply` and `cluster kubeadm`.
### Fixed
- `prepare_host` compares the CRI's `sha256` instead of just "the service is alive", ensuring that a fix reaches an existing node (#242).
- `resolve_cri_bin` no longer compiles the source code near the cwd; only with `DELONIX_CRI_FROM_SOURCE=1` (#243).

## [4.1.0] - 2026-09-19
### Fixed
- A fresh root node again accepts `-m`/`--cpus`/`--cpu-weight`: `cgroup_limits_apply()` confirms that the controllers are in `cgroup.subtree_control` and names the missing controller instead of reporting "Permission denied" (#415).
### Changed
- ADR-0043 closes on the last five crates (`delonix-vm`, `-truenas`, `-proxmox`, `-linux`, `-sdn`); the `DX-CDNN` dictionary goes from 61 to 198 codes and the ADR becomes Accepted.

## [4.0.0] - 2026-09-17
### Changed
- BREAKING: `delonix-scan`→`delonix-scanner`, `delonix-image`→`delonix-oci`, `delonix-net`→`delonix-sdn`, `delonix-runtime`→`delonix-linux` (ADR-0040 phase P3).
- Engine restructuring in layers (ADR-0040): contexts, ports and adapters; `serve cri`/`serve api`/`mcp` now run their own binaries instead of embedding them.
### Removed
- `delonix-runtime-core` removed; shared types moved to `delonix-model` and the persisted state to the new `delonix-state`.
### Added
- The `DX-CDNN` code dictionary (ADR-0043) with `delonix explain <code>`; `delonix compatibility docker`.
- `delonix init` adopts an existing directory instead of dumping examples; `network ipam ls`/`prune`.
### Fixed
- A new root node booted pods again (#310); the single-node control-plane stopped crash-looping when declaring `cgroupDriver: cgroupfs` (#315).
- The cloud-hypervisor `boot` only returns once the VMM confirms the VM is running (#350); `POST /wait` returns the real exit code.

## [3.1.0] - 2026-09-11
### Fixed
- The `--health-cmd` probe left two zombies per execution, exhausting `pids.max`; the watchdog now reaps its own child.
- A requested `stop` could end up `Crashed`; `start`/`restart` lost the supervisor and the exit code; `container healthcheck` ignored the container's own probe.
- `image load` deleted the other names of the same image; `secret create` over an existing secret deleted non-repeated keys.
- `stack apply` adopted someone else's resource saying "nothing to do"; the resource ceiling covered only PID 1.
### Changed
- README and `delonix --help` aligned with the CLI restructuring (33 groups, categorized COMMAND MAP), with two new gates (ACH-032, ACH-033).

## [3.0.0] - 2026-09-09
### Removed
- `image build|convert|import|init|ls-remote` (the `--vm` forms), `sharevolume apply|describe|ls|rm|migrate`, `storage create|ls`, `pod ls` and `vm status` leave without an alias (249 → 244 leaves).
### Added
- `kind: Service` (ADR-0032) and `kind: App` with Cloud Native Buildpacks (ADR-0035); `image sign` (ECDSA-P256 cosign), `cluster drain`/`uncordon`/`upgrade`, `vm migrate`, `vm pause`/`unpause`, `net capture`, `pod port-forward`.
- Full `docker compose`: `profiles:`, `extends:`, `build.target`, `deploy.replicas`, `ipv4_address`, `configs:`/`secrets:`, anonymous volumes and multi-file.
### Fixed
- `PodSandboxStatus` returned `linux: null` and no `kubeadm` came up (#233); a privileged container took the `masked`/`readonly paths` and brought down `kube-proxy` (#239).
- `mount(2)` truncated the `lowerdir=` above ~4 KB; the overlay moves to the `fsopen`/`fsconfig`/`fsmount` API (ADR-0037).

## [2.0.0] - 2026-09-02
### Changed
- `image list`/`backup list` revert to `image ls`/`backup ls` without an alias (the `list` spelling fails with `unrecognized subcommand`, rc=2); `image remove` stays.
### Added
- `delonix system resources` reports the cgroup controllers that are actually applicable, the PSI pressure and `DLX-RES-nnn` findings; `system regulate` cuts and restores `cpu.weight` and `memory.high` per cause.
- `delonix diff <kind> <name>`, `cluster health`, `cluster describe`, `system metrics`, `config get|set|unset`, `secret rotate`; `get`/`describe`/`delete` reach `NetworkRoute` and `NetworkPolicy`.
- New columns in `volume`/`network`/`pod`/`secret`/`image`/`container`/`cluster ls` (SIZE, USED BY, AGE, RESTARTS, ORPHAN, K8S VERSION).
### Fixed
- The aggregate ceiling now exists in rootless on the delegated `dlx-containers`; `image remove alpine:latest` no longer unmarks an image that a live container depends on.

## [1.1.0] - 2026-08-30
### Added
- `delonix-security-runtime`: the VM path goes through admission before resolving/downloading, with the `denyDevicePassthrough`, `denyLatestVmImage` and `allowedImageUrlHosts` guards (ADR-0026).
- `delonix mcp serve|doctor|capabilities`: a local stdio MCP server for agents, with explicit `confirm` above `SAFE_WRITE`.
### Security
- The VM guards are born disabled so as not to break fleets; the node points out the open half with a lint silenceable via `DELONIX_POLICY_LINT=0`.
### Fixed
- An `exec` right after `run -d` could run on the host filesystem; fixed by waiting for `setup_rootfs`.
- The CRI's `exec` killed by pid number (it could reap another process); it moved to `pidfd`.
- Nine errors said "does not exist" about things that do exist (the misleading `ENOENT` pattern); `kind: Ingress` no longer silently discards `tls[].hosts` and other fields.
### Removed
- `storage dash|inspect|rm` collapse into `volume` (a non-stable group); `storage create|ls` stay.

## [1.0.0] - 2026-08-29
### Added
- First stable release: `docs/cli-stability.md` becomes the semver contract.
- Rootless-first and daemonless container and microVM engine in Rust: complete lifecycle, multi-container pods, GPU via CDI, build with Dockerfile/Delonixfile, rootless SDN, VMs (Cloud Hypervisor/libvirt/Proxmox), `delonix-cri`, `cluster kubeadm` and declarative infrastructure via Kinds.
### Changed
- The public surfaces now say "Delonix Engine"; the repository name and the internal doc-comments keep "Delonix Runtime".
### Removed
- The six top-level shortcuts `ps`, `run`, `exec`, `logs`, `rm` and `images` leave without an alias; use `container <verb>` and `image list`.
### Fixed
- `image rm`, promised stable but dead since B2 (it became `image remove` without an alias), fixed in the same window.

---

Versions prior to `v1.0.0` (the `v0.x` development line) in
`docs/releases/archive/`.
