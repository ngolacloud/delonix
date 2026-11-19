# Sprint 08 — Provider validation (§8) and installation E2E (clean environment)

- **Target release:** Delonix v0.1.1 (`0.x` line)
- **Measured binary:** `/home/walter/dlx-wt/stabilize-0.1.1/target/debug/delonix` → `delonix 0.1.0, catalog 1.4.0` (149 capabilities in the catalog)
- **Date:** 2026-10-31
- **Role:** SRE. Nothing was built, edited, committed or pushed. No heavy VM brought up. Read-only probes only.
- **§18 (golden rule):** absence of environment ≠ approval. Below I distinguish **validated-live** / **available-not-validated** / **blocked-by-environment**.

---

## A) §8 — Providers measured on the host

Real output of `delonix provider ls` (count by state against the catalog):

| ID | Kind | Avail | Health | SUP | PART | UNSUP | EXT | NOT-IMPL |
|----|------|-------|--------|-----|------|-------|-----|----------|
| cloud-hypervisor | compute | yes | **Ok** | 37 | 4 | 41 | 2 | 6 |
| libvirt | compute | yes | **Ok** | 41 | 10 | 30 | 2 | 7 |
| proxmox | compute | **no** | **NotConfigured** | 40 | 12 | 26 | 3 | 9 |
| linux | compute | yes | **Ok** | 19 | 3 | 67 | 0 | 1 |
| linux | network | yes | **Ok** | 20 | 6 | 20 | 1 | 1 |
| proxmox | network | **no** | **NotConfigured** | 7 | 7 | 25 | 0 | 9 |
| linux | storage | yes | **NetworkSharesNeedRoot** | 6 | 0 | 0 | 2 | 0 |
| opnsense | gateway | **no** | **NotConfigured** | 10 | 0 | 24 | 2 | 12 |

### State per provider

#### libvirt (compute) — available, Ok
- `available=true`; `provider ls` probes `qemu:///system` and passes. libvirt 10 / QEMU 8.2.2 alive on the host.
- **Validated-live here:** availability and health (`Ok`) are measured on this host. The 41 `supported` cells cite evidence (battery/scenario/test) per `describe`. It is the `defaultProvider` recommended by install.sh.
- **Note:** the 1st capability (`provider.availability`) is marked `partial` with a written limit — `available()` only sees the 2 binaries; it is `provider ls` that actually probes `qemu:///system`.

#### cloud-hypervisor (compute) — available, Ok
- `available=true`; `provider ls` probes `/dev/kvm` + firmware and passes.
- **Validated-live here:** availability/health measured. 37 `supported` with declared evidence.

#### linux (compute / network / storage) — available
- compute/network **Ok**; storage **NetworkSharesNeedRoot** (nfs/cifs/webdav need `CAP_SYS_ADMIN` to mount; local volumes unaffected → the `3 unavailable-on-host` are declared, not failures).
- **Validated-live here.** `delonix system doctor` passes on all prerequisites (see section B).

#### proxmox (compute + network) — **NotConfigured** → BLOCKED-BY-ENVIRONMENT
- Confirmed `available=false`, `NotConfigured`. Hint: `set DELONIX_PROXMOX_URL/_NODE and a credential to register a target`.
- The **pve92 appliance was pruned today** (disk no longer exists). `virsh list --all` shows `pve92 shut off` with no usable disk.
- All **47 `supported` cells** (40 compute + 7 network) have evidence pointing to `crates/providers/delonix-proxmox/tests/live.rs` (e.g. `provider.readback`, `vm.create`, `net.dns.records`, `net.ipam.*`). Those tests are `#[ignore]` and require a PVE reachable via `DELONIX_PROXMOX_URL/_NODE` + credential.
- **Quantifying what is left undemonstrated:** without re-provisioning a PVE target (forbidden in this session), **none** of the 47 supported cells can be re-proven live for v0.1.1. Also left unexercised are the 12+7 `partial` and the error paths. The historical evidence in `live.rs` exists but is **not re-runnable on this host** → classify as **available-not-validated-in-this-release** (code present, no live re-proof).
- **Recommendation:** mark proxmox as "regression not run" in the v0.1.1 matrix; the live proof needs 1 dedicated PVE node (re-provisioning pve92 or equivalent) — out of scope and out of the environment budget for this session.

