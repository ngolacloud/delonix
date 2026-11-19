# E2E Audit — Delonix Runtime
> Offensive audit of the whole ecosystem (~50k LOC, 9 crates): bugs, gaps, design errors,
> performance, concurrency, memory, and resources. **14 finders per subsystem ×
> multiple lenses**, each finding passed through **2 independent adversarial skeptics**
> who try to refute it by reading the real code and cross-checking it against the limitations already
> documented in `AGENTS.md`. Date: 2026-07-21.

**Coverage note:** the run hit the session limit halfway through the verification phase.
The **24 findings below survived full adversarial verification** (2 skeptics, 0
refutations unless noted). Another **11 findings remain unverified** due to quota — mostly
in the highest-risk subsystems (`delonix-runtime/lib.rs`, `delonix-net/infra.rs`,
`container.rs`) — and are listed separately, pending confirmation.

> ## ⚠️ CURRENT STATUS (2026-08-04 re-triage) — this document was STALE
>
> `AGENTS.md` claimed that "27 of the 35 remain open." **They do not.**
> A finding-by-finding re-triage was done, reading the CURRENT code instead of
> trusting this table — and practically all of them had already been fixed, each
> with its `BUG FOUND`/`BUG FIXED HERE` comment in place and, in most
> cases, a dedicated regression test.
>
> **Verified CLOSED by direct code reading:**
> #7, #8, #9, #10, #11 (see the caveat below), #12, #13, #14, #15, #16, #17, #18,
> #19, #20, #21, #22, #23, #24, and the unverified NV-3 through NV-11.
>
> A sample of what was confirmed, to give confidence in the triage rather than ask for faith:
>
> | Finding | How it is closed today |
> |---|---|
> | #10 secrets traversal | `load`/`remove` call `valid_name` before touching the path |
> | #13 route confusion | `path_prefix_matches` with a segment boundary (Gateway API semantics) |
> | #15 fail-open CVE gate | `valid_admission_policy` refuses BEFORE scanning |
> | #18 O(n) DNS | moved to `dns_index()` (a map), not a scan |
> | #20 double proxy spawn | a dedicated lock over the check-then-spawn |
> | #22 dash reconciles 2× | `delonix_vm::list` already returns reconciled |
> | NV-5 DNS hijack | `valid_container_name` deliberately excludes `.` |
> | NV-9 ambiguous prefix | `find` errors out; two regression tests proving it |
> | NV-10 fd leak | `open_container_ns` returns `OwnedFd` (closes via RAII) |
>
> **A real residual found in THIS triage and fixed in v0.38.3** — #11 was
> only half-closed: the temp file was already per-writer, but `SecretStore::save`
> (a) did not `fsync`, so a crash could publish an encrypted blob
> pointing at unwritten blocks, and (b) created the file under the umask and only
> afterwards tightened it to `0600` — the same TOCTOU window finding #4
> (kubeconfig) had already closed with `OpenOptions::mode`. Moved to
> `write_atomic_mode`.
>
> **Conclusion**: the debt at this point was one of DOCUMENTATION, not code. This
> header stands as a record of the verification; the table below is historical.

## Executive summary

| Severity | Confirmed | Unverified (quota) |
|---|---|---|
| 🟠 HIGH | 6 | 2 |
| 🟡 MEDIUM | 12 | 6 |
| ⚪ LOW | 6 | 3 |
| **Total** | **24** | **11** |

By category (confirmed): bug 12, resource 3, concurrency 3, gap 2, performance 2, memory 1, design 1.

**Verdict:** the rootless→root boundary and CLI/manifest input validation (audited
earlier) remain solid — no new RCE. The dominant pattern among **HIGH** findings is
**writing/deleting files outside the sandbox from untrusted input** (OCI whiteouts,
CRI IDs, VM name, symlink `COPY`, kubeconfig in `/tmp`) and **lack of peer
authentication on the management socket** — all exploitable with no privilege, under the
normal threat model of running a third-party image/manifest.

## Confirmed findings (adversarially verified)

### 1. 🟠 HIGH — Path traversal in OCI whiteout handling deletes arbitrary files outside the rootfs

- **Location:** `crates/adapters/delonix-image/src/overlay.rs:81`
- **Category:** bug · finder confidence: high

**Description.** In `apply_layer_flat`, non-whiteout entries are extracted safely via `entry.unpack_in(dest)` (tar 0.4.46 guards `..`/absolute escapes) and pre-created parents go through `safe_rel`. But the OCI whiteout branch (lines 62-84) takes the RAW `entry.path()` from the tar header — never passed through `safe_rel` — and joins it directly into `std::fs::remove_dir_all`/`remove_file`. `path.parent()` may contain `..` components, and `dest.join(<..-path>)` does NOT normalize (PathBuf::join appends literally); the OS resolves `..` at unlink time, escaping `dest`. The regular whiteout deletes `parent/<target>`; the opaque marker (`.wh..wh..opq`) does `read_dir(parent)` then `remove_dir_all` on every child. It also follows symlinks planted by an earlier layer (CVE-2019-14271 class). Reachable on the DEFAULT rootless path: `prepare_rootfs` (cmd/util.rs:83) -> `export_rootfs` (overlay.rs:162) -> `apply_layer_flat` runs on every rootless `container run`, plus `image export` (cmd/image.rs:632) and rootless `build`. Not covered by the documented `safe_rel`/`safe_join` mitigations (those cover file writes and Dockerfile COPY, not whiteout unlinks). Impact reaches critical (wiping the invoking user's home dir); rated high because it is deletion/integrity, not code execution.

**Failure scenario.** User runs `delonix container run <malicious-image>` (rootless, default). The image's top layer tar contains an entry named `../../../../../../home/walter/.wh..wh..opq`. In `apply_layer_flat`, name=`.wh..wh..opq`, target after strip=`.wh..opq` matches the opaque case; parent = dest.join("../../../../../../home/walter") which resolves to /home/walter; the code `read_dir`s it and `remove_dir_all`s every entry -> the user's entire home directory contents are deleted. A regular-whiteout variant `../../../.ssh/.wh.authorized_keys` deletes ~/.ssh/authorized_keys.

**Suggested fix.** Run the whiteout `path` through `safe_rel` (reject any `..`/absolute/Prefix component) before computing `parent`/`victim`; return None -> skip the entry. Additionally resolve the victim within `dest` without following symlinks (e.g. openat-based or verify canonicalized parent starts_with(dest)) to close the planted-symlink vector.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev high) · confirms (sev high)

### 2. 🟠 HIGH — Path traversal via unvalidated CRI container/pod IDs → arbitrary *.json deletion and file reads

- **Location:** `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:745`
- **Category:** bug · finder confidence: high

**Description.** Incoming `container_id`/`pod_sandbox_id` from CRI requests (StopContainer/RemoveContainer/ContainerStatus/RemovePodSandbox/PodSandboxStatus/CreateContainer) are used verbatim to build filesystem paths without any validation. `read_rec`/`write_rec`/the raw `remove_file` calls do `dir.join(format!("{id}.json"))` (lines 151, 157, 745, 390) and `read_rec(&sb_dir, &req.pod_sandbox_id)` (line 551). An `id` containing `../` escapes `<base>/cri/{containers,sandboxes}`. Only server-generated ids are safe (generate_id → hex), but the lifecycle mutation/read paths accept the id straight from the request. Note the inconsistency: `log_path` IS explicitly checked for `..`/absolute (line 546), but the id — which also becomes a path — is not. This is the same class as the ALTO VM-name traversal already fixed in delonix-vm (valid_vm_name), but the CRI store paths were never hardened.

**Failure scenario.** A client on the CRI socket (malicious/compromised kubelet, or any process with access to /run/delonix-cri.sock) calls RemoveContainer with container_id = "../../../../home/walter/.config/delonix/somefile". `delonix container rm -f cri-<traversal>` returns "not found" → stderr_not_found=true → gone=true → `std::fs::remove_file(ct_dir(base).join("../../../../home/walter/.config/delonix/somefile.json"))` deletes an arbitrary *.json file owned by the runtime user (root on many k8s nodes). ContainerStatus/PodSandboxStatus with a traversal id similarly attempt arbitrary-path reads (parsed as a record).

**Suggested fix.** Validate every incoming CRI id with a strict whitelist (e.g. hex/[A-Za-z0-9._-], reject empty/`..`/`/`) at the top of each lifecycle handler before it touches any path, mirroring delonix_vm::valid_vm_name and the existing log_path check.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev high)

### 3. 🟠 HIGH — VM name path-traversal bypasses the valid_vm_name fix: generate_seed_iso writes seed files outside the state dir before create() rejects the name

- **Location:** `bins/delonix-runtime-bin/src/cmd/vm.rs:1043`
- **Category:** bug · finder confidence: high

**Description.** The documented audit-#2 fix placed valid_vm_name at the ENGINE boundary (delonix_vm::create, lib.rs:1120) on the claim that "any consumer of the API inherits it". But the bin's generate_seed_iso() is NOT an API consumer that goes through create() first — it runs BEFORE it and builds filesystem paths straight from the raw VM name: work_dir = state_root().join("vms").join(vm_name) followed by std::fs::create_dir_all(&work_dir) and fs::write of user-data / meta-data / network-config / seed.iso (vm.rs:1043-1099). Neither call site validates the name first: the CLI path calls generate_seed_iso(&name, ...) at vm.rs:492 (name is the raw positional arg) and the manifest apply path calls generate_seed_iso(name=&doc.metadata.name, ...) at vm.rs:384, both strictly BEFORE delonix_vm::create() (vm.rs:523 / 412). The manifest layer performs no name validation either (manifest.rs of_kind/spec_of do not check metadata.name). So a name containing '../' traverses out of the state directory: create_dir_all fabricates arbitrary directory trees and four fixed-basename files (including user-data/seed.iso whose content is fully attacker-controlled via --user-data, copied verbatim at vm.rs:1049) are written wherever the invoking user can write. create() then rejects the name, but the side-effecting writes have already happened. This is the SAME arbitrary-file-write class the project itself rated HIGH in audit #2 ("wrote/overwrote files OUTSIDE the state directory") — the fix is incomplete because it guards create()/remove() but not the seed-generation pre-step in the bin.

