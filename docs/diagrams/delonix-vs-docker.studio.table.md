# Delonix vs Docker — confirmation table

A drawing from the dxtutor architecture Studio (`delonix-vs-docker.studio.arch.json`). The
**Delonix** panel is *strict* and *daemonless*: every box, boundary and arrow has its source in the
code, and the Studio checks it with the backend rules (0 findings). Lines confirmed at commit
`6b8d18f3` of the `delonix` repository.

The **Docker** panel is the well-known point of comparison (the reference image and Docker's public
architecture, with `containerd-shim`): it is not code from this repository, so it is not *strict*.

The drawing's labels are in Portuguese, like the other diagrams in this folder, so each element
is named here by its `id` in the `.arch.json`, with a short English gloss.

| Type | Element | Confirmed in |
|---|---|---|
| Boundary | `dlx-body` — Delonix, no daemon | AGENTS.md:236 (guardrail «Daemonless — no resident process by default»); dist/: opt-in units only (delonix-cri.service, delonix-node-api.socket, delonix-storage-helper.socket) |
| Boundary | `dlx-host` — host | crates/adapters/delonix-linux/src/lib.rs:9759 clone, in-process from cmd_run (bins/delonix/src/cmd/container.rs:3622) |
| Boundary | `dlx-state` — state under $DELONIX_ROOT | crates/adapters/delonix-state/src/store.rs:298, :314 |
| Boundary | `dlx-per` — per container (optional) | crates/adapters/delonix-linux/src/lib.rs:8966; bins/delonix/src/cmd/container.rs:3563; bins/delonix/src/cmd/container.rs:4802 |
| Boundary | `dlx-net` — rootless network (on demand) | crates/adapters/delonix-sdn/src/infra.rs:1933 ensure_up; :1783 acquire; :1794 release |
| Box | `cli` — delonix CLI (`delonix run nginx`) | bins/delonix/src/main.rs:1917 (run → container run); bins/delonix/src/cmd/container.rs:3622 cmd_run |
| Box | `supervisor` — supervisor, forked by the CLI (-d) | crates/adapters/delonix-linux/src/supervise.rs:81 run_supervised; :103 fork |
| Box | `ctr` — container | crates/adapters/delonix-linux/src/lib.rs:9759 clone; crates/adapters/delonix-linux/src/lib.rs:5541 execvp |
| Box | `state` — JSON state | crates/adapters/delonix-state/src/store.rs:298 Store; :314 default_root ($DELONIX_ROOT); :681 JsonStore; :94 flock |
| Box | `logshim` — log shim | crates/adapters/delonix-linux/src/lib.rs:1445 log_shim; :9923 fork |
| Box | `slirp` — slirp4netns, one per container | crates/adapters/delonix-sdn/src/lib.rs:2235 slirp_attach |
| Box | `watcher` — watcher --rm | bins/delonix/src/cmd/container.rs:4802 spawn_rm_watcher; :4804 fork |
| Box | `pin` — netns pin | crates/adapters/delonix-sdn/src/infra.rs:455 PidKind::Pin; bins/delonix/src/main.rs:2259 |
| Box | `control` — netns control: delonix0, nft, DHCP | crates/adapters/delonix-sdn/src/infra.rs:455 PidKind::Control; :28 delonix0; :3638 start_dhcp; :9829 dns_server_main; main.rs:2269 |
| Box | `slirp1` — slirp4netns, single and shared | crates/adapters/delonix-sdn/src/infra.rs:455 PidKind::Slirp; :2592 start_slirp |
| Box | `vm` — microVM VmBackend | crates/contexts/delonix-compute/src/domain/vm_backend.rs:558 VmBackend; providers cloud-hypervisor lib.rs:861, libvirt lib.rs:3260 |
| Box | `dlx-kernel` — Linux kernel · namespaces · cgroups v2 | crates/adapters/delonix-linux/src/lib.rs:9558 clone_flags; :7877 setup_cgroup; :429 apply_seccomp; crates/adapters/delonix-sdn/src/infra.rs:5 nft |
| Arrow | `cli` → `ctr` «clone» | bins/delonix/src/cmd/container.rs:4374 launch::start → crates/contexts/delonix-compute/src/domain/launch.rs:193 → crates/adapters/delonix-linux/src/workload.rs:53 → crates/adapters/delonix-linux/src/lib.rs:9759 clone; pivot_root crates/adapters/delonix-linux/src/lib.rs:3145; setup_cgroup :7877 |
| Arrow | `cli` → `supervisor` «-d: fork» | crates/contexts/delonix-compute/src/domain/launch.rs:165 should_supervise (detach); crates/adapters/delonix-linux/src/supervise.rs:103 fork |
| Arrow | `supervisor` → `ctr` «clone» | crates/adapters/delonix-linux/src/supervise.rs:132 create_with |
| Arrow | `cli` → `state` «reads and writes» | bins/delonix/src/cmd/container.rs:3622 cmd_run(store: &Store) |
| Arrow | `cli` → `logshim` | crates/adapters/delonix-linux/src/lib.rs:8966 start_log_shim (in-process spawn) |
| Arrow | `cli` → `slirp` | bins/delonix/src/cmd/container.rs:3563 delonix_sdn::slirp_attach |
| Arrow | `cli` → `watcher` | bins/delonix/src/cmd/container.rs:4388 → :4802 spawn_rm_watcher (only without a supervisor) |
| Arrow | `cli` → `pin` «ensure_up» | crates/adapters/delonix-sdn/src/infra.rs:1933 ensure_up; :1783 acquire / :1794 release |
| Arrow | `cli` → `vm` «VmBackend» | bins/delonix/src/cmd/vm.rs:3468 delonix_vm::create |
