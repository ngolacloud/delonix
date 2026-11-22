# Supply chain and release — `fa8c6297` / release v3.1.0

| Control | Evidence | State |
|---|---|---|
| `Cargo.lock` present and `--locked` on all gates | baseline | PASS |
| Git dependencies | none in the workspace `Cargo.toml` | PASS |
| Advisories / licences / bans / sources | `cargo deny check`: all ok (`raw/deny.log`) | PASS |
| `cargo audit` | tool absent | NOT TESTED |
| Actions pinned by commit | **0 of 20** `uses:` with SHA (`@v4`, `@v2`, `@v1`) | FAIL (G-020) |
| Release signature | `SHA256SUMS.minisig` published; install.sh verifies with the embedded public key and fails closed | PASS (design) |
| SLSA provenance | `actions/attest-build-provenance@v1` in release.yml | PASS (design) |
| SBOM | `delonix-sbom.spdx.json` published and inside the signed SHA256SUMS | PASS (design) |
| Real verification of the v3.1.0 assets (`minisign -V`, `gh attestation verify`) | requires downloading ~100 MB — **not authorised in this session** | NOT TESTED |
| Installer with wrong hash/signature, interrupted download, wrong arch | — | NOT TESTED |
| Artefacts per architecture | only `x86_64` and `x86_64-v3` | PARTIAL (G-021) |
| Third-party downloads in the installer (cloud-hypervisor, firmware) | HTTPS only, no checksum (documented risk) | PARTIAL |
| Reproducible builds | — | NOT TESTED |
| Secret scanning | — | NOT TESTED |