**Failure scenario.** Untrusted manifest applied via `delonix stack apply -f evil.yaml` with kind: VirtualMachine, metadata.name: "../../../../home/walter/.config/pwn", spec.volumes referencing any existing volume (so resolve_vm_volumes succeeds and the generate_seed_iso branch at vm.rs:383 fires): create_dir_all + writes land under $HOME/.config/pwn, outside state_root, before create() rejects the name. Even without a manifest: `delonix vm create '../../../../tmp/pwned' --user-data attacker.yaml` writes an attacker-controlled `user-data` and a `seed.iso` into /tmp/pwned/ then errors out. Directory creation is fully arbitrary; file writes are limited to the four fixed basenames.

**Suggested fix.** Call delonix_vm::valid_vm_name(name) (or equivalent) at the top of generate_seed_iso and/or before every call site in the bin (vm.rs:384, vm.rs:492, and validate metadata.name during manifest load), so the seed-generation path enforces the same whitelist as the engine boundary rather than trusting create() to reject it after the writes.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev high)

### 4. 🟠 HIGH — cluster-admin kubeconfig exposed in /tmp with mode 0644 and a predictable path on the remote host

- **Location:** `bins/delonix-runtime-bin/src/cmd/cluster.rs:1115`
- **Category:** bug · finder confidence: high

**Description.** fetch_kubeconfig runs on the control-plane host, as root via `sudo -n bash -c`: `cp /etc/kubernetes/admin.conf /tmp/delonix-admin.conf && chmod 644 /tmp/delonix-admin.conf`. admin.conf contains the embedded cluster-admin credentials (client-certificate-data + client-key-data). It is copied to a FIXED and PREDICTABLE path (`/tmp/delonix-admin.conf`) and made world-readable (0644) during the scp window. The cleanup `rm -f` is `let _ = ...` (error ignored), so a failure leaves the file behind. This is exactly the same pattern as the MEDIUM finding already fixed elsewhere (ensure_libvirt_network → OpenOptions::create_new O_EXCL + mode(0o600)), but here it was not applied and the content is MUCH more sensitive (admin of the entire cluster).

**Failure scenario.** On a multi-user control-plane host, an unprivileged local user: (a) simply reads `/tmp/delonix-admin.conf` (0644) during the cluster apply and gains full cluster-admin access; or (b) pre-creates `/tmp/delonix-admin.conf` as their own file before the apply — the `cp` (no -i, non-interactive under sudo) writes admin.conf's content into that pre-existing file without changing its owner, and the attacker reads it afterward. Result: total compromise of the Kubernetes cluster.

**Suggested fix.** Use an unpredictable filename (mktemp) OR write with O_EXCL/0600 and chown to the SSH user instead of chmod 644; alternatively read admin.conf via `sudo cat` to the ssh_run stdout and never land it in a world-readable file. Check the result of the cleanup `rm`.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev high) · confirms (sev high)

### 5. 🟠 HIGH — safe_join in COPY is purely lexical — symlinks bypass it, re-opening the arbitrary host file read/write it was added to close

- **Location:** `bins/delonix-runtime-bin/src/cmd/build.rs:282`
- **Category:** bug · finder confidence: high

**Description.** safe_join (build.rs:258) rejects only lexical `..`/absolute/prefix components; it never resolves symlinks, and copy_into_rootfs then uses raw std::fs::copy / std::fs::create_dir_all (lines 282, 292, 301-311) which FOLLOW symlinks. The security-audit fix documented in AGENTS.md ('COPY ../../../etc/passwd read arbitrary host files; a dst with .. wrote outside the rootfs') is therefore incomplete: a symlink achieves the exact same escape that the lexical `..` check blocks. Confirmed there is zero canonicalize/symlink_metadata/is_symlink handling in build.rs. copy_dir_all (line 316) uses DirEntry::file_type() (does not follow the link) so a symlink entry falls to the else branch and std::fs::copy follows it, copying the target's contents. This affects both the src side (read host file into image) and the dst side (write host file), and build_from_spec is also reachable via `delonix image apply` (kind: Image, spec.build) from a manifest.

**Failure scenario.** SRC read: build context contains a symlink `creds -> /home/walter/.ssh/id_rsa`; Dockerfile has `COPY creds /app/creds`. safe_join(context,"creds") = context/creds (single Normal component, passes), then std::fs::copy(context/creds, ...) follows the link and bakes the host private key into the image layer — exfiltrated on push/run. DST write: `FROM attacker/base` whose rootfs ships a symlink `/opt/hook -> ../../../../home/walter/.bashrc` (tar stores the link target verbatim, extraction keeps it inside rootfs); Dockerfile `COPY payload /opt/hook`. abs_dst=/opt/hook → safe_join(rootfs,"opt/hook") passes → create_dir_all + std::fs::copy traverse the symlink and overwrite /home/walter/.bashrc on the host as the build user during a rootless build.

**Suggested fix.** After safe_join, canonicalize the resolved src/dst and verify the real path is still inside context/rootfs (starts_with the canonical base); or lstat each path component and refuse symlinks, and use O_NOFOLLOW / symlink_metadata on the final component before copy. copy_dir_all must apply the same check per entry.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev high) · confirms (sev high)

### 6. 🟠 HIGH — Management API unix socket has no peer authentication and no restrictive file mode — full control-plane (incl. container exec) exposed to any local process, gated only by ambient umask

- **Location:** `crates/interfaces/delonix-mgmt/src/lib.rs:63`
- **Category:** design · finder confidence: high

**Description.** `serve_blocking`/`serve_over_uds` bind the management socket (default `unix:///run/delonix-mgmt.sock`, see delonix-runtime-bin/src/main.rs:266-267) with `UnixListener::bind` and NEVER (a) `set_permissions`/chmod the socket, nor (b) check the peer credential (SO_PEERCRED) on accept. The `router()` (lib.rs:102-150) installs no auth/tower middleware — every route (`/v1/containers/:id/exec`, `run`, `rm`, `/v1/images/build`, `network create`, `vm rm`, ...) is served to anyone who can `connect()`. `container_exec_ep` (lib.rs:439-462) runs `container exec <id> sh -c <cmd>` = arbitrary code execution inside any container (often running as root inside its userns). This is the HIGHEST-privilege surface in the runtime, yet it is the ONLY unix-socket server in the repo with zero auth: the holder control socket deliberately sets BOTH `from_mode(0o600)` AND validates `peer_uid() == own_uid` via SO_PEERCRED (delonix-net/src/infra.rs:545-616, `control_loop`), precisely to stop a non-privileged local user from driving the engine. The mgmt socket falls below that bar. Whether another local UID can actually open the socket is left entirely to the ambient umask at bind time (default 022 -> mode 0755 -> connect denied for other users, but a daemon launched with a permissive umask, e.g. systemd `UMask=000`, or a world-traversable `/run` placement, yields mode 0777 -> any local user obtains full control-plane RCE). The same gap exists in delonix-cri/src/lib.rs:283, though that socket is kubelet-facing.

**Failure scenario.** The `delonix api` process is started under a service manager (or shell) with umask 0/002 -> `/run/delonix-mgmt.sock` is created mode 0777. Unprivileged local user `mallory` runs `curl --unix-socket /run/delonix-mgmt.sock -XPOST http://x/v1/containers/<id>/exec -d '{"cmd":"id > /root/pwned"}'` and executes arbitrary commands inside a root-in-userns container, or `POST /v1/containers` to launch new containers, `DELETE` to destroy them, `network create`, etc. No credential is ever checked.

**Suggested fix.** Mirror the holder's belt-and-suspenders: after bind, `set_permissions(path, from_mode(0o600))` on the socket, AND check `SO_PEERCRED` of each accepted connection against the server's own euid (reuse the `peer_uid` pattern from delonix-net infra.rs), rejecting mismatches before dispatching to the router. Apply the same to delonix-cri.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev medium)

### 7. 🟡 MEDIUM — Unbounded blob buffering with attacker-controlled Content-Length aborts/OOMs the pull

- **Location:** `crates/adapters/delonix-image/src/registry.rs:242`
- **Category:** resource · finder confidence: high

**Description.** `blob_with_progress` does `Vec::with_capacity(total.unwrap_or(0) as usize)` where `total = resp.content_length()` is the registry's raw HTTP Content-Length (untrusted). A malicious or MITM registry returning a huge Content-Length (e.g. near u64::MAX) makes `Vec::with_capacity` attempt a giant reservation, which aborts the process on allocation failure (non-recoverable). Independently, the read loop (245-257) accumulates the entire blob into `buf` with no size cap, and the digest verification for OCI artifacts happens only AFTER full buffering (pull_oci_artifact:899), so it cannot prevent exhaustion. A multi-GB (or lie-then-stream) blob OOM-kills the CLI. Same buffering pattern is used by `pull_from_registry_with_creds`, `pull_oci_artifact`, and `RegistryClient::get_blob`, i.e. every `image pull` / `vm pull`.

**Failure scenario.** `delonix vm pull ghcr.io/attacker/img:tag` against a hostile registry that responds to the blob GET with `Content-Length: 9223372036854775807`: `Vec::with_capacity(9.2e18)` aborts the delonix process immediately. Alternatively the server streams an endless chunked body with no declared length -> `buf` grows until the machine OOMs.