#### opnsense (gateway) — **NotConfigured**; VM alive but UNREACHABLE + NO CREDENTIAL → BLOCKED-BY-ENVIRONMENT
Investigation done (all read-only):
- The libvirt VM `opnsense` is **running** (`virsh list` → running; disk `/var/lib/libvirt/images/opnsense.qcow2`). It has 5 NICs on bridges: `br-wan` (10.0.99.0/24), `br-lan` (192.168.10.0/24), `br-dmz` (192.168.20.0/24), `br-dev`, `br-backup`.
- **No guest-agent:** `virsh domifaddr opnsense` (lease and arp) returns empty — the host does not know the IP of any appliance interface.
- **GUI/API unreachable from the host:**
  - `curl -sk https://<ip>/` on the likely IPs (192.168.10.1/.2/.254, 10.0.99.2, 192.168.20.1) → all `000` (no response).
  - Read-only ping sweep over `192.168.10.0/24`: only `192.168.10.1` responds (which is the **host itself** on br-lan); all ARP entries for the appliance stay `INCOMPLETE`. The appliance does not respond on the LAN segment from the host (likely: WAN firewall blocking + the interface the host sees is not the management one, or the GUI listens on another segment/port not exposed to the host).
- **opnsense provider requirements** (from `crates/providers/delonix-opnsense/src/lib.rs` and `capabilities.rs`):
  1. `DELONIX_OPNSENSE_URL` **https:// mandatory** (`validate_target_url` refuses http — the key/secret would go in the clear; refuses userinfo in the URL);
  2. **credential = API key + secret** (OPNsense uses a key/secret pair, not user/password), via `auth` in `providers.yaml` or `DELONIX_OPNSENSE_KEY/_SECRET`;
  3. for the self-signed certificate: `tls.insecureSkipVerify` / `tls.caFile` or `DELONIX_OPNSENSE_INSECURE_TLS=1`;
  4. `Client::connect` **proves the credential up front** with `GET core/firmware/status` before anything else.
- **What exactly is missing to register and probe (§8 firewall/NAT/zones):**
  - (a) **A reachable management URL** — the appliance GUI is not exposed to the host on any known IP; it would require serial console access (`/dev/pts/1`) to discover/configure the management interface, which is more than a read-only probe and touches the owner's VM → **not done**.
  - (b) **API key + secret** — these must be created INSIDE the GUI (System › Access › Users › API). They were not provided and cannot be generated without login → **absent**.
  - Conclusion: even if the URL were discovered, without a credential the `connect` would fail at `firmware/status`. **Two independent blocks.**
- **The 10 gateway `supported` cells** (`net.gateway.filter`, `net.gateway.alias`, `net.gateway.rule-order`, `net.nat.snat`, `net.nat.dnat`, `net.apply.staged`, `net.observe`, `net.ownership-marker`, `firewall.stateless`, `firewall.logging`) **all cite live evidence** (`crates/providers/delonix-opnsense/tests/live.rs::*`, `#[ignore]` tests that require `DELONIX_OPNSENSE_TEST_URL/_KEY/_SECRET` + a reachable appliance).
- **opnsense verdict:** **blocked-by-environment** (unreachable + no credential). Zero §8 cells re-proven live in this session. The VM being "alive" does NOT count as approval (§18).

### §8 summary — what can be validated here vs. blocked

| Provider | Validatable in this session | v0.1.1 classification |
|----------|------------------------|----------------------|
| libvirt (compute) | YES — avail/health Ok measured | **validated-live** (availability/health) |
| cloud-hypervisor (compute) | YES — avail/health Ok measured | **validated-live** (availability/health) |
| linux (compute/network/storage) | YES — doctor passes; storage shares need root | **validated-live** |
| proxmox (compute+network) | NO — NotConfigured, pve92 pruned | **blocked-by-environment** (47 supported to re-prove; needs a dedicated PVE) |
| opnsense (gateway) | NO — VM alive but unreachable + no API key/secret | **blocked-by-environment** (10 supported §8 to re-prove) |

