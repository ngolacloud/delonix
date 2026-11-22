# 61 — P4b: the measured plan for carving up `delonix-vm` (ADR-0044, accepted 2026-09-24)

**Date:** 2026-09-24 · **Base:** `origin/main` (`355aea8a`, v4.4.0 +3 commits) ·
**What this is:** not a spike — it is the measurement that decides the order in which P4b
is cut, done BEFORE moving a single line, so each slice has its own gate (ADR-0044 D9) and
none leaves `arch_fitness.py` red along the way.

## The question

ADR-0044 D9 describes P4b in one line: «`VmSpec`/`Extensions`/`VmProvider` land in
`delonix-compute`; `delonix-provider-cloud-hypervisor`/`-libvirt` split out of
`delonix-vm`, `delonix-vm` retired». The crate has 8,561 lines in a single `lib.rs` and six
consumers. The question is what has to come out FIRST for the two provider crates to be
able to exist without violating layer direction, and the answer comes from a rule the ADR
cites but does not confront against the code: **a `PROVIDER` crate may only depend on
`FOUNDATION` and `CONTEXT`** (`scripts/arch_fitness.py`, `ALLOWED`). A
`delonix-provider-libvirt` that depended on `delonix-vm` (an `ADAPTER`) fails the gate on
the first commit. So everything a backend implements or calls has to be in a context before
the backend changes crate — and that is more than the D1–D3 port.

## What was measured