**Suggested fix.** Cap the initial `with_capacity` to a sane bound (e.g. min(total, cap)); enforce a maximum blob size in the read loop and error out when exceeded before buffering the rest; ideally stream large blobs to a temp file in the CAS and hash incrementally instead of holding the whole blob in memory.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev medium)

### 8. 🟡 MEDIUM — exec/attach child process and threads leaked when the streaming client disconnects (no kill on stream close)

- **Location:** `crates/interfaces/delonix-cri/src/spdy.rs:762`
- **Category:** resource · finder confidence: high

**Description.** The SPDY (`run_exec`, line 762 `input.close()`) and WebSocket (`streaming.rs` exec_tty line 453 / exec_pipes line 564) streaming handlers never kill the spawned `delonix exec`/`delonix logs -f` child when the connection drops. On disconnect the read loop ends and `input.close()` only closes the pty master (Tty branch); the Pipe branch closes nothing, and the child handle is owned by a detached `child.wait()` thread with no kill path. For attach the child is `delonix container logs -f`, which never exits on its own — the ONLY termination is client disconnect, which does not stop it.

**Failure scenario.** kubectl/crictl attaches to a pod (or execs `sleep 3600`) then aborts (Ctrl-C / API-server timeout). writer_task dies → out_rx dropped → reader thread's send fails and it exits → nobody drains the child's stdout → the `delonix logs -f`/exec process blocks on a full 64 KB pipe forever, and the `child.wait()` thread blocks forever. Each aborted exec/attach permanently leaks one process (plus its in-container setns child) and at least one OS thread; repeated attaches exhaust PIDs/threads on the node.

**Suggested fix.** Hold the Child (or its pid) in the select loop and call child.start_kill()/libc::kill on the process group when the client stream closes (ws_rx None / rd.read_exact error), before joining the wait thread — as containerd/CRI-O do on exec stream teardown.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev medium)

### 9. 🟡 MEDIUM — ContainerStatus reports a fabricated finished_at = now() on every poll for exited containers

- **Location:** `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:795`
- **Category:** bug · finder confidence: medium

**Description.** `container_status` sets `finished_at: if exit.is_some() { now_ns() } else { 0 }` — the real death time is not stored, so the CRI FinishedAt timestamp is recomputed to the current time on each ContainerStatus call. `started_at` is likewise fabricated as `created_at` (line 794) rather than the real start time. The kubelet uses FinishedAt for restart back-off and container age.

**Failure scenario.** A crashed container (restartPolicy OnFailure) sits in Exited. Each time the kubelet polls ContainerStatus, finished_at jumps forward to now, so the container perpetually looks like it 'just died' (age ≈ 0). The kubelet's CrashLoopBackOff / age computations and `kubectl describe` FinishedAt flap forward in time instead of reflecting the actual termination instant, distorting back-off timing.

**Suggested fix.** Persist the real finished/started timestamps when the container transitions to a terminal state (from the Store's reconcile) and echo those fixed values, rather than now_ns() at read time.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev low)

### 10. 🟡 MEDIUM — SecretStore/CredVault path lookups don't sanitize the name → traversal on read/delete

- **Location:** `crates/foundation/delonix-runtime-core/src/secret.rs:78`
- **Category:** bug · finder confidence: medium

**Description.** SecretStore::path() builds `<root>/secrets/{name}.json` via a raw format! with no sanitization. Only save() enforces valid_name(); load(), remove(), resolve_env() and materialize() accept an arbitrary name. This is inconsistent with the rest of the tree, which routes every externally-influenced key through safe_key (Store/JsonStore, store.rs:67), VolumeStore::valid_name (lib.rs:113) or safe_snapshot_name (lib.rs:572). CredVault has the identical gap: cred_path() (cred_vault.rs:102) is only guarded on put(), while get()/remove()/exists() take the name unchecked. The secret name flows in unvalidated from `container run --secret <name>` (container.rs:1346 → resolve_env) and from manifest `spec.secrets` under `stack apply -f <untrusted.yaml>` — the same untrusted-manifest surface the project already treats as hostile (cf. the VM `metadata.name` traversal finding in AGENTS.md). resolve_env reads `<root>/secrets/../<path>.json` and injects any Secret-JSON-shaped file's `data` map as container env (bounded exfil); `secret rm <name>` / `secret unset --all <name>` reach remove() and unlink `<root>/secrets/../../<path>.json` if it ends in .json (arbitrary file delete within the invoking user's privileges).

**Failure scenario.** `delonix secret rm ../../../home/walter/.local/share/delonix/vm-images/foo` (or any path resolving to an existing .json) → remove(name) → fs::remove_file(<root>/secrets/../../../.../foo.json) deletes a file outside the secrets dir. Or a manifest `kind: Container` with `spec.secrets: ["../../containers/<id>"]` applied via `stack apply` → resolve_env reads a file outside <root>/secrets and, if it parses as Secret JSON, injects its data as env.

**Suggested fix.** Route every name through valid_name()/safe_key in SecretStore::path (and valid_cred_name in CredVault::cred_path) on load/remove/exists/resolve_env/materialize, not just on save/put — reject or map the name before any PathBuf::join, matching Store/JsonStore/VolumeStore.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev low)

### 11. 🟡 MEDIUM — SecretStore::save uses a fixed temp file and holds no lock → torn encrypted blob + lost updates

- **Location:** `crates/foundation/delonix-runtime-core/src/secret.rs:103`
- **Category:** concurrency

**Description.** SecretStore::save() writes to a per-name-fixed temp `.{name}.tmp` and renames it over the final file, with no flock anywhere in SecretStore. This is exactly the two failure modes Store deliberately engineered against: Store::save uses a per-writer temp `.{id}.{pid}.{seq}.tmp` (store.rs:140-143) precisely because "two processes writing the SAME container would write over each other in the same temp and the rename would publish an interleaved JSON", and Store::update wraps read-modify-write in FileLock/flock (store.rs:166-182) to prevent lost updates in this daemonless, concurrent-by-design runtime. SecretStore has neither. Two concurrent writers of the same secret name (e.g. two `delonix secret set` invocations, or `stack apply` racing an automation) both fs::write (O_TRUNC) the same `.{name}.tmp` and rename — the published file can contain interleaved/truncated bytes. Because the blob is XChaCha20-Poly1305 AEAD, any corruption makes decode() (secret.rs:82) fail permanently → the secret becomes undecryptable (data loss / DoS), not merely stale. Separately, the load-modify-save in `secret set`/`unset` (secret.rs:247-259, 271-278) has no lock, so concurrent edits silently drop keys (classic lost update). CredVault::write_0600 (cred_vault.rs:55-61) shares the fixed-temp issue via path.with_extension("tmp").

**Failure scenario.** Two `delonix secret set db PASS=...` (or a `stack apply` re-run) touch secret `db` at the same instant: both fs::write `.db.tmp`, their writes interleave, the rename publishes a partially-written SEALED blob → subsequent `store.load("db")` fails to decrypt forever (secret lost). Milder path: concurrent `secret set db A=1` and `secret set db B=2` → one save overwrites the other's read-modify, losing a key with no error.

**Suggested fix.** Make SecretStore::save use a per-writer unique temp (pid+atomic seq, like Store::save) and add an flock-guarded read-modify-write path (mirror Store::update) for the CLI set/unset flows; apply the same unique-temp fix to CredVault::write_0600 and rotate_key.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev medium)

### 12. 🟡 MEDIUM — kubeadm join appends `:6443` to an endpoint that may already contain a port → malformed command on the HA path

- **Location:** `bins/delonix-runtime-bin/src/cmd/cluster.rs:1090`
- **Category:** bug · finder confidence: high

**Description.** valid_endpoint explicitly accepts `host:port` (the test `valid_endpoint("10.0.0.10:6443")` passes; the doc comment says `host[:port]`). kubeadm_init interpolates the RAW endpoint into `--control-plane-endpoint={endpoint}` (kubeadm accepts host:port there). But kubeadm_join does `format!("kubeadm join {endpoint}:6443 ...")`, unconditionally appending `:6443`. Inconsistent port handling between init and join.

**Failure scenario.** An HA cluster (mode: ssh) with `spec.controlPlaneEndpoint: lb.example.com:8443` (LB/VIP on a non-6443 port, a legitimate scenario since both the validation and the schema allow a port). init runs `--control-plane-endpoint=lb.example.com:8443` (OK); join runs `kubeadm join lb.example.com:8443:6443 ...` — a malformed address, and every secondary control-plane's and worker's join fails. The HA path becomes unusable with a custom-port endpoint.

**Suggested fix.** Only append `:6443` when the endpoint does not already contain a port (e.g. detect a `:` in a non-IPv6 host, or parse host/port), guaranteeing symmetry with the usage in kubeadm_init.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev medium)

### 13. 🟡 MEDIUM — pick_route path-prefix match ignores segment boundary → route confusion to wrong backend

- **Location:** `bins/delonix-runtime-bin/src/cmd/ingress_proxy.rs:149`
- **Category:** bug · finder confidence: high

**Description.** The request-time matcher uses a raw `path.starts_with(&r.path)` with no path-segment boundary check. The docstring and AGENTS.md explicitly align the HTTPRoute semantics to the Kubernetes Gateway API, whose PathPrefix match of `/foo` matches `/foo` and `/foo/bar` but NOT `/foobar`. Here `/foobar` DOES match the route `/foo` because it is a plain string prefix. `valid_path_prefix` in httproute.rs only sanitizes config-time paths; it does nothing about how request paths are matched. Combined with the longest-prefix tie-break (`max_by_key` on `r.path.len()`), a request for an unrelated sibling path is silently dispatched to a more-specific backend it was never meant to reach.