> Note on "supported": the matrix marks `supported` based on evidence (test/scenario/battery) recorded in the catalog. For proxmox and opnsense that evidence comes from **live** `#[ignore]` tests that are **not re-runnable on this host at v0.1.1**. For the release gate, treat as "code present + historical evidence, live re-proof pending environment", not as "approved now".

---

## B) Installation E2E in a clean environment (`scripts/install.sh`)

### What install.sh does (1907 lines, read)
- Installs a **published release** (signed binary + siblings + runtime deps + host config). **Does not compile** (that is the Makefile). Documented usage: `curl … | bash` — the whole body is inside `{ … }` as a guard against a truncated download.
- **Distro detection** via `ID`/`ID_LIKE` from `/etc/os-release`, with a fallback by the manager present:
  - `apt` (Debian/Ubuntu/Mint/Zorin/Pop) · `dnf` (Fedora/RHEL/CentOS/Rocky/Alma) · `zypper` (openSUSE/SLES) · `pacman` (Arch/Manjaro/Endeavour). A distro with none → `die`.
- **Runtime deps** (lesson from real installs): `slirp4netns`, `uidmap` (newuidmap/newgidmap setuid), `nftables`, `iproute2`, `conntrack`; VMs: `libvirt+qemu`, `qemu-img`, `cloud-localds`; optional `cloud-hypervisor`, `nvidia-container-toolkit`, `minisign` (signature verification), `delonix-cri` (`--with-cri`).
- **Rootless host config:** `/etc/subuid` + `/etc/subgid` (`ensure_subid`), AppArmor profile, **cgroup delegation** drop-in (`Delegate=cpu cpuset io memory pids` in `/etc/systemd/system/user@.service.d/`), sysctls/modules (`--no-tune` turns off), `--production`/`--performance` tuning.
- **providers.yaml** (ADR-0054): written with `defaultProvider` = `--vm-provider` (default libvirt) **only if it does not exist**; never rewritten. Location: `/etc/delonix/providers.yaml` (system) or `~/.config/delonix/providers.yaml` / `$XDG_CONFIG_HOME` (with `--user`).
- **Final check (embedded doctor):** checks the binary, `slirp4netns`, `newuidmap` (+ setuid/cap privilege), `nft`, subuid/subgid ranges, `/dev/net/tun`, user-namespaces (via the AppArmor profile where `kernel.apparmor_restrict_unprivileged_userns=1`), VM backend, default provider as read BY the binary (`delonix vm default-backend`), and **real cgroup delegation** (creates a child cgroup + tries `+memory`, and checks cpu/cpuset/io/memory/pids). It is equivalent — but not identical — to `delonix system doctor`.

### E2E pieces and what each step requires

| Step | Requires | Observations |
|-------|-------|-------------|
| **Install** (binary + deps + config) | **root/sudo** (apt/dnf/zypper/pacman, writing `/etc`, `/usr/local/bin`, AppArmor, drop-ins) + the **distro package manager** + network | The 1st sudo call is the binary install (`|| die` with an actionable message). `minisign` to verify the signature (or `--insecure-skip-signature`). |
| **`system doctor`** | nothing destructive (read-only) | Needs **cgroup v2 + systemd user** for delegation to read `Ok`; `--strict` for a CI gate. |
| **Re-run = upgrade** | same as install | Preserves state/images/volumes/config/sockets/services. Migration warnings **only** if `PREV_VER` is `0.64.*` (3 Kinds removed) or `1.*–5.*` (re-base of the `delonix-runtime` line → `0.x`, deliberate, not a downgrade). An existing providers.yaml **never** rewritten → proves preservation. |
| **Remove** | **manual** | ⚠️ **There is no uninstall subcommand** in install.sh nor a `--uninstall`. Reverting is manual: `rm` the binaries, `rm /etc/delonix`, remove lines from `/etc/subuid`/`/etc/subgid`, `rm` the drop-ins (`user@.service.d`, `delonix-performance`, AppArmor profile) + `systemctl daemon-reload`. **A release gap for a clean, scriptable "remove".** |

