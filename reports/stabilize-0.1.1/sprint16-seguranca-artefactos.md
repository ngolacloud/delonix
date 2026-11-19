# §16 — Artefact security and integrity (Delonix v0.1.0 → v0.1.1 campaign)

Release/Security Engineering. Date: 2026-10-29.
Repo: `delonix-runtime` @ `8ab783dd` (branch `docs/auditoria-consolidacao`, tree **clean** reconfirmed before and after).
Audited binary: `target/release/delonix` → `delonix 0.1.0`.
Published release: `v0.1.0` on `github.com/ngolacloud/delonix` (immutable tag, NOT touched).

Status legend: **PASS** (verified passing) · **FAIL** (verified failing) · **MECHANISM** (exists, not exercised here) · **ABSENT**.

---

## 1. Verification → result → evidence table

| # | Verification | Result | Evidence |
|---|---|---|---|
| 1.1 | `scripts/release_verify.py` runs offline | **PASS** | `python3 scripts/release_verify.py --bin target/release/delonix` → `ok: 6 referência(s) de evidência, todas presentes na árvore` (exit 0) |
| 1.2 | Pipeline produces SHA256SUMS | **PASS** | `release.yml` L229: `sha256sum ... > SHA256SUMS`; asset present in the release |
| 1.3 | Pipeline produces SPDX SBOM and puts it inside the signed SHA256SUMS | **PASS** | `release.yml` L230-236: `scripts/sbom.py --version ... > delonix-sbom.spdx.json` and `sha256sum delonix-sbom.spdx.json >> SHA256SUMS` |
| 1.4 | minisign signature **fail-closed** in the pipeline | **PASS** | `release.yml` L266-317: if `MINISIGN_PUBKEY` is real and the secret is missing → `exit 1` ("ALL installers would refuse it"); minisign downloaded by fixed digest; verifies the signature against the embedded pubkey before publishing |
| 1.5 | SLSA provenance (attest-build-provenance) | **MECHANISM** | `release.yml` L324-343: `actions/attest-build-provenance@v1` over the 15 binaries + SBOM; OIDC Sigstore/Fulcio, no stored key. Verifiable with `gh attestation verify <file> --repo ngolacloud/delonix` (not exercised because binaries not downloaded) |
| 2.1 | minisign signature of SHA256SUMS **verifies** with the embedded pubkey | **PASS** | `minisign -V -p <pubkey install.sh> -m SHA256SUMS` → `Signature and comment signature verified / Trusted comment: delonix-runtime v0.1.0` |
| 2.2 | Pubkey in the published `install.sh` == repo and is real (not a placeholder) | **PASS** | Both `RWSiOqlKAnVVB+pJLQxgYHq/kdN6RbBQdlL5gOcZ6H/xkwSAPIqTo+GB` (≠ `__POR_PREENCHER__`) → fail-closed ARMED |
| 2.3 | `install.sh` fail-closed on the client side | **PASS** | `install.sh` L483-498: aborts if the pubkey is a placeholder, if minisign cannot be installed, if the `.minisig` is missing, or if the signature does not verify; only `--insecure-skip-signature` bypasses |
| 2.4 | SHA256SUMS consistent for the downloaded files | **PASS** | `sha256sum -c` → `install.sh: OK`, `delonix-sbom.spdx.json: OK`, `delonix-vscode: OK` (18 entries = 15 binaries + install.sh + sbom + vscode) |
| 3.1 | cargo-audit / cargo-deny / deny.toml present | **PASS** | `cargo-audit 0.22.2`, `cargo-deny 0.20.2`, `deny.toml` (schema v2) |
| 3.2 | `cargo deny check` — licenses / bans / sources | **PASS** | `advisories FAILED, bans ok, licenses ok, sources ok`; only permissive (Apache-2.0/MIT/BSD/ISC/…), 100% crates.io, no git |
| 3.3 | `cargo deny check advisories` / `cargo audit` | **FAIL** | `error: 9 vulnerabilities found! / 16 allowed warnings`; detail in §findings |
| 4.1 | `unsafe` inventory | **PASS (measured)** | 478 lines: adapters 295, delonix-runtime-bin 96, interfaces 36, contexts 19, providers 11, storage-helper 8, foundation 1, node-api-bin 1 (concentration in adapters = kernel-native path, expected) |
| 4.2 | storage-helper least-privilege | **PASS (code)** | `SO_PEERCRED` (uid comes from the kernel), never accepts uid/path/device/command from the request, one-shot socket-activated, refuses if stdin is not a unix stream socket; socket mode set by the systemd `.socket` unit |
| 4.3 | container `cp` path-traversal hardened | **PASS (code)** | public `safe_bind_target()` in `delonix-linux/src/lib.rs:1278` (symlink/`..` escape closed) |
| 4.4 | Layer / zip-bomb limits (LayerLimits) | **PASS (code)** | `delonix-oci/src/overlay.rs`: `LimitReader` behind the decompressor with `LayerLimits::max_bytes`/`max_entries`; detection by magic bytes (gzip/zstd/tar) |
| 5.1 | Real secrets committed | **ABSENT (none)** | Hits in `redact.rs` (redaction logic), `secret.rs`/`gitops.rs` (handling), and DEMO keys in `examples/gateway/*.yaml` — see finding SEC-05 |
| 5.2 | Insecure defaults | **ABSENT (fail-closed)** | TLS verified by default; `insecure_tls`/`insecureSkipVerify` is an EXPLICIT opt-out (`danger_accept_invalid_certs(target.insecure_tls)`, proxmox L7959: "TLS verified by default … explicit opt-out") |
| 6.1 | SBOM is a valid inventory | **PASS** | SPDX-2.3, `dataLicense CC0-1.0`, `name delonix-0.1.0`, 402 packages (**401 3rd-party deps, all with SHA256 checksum**), 402 relationships, deterministic `created` (1970-01-01, reproducible by content) |