**Failure scenario.** HTTPRoute has two path rules on the same host: `/api` → internalApiBackend and `/` → publicBackend. A client requests `/api-docs` (intended for publicBackend). `"/api-docs".starts_with("/api")` is true and `/api` is the longest matching prefix, so pick_route routes the request to internalApiBackend instead of publicBackend — traffic reaches the wrong (internal) service, contradicting the documented k8s PathPrefix semantics.

**Suggested fix.** Match with a segment-boundary rule: accept only when `path == r.path` or `path[r.path.len()..]` begins with `/` (treating a trailing `/` in the configured prefix consistently), i.e. `path.starts_with(&r.path) && (r.path.ends_with('/') || path.len()==r.path.len() || path.as_bytes()[r.path.len()]==b'/')`.

> **Skeptic consensus:** 1 refutation(s) · refutes (sev low) · confirms (sev medium)

### 14. 🟡 MEDIUM — Composed proxy config (config.json) is built outside the flock → concurrent --expose auto-route silently dropped from the live proxy

- **Location:** `bins/delonix-runtime-bin/src/cmd/ingress_proxy.rs:666`
- **Category:** concurrency · finder confidence: high

**Description.** AGENTS.md states the `--expose` auto-registration uses a flock read-modify-write on auto.json to prevent lost updates. `with_auto_locked` does correctly serialize writes to auto.json. But the subsequent `rebuild()` (read manual+auto → compose → `ensure_running` writes config.json → SIGHUP) is called OUTSIDE that lock, in both auto_register (line 666) and auto_deregister (line 674), and rebuild re-reads auto.json unlocked (line 587) then writes config.json via non-atomic `std::fs::write`. The proxy serves config.json (via SIGHUP reload), NOT auto.json — so a stale composed config becomes the live state even though auto.json is correct. The flock guarantee therefore does not extend to the config the proxy actually serves.

**Failure scenario.** Two `container run --expose` run in parallel (e.g. parallel launches or a stack). Thread A: with_auto_locked adds X (auto.json=[X]); A enters rebuild and reads auto.json=[X]. Thread B: with_auto_locked adds Y (auto.json=[X,Y]); B completes its whole rebuild, writing config.json=[X,Y] and SIGHUP. Thread A now finishes its rebuild with its stale snapshot, writing config.json=[X] and SIGHUP last. The proxy reloads config.json=[X] — container Y's route is missing from the live proxy despite auto.json=[X,Y] being correct, and no further trigger re-composes it, so Y stays unreachable via the FQDN indefinitely.

**Suggested fix.** Perform the whole compose+write-config (rebuild/ensure_running) inside the same flock as the auto.json mutation, or make config.json writes atomic (temp file + rename) and serialize rebuild under the lock so the last writer always reflects the final auto.json.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev medium)

### 15. 🟡 MEDIUM — Admission gate (DELONIX_SCAN_ON_PULL) fails OPEN on an unrecognized threshold — a typo silently disables the fail-closed CVE gate

- **Location:** `bins/delonix-runtime-bin/src/cmd/scan.rs:338`
- **Category:** gap · finder confidence: high

**Description.** admission_scan_on_pull is documented as a 'fail-closed GATE'. But an invalid (non-empty, non-'warn', non-severity) policy value is not rejected: admission_rejects (line 303) does Severity::parse(policy) → None → returns false, so the image is admitted (line 338 not taken). Only afterward, at line 345, a warning is printed to stderr. There is no early validation of the policy string, so a misconfiguration silently downgrades a security control from enforcing to advisory. In automated CI the stderr warning is easily lost while the pull succeeds.

**Failure scenario.** Operator sets DELONIX_SCAN_ON_PULL=criticl (typo for 'critical') intending to block critical-CVE images. On pull, scan_image runs, admission_rejects(worst, "criticl") returns false because Severity::parse fails, the image is NOT removed and the pull succeeds; a vulnerable image is admitted with only a stderr warning. Same for any casing/spelling slip (e.g. 'crit', 'high '→trimmed ok, but 'hihg').

**Suggested fix.** Validate the policy once after reading the env var: accept only {warn, low, medium, high, critical}; on any other non-empty value return Err (fail closed) instead of scanning-then-warning, so a misconfigured gate refuses rather than admits.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev medium)

### 16. 🟡 MEDIUM — CIFS/SMB password embedded in world-readable mount argv (defeats --password-secret vault)

- **Location:** `bins/delonix-runtime-bin/src/cmd/storage.rs:163`
- **Category:** bug · finder confidence: high

**Description.** build_mount() for the cifs/smb driver pushes the resolved password inline into the mount options string as `password=<secret>` (storage.rs:158-165). That string is later handed to `delonix-volume::VolumeStore::ensure_mounted`, which invokes `Command::new("mount").args(["-t","cifs",device,mountpoint,"-o",options])` (crates/adapters/delonix-volume/src/lib.rs:202-213). The password thus appears as a plain process argument. Mounting CIFS requires CAP_SYS_ADMIN, so in the supported/privileged path `mount.cifs` runs as root and its `/proc/<pid>/cmdline` is world-readable — any unprivileged local user can read the NAS credential while the mount runs. This directly undermines the documented purpose of `--password-secret`/`kind: Secret` (AGENTS.md and storage.rs:47-49 claim the vault path avoids exposing the password). mount.cifs(8) explicitly warns against `password=` on the command line and provides `credentials=<file>` for exactly this reason; that safer path is not used.

**Failure scenario.** Admin stores the NAS password in the vault and runs `delonix storage create nas --type cifs --server nas --share media --username alice --password-secret nascreds` (or `stack apply` with a Storage referencing passwordSecret). During the mount, an unprivileged local attacker runs `grep -a password= /proc/*/cmdline` (or `ps aux`) and reads `//nas/media -o username=alice,password=<cleartext>`, capturing the secret the vault was supposed to protect.

**Suggested fix.** Do not pass credentials via `-o password=`; write them to a mode-0600 temporary credentials file and pass `credentials=<file>` (or feed the password to mount.cifs on stdin). Keep the secret out of any process argument list.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev medium)

### 17. 🟡 MEDIUM — CIFS credentials/options are comma-joined without escaping — commas in a password inject/break mount options

- **Location:** `bins/delonix-runtime-bin/src/cmd/storage.rs:177`
- **Category:** bug · finder confidence: high

**Description.** build_mount() assembles the cifs option list by pushing `username={u}`, `password={p}`, `ro`, and the user's `extra` string, then joins them with ',' (storage.rs:157-178). CIFS mount options are delimited by commas and there is no escaping. A perfectly legitimate password (or username) that contains a comma is split by mount.cifs: everything after the comma is reinterpreted as further mount options. This both breaks correct credentials (a password like `a,b3!` fails to authenticate or the mount errors on an unknown option) and, when the value is attacker-influenced through an untrusted `stack apply -f` manifest Secret, lets the trailing text act as injected CIFS mount options (e.g. `uid=0,file_mode=0777,dir_mode=0777`). mount.cifs has no way to escape a comma inside `-o`; the canonical fix is a credentials file.

**Failure scenario.** A user creates a Secret whose `password` key is `S3cr,et` and a `kind: Storage` (cifs) referencing it. resolve_password returns `S3cr,et`; build_mount produces options `username=...,password=S3cr,et`; mount.cifs treats `et` as a bogus mount option and fails — or, with a crafted value such as `x,file_mode=0777,dir_mode=0777`, silently mounts the share world-writable. The failure is confusing (looks like a wrong password) and the injection path exists.

**Suggested fix.** Move credentials out of the comma-joined `-o` string entirely (credentials file / stdin). At minimum, reject or reject-with-clear-error any username/password containing a comma or newline before building the option string.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev low)

### 18. 🟡 MEDIUM — Node DNS resolver does a full O(n) scan of every container+VM record on every A query, single-threaded and serialized behind blocking upstream forwards

- **Location:** `crates/adapters/delonix-net/src/infra.rs:3206`
- **Category:** performance · finder confidence: high

**Description.** The holder's service-discovery DNS server (`dns_server_main`, infra.rs:3087) is a single std::thread with one blocking `recv_from` loop: it handles exactly one query at a time and only reads the next datagram after the current one is fully answered. Two costs compound on that serial path:

1. O(n) disk scan per query: `handle_dns` calls `dns_resolve(&name)` for EVERY A query (infra.rs:3132-3133), including external names. `parse_internal_name` returns `Some((whole_name, None))` for an external domain like `github.com`, so `dns_resolve` (infra.rs:3210-3251) still `read_dir`s `<base>/containers`, `fs::read`s and JSON-parses EVERY container record, then `read_dir`s+parses EVERY VM record, before giving up and forwarding. So all DNS traffic on the node — including every container's outbound internet lookup — pays a full re-read+parse of all container and VM JSON files, with no cache.

2. Head-of-line blocking on forward: on a miss, `forward_dns` (infra.rs:3158-3171) does a synchronous `send_to`+`recv_from` with a 3s read timeout against SLIRP_DNS and then 1.1.1.1 (up to ~6s total). Because the server thread is serial, that stall freezes DNS for the whole node — no other container can resolve anything until it returns.

**Failure scenario.** On a Kubernetes node (kubelet + many pods) with, say, 50 container records, each pod DNS lookup re-reads and JSON-parses all 50 files single-threaded; concurrent lookups queue behind each other. If one container queries a black-holed/slow domain, `forward_dns` blocks the single server thread for up to ~6s, during which every other container's DNS (including internal service discovery <name>.<ns>.delonix.internal) is stalled — a single misbehaving client degrades DNS for all workloads.

**Suggested fix.** Handle each datagram in its own task/thread (or an async UDP loop) so a slow forward can't block other queries; and resolve internal names from an in-memory index refreshed on record change (or short-TTL cache / inotify) instead of a full directory scan per query. Fast-path: only scan the store when the name actually carries an internal suffix, forwarding external names immediately.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev medium) · confirms (sev medium)