**The crate by zones** (`crates/adapters/delonix-vm/src/lib.rs`, today's line numbers):

| Zone | Lines | What it is |
|---|---|---|
| shared helpers | 1–1602 | `VmConfig` (67), the `VmBackend` port (849), the registry (`BackendRegistration`/`register_backend`/`select_backend*`/`require_capabilities`/`auto_detect`, 1066–1478), default backend (1495–1560), `mem_mib`, `mac_for`, `vm_namespace_supported`, RAM admission, `terminate_vmm`, `qemu-img`/lease parsers |
| Cloud Hypervisor | 1603–2378 | `CloudHypervisorBackend`: `boot_ch`, console/serial, offline snapshots |
| libvirt | 2379–4224 | `LibvirtBackend`: `libvirt_domain_xml`, `with_stopped_domain`, snapshots, DHCP reservation |
| orchestration | 4225–5417 | `create`/`create_with`, `remove`/`destroy_with`, `stop`/`pause`/`unpause`, snapshots, `backup_disk_live`, `start`/`restart`/`status`/`list` |
| tests | 5418–8561 | four `#[cfg(test)]` modules |

Plus `capabilities.rs` (399, the two ADR-0050 declarations with host probing),
`cloudinit.rs` (320), `error.rs` (465) and `provider_spike.rs` (412), which this slice promotes.

**The consumers**, by `grep -rho 'delonix_vm::…'`:

| Consumer | Distinct symbols | The ones that weigh |
|---|---|---|
| `delonix-runtime-bin` | 46 | `list` (24 sites), `status` (19), `create` (12), `valid_backend_name` (9), `stop`, `remove_force`, `VmConfig`, `create_with`, `backup_disk_live`, snapshots, `set_network` |
| `delonix-proxmox` | 5 | `BackendRegistration`, `register_backend`, `Error::Engine`, `cloudinit::DEFAULT_CI_USER`, `VmVolume` (already belongs to compute) |
| `delonix-linux`, `delonix-mgmt`, `delonix-mcp` | 4 | `create`, `list`, `start`, `status` |

**The dependency nodes of each zone** — what stops it from living in a context as it
stands (`grep` over each range, not reading):

| Zone | External nodes | Where each belongs |
|---|---|---|
| orchestration | `JsonStore::update` + `store(base)` (delonix-state); `std::fs::{remove_file, canonicalize, copy, create_dir_all, read_dir, remove_dir_all, symlink_metadata}` over the state root; `qemu-img create` (overlay) and `cloud-localds` (seed) via `Command` | the registry is the D6 `StateRepository<Vm>` (the trait already lives in `delonix_model::ports`, the `impl` for `JsonStore<T>` already exists); the `fs` work over the state root itself is what `delonix-node` already does today in a context; the overlay and the seed are LOCAL PROVIDER work, not use-case work |
| Cloud Hypervisor | `stable_cmd("qemu-img")`, `network()` (the `VmNetwork` port, already in compute) | stays in the provider; `VmNetwork` is already a port |
| libvirt | `virsh` (15 sites), `delonix_state::write_atomic` (3: the domain XML and preserved snapshots), `libc::getuid/getgid` (rootless seclabel) | `virsh` stays in the provider; `write_atomic` is the `ConfigWriter` the 2026-09-19 D6 addendum already mapped to this crate; `libc` is a legitimate workspace dependency in a provider |
| helpers | `terminate_vmm`/`proc_state`/`vmm_left` (signals and `/proc`, via `delonix_node::safe_to_signal`), RAM admission (`/proc/meminfo`) | the two belong to shared locals: either `delonix-node` (questions to the host, where `safe_to_signal` already lives) or the context |

Two facts only the measurement produced, which the ADR does not have: (1) **orchestration
does not belong to the providers** — `create_with` canonicalizes the disk, creates the
overlay, generates the seed, asks the holder for the `tap` and only then calls
`backend.boot`; none of that is the backend's job, which is exactly why
`manages_own_storage` exists (ADR-0008): local orchestration is intertwined with the two
local backends **inside the same file**, and the cut has to separate them before separating
the backends themselves; (2) **`delonix-proxmox` can become dep-clean BEFORE P4c** — of the
five symbols it uses, only `BackendRegistration`/`register_backend` and `Error::Engine`
belong to `delonix-vm`; once the registry and the error are in the context, the exception
`("dep", "delonix-proxmox", "delonix-vm")` falls away without Proxmox changing to the new port.

## The slices, in the order layer direction forces

**P4b.1 — the port in the context (this PR).** `delonix_compute::vm_provider`: `VmSpec`,
`Extensions` (+ `CloudHypervisorExt`/`LibvirtExt`), `ProviderId`, `Provider` (id +
`capabilities()` = ADR-0050's `ProviderReport`, `health()` derived from it),
`IpConfidence`, `VmHandle`, `VmObservation`, `VmProvider`. `delonix-vm`'s `provider_spike.rs`
becomes `provider.rs`: `spec_to_config` (the only thing that knows `VmConfig`),
`LocalVmProvider` and `registry(id)`, implementing the context's port. A correction to the
spike, from the ADR's own D1: `bridge` is universal (Proxmox reads it) and the spike had it
in `LibvirtExt`; it moves to `VmSpec`. Nothing changes for any caller. **Gate:**
`delonix-compute` and `delonix-vm` tests, `provider_live` compiling, fitness unchanged (the
context gains no new dependencies), `cargo check --workspace`.

**P4b.2 — what the providers implement leaves the adapter.** To `delonix-compute`:
`VmConfig`, `Boot`, `CreateStage`/`DestroyStage`, the `VmBackend` trait, the whole registry
(`BackendRegistration`, `BackendFactory`, `ReportFactory`, `register_backend`,
`select_backend*`, `resolve_required_capabilities`, `require_capabilities`,
`auto_detect`, `provider_reports`), `mem_mib`, `valid_vm_name`, `mac_for`,
`vm_namespace_supported`/`restart_policy_unsupervised`, `cloudinit::DEFAULT_CI_USER`, and
the `Error` variants Proxmox builds. `delonix-vm` re-exports everything via
`pub use` — zero caller changes a line. **What gets decided here**: the registry is a
`static` with `OnceLock<RwLock<Vec<…>>>`; in a context it is still a map populated at
startup by composition (`cmd/vmbackends.rs`), with no I/O at registration time (ADR-0008).
**Gate:** the exception `("dep", "delonix-proxmox", "delonix-vm")` removed — Proxmox now
depends only on `delonix-compute` — and `delonix-vm`'s 126 tests unchanged.

**P4b.3 — orchestration becomes context use cases.** `create_with`/`stop`/`start`/
`status`/`list`/`remove`/`destroy_with`/snapshots/`backup_disk_live` move to
`delonix_compute` (a `vm` module, D4's `app`), receiving `&impl StateRepository<Vm>`
from composition instead of opening the `JsonStore` directly (closing half of exception
`("dep", "delonix-vm", "delonix-state")`). The overlay (`qemu-img`) and the seed
(`cloud-localds`) move out of `create_with` into a `LocalDiskImages`/`SeedBuilder` port
that the two local providers use and an adapter implements — `delonix-guestfs`, which
ADR-0040 D2.3 already names for `qemu-img`/`virt-customize` —, because a provider cannot
depend on an adapter and duplicating `qemu-img create` in both crates would be the second
copy this repo has already paid for with `mem_mib`. **This is the series' design point**
and gets its own addendum before the code. **Gate:** the `-bin`'s 46 symbols keep
resolving (via `pub use`), the `scripts/e2e.sh` `vm` section battery and the
`control_restart` chaos scenario without regression, `VmNetwork` remains the only path to
the holder.

**P4b.4 — the two provider crates, and `delonix-vm` retires.**
`crates/providers/delonix-provider-cloud-hypervisor` and `-libvirt`, each with its own
`VmBackend`/`VmProvider`, its own capability declaration and its own probe (`capabilities.rs`
splits in two), registering through composition the way Proxmox already does. What is left
in `delonix-vm` at that point is measured then; the intent is zero. `LAYERS` gains the two
new names, `dev_docs.py` regenerates the manual, `install.sh` does not change (the providers
are libraries linked into `delonix`). **Gate:** `delonix-vm` out of the workspace, the
remaining P4 exceptions for this crate out of the table, `provider_live` and the battery's
`vm` section green against the binary built from the new crates.

## What is deliberately left out

Proxmox on the `VmProvider` port (P4c), `delonix-launcher` (P4d, it is another adapter),
`delonix-provider-mount`/`-truenas` (P4e), and a typed `DiskSource` — each with its own line
in D9. And **no slice changes the on-disk format of `Vm`/`VmBootSpec`**: P4b moves code, not
records.

## Proven vs not validated

Proven: the zones, the consumers and the nodes above, via `grep`/`sed` over the stated
base; that `delonix-proxmox` only touches five symbols; that the `PROVIDER →
{FOUNDATION, CONTEXT}` rule is in fitness's `ALLOWED` and is not opinion. Not validated:
the real cost of slices 3 and 4 (the orchestration↔backends entanglement can only be
measured by undoing it), and whether `delonix-linux`/`-mgmt`/`-mcp` reach the use cases via
`pub use` or need their import changed — the four symbols are the same, so it should be
`pub use`.

## P4b.2 Addendum (2026-09-26) — what moved, and the registry left for P4b.4

**Done.** To `delonix-compute`: `vm_backend` (`VmConfig`, `CloudInitIntent`, `Boot`,
`CreateStage`/`DestroyStage`, the `VmBackend` trait with the three `unsupported_*`,
`BackendFactory`/`ReportFactory`/`BackendRegistration`, `mem_mib`/`parse_mem_mib`,
`DEFAULT_CI_USER`), `vm_error` (the whole `Error`/`Result`, with `From<Error> for Dx`) and
`vm_firewall`. `delonix-vm` re-exports everything under the usual names; no caller in
`-bin`, `delonix-linux`, `mgmt` or MCP changed a line. `delonix-proxmox` now depends only
on `delonix-compute`, and the exception `("dep", "delonix-proxmox", "delonix-vm")`
dropped out of `arch_fitness.py` (10 → 9 exceptions).

**The deviation from the plan, and why.** The plan put the REGISTRY (the `static`,
`register_backend`, `select_backend*`, `auto_detect`) in this slice. Measured while
cutting: the registry is seeded with `CloudHypervisorBackend` and `LibvirtBackend`, which
only leave `delonix-vm` in P4b.4. Moving it now would have required a cross-crate seeding
hook, or letting auto-detection order depend on who registers first. This slice's gate does
not need it: Proxmox stops registering itself and returns a `BackendRegistration`
(`delonix_proxmox::registration`), which the composition root (`cmd/vmbackends.rs`)
registers — the shape ADR-0008 describes. The registry moves down in P4b.4, when the two
local backends are also registered by composition and there is no seeding left to carry.

**Two conversions stayed in the adapter**, by the orphan rule: `From<cloudinit::Error>`
(the source type belongs to `delonix-vm`, stays as `impl`) and `From<delonix_state::Error>`
(the two types now belong to other crates, so a `state_err` function took over, called at
the 14 sites that used to do `?` over the `JsonStore`).

## P4b.3 Addendum (2026-09-29) — the design, measured before the code (proposal)

**Base:** `origin/main` `0f851aab`. The plan above said P4b.3 «gets its own addendum
before the code». This is it. None of what follows has been written in Rust yet.

### What was measured

Orchestration today occupies lines **3905–5361** of `crates/adapters/delonix-vm/src/lib.rs`
(1,457 lines; the plan measured 4225–5417 over `355aea8a` — it grew with `resize`, ADR-0053's
`set_cloud_init`, `move_to_node` and `guest_info`). The public functions are the ones
`-bin`, `delonix-linux`, `mgmt` and MCP call: `create`/`create_with`,
`remove`/`remove_force`/`destroy`/`destroy_with`, `stop`, `pause`/`unpause`, `set_cloud_init`,
`resize`, `move_to_node`, `guest_info`, `snapshot`/`restore`/`snapshots`/`delete_snapshot`,
`backup_disk_live`, `apply_firewall`/`read_firewall`, `start`/`restart`/`status`/`list`.

The external nodes of that range (`grep` over the range, calls counted without comments
or messages):

| Node | Sites | Where it belongs |
|---|---|---|
| `store(base)` / `JsonStore` (`load`/`save`/`update`/`remove`/`list`) | 15 + 18 | D6's `StateRepository<Vm>`, injected |
| `backend_for` / `select_backend_requiring` / `require_capabilities` / `resolve_required_capabilities` / `firmware_boot_preference` / `standing_backend_choice` | 18 + 6 | the registry — which **stayed in `delonix-vm`** in P4b.2 (see the previous addendum) |
| `qemu-img` (overlay `create -b`, `info`) | 3 calls | a local-disk port |
| `cloudinit::generate_seed_iso` → `cloud-localds` | 1 | a seed port |
| `virsh` (`domblklist`, `blockcommit`) + `libvirt_domain_uri`/`libvirt_cleanup`/`libvirt_poweroff` | 4 calls + 5 | **the libvirt backend**, not orchestration |
| `network()` (the `static` behind `set_network`) | 1 | the `VmNetwork` port, already in compute — passed, not global |
| `vm_admission_check` → `/proc/meminfo` | 1 | a question to the host: `delonix-node` |
| `std::fs` over the VM's own directory (creating, deleting VM files, `path_size`, the lock) | 13 | stays with the use case: it is the context's own state root, which `delonix-node` already does today |

And one fact about the contexts: **no crate under `crates/contexts/` calls `Command::new`
today** (`grep`: zero). `qemu-img` and `cloud-localds` cannot simply descend with
orchestration.

### Four findings that change the plan's wording

1. **`backup_disk_live` is not orchestration** (4880–5025, with `blockcommit_argv`): it is
   entirely `virsh domblklist` + `qemu-img` + `virsh blockcommit --pivot`, i.e. the libvirt
   backend's mechanism. A method moves up to the port — `VmBackend::backup_disk_live(vmdir, vm, dest,
   quiesce)`, refusing by default with `unsupported_*` as the other verbs already do — and the
   use case is left with `load` + dispatch. As the plan stood, this block would have landed
   in the context with `virsh` inside it.
2. **The "unregistered libvirt domain" lives in orchestration** — `remove_inner` (cleans up
   an orphaned domain from an old `rm`) and `stop` (powers it off instead of answering
   "no such VM"). This is libvirt-specific knowledge. It becomes a question to the backend
   set by name, with no registry lookup: `stop_unrecorded(name) -> Result<bool>` and
   `remove_unrecorded(name) -> Result<bool>`, which only libvirt answers `true` to.
3. **The registry stays in the adapter** (the P4b.2 deviation), so a compute use case
   cannot call `backend_for`. Backend resolution is also injected — a `VmBackends` port
   that `delonix-vm` implements over its registry until P4b.4, and that composition
   implements once the registry moves down.
4. **`StateRepository<T>` is not object-safe** (`update<F>` is generic). The use cases are
   generic over the ports, exactly like `container run`'s `resolve_run<I, S, D, H>`
   (`docs/discovery/54`), and do not receive `&dyn`.

### The ports (in `delonix_compute::ports`, next to `VmNetwork`)

```rust
pub trait VmBackends {
    fn for_vm(&self, vm: &Vm) -> Result<Box<dyn VmBackend>>;
    /// A escolha do `create`: o nome pedido, o que o operador fixou, a auto-detecção
    /// por capacidades e a preferência de firmware — a política inteira do registo.
    fn select(&self, root: &Path, cfg: &VmConfig) -> Result<Box<dyn VmBackend>>;
    fn manages_own_storage(&self, want: Option<&str>) -> bool;
    fn stop_unrecorded(&self, name: &str) -> Result<bool>;
    fn remove_unrecorded(&self, name: &str) -> Result<bool>;
}
pub trait LocalDiskImages {
    /// O overlay qcow2 de uma VM sobre o disco base: `canonicalize`, formato do
    /// backing lido do ficheiro (nunca da extensão), tamanho pedido ≥ o do base.
    fn overlay(&self, vmdir: &Path, name: &str, base: &Path, size_gib: Option<u32>) -> Result<PathBuf>;
}
pub trait SeedBuilder {
    fn seed(&self, vmdir: &Path, name: &str, intent: &CloudInitIntent) -> Result<PathBuf>;
}
```

`LocalDiskImages` and `SeedBuilder` are two ports, not one: the seed is cloud-init, not a
disk image, and a provider with its own storage (Proxmox, `manages_own_storage`) uses
neither. In P4b.3 all three are implemented by `delonix-vm` itself (it is an adapter;
`Command` is legitimate there). In P4b.4 the disk and seed implementation moves to its own
adapter — `delonix-guestfs`, which ADR-0040 D2.3 already names for `qemu-img` —, and the two
local providers receive it from composition: a provider cannot depend on an adapter, and
duplicating `qemu-img create` in both crates is the second copy `mem_mib` has already cost.

RAM admission splits in two: reading `/proc/meminfo` moves down to `delonix-node`
(next to `proc_starttime`, which `adopt_pid_starttime` already uses), and the verdict
(`admission_verdict`, pure and already tested) goes with the use case.

### What does not change

- **`delonix-vm`'s public surface**: the `-bin`'s 46 symbols and the four from
  `delonix-linux`/`mgmt`/MCP keep resolving, now as thin wrappers that open the
  `JsonStore`, pass the registry, the disk, the seed and the network, and call the use case.
- **The on-disk format** of `Vm`/`VmBootSpec` and the per-VM lock file.
- The exception `("dep", "delonix-vm", "delonix-state")` stays until P4b.4 (the wrappers
  still open the `JsonStore`), but the use cases stop depending on `delonix-state`.

### In two slices, each with its own gate

- **P4b.3a — backend-specific knowledge leaves orchestration, still inside `delonix-vm`.**
  `VmBackend::backup_disk_live` (libvirt implements it, the others refuse by name), the
  `stop_unrecorded`/`remove_unrecorded`, reading `/proc/meminfo` in `delonix-node`, and the
  ports declared in compute with the implementation in `delonix-vm`. No function changes
  crate. **Gate:** `delonix-vm`'s tests unchanged, the `scripts/e2e.sh` `vm` section
  (the `backup create vm` of a running libvirt VM with two disks, the case the 2026-09-24
  battery already covers), and zero `virsh` in the orchestration range.
- **P4b.3b — the use cases move down to `delonix_compute::vm`**, generic over
  `StateRepository<Vm>`/`VmBackends`/`LocalDiskImages`/`SeedBuilder`/`VmNetwork`, with tests
  in compute against fake ports (an in-memory repository, a backend that records calls) —
  the gain that justifies the cut: today these paths can only be tested against a real
  state root. **Gate:** the 46 symbols, the battery's `vm` section and the
  `control_restart` chaos scenario without regression, and a new counter in
  `arch_fitness.py` — `context_spawns` (`Command::new` in `crates/contexts/`) with a
  baseline of **0**, so the context never gains its first `Command` along the way.

### Proven vs not validated

Proven, via `grep`/`sed` over `0f851aab`: the range, the nodes and the table's counts; that
`backup_disk_live` and handling of the orphaned domain only call `virsh`/`libvirt_*`; that
no context calls `Command::new`; that `delonix-compute` already depends on `delonix-node`;
that `StateRepository<T>` is not object-safe. Not validated: whether `select` fits in a
signature without dragging in more policy than listed (auto-detection and firmware are
measured while cutting), and the real cost of P4b.3b — the entanglement can only be
measured by undoing it, as the plan already said.

## P4b.3a Addendum (2026-09-29) — backend knowledge left orchestration

**Done, all still inside `delonix-vm`.** Orchestration went from 1,457 to 1,165 lines and
today has **zero** `virsh`, `libvirt_domain_uri`/`libvirt_cleanup`/`libvirt_poweroff`,
`qemu-img`, `cloudinit::` and `backend_for` — counted over the range from `valid_vm_name` to
the first test module. The 15 calls to `store(base)` remain, which belong to P4b.3b.

- **`VmBackend::backup_disk_live`** in the port (compute), with the same default refusal
  orchestration used to give, same text and same DX-1514; libvirt implements it with the
  same body as before (`libvirt_backup_disk_live`, now next to `LibvirtBackend`).
- **The three ports** in `delonix_compute::ports` and the implementation in `delonix-vm`
  (`local_ports.rs`: `RegistryBackends`, `QemuImgDisks`, `CloudLocaldsSeed`). Choosing a
  new VM's backend moved out of `create_with` into `select_for_create`, next to the
  registry; `prepare_local_overlay` moved inside `QemuImgDisks`.
- **Reading `/proc/meminfo`** moved down to `delonix-node` (`mem_available_mib`), and the
  admission verdict stayed with orchestration.
- **One fewer `eprintln!`** in a library: the "cloud image on Cloud Hypervisor without
  libvirt" warning became `tracing::warn!` (ratchet `library_prints` 90 → 89).

**Deviations from the previous addendum's sketch, measured while cutting:**

- `VmBackends` needed two more questions to the registry: `require` (the required
  capabilities, which `start` also checks) and `declares` (restart policy, namespace and
  the anti-spoof check ask a backend's report). The "unregistered domain" ended up as
  three methods instead of two: `unrecorded(name) -> Option<&'static str>` says WHICH
  backend has it (`rm`'s progress announces it), and `stop_unrecorded`/`remove_unrecorded` act.
- `LocalDiskImages::overlay` receives the progress callback (`CreateStage::Disk` only
  fires once the overlay is actually created) and returns `(base, overlay)`, because the
  VM record stores both. `SeedBuilder::seed` receives the entire `VmConfig`: the seed
  reads the hostname, the user, the keys and the volumes.

**A correction to the previous addendum.** It said this slice's gate was "the `backup
create vm` of a running libvirt VM with two disks, the case the 2026-09-24 battery already
covers". It did not: the catalog's `vm.backup-disk` line is `partial` precisely because
the battery only backed up a container. This slice adds the case to the `scripts/e2e.sh`
libvirt snapshot section: with the VM running, the archive is created, the VM stays
`running`, and `domblklist` shows it writing to its own overlay (not the temporary one,
not the base image — the defect a `blockcommit` without `--top`/`--base` hides behind
"Successfully pivoted"). **Measured:** the battery's VM sections against this tree's binary
gave 101 PASS, 0 FAIL, with the four new checks among them. The catalog line was not
promoted: that is an ADR-0050 decision, with a restore check alongside it.

## P4b.3b Addendum (2026-09-29) — the use cases moved down to the context

**Done.** Orchestration left `delonix-vm` for `delonix_compute::vm` (1,612 lines including
the new tests): methods of a `VmEngine<'a, R, B, D, S>` that receives the root, the
`StateRepository<Vm>`, the three ports and the network, generic over them like `container
run`'s `resolve_run`. The pure functions it used came down with it (`valid_vm_name`,
`vm_namespace_of`, RAM admission, `resolve_required_capabilities`,
`adopt_pid_starttime`/`argv_is_vmm_for`, `boot_spec_of`/`config_from`) and `Destroyed`.
`delonix-vm` went from 9,249 to 8,331 lines: it assembles the engine per call (`engine(base)`:
the `JsonStore`, `RegistryBackends`, `QemuImgDisks`, `CloudLocaldsSeed` and the registered
network) and keeps the 24 public functions as one-line wrappers. No caller in `-bin`,
`delonix-linux`, `mgmt` or MCP changed; `delonix-vm`'s tests still reach the pure functions
under their usual names (imported back only for tests).

- **What P4b.3a let through:** `check_allow_mac_spoofing` is libvirt-specific knowledge
  (the backend name and the anti-spoof filter) and `create_with` used to call it. It
  became `VmBackends::admit(backend_id, cfg)`, implemented in `delonix-vm` by the same old
  function — same text, same code.
- **`into_root()` on the store's error** gave way to `e.is_not_found()` (ADR-0043 D4): the
  error arriving through the port is the shared one, and it is the class being asked, not
  the variant.
- **The context does not run programs:** `arch_fitness.py` gained the `context_spawns`
  counter (`Command::new` in `crates/contexts/`), with a baseline of **0**. Verified: a
  temporary `Command::new` in compute makes the gate fail with "new debt entered".
- **Tests against fake ports** (`vm::tests`, 6): an in-memory repository, a backend that
  records calls, fake disk and seed — overlay and seed for a local VM and neither for a
  backend with its own storage, an invalid name that never reaches any port, `stop` on an
  unregistered name asking the backends (4501 when none has it), `status` reconciling a VM
  the backend stopped, and `stop` recording `Stopped`. Removing the `!own_storage` guard
  from the seed makes the respective test fail (verified).

**Left for P4b.4:** the exception `("dep", "delonix-vm", "delonix-state")` (`engine(base)`
still opens the `JsonStore`), the registry, and the two local backends, which move out to
their own crates.

## P4b.4 Addendum (2026-09-29) — the design, measured before the code (proposal)

**Base:** `origin/main` after #597. Like P4b.3, this slice gets a design before the code,
because the measurement found a point the plan did not confront: **where the engine gets
assembled once `delonix-vm` leaves.**

### What was measured

`delonix-vm` after P4b.3b: 8,329 lines in `lib.rs` (1–1150 helpers and registry; 1151–1952
Cloud Hypervisor; 1953–4038 libvirt; 4039–4405 engine assembly and the 24 wrappers;
4406–8329, **3,924 lines of tests**), plus `capabilities.rs` (749: the two ADR-0050
declarations and the `LibvirtHost`/`CloudHypervisorHost` probes), `cloudinit.rs` (317),
`local_ports.rs` (157) and `provider.rs` (311, P4b.1's `LocalVmProvider`).

**What the two backends share is little** (calls per zone, `grep`):

| Helper | CH | libvirt | Nature |
|---|---|---|---|
| `stable_cmd` / `capture` / `binary_in_path` | 1 / 1 / 1 | 5 / 14 / 1 | run programs |
| `mac_for` | 1 | 1 | pure |
| `network()` | 5 | 0 | the `VmNetwork` port |
| `terminate_vmm`, `vmm_left`, `wait_vmm_left`, `vmm_to_signal`, `shq`, `memory_arg`, `cpus_arg` | yes | 0 | CH only |
| `is_rootless`, `run_quiet`, `disk_backing_format`, `pick_lease_ip`, `leases_max_expiry` | 0 | yes | libvirt only |

**The tests**, classified by what they exercise (114): libvirt 46, Cloud Hypervisor 27,
registry 7, public wrappers 4, other 30.

**Who depends on `delonix-vm`** (real `Cargo.toml` dependencies, not mentions): the `-bin`
and three interfaces — `delonix-mgmt`, `delonix-mcp` and `delonix-node-api` —, and these use
the whole engine (`create`, `start`, `status`, `list`) and the capability reports.

### The point the plan did not confront

The plan said "`delonix-vm` out of the workspace" at the end of P4b.4. But **assembling**
the engine — the registry with the two local backends in an order that decides
auto-detection, the `JsonStore`, the local disk and seed, and the registered network — has
to live somewhere the four consumers share, and the `ALLOWED` table has no such place:

- an **adapter** does not depend on providers;
- an **interface** does not depend on another interface;
- a **context** knows nothing about providers and runs no programs;
- assembling it in each consumer means **four copies** of the registry, and registration
  order is what decides which backend a VM without `--backend` receives — four copies diverge.

This is what ADR-0040's application layer (P5) exists to solve, and it does not exist yet.

### The proposal

1. **Two provider crates**: `delonix-provider-cloud-hypervisor` and `delonix-provider-libvirt`,
   each with its own `VmBackend`, its own ADR-0050 declaration and its own probe
   (`capabilities.rs` splits in two), its own helpers and its own tests; each exposes
   `registration()`, as Proxmox already does. libvirt also carries what P4b.3 gave it by
   name: hot backup, the unregistered domain
   (`unrecorded`/`stop_unrecorded`/`remove_unrecorded`) and `admit`.
2. **`mac_for`** (pure) moves down to compute. **`stable_cmd`/`capture`/`binary_in_path`**
   stay as one copy in each provider (≈20 lines), each with the test that pins
   `LC_ALL=C` — the regression these functions exist to prevent (`virsh` is gettext). There
   is no other legal place: a provider does not depend on another provider or on an
   adapter, and a context no longer runs programs (`context_spawns` = 0). The copy is
   small and the test is what stops the two from diverging on the one thing that matters.
3. **The registry moves down to compute** (`delonix_compute::vm_registry`, what the ADR's
   D4 already anticipated), empty: nobody seeds it from inside; assembly registers the
   local backends in an explicit order written in one place.
4. **A `delonix-guestfs` adapter** (the name ADR-0040 D2.3 already reserved for
   `qemu-img`) carries `cloudinit.rs` and the `LocalDiskImages`/`SeedBuilder` implementations.
5. **`delonix-vm` stays, reduced to assembly, until P5** — the per-call engine, the
   registration of the two locals, the 24 public wrappers and `LocalVmProvider` —, with
   **two new, declared exceptions** in `arch_fitness.py`, `("dep", "delonix-vm",
   "delonix-provider-libvirt")` and `("dep", "delonix-vm", "delonix-provider-cloud-hypervisor")`,
   phase **P5**, reason written: it is the composition root that the application layer will
   absorb. The gate rule ("an exception must name the phase that removes it") is what
   makes this honest: the debt is visible, with an exit date, instead of four copies of the
   registry scattered across consumers.

   **This deviates from the plan** ("`delonix-vm` out of the workspace") and that is why it
   goes in writing before the code. The alternative that follows the plan to the letter is
   to add a layer to the `ALLOWED` table — a structural decision (ADR-0040), not a slice's.

### In slices, each with its own gate

- **P4b.4a** — `mac_for` in compute, the registry in compute (seeded by assembly), the
  `delonix-guestfs` adapter with `cloudinit.rs` and the two ports. No backend changes crate.
- **P4b.4b** — the `delonix-provider-libvirt` crate (backend, declaration, probe, helpers, tests).
- **P4b.4c** — the `delonix-provider-cloud-hypervisor` crate, likewise.
- **P4b.4d** — `delonix-vm` reduced to assembly, the two declared exceptions, `LAYERS` with
  the two new names, and the regenerated manual.

**Each one's gate:** the four consumers' public symbols resolving without changing a line,
the battery's `vm` section (101 checks today) and `control_restart` chaos without
regression, `provider ls`/`provider matrix` byte-for-byte identical (the declarations only
change crate), and `arch_fitness.py` green with the exceptions each slice declares or removes.

### Proven vs not validated

Proven, via `grep` over #597's code: the zones and counts, the shared helpers, the test
breakdown, and that only `-bin` and the three interfaces depend on `delonix-vm`. Not
validated: the cost of splitting the 3,924 lines of tests (the classification above is by
keyword, not by reading) and whether `LocalVmProvider` fits into assembly without dragging
anything else along.

## P4b.4 Close-out (2026-09-29) — #599, #602, #608 and P4b.4d

**What landed as the addendum said:** the two local backends live in their own crates
(`delonix-provider-libvirt`, #602; `delonix-provider-cloud-hypervisor`, #608), the registry
and `mac_for` in compute (#599), and `delonix-vm` is the composition root until P5, with
the two declared exceptions. VM networking (`set_network`/`network()`) also moved down to
compute's `vm_registry`, because the CH provider needs it and cannot depend on the adapter.

**What deviated from the addendum, and why:** the `delonix-guestfs` adapter was not
created. No provider needs `cloudinit.rs` or the local disk, and creating it would have
given a third exception with no consumer. The disk and the seed stay in `delonix-vm`'s
`local_ports.rs`, with the `qemu-img` helpers that belong to them (P4b.4d).

**What P4b.4d found and closed:**

- **Point 2's promise had not been kept.** The addendum said each `stable_cmd` copy would
  carry the test pinning `LC_ALL=C`; in P4b.4b/c the two tests stayed in `delonix-vm`, and
  neither provider had its own. Each provider now has its own locale test.
- **A hollow test.** `os_estados_comparados_sao_os_literais_en_do_virsh` looked for
  `state == "shut off"` in `delonix-vm`'s `lib.rs`, from which the comparison had left in
  P4b.4b; it passed because the string is in the test's own code. It was removed from
  there, and libvirt gained the version that reads only the code (without comments or the
  test module).
- **The reason for the `delonix-vm → delonix-state` exception was out of date** (it talked
  about a port that already exists and XML writes that are already in the provider). It
  moved to phase P5, with what is true today: the `JsonStore` that assembly hands to the
  engine, and the default backend's marker.

**Numbers:** `delonix-vm` is left with 4,346 lines (it was 9,249 before P4b.3b), of which
`lib.rs` is assembly, the public wrappers and their tests. Unit tests (`#[test]` in `src/`):
64 in `delonix-vm`, 38 in libvirt, 31 in Cloud Hypervisor.