### Container vs VM as a clean environment — assessment

- install.sh is **systemd-centric and root-centric**: `systemctl enable --now libvirtd.socket`, `user@.service` drop-ins, cgroup delegation that **only takes effect at the next login**, AppArmor, kernel modules. None of this is faithfully exercisable without **PID1 = systemd** and **root**.
- **Container via delonix itself (rootless):** good for a *smoke* of **distro detection** and the per-distro package path (apt/dnf/zypper/pacman) **if** the image ships systemd as PID1 and runs privileged — which contradicts delonix's rootless model. A common rootless container does **not** have systemd PID1, **cannot** load kernel modules, **does not** apply the host AppArmor, and cgroup delegation/`user@.service` do not exist inside it. So a container does **not** prove the steps that most often fail in the field (delegation, libvirtd, AppArmor, host-level subuid).
- **Verdict:** the faithful per-distro E2E **requires a VM** (one per package family: Debian/Ubuntu→apt, Fedora/Rocky→dnf, openSUSE→zypper, Arch→pacman), with **systemd + root + /dev/kvm** to also cover the VM backend. A container serves, at most, as a **cheap pre-smoke** of the distro branching, not as an acceptance proof. **None of these VMs were brought up in this session** (bringing up heavy VMs forbidden) → the multi-distro E2E is **blocked-by-environment / pending**.

### The rootless `--user` path — is it testable WITHOUT touching the real HOME or sudo?

- `--user` installs the binary in `~/.local/bin` and writes providers.yaml in `~/.config/delonix/` (or `$XDG_CONFIG_HOME`). **Without `--user`, the 1st sudo is the binary install.**
- With `--user` AND `--no-binary` (or a binary already present), and **redirecting HOME** (`HOME=<tmp> XDG_CONFIG_HOME=<tmp>/.config`), the `--user` stretch writes only to a temporary dir. **BUT**: the real install.sh still calls `require_dep`/`pkg_install` (which use sudo) for the runtime deps, and the subuid/AppArmor/delegation section needs root. A "binary-only" `--user` covers none of what matters to validate in §B (deps + host config).
- **Decision (honest):** I **did not run** a `--user` install on this dev machine. Even with HOME redirected, the added value would be almost nil (it would only prove the binary lands in `~/.local/bin`), and the dev host already has a `delonix` and an environment I do not want to pollute/mask (install.sh explicitly warns about binaries shadowing each other on the PATH). Left **documented as pending**: testable in a clean VM, not here.

### Installation E2E verdict
- **Install / upgrade / doctor:** solid and complete design (incl. a rigorous embedded doctor and migration warnings 0.64→0.65 and 5.x→0.x). **Needs a VM with systemd+root+pkg-manager per distro family** for a faithful acceptance proof. **Pending — blocked-by-environment** (VMs not brought up in this session).
- **Remove:** **a real gap** — there is no scriptable uninstall path; removal is manual and multi-file. For v0.1.1, recommend either an `install.sh --uninstall`/`uninstall.sh` script, or at least documenting the reversal procedure in one single place.
- **Container:** insufficient as an acceptance environment (no systemd PID1 / root / cgroup-deleg / AppArmor / modules); good only as a pre-smoke of the distro branching.

---

## Global verdict (§18-aware)

- **Validated-live on this host:** libvirt, cloud-hypervisor and linux (availability + health `Ok`; `system doctor` passes on all prerequisites; storage-shares need root, by design).
- **Blocked-by-environment (NOT approved):** proxmox (47 supported cells, pve92 pruned — needs a dedicated PVE) and opnsense (10 §8 cells, VM alive but unreachable + no API key/secret — two independent blocks).
- **Installation E2E:** complete design for install/upgrade/doctor; **needs a VM** (systemd+root+pkg-manager) per distro family; **a container is not enough**; **removal has no scriptable path** (gap). None of the multi-distro E2E was run (bringing up VMs forbidden) → **pending**.
- The absence of proxmox/opnsense targets and of installation VMs is **not** approval — it is verification debt to schedule in a session with a dedicated environment.