### 19. ⚪ LOW — valid_version accepts a patch version (1.31.2) but k8s_repo_version builds a nonexistent pkgs.k8s.io path

- **Location:** `bins/delonix-runtime-bin/src/cmd/k8s_recipes.rs:37`
- **Category:** gap · finder confidence: medium

**Description.** k8s_repo_version does `format!("stable:/v{v}")` with the RAW version. The pkgs.k8s.io repository only publishes directories at the minor level (`stable:/v1.31/deb/`), not patch. However valid_version (cluster.rs) accepts it and its doc comment explicitly states that `1.31` OR `1.31.2` are both valid. So a patch version passes validation but generates a 404 repository URL.

**Failure scenario.** `spec.k8sVersion: "1.31.2"` (accepted by valid_version) → the repository recipe writes `deb [...] https://pkgs.k8s.io/core:/stable:/v1.31.2/deb/ /` → `apt-get update && apt-get install` fails with a 404 on EVERY host in the cluster during prepare_host, aborting the bootstrap. The docs say this value is valid, creating a wrong expectation.

**Suggested fix.** Truncate the version to major.minor when building the repository URL (`stable:/vMAJOR.MINOR`), keeping the full version only for kubeadm's `--kubernetes-version`; or restrict valid_version/documentation to major.minor for the repo path.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev low)

### 20. ⚪ LOW — Concurrent first-time ensure_running double-spawns two proxies → orphaned proxy with no pidfile

- **Location:** `bins/delonix-runtime-bin/src/cmd/ingress_proxy.rs:720`
- **Category:** concurrency · finder confidence: medium

**Description.** ensure_running checks running_pid() and, if None, calls spawn_proxy() + publish_listeners() with no lock. When the proxy does not yet exist and two callers (e.g. an httproute apply and a container run --expose, or two --expose launches) run concurrently, both observe running_pid()==None and both spawn a proxy. Both proxies try to bind the same listener port(s) inside the holder netns; one wins, the other crashes at bind. The pidfile is overwritten by whichever spawn writes last (line 807). If the crashed proxy's pid was written last, running_pid() later cleans it as dead while the surviving proxy has no pidfile recorded — it becomes an orphan that can no longer be targeted by SIGHUP (reload) or SIGTERM (stop).

**Failure scenario.** No proxy is running. `delonix httproute apply` and `delonix container run --expose ... ` (or two --expose runs) execute at the same moment. Both reach spawn_proxy; proxy A binds :8080 and lives, proxy B fails to bind and exits after writing proxy.pid with B's pid. running_pid() finds B dead, removes the pidfile; proxy A keeps serving but is untracked — a later `httproute rm`/apply cannot signal it, and stop() leaves it running.

**Suggested fix.** Guard the running_pid()-check → spawn → pidfile-write sequence in ensure_running under the same lock file already used for auto.json (or a dedicated proxy.lock), so the spawn decision is serialized and only one proxy is ever started.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev low)

### 21. ⚪ LOW — kube generate emits YAML with unescaped quoting — command args break or inject YAML

- **Location:** `bins/delonix-runtime-bin/src/cmd/kube.rs:79`
- **Category:** bug · finder confidence: high

**Description.** pod_manifest() serializes container command/args by wrapping each token with `quote()` = `format!("\"{s}\"")` (kube.rs:59-63, 78-80), which does not escape embedded double-quotes, backslashes, or newlines. A container whose command contains a `"` yields a token like `"echo "x""`, which is invalid YAML; a token containing a newline injects raw line breaks into the document. Since the whole point of `kube generate` is to pipe the output into `kubectl apply -f -`, a container created with a quote/newline in its command produces a manifest that either fails to parse or is structurally altered (extra keys after a newline-carrying value).

**Failure scenario.** User runs `delonix container run --name web alpine sh -c 'echo "hi"'` then `delonix kube generate web | kubectl apply -f -`. pod_manifest emits `args: ["-c", "echo "hi""]`, which kubectl rejects as malformed YAML; a token containing an embedded newline could instead inject sibling keys into the pod spec.

**Suggested fix.** Build the Pod value with a real YAML serializer (serde_yaml, already a dependency here) or properly escape/emit block/flow scalars, instead of hand-formatting with an unescaped `format!("\"{s}\"")`.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev low)

### 22. ⚪ LOW — Dashboard reconciles every VM twice per 1s refresh, doubling virsh subprocess spawns

- **Location:** `bins/delonix-runtime-bin/src/cmd/dash.rs:112`
- **Category:** performance · finder confidence: high

**Description.** `DashData::collect` (called every 1s in the TUI loop, dash.rs:429-430) builds the VM list as `delonix_vm::list(&root).into_iter().map(|v| delonix_vm::status(&root, &v.name)...)`. But `delonix_vm::list` (delonix-vm/src/lib.rs:1367-1374) ALREADY reconciles each VM by calling `status(base, &vm.name)` per entry. The `.map(status)` in dash therefore reconciles every VM a second time. For the libvirt backend each `status` is not cheap: `is_running`→`libvirt_domain_uri` probes up to two URIs via `virsh domstate` (infra.rs:664-668) plus another `domstate`/`domifaddr` for the IP — several `virsh` process spawns per VM. So every dashboard tick forks roughly twice as many `virsh` processes as needed.

**Failure scenario.** A host running the libvirt backend with e.g. 8 VMs open in `delonix dash`: each 1s tick spawns ~4 virsh subprocesses per VM twice = ~64 forks/sec, half of them pure waste, making the dashboard laggy and adding steady CPU load for a read-only view.

**Suggested fix.** Drop the redundant `.map(|v| delonix_vm::status(...))` and use the already-reconciled result of `delonix_vm::list(&root)` directly.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev low)

### 23. ⚪ LOW — Image pull buffers each layer fully in RAM before writing to CAS

- **Location:** `crates/adapters/delonix-image/src/registry.rs:662`
- **Category:** memory · finder confidence: high

**Description.** During pull, each layer is fetched by `blob_with_progress` (registry.rs:227-259) which reads in 64KB chunks but accumulates the whole blob into a single `Vec<u8>` (`buf` pre-sized to Content-Length), then `store.cas().write(&data)` (cas.rs:50) takes `&[u8]` and writes it out — no streaming to disk. Peak resident memory equals the size of the largest layer. The chunked read exists only for progress reporting, not to bound memory.

**Failure scenario.** Pulling a large image (e.g. `kindest/node` or the golden VM artifact, layers of hundreds of MB to multiple GB) on a small/memory-constrained node causes a RAM spike of one full layer, risking OOM-kill of the pull (or of co-located workloads) on nodes where that headroom isn't available.

**Suggested fix.** Stream the blob to a temp file while updating the running sha256, then rename into CAS (a `write_streaming(reader) -> digest` on the CAS), so peak memory is the 64KB chunk rather than the whole layer.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev low)

### 24. ⚪ LOW — Management (and CRI) accept loop terminates the entire server on any transient accept() error, permanently downing the control-plane

- **Location:** `crates/interfaces/delonix-mgmt/src/lib.rs:80`
- **Category:** resource · finder confidence: high

**Description.** In `serve_over_uds` the accept loop does `let (socket, _) = uds.accept().await.map_err(|e| Error::Runtime{...})?;` — the `?` propagates ANY accept error out of `serve_over_uds`, which returns from `serve_blocking`, which exits the `delonix api` process. `accept()` can fail with recoverable, per-connection errors — most importantly EMFILE/ENFILE (per-process/system fd exhaustion) and ECONNABORTED. Because each accepted connection is `tokio::spawn`ed with no concurrency/fd limit, a burst of connections (each holding an fd until its handler finishes; long ops like `image build`/`pull` hold them for minutes) can drive the process to its fd ceiling; the next `accept()` returns EMFILE and the whole management API dies permanently instead of shedding load and recovering. The identical pattern exists in delonix-cri/src/lib.rs (its accept path), so a single recoverable condition takes down the kubelet-facing runtime endpoint too.

**Failure scenario.** A client (or bug) opens many concurrent connections / triggers several concurrent long builds until the process hits RLIMIT_NOFILE. The subsequent `uds.accept()` returns EMFILE; `serve_over_uds` returns Err; `serve_blocking` returns and the `delonix api` process exits. All subsequent control-plane calls fail with connection-refused until an operator manually restarts it — a transient, self-clearing condition became a hard outage.

**Suggested fix.** Do not `?` out of the accept loop on per-connection errors: log and `continue` on EMFILE/ENFILE/ECONNABORTED (optionally with a short backoff), reserving fatal exit for truly unrecoverable listener errors. Consider bounding in-flight connections. Apply the same fix to delonix-cri.

> **Skeptic consensus:** 0 refutation(s) · confirms (sev low) · confirms (sev low)

## Unverified findings (verification interrupted by session limit)

⚠️ These come from the finders but their two skeptics failed due to quota — they **did not** go through adversarial verification. Treat them as **candidates to confirm**, not as confirmed. They concentrate on the highest-risk subsystems (syscall core, SDN holder, `container run`).

### 1. 🟠 HIGH — `container run --rm` leaks the entire rootfs directory in rootless mode (both foreground and watcher paths)

- **Location:** `bins/delonix-runtime-bin/src/cmd/container.rs:1597`
- **Category:** resource · finder confidence: high