---

## 2. Security findings

### SEC-01 — zstd/zstd-safe memory-safety in the image decompression path — **MEDIUM** — *gating for v0.1.1*
- **Advisories:** RUSTSEC-2026-0391 (data leak from an abandoned stream in a reused context), 0398 (`Seekable::decompress` sets the `Vec` `len` beyond capacity), 0403 (uninitialized memory exposed in legacy frame decompression), 0405 (OOB read), 0406 (use-after-free in a multithreaded context).
- **Shipping path:** `zstd v0.13.3 → delonix-oci → delonix-{cri,mgmt,mcp,node-api,scanner,runtime-bin}` — present in **ALL 5 binaries**.
- **Impact:** `delonix-oci/overlay.rs` decompresses OCI image layers `tar+zstd` (magic `28 b5 2f fd`). UNTRUSTED input (image pulled from a registry). Memory-safety class (UAF/OOB/memory exposure) reachable via a forged zstd frame.
- **Existing mitigation:** `LayerLimits`/`LimitReader` bounds the volume (decompression bombs) — **but does not cover** the memory-safety class (triggered by frame content, not by size).
- **Fix:** `cargo update -p zstd -p zstd-safe -p zstd-sys` (published patches: zstd ≥0.14.3, zstd-safe ≥8.2.0, zstd-sys ≥2.1.2). Dependency bump, no change to our code.
- **Deadline:** v0.1.1 (it is precisely the target of the stabilization campaign).

### SEC-02 — quinn-proto (5 advisories, 2× 7.5 high) — **INFORMATIONAL / no exposure**
- RUSTSEC-2026-0338/0339/0340/0341/0342 (QUIC DoS/panics, two 7.5 high).
- **Does NOT affect the artefacts:** `cargo tree -i quinn-proto --target all` → *nothing to print*. `quinn` is an **optional dependency of `reqwest`** (QUIC/http3 feature off); it is in the `Cargo.lock` (which cargo-audit scans in full) but **not compiled** into any binary. cargo-audit over-report.
- **Action:** optional — `cargo update`/lock cleanup to reduce gate noise; no security urgency.

### SEC-03 — lru (unsound + UAF) — **LOW**
- RUSTSEC-2026-0002 (`IterMut` violates Stacked Borrows), 0253 (UAF from missing panic-safety in `LruCache::pop()`).
- **Path:** `lru v0.12.5 → ratatui → delonix-runtime-bin` (only the `delonix dash` TUI). Local/interactive surface, no remote input.
- **Fix:** depends on `ratatui` dropping/bumping `lru`; low priority.

### SEC-04 — Unmaintained dependencies — **INFORMATIONAL / accepted with justification**
- `paste` (RUSTSEC-2024-0436, transitive via ratatui), `rustls-pemfile` (RUSTSEC-2025-0134, direct in the TLS proxy), `unsafe-libyaml` (RUSTSEC-2024-0449, transitive via serde_yaml).
- All **consciously ignored** in `deny.toml` with a reason and a follow-up note. They are `unmaintained`, not CVE. Correct governance (not blindly silenced).

### SEC-05 — DEMO private keys in example manifests — **LOW**
- `examples/gateway/ingress.yaml` and `httproute.yaml` embed cert+key PEM inline as a demonstration of `kind: Ingress`/TLS.
- They are example/disposable keys (not production secrets), a common pattern in examples. **Note:** never reuse them; ideally reference a generated Secret instead of embedding. Not a secret leak.

---

## 3. §16 verdict — is there a blocker to production publication?

**v0.1.0 artefact integrity — GREEN. No blocker.**
The minisign signature verifies against the embedded pubkey, the SHA256SUMS is consistent and also covers the SBOM and the extension, install.sh is fail-closed (client and pipeline), the SPDX SBOM is valid (402 packages, 401 deps with checksum), and SLSA provenance is wired. No integrity failure.

**Supply-chain / advisories — RED GATE, but no remotely exploitable critical in the delivered artefacts.**
- The high advisories (quinn-proto 7.5) are **not compiled** into the binaries (optional feature off) → no exposure.
- The real exposure is **SEC-01 (zstd, MEDIUM)**: memory-safety reachable when decompressing a malicious OCI image; bomb bounded by `LayerLimits`, but the memory-safety class is not; **patch available by a simple bump**.
- lru (LOW, local TUI) and the unmaintained ones (accepted in deny.toml) are not blockers.

**Conclusion:** there is NO critical blocker that forces pulling v0.1.0 (integrity intact, high not shipped). **BUT** the v0.1.1 campaign **cannot close §16 as "production-hardened" without resolving SEC-01** — bumping `zstd`/`zstd-safe`/`zstd-sys` and restoring the `cargo deny check advisories` gate to green is the mandatory item of this stabilization. SEC-02 (quinn lock cleanup) and SEC-03 (lru) are lower-priority follow-ups.