**Description.** The two `--rm` auto-removal paths — the foreground branch (lines 1592-1598) and the detached `spawn_rm_watcher` (lines 1651-1666) — both do `runtime::remove` + `unpublish_ports` + `images.unmount_rootfs(&c.id)`, but NEITHER calls `images.remove_container_dir(&c.id)`. In rootless mode (the product's default/primary mode) the container's rootfs is a FLAT copy at `containers/<id>/rootfs`, and `unmount_rootfs` DELIBERATELY preserves that directory (overlay.rs:263-273: `if base.join("rootfs").exists()` it only removes merged/upper/work and keeps `rootfs/`). `runtime::remove` (lib.rs:4288-4303) only removes the cgroup and the store JSON record — it never touches the container directory. So every `--rm` container in rootless leaves its full rootfs behind forever. This is exactly the leak documented as fixed in `cmd_rm` (container.rs:2369-2375 — '49 directories (45 GiB) piled up ... kubelet marked disk-pressure'), where the fix WAS to add `remove_container_dir`; the fix was applied only to the explicit `rm` path, not to the `--rm` auto-remove paths that share the same intent. `delonix system prune` is the only backstop, but `--rm`'s entire contract is automatic full cleanup (as in Docker, which removes the container filesystem).

**Failure scenario.** Rootless: `delonix container run --rm alpine echo hi` (foreground) or `delonix container run -d --rm ...` (detached, watcher path). Container exits → record removed, cgroup removed, but `~/.local/share/delonix/containers/<id>/rootfs` (tens–hundreds of MB per image) is never deleted. Repeated ephemeral runs (CI, `--rm` loops, build work containers) accumulate orphan rootfs dirs until the disk fills and, on a Delonix-hosted k8s node, the kubelet applies `disk-pressure` — the identical incident AGENTS.md records as already fixed for `rm`.

**Suggested fix.** In both `--rm` branches call `images.remove_container_dir(&c.id)` after `unmount_rootfs`, mirroring `cmd_rm`/`remove_container` (container.rs:2355/2375). In rootless the flat `rootfs/` must be destroyed on `--rm`, not preserved.

### 2. 🟠 HIGH — Global `egress` command silently deletes all per-network egress restrictions (fail-open)

- **Location:** `crates/adapters/delonix-net/src/infra.rs:1531`
- **Category:** design · finder confidence: high

**Description.** `do_egress` (the GLOBAL egress policy, reached from `set_egress_policy`/`delonix ... egress deny|allow`) begins by deleting EVERY rule in the `fwdeny` chain whose listing line contains both `oifname "tap0"` and `drop`. Its intent is to remove only the single global blanket rule `oifname "tap0" drop`, but the substring match is too broad: the PER-NETWORK egress restrictions installed by `apply_egress_from_state`/`egress_specs` (infra.rs:2179-2203) are of the form `iifname "<bridge>" oifname "tap0" ... drop` — they also contain `oifname "tap0"` and `drop`, so they are deleted too. Once the per-network terminal `drop` is gone, egress falls through `fwdeny` into the `forward` chain, which unconditionally accepts `oifname "tap0"` (infra.rs:152), so the affected network regains full unrestricted Internet egress. The interaction is asymmetric: `do_egress_net` scopes its own cleanup to `iifname "<bridge>"` and never touches the global rule, so only the global command corrupts per-network state, and it does so silently.

**Failure scenario.** User configures a restricted network: `egress net secure deny` (or `allowlist:...`), installing `iifname "dlxnXXXX" oifname "tap0" drop`; containers on `secure` cannot reach the Internet. Later any global `egress deny` followed by `egress allow` (or a lone `egress allow`) invokes `do_egress`, whose removal loop deletes the `secure` drop rule (it matches `oifname "tap0"`+`drop`). The `allow` branch adds nothing back. Result: containers on `secure` now have unrestricted egress to the Internet, with no error and no indication the per-network policy was wiped — a data-exfiltration path reopened by an unrelated command.

**Suggested fix.** Restrict the removal loop to the global rule only: skip any listing line that also contains `iifname` (the global blanket rule has no `iifname`), or delete the global rule by a stored handle/tag rather than by broad substring match. Add a regression test that installs a per-network egress drop, runs global `egress allow`, and asserts the per-network drop survives.

### 3. 🟡 MEDIUM — VFIO PCI address interpolated into libvirt domain XML without hex-validation or xml_escape (manifest-reachable)

- **Location:** `crates/adapters/delonix-vm/src/lib.rs:862`
- **Category:** bug · finder confidence: high

**Description.** In libvirt_domain_xml, every user-influenced value (name, overlay, seed, mac, net, bridge, cpuset) is passed through xml_escape() — except the PCI passthrough address. parse_pci_addr (lib.rs:914) merely splits the device string on '/', '.', ':' and returns the four raw substrings with NO validation that they are hexadecimal; libvirt_domain_xml then interpolates them directly into <address domain='0x{dom}' bus='0x{bus}' slot='0x{slot}' function='0x{func}'/> (lib.rs:862-864) with no escaping. cfg.devices is attacker-influenced: the manifest spec.devices list is passed through untouched (vm.rs:405 devices: spec.devices) and applies() reaches libvirt_domain_xml via create(). Because the first-colon token (dom) may contain any character except ':' '.' '/', a value like `0' foo='bar:00:00.0` yields dom=`0' foo='bar` and produces `<address domain='0x0' foo='bar' bus='0x00' .../>` — an injected XML attribute; a value like `a':00:00.0` yields a stray quote that makes the XML malformed so `virsh define` fails. Full new-element injection is blocked because closing tags need '/' (consumed by rsplit('/')), which caps the impact at attribute injection / define-time DoS rather than arbitrary XML, but the missing escape+validation is a real gap that breaks the module's own escaping discipline and is exactly the 'endereço' validation the VFIO lens asks about.

**Failure scenario.** Manifest `spec: { devices: ["0' foo='x:00:00.0"] }` (or `--device "a':00:00.0"`) with backend libvirt → libvirt_domain_xml emits malformed/attribute-injected <address>, so `virsh define` fails (VM cannot be created) or an unintended attribute is injected into the hostdev address element. No hex sanity check means a bogus address also reaches libvirt unfiltered.

**Suggested fix.** In parse_pci_addr, reject any component that is not valid hex of the expected width (domain 4, bus 2, slot 2, func 1) and return None otherwise; additionally wrap the four values in xml_escape() at the interpolation site as defense-in-depth, matching every other field in libvirt_domain_xml.

### 4. 🟡 MEDIUM — Re-exec spec file written world-readable with plaintext `-e` secrets, persisting for the whole foreground container lifetime

- **Location:** `bins/delonix-runtime-bin/src/cmd/container.rs:1993`
- **Category:** resource · finder confidence: medium

**Description.** `reexec_into_netns` (used by every `--net <custom>` and `--pod` run) serializes the full `RunOpts` to `state_root()/.reexec-<id>.json` via `std::fs::write` (line 1991-1993). `std::fs::write` creates the file with mode 0666 & ~umask (typically 0644 = world-readable), unlike the rest of the project's security-sensitive writes which use `OpenOptions::create_new().mode(0o600)` (the fix applied to the libvirt XML temp file per Audit #2). `RunOpts.env` carries the raw `-e KEY=VALUE` pairs the user passed on the command line — commonly credentials (`-e DB_PASSWORD=...`, `-e API_TOKEN=...`). The spawn uses `.status()` which BLOCKS until the child exits; for a FOREGROUND container the child's `cmd_run` blocks in `create_with` until the container's process terminates, so the world-readable JSON containing plaintext secrets lives in the state root (rootless: `$XDG_DATA_HOME/delonix` or `$HOME/.local/share/delonix`, which are often 0755) for the container's entire lifetime, only removed at line 2003 after exit.

**Failure scenario.** On a shared host: user runs `delonix container run --net mynet -e DB_PASSWORD=s3cr3t postgres` (foreground). A co-tenant local user does `readdir` on the state root and `cat ~victim/.local/share/delonix/.reexec-*.json` for the whole time the container runs, reading `s3cr3t` in cleartext. In root mode the file lands in `/var/lib/delonix` (0755) — readable by any local user.

**Suggested fix.** Create the spec file with `OpenOptions::new().write(true).create_new(true).mode(0o600)` (same pattern the project already adopted for `ensure_libvirt_network`), or write it under a 0700 per-user runtime dir. Consider excluding raw secret-bearing env from the serialized spec and re-resolving from the SecretStore in the child.

### 5. 🟡 MEDIUM — DNS external-domain hijack via dotted container name (unenforced anti-hijack assumption)

- **Location:** `crates/adapters/delonix-net/src/infra.rs:3199`
- **Category:** bug · finder confidence: high

**Description.** parse_internal_name (infra.rs:3179) maps any name that is NOT suffixed .delonix.internal/.delonix.io to a whole-name match with no namespace: `api.github.com` -> ("api.github.com", None) (infra.rs:3199-3203, and the test at 3780 asserts exactly this). dns_resolve (infra.rs:3206) then iterates ALL container records in <base>/containers/*.json and returns the container's `ip` whenever v["name"].to_lowercase() == the queried name (infra.rs:3217-3229), short-circuiting BEFORE forward_dns (handle_dns, infra.rs:3132-3148). The documented and unit-tested anti-hijack guarantee (AGENTS.md 'DNS interno'; test parse_internal_name_handles_all_schemes comment: 'matches no container foo.com -> forwards') is load-bearing on the claim that container names contain no dots. That claim is NOT enforced anywhere: `--name`/manifest `metadata.name` flows raw into Container.name (cmd/container.rs:1228-1248, cmd/container.rs:787) with no character validation (no valid_name in delonix-runtime-core for containers; sanitize() is only applied to netns/device paths, never to the stored name). A container literally named `api.github.com` therefore matches, and because the whole-name/legacy path resolves in ANY namespace, the hijack crosses the namespace boundary that AGENTS.md presents as a security guarantee ('isolation also in DNS'). This EXTENDS the documented guarantee: the docs assert the hijack is impossible; the missing name validation makes it possible. The DNS server is always spawned in the holder (infra.rs:560) and serves every container/VM on the SDN.

**Failure scenario.** Operator applies an innocuous-looking untrusted manifest `delonix stack apply -f third-party.yaml` containing `kind: Container` with `metadata.name: api.github.com` (a syntactically valid name). The container is created with Container.name = "api.github.com" and gets an SDN IP. Any other container on the shared SDN (including one in a different namespace, e.g. `prod`) that resolves `api.github.com` receives an A record pointing at the attacker container's internal IP instead of forwarding to the real upstream (dns_resolve returns before forward_dns) -> silent MITM of the external service for the whole SDN. The repo's own trust model treats `stack apply -f` manifest metadata.name as untrusted (see the VM path-traversal finding in AGENTS.md), so this is in-scope for that model.

**Suggested fix.** Enforce the assumption the resolver depends on: reject/validate container (and VM) names against a whitelist that forbids '.' (and other DNS-significant characters) at the engine boundary (delonix-runtime-core Container::new or cmd_run, analogous to delonix_vm::valid_vm_name), OR make dns_resolve only match names that cannot contain a '.' by construction. At minimum, in the whole-name/legacy path of parse_internal_name, refuse to treat a multi-label name as an internal name unless it carries a delonix suffix, so a dotted external FQDN can never match a container record.

### 6. 🟡 MEDIUM — Bind-mount target is not resolved with securejoin — a symlink in the image escapes the rootfs (mkdir/file creation on the host as root)

- **Location:** `crates/adapters/delonix-runtime/src/lib.rs:967`
- **Category:** bug · finder confidence: medium

**Description.** `bind_volume` (called by `setup_rootfs` in the `for m in mounts` loop at lib.rs:1193, BEFORE `pivot_root`) builds the target by raw concatenation `dst = format!("{rootfs}{}", m.target)` and then does `std::fs::create_dir_all(&dst)` / `OpenOptions...open(&dst)` (file branch) and finally `mount(source, dst)`. `mount_target_safe` only rejects `..`/relative paths in the LOGICAL path — it does NOT confine the resolution of symlinks that exist INSIDE the image's rootfs. Since these operations run before the pivot, `/` is still the host's filesystem, shared (`MS_PRIVATE` only isolates mount propagation, not the inodes). In ROOT mode (the engine with sudo, no userns — `userns=false`), `setup_rootfs`/`bind_volume` run as the host's REAL root. The same pattern applies to `apply_tmpfs` (create_dir_all of the target) and to the mount points created by other helpers. The path-traversal audit documented in AGENTS.md covers `mount_target_safe` (only `..`), the build's COPY, and the VM name — but does NOT cover the resolution of symlinks in the bind target inside the rootfs, so this is a new vector. runc/Docker use securejoin (component-by-component confined resolution with O_NOFOLLOW) precisely to close this.

**Failure scenario.** A malicious image ships `/var` as a symlink to `/etc` (or `/var -> /`). The operator runs, in root mode, `delonix container run -v data:/var/data <malicious-image>`. While preparing the rootfs, `create_dir_all("{rootfs}/var/data")` follows the symlink `{rootfs}/var` (absolute target `/etc`, resolved against the host's root pre-pivot) and creates `/etc/data` ON THE HOST as root; the file branch creates an empty file at a path chosen by the image (e.g. `/etc/nologin` → login DoS). An integrity/DoS primitive controlled by the image (mkdir + empty-file creation at an arbitrary absolute path) on the host, as root. In rootless (default) the mapped uid (100000) is not root on the host, which mitigates it; that is why it is rated MEDIUM and limited to root mode.

**Suggested fix.** Resolve the bind target (and --tmpfs/--device) with a join confined to the rootfs (per-component O_NOFOLLOW / openat2 RESOLVE_IN_ROOT, or the same `safe_join`/`safe_rel` pattern already used in `delonix-image::overlay` and `cmd::build`), rejecting any symlink component that escapes the rootfs, before create_dir_all/File::create/mount.

### 7. 🟡 MEDIUM — Blocking `read` after a failed `poll` defeats the 10s guard and can hang `run` forever

- **Location:** `crates/adapters/delonix-net/src/infra.rs:508`
- **Category:** concurrency · finder confidence: high

**Description.** `start_slirp` guards the slirp ready-fd with `wait_readable(rd, 10_000)` (a `poll`), whose entire documented purpose is to avoid a bare blocking `read` that hangs forever when the child never signals and never closes the write end (a grandchild inheriting `wr` — created by `libc::pipe` WITHOUT O_CLOEXEC — is enough to prevent EOF). However, when `wait_readable` returns `false` (10s timeout: the fd is neither readable nor hung-up), the code only logs a warning and then unconditionally executes `libc::read(rd, ...)` on the still-blocking fd (lines 511-516). Since there is no data and no EOF, that `read` blocks indefinitely, reintroducing the exact deadlock the poll was added to prevent — and because `slirp_attach` now runs before the container entrypoint is released, this hangs the whole `run` with no log and no exit. The sibling per-container path `slirp_attach` (lib.rs:2232) has an even barer version: a blocking `read` with no poll guard at all.

**Failure scenario.** slirp4netns starts but stalls before writing its ready byte (slow host, resource pressure, or a forked helper) while keeping the inherited write-end open. `wait_readable` times out at 10s and returns false; execution falls through to `libc::read(rd,...)`, which blocks forever because no ready byte is written and `wr` is still held open (no EOF). `delonix container run` hangs indefinitely instead of surfacing the warned error downstream.

**Suggested fix.** Only perform the `read` when `wait_readable` returned true; on timeout, skip the read (and `close(rd)` regardless) so the flow proceeds to surface the error. Apply the same `wait_readable` guard to `slirp_attach` in lib.rs, and/or create the pipe with `O_CLOEXEC`/`O_NONBLOCK` so an inherited copy cannot pin the write end open.

### 8. 🟡 MEDIUM — `system prune` can tear down the entire ingress infra mid-`run` (ref-marker TOCTOU)

- **Location:** `crates/adapters/delonix-net/src/infra.rs:304`
- **Category:** concurrency · finder confidence: medium

**Description.** `attach_container` writes a container's ingress ref-marker via `acquire(id)` (container.rs:1429) well before the container record is persisted to the Store (container.rs:1575). `reap_orphan_refs` (infra.rs:296-306), invoked by `system prune` (system.rs:239), builds its `live` set exclusively from the Store (running containers) plus `cri-*`/`vm-*` prefixed markers. A marker written by an in-flight `run` whose record is not yet saved is therefore classified as an orphan, removed, and — if it is the only remaining marker — triggers `teardown()`, which SIGTERMs the holder and slirp and destroys the shared netns. This is under `FileLock`, so it is not a data race, but the reaper's view of "live" is stale relative to the acquire-before-save window, and teardown destroys networking for the racing container (and drops all veths/nft state of any others, since the holder netns is shared).

**Failure scenario.** A single container is being created: `attach_container` has written its ref-marker but `store.save` has not yet run. Concurrently the operator runs `delonix system prune`; `live_refs` is assembled from the Store and does not contain the new id, so `reap_orphan_refs` removes its marker, finds the refs dir now empty, and calls `teardown()` — killing the holder and slirp. The container's subsequent `control_send(attach ...)` hits a dead holder and the run fails; if other containers were relying on the ingress they lose networking too.

**Suggested fix.** Have `attach_container` acquire the ref-marker under a broader guarantee (e.g., persist a minimal Store placeholder before acquire, or have prune take the ingress `FileLock` and additionally cross-check `attached_refs` against in-flight leases in the IPAM registry), so a marker for an id with a live IPAM lease is never treated as orphan.

### 9. ⚪ LOW — `find` silently resolves an ambiguous id prefix to the newest container instead of erroring

- **Location:** `bins/delonix-runtime-bin/src/cmd/util.rs:72`
- **Category:** design · finder confidence: high

**Description.** `find` matches with `c.id == q || c.id.starts_with(q) || c.name == q` and returns the FIRST hit from `store.list()`, which is sorted by `created_unix` descending (store.rs:211). When a short id prefix matches more than one container, `find` silently picks the most-recently-created one rather than reporting ambiguity. Docker/Podman refuse an ambiguous prefix (`multiple ... found`). This drives destructive verbs — `container stop`/`rm` all resolve via `find` — so an ambiguous prefix can act on an unintended container without warning. Also, an id prefix match takes precedence position-wise over a container whose exact NAME equals `q` only by list ordering, so `q` intended as a name could resolve to a different container whose id happens to start with `q`.

**Failure scenario.** Two containers with ids `a1f3...` and `a1f9...`; user runs `delonix container rm a1` intending the older one. `find` returns the newest matching container (list is created-desc) and `rm` destroys the wrong container, silently — no ambiguity error.

**Suggested fix.** Collect all matches; if more than one distinct container matches (and there is no exact id/name equality that uniquely disambiguates), return an error listing the candidates, matching Docker semantics. Prefer exact `id ==`/`name ==` matches before falling back to prefix.

### 10. ⚪ LOW — `exec` leaks the container's namespace fds in the parent process (namespaces stay pinned after the container dies)

- **Location:** `crates/adapters/delonix-runtime/src/lib.rs:3591`
- **Category:** resource · finder confidence: high

**Description.** In `exec`, the `/proc/<pid>/ns/{user,uts,net,pid,mnt}` fds are opened as RAW i32s in the parent (lib.rs:3585-3591) and stored in `fds: Vec<(&str,i32)>`. Only the CHILD of the 1st fork closes them (via `OwnedFd::from_raw_fd(*fd)`, which closes on drop after the setns, 3624-3631). In the `ForkResult::Parent` branch (3734-3763) the fds are NEVER closed; the `Vec<(&str,i32)>` being dropped does not close raw i32s, and `O_CLOEXEC` only helps on an `execve` by the parent itself (which does not happen for a persistent caller). This contrasts with `mount_live`/`unmount_live`, which use `OwnedFd` (via `open_container_ns`) and therefore close correctly in the parent branch — the asymmetry confirms the bug. There is no UAF/double-free, it is a pure leak. The real impact today is low (the callers — the CLI and the CRI's `delonix exec` subprocess — are short-lived and exit soon after); but `delonix_runtime::exec` is a public API, and an in-process, long-lived supervisor that execs repeatedly would exhaust fds AND, worse, each ns fd KEEPS the namespace (mnt/net/pid/user) alive even after the container dies, preventing the release of its associated resources.

**Failure scenario.** A long-lived, in-process caller (e.g. a future supervisor, or if the CRI stopped delegating and called `exec` directly) makes N `exec` calls on a container and then the container is removed/dies. The up to 5 ns fds per call accumulate in the parent (fd exhaustion) and the pinned mnt/net/pid fds prevent teardown of the dead container's namespaces (netns/mount-tree leak). In the current CLI the process exits and cleans up, so severity is low.

**Suggested fix.** Wrap the fds in `OwnedFd` (as `open_container_ns` does), or explicitly close all `fds` in the `ForkResult::Parent` branch before returning.

### 11. ⚪ LOW — `apply_sysctls` writes `net.*` sysctls into the HOST's netns when the container uses `--net host`

- **Location:** `crates/adapters/delonix-runtime/src/lib.rs:1728`
- **Category:** design · finder confidence: high

**Description.** `sysctl_namespaced` (lib.rs:1728) returns `true` for all `net.*`, on the assumption that network sysctls are always namespaced and therefore safe for the container to change. That is only true if the container has ITS OWN netns (`CLONE_NEWNET`). In a `--net host` container (no `CLONE_NEWNET`, see `spawn` lib.rs:3062 — the NEWNET flag is not added), `/proc/sys/net` reflects the HOST's netns. `apply_sysctls` runs in `setup_rootfs` (called at lib.rs:1227) BEFORE `/proc/sys` becomes read-only (`mask_proc_paths`) and still with all capabilities, writing directly into `/proc/sys/net/...`. Missing the cross-check Docker does (Docker refuses `--sysctl net.*` combined with host networking for precisely this reason), the value gets applied to the host. It is operator input (not the container's), hence low; still, it is a silent containment failure (the operator believes they are tuning the container).

**Failure scenario.** `delonix container run --net host --sysctl net.ipv4.conf.all.rp_filter=0 <img>` (or `net.ipv6.conf.all.forwarding=1`) writes the host netns's GLOBAL knob, turning off reverse-path filtering / turning on forwarding machine-wide, instead of affecting only the container.

**Suggested fix.** Refuse `--sysctl net.*` (with a clear error, consistent with the "no silent failure" invariant) when the container shares the host's netns (no `CLONE_NEWNET`), mirroring Docker; or apply the net.* only on the path with its own netns.

## Refuted during verification

- ~~write_rec is not atomic for concurrent writes to the same record id (temp name only per-process)~~ (`crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:152`) — struck down by the skeptics.

## Completeness-critical (next step)

Adversarial verification did NOT cover, due to quota, these subsystems — they deserve a 2nd
run (starting 18:10 Luanda time, when the session resets):

- `delonix-runtime/src/lib.rs` — the 104-`unsafe` core (clone/mount/setns/seccomp): the
  **memory-safety** lens was practically unverified. It is the repo's highest-risk point.
- `delonix-net/src/infra.rs` — holder + control socket + nft: the 5 finder findings here
  (including 1 HIGH destructive global `egress`) went without adversarial verification.
- `container.rs` and `vm.rs` — the HIGH rootfs-leak finding on `--rm` and the VFIO-in-XML one.

Furthermore, the finder phase ran 1 pass per subsystem; a 2nd pass with a
**concurrency-under-load** lens (CRI + mgmt + proxy simultaneously) and a
**parser fuzzing** pass (DNS, .po, YAML manifest, layer tar) would catch what static reading
does not.

---

## Focused pass on `delonix container` (2026-10-22, v5.1.1)

Scope: the `delonix container` path (lifecycle, state, recovery, isolation,
images, GC). Baseline: workspace `cargo check` green; `delonix-state` 38/38;
`clippy` on the touched crates with no warnings. Gaps L1–L6 from
`docs/paridade-docker-podman.md` were already closed (incl. L1's silent
network degradation, now `host (degraded: …)` with a test). Method: 4
adversarial agents (lifecycle, image pull, isolation, GC) + direct reading.

### CLOSED · `container cp` followed symlinks/`..` outside the rootfs (HIGH, CVE-2019-14271 class)

**Description.** `cmd_cp` (`bins/delonix-runtime-bin/src/cmd/container.rs`) joined
the `container:/path` side to the rootfs with `root.join(cpath.trim_start_matches('/'))`
— with no `..` normalization — and copied with `copy_recursive`, which used
`src.is_dir()` + `std::fs::copy`, both **following symlinks**. In a RUNNING
container the root is `/proc/<pid>/root`, which the kernel confines; but in a
STOPPED container the root is a plain host directory (`merged`/`rootfs`, or an
`__ovlhold` that does **not** `pivot_root` — `cmd/mapped.rs:678`), with no
confinement at all. An image shipping `escape -> /etc/shadow`, or a `..` in the
path, led `cp` to read/write host files as the engine (root in rootful mode;
the uid in rootless). It is the same class the repo had already closed for
binds and build's `COPY` via `safe_bind_target`, and that `cp` had been left
out of — just like `write_etc_files` once was (see L3 of the parity doc).

**Failure scenario.** Read: `delonix container cp web:/escape /tmp/theft` with
`escape -> /etc/shadow` in the rootfs exfiltrates the host's `/etc/shadow`. `..`:
`cp web:/../../../../etc/passwd /tmp/x`. Write: `cp ./payload web:/hook` with
`hook -> /host/victim` overwrites the host file.

**Fix.** (1) `cmd_cp` resolves the container side with
`delonix_linux::safe_bind_target` (now `pub` — reused, not rewritten a 3rd
time), which clamps `..` to the rootfs and resolves a symlink INSIDE the
rootfs (an absolute target restarts at the rootfs, never at the host's `/`).
(2) `copy_recursive` now uses `symlink_metadata` (lstat) and NEVER dereferences
a symlink: it recreates it as-is (like `docker cp`), and unlinks a symlink
already present at the destination before writing — closing the escape at
EVERY level of the tree, not just at the top.

**Regression tests** (`container.rs`, test module):
`cp_nao_segue_symlink_do_container_para_fora` (read),
`cp_nao_escreve_atraves_de_symlink_no_destino` (write),
`cp_confina_dot_dot_e_symlink_do_caminho_do_container` (top-level
confinement). Proven: the first two FAIL with the old `copy_recursive` and
pass with the new one; `clippy` clean. **Validated by test; NOT validated
live** against a real stopped container (no environment in this session) —
the live container's `/proc/<pid>/root` path still relies on kernel
confinement.

### Priority queue (reproduced and traced; NOT fixed — one session each)

1. **HIGH · IPC sysctls written into the host's namespace with `--host-ipc`** —
   `sysctl_namespaced` (`delonix-linux/src/lib.rs:3213`) returns `true`
   unconditionally for `kernel.sem/shm*/msg*` and `fs.mqueue.*`; `apply_sysctls`
   only receives `has_own_netns`, never `has_own_ipcns`. `run --host-ipc --sysctl
   kernel.shmall=…` changes the host's GLOBAL knob (DoS). It is the IPC sibling
   of the `net.*` finding already documented above — the same hardening applied
   to `net.*` was never applied to IPC.
2. **MEDIUM (data loss) · `prune` releases an IPAM lease / unmounts the rootfs of
   a container whose `remove` was PRESERVED** — `cmd/prune.rs:366-372` swallows
   the `Err` from `delonix_linux::remove` (when a concurrent `start` wins the
   race and the record is preserved) and still runs `release_leases` +
   `unmount_rootfs` + `remove_container_dir` → two live containers on one IP.
   The real `cmd_rm` propagates the error; `prune` diverges. Fix: gate the three
   steps on `remove(...) == Ok`.
3. **MEDIUM-HIGH (availability) · supervisor restart counter never resets** —
   `supervise.rs:125/251` accumulates for the supervisor's entire lifetime:
   `on-failure:N` becomes a lifetime budget (never restarts again after N
   failures spaced months apart) and the `always` backoff never returns to
   baseline. Diverges from docker (which resets after stable uptime), contrary
   to the comment in `supervise.rs:255`.
4. **MEDIUM · `wait` returns "exit code not captured" for a supervised
   container** — a race in `wait_for_exit` (`container.rs:5056`): a concurrent
   reconcile marks `Crashed/process_gone` before the supervisor does
   `wait_and_record`, and `wait` gives the "runs in FOREGROUND" error to a
   container that has a supervisor precisely so the exit code is known.
5. **MEDIUM · silent SELinux degradation** — `apply_selinux`
   (`delonix-linux/src/lib.rs:3024`) swallows the MAC-context write error
   (`let _ =`), with no fail-closed (126) like AppArmor has. Reachable only via
   CRI `SELinuxOptions`; on a SELinux-enforcing node it leaves the container in
   the default domain with no error.
6. **MEDIUM · `apply_layer_flat` follows planted symlinks when pre-creating/
   chmod-ing parent dirs** — `delonix-oci/src/overlay.rs:229-252`: effects
   (`mkdir`/`chmod`) OUTSIDE the extraction root (the content write itself is
   blocked by tar-rs). Affects the flat path (`image export`, rootless work
   containers), not `run`.

Recommended next priority: **#1 (HIGH, same silent-containment class).**
