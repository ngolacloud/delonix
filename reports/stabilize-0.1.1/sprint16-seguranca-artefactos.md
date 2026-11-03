# §16 — Segurança e integridade dos artefactos (Delonix v0.1.0 → campanha v0.1.1)

Release/Security Engineering. Data: 2026-10-29.
Repo: `delonix-runtime` @ `8ab783dd` (branch `docs/auditoria-consolidacao`, árvore **limpa** reconfirmada antes e depois).
Binário auditado: `target/release/delonix` → `delonix 0.1.0`.
Release publicada: `v0.1.0` em `github.com/ngolacloud/delonix` (tag imutável, NÃO tocada).

Legenda de estado: **PASSA** (verificado a passar) · **FALHA** (verificado a falhar) · **MECANISMO** (existe, não exercitado aqui) · **AUSENTE**.

---

## 1. Tabela verificação → resultado → evidência

| # | Verificação | Resultado | Evidência |
|---|---|---|---|
| 1.1 | `scripts/release_verify.py` corre offline | **PASSA** | `python3 scripts/release_verify.py --bin target/release/delonix` → `ok: 6 referência(s) de evidência, todas presentes na árvore` (exit 0) |
| 1.2 | Pipeline produz SHA256SUMS | **PASSA** | `release.yml` L229: `sha256sum ... > SHA256SUMS`; asset presente na release |
| 1.3 | Pipeline produz SBOM SPDX e mete-o dentro do SHA256SUMS assinado | **PASSA** | `release.yml` L230-236: `scripts/sbom.py --version ... > delonix-sbom.spdx.json` e `sha256sum delonix-sbom.spdx.json >> SHA256SUMS` |
| 1.4 | Assinatura minisign **fail-closed** no pipeline | **PASSA** | `release.yml` L266-317: se `MINISIGN_PUBKEY` real e falta o secret → `exit 1` ("TODOS os instaladores a recusariam"); minisign baixado por digest fixo; verifica a assinatura contra a pubkey embutida antes de publicar |
| 1.5 | Proveniência SLSA (attest-build-provenance) | **MECANISMO** | `release.yml` L324-343: `actions/attest-build-provenance@v1` sobre os 15 binários + SBOM; OIDC Sigstore/Fulcio, sem chave armazenada. Verificável com `gh attestation verify <file> --repo ngolacloud/delonix` (não exercitado por binários não descarregados) |
| 2.1 | Assinatura minisign do SHA256SUMS **verifica** com a pubkey embutida | **PASSA** | `minisign -V -p <pubkey install.sh> -m SHA256SUMS` → `Signature and comment signature verified / Trusted comment: delonix-runtime v0.1.0` |
| 2.2 | Pubkey no `install.sh` publicado == repo e é real (não placeholder) | **PASSA** | Ambos `RWSiOqlKAnVVB+pJLQxgYHq/kdN6RbBQdlL5gOcZ6H/xkwSAPIqTo+GB` (≠ `__POR_PREENCHER__`) → fail-closed ARMADO |
| 2.3 | `install.sh` fail-closed do lado do cliente | **PASSA** | `install.sh` L483-498: aborta se pubkey placeholder, se minisign não instalável, se falta `.minisig`, ou se a assinatura não verifica; só `--insecure-skip-signature` contorna |
| 2.4 | SHA256SUMS coerente para os ficheiros descarregados | **PASSA** | `sha256sum -c` → `install.sh: OK`, `delonix-sbom.spdx.json: OK`, `delonix-vscode: OK` (18 entradas = 15 binários + install.sh + sbom + vscode) |
| 3.1 | cargo-audit / cargo-deny / deny.toml presentes | **PASSA** | `cargo-audit 0.22.2`, `cargo-deny 0.20.2`, `deny.toml` (schema v2) |
| 3.2 | `cargo deny check` — licenses / bans / sources | **PASSA** | `advisories FAILED, bans ok, licenses ok, sources ok`; só permissivas (Apache-2.0/MIT/BSD/ISC/…), 100% crates.io, sem git |
| 3.3 | `cargo deny check advisories` / `cargo audit` | **FALHA** | `error: 9 vulnerabilities found! / 16 allowed warnings`; detalhe no §achados |
| 4.1 | Inventário `unsafe` | **PASSA (medido)** | 478 linhas: adapters 295, delonix-runtime-bin 96, interfaces 36, contexts 19, providers 11, storage-helper 8, foundation 1, node-api-bin 1 (concentração em adapters = caminho kernel-native, esperado) |
| 4.2 | storage-helper least-privilege | **PASSA (código)** | `SO_PEERCRED` (uid vem do kernel), nunca aceita uid/path/device/comando do pedido, one-shot socket-activated, recusa se stdin não é unix stream socket; modo do socket definido pela unit `.socket` systemd |
| 4.3 | container `cp` path-traversal endurecido | **PASSA (código)** | `safe_bind_target()` pública em `delonix-linux/src/lib.rs:1278` (fuga symlink/`..` fechada) |
| 4.4 | Limites de camada / zip-bomb (LayerLimits) | **PASSA (código)** | `delonix-oci/src/overlay.rs`: `LimitReader` por trás do descompressor com `LayerLimits::max_bytes`/`max_entries`; detecção por magic bytes (gzip/zstd/tar) |
| 5.1 | Segredos reais commitados | **AUSENTE (nenhum)** | Hits em `redact.rs` (lógica de redacção), `secret.rs`/`gitops.rs` (manuseio), e chaves DEMO em `examples/gateway/*.yaml` — ver achado SEC-05 |
| 5.2 | Defaults inseguros | **AUSENTE (fail-closed)** | TLS verificado por omissão; `insecure_tls`/`insecureSkipVerify` é opt-out EXPLÍCITO (`danger_accept_invalid_certs(target.insecure_tls)`, proxmox L7959: "TLS verified by default … explicit opt-out") |
| 6.1 | SBOM é inventário válido | **PASSA** | SPDX-2.3, `dataLicense CC0-1.0`, `name delonix-0.1.0`, 402 pacotes (**401 deps 3rd-party, todas com checksum SHA256**), 402 relationships, `created` determinístico (1970-01-01, reprodutível por conteúdo) |

---

## 2. Achados de segurança

### SEC-01 — zstd/zstd-safe memory-safety no caminho de descompressão de imagens — **MÉDIA** — *gating para v0.1.1*
- **Advisories:** RUSTSEC-2026-0391 (fuga de dados de stream abandonado em contexto reutilizado), 0398 (`Seekable::decompress` põe `len` do `Vec` além da capacidade), 0403 (memória não-inicializada exposta em descompressão de legacy frame), 0405 (OOB read), 0406 (use-after-free em contexto multithreaded).
- **Caminho de shipping:** `zstd v0.13.3 → delonix-oci → delonix-{cri,mgmt,mcp,node-api,scanner,runtime-bin}` — presente em **TODOS os 5 binários**.
- **Impacto:** `delonix-oci/overlay.rs` descomprime camadas de imagem OCI `tar+zstd` (magic `28 b5 2f fd`). Input NÃO-confiável (imagem puxada de registo). Classe de memory-safety (UAF/OOB/exposição de memória) alcançável por frame zstd forjado.
- **Mitigação existente:** `LayerLimits`/`LimitReader` limita o volume (bombas de descompressão) — **mas não cobre** a classe memory-safety (disparada por conteúdo do frame, não por tamanho).
- **Correcção:** `cargo update -p zstd -p zstd-safe -p zstd-sys` (patches publicados: zstd ≥0.14.3, zstd-safe ≥8.2.0, zstd-sys ≥2.1.2). Bump de dependência, sem tocar código nosso.
- **Prazo:** v0.1.1 (é precisamente o alvo da campanha de estabilização).

### SEC-02 — quinn-proto (5 advisories, 2× 7.5 high) — **INFORMATIVO / sem exposição**
- RUSTSEC-2026-0338/0339/0340/0341/0342 (DoS/panics QUIC, duas 7.5 high).
- **NÃO afecta os artefactos:** `cargo tree -i quinn-proto --target all` → *nothing to print*. O `quinn` é dependência **opcional do `reqwest`** (feature QUIC/http3 desligada); está no `Cargo.lock` (que o cargo-audit varre por inteiro) mas **não é compilado** em nenhum binário. Over-report do cargo-audit.
- **Acção:** opcional — `cargo update`/limpeza do lock para reduzir ruído do gate; sem urgência de segurança.

### SEC-03 — lru (unsound + UAF) — **BAIXA**
- RUSTSEC-2026-0002 (`IterMut` viola Stacked Borrows), 0253 (UAF por falta de panic-safety em `LruCache::pop()`).
- **Caminho:** `lru v0.12.5 → ratatui → delonix-runtime-bin` (só o TUI `delonix dash`). Superfície local/interactiva, sem input remoto.
- **Correcção:** depende do `ratatui` largar/bumpar `lru`; baixa prioridade.

### SEC-04 — Dependências não mantidas — **INFORMATIVO / aceite com justificação**
- `paste` (RUSTSEC-2024-0436, transitivo via ratatui), `rustls-pemfile` (RUSTSEC-2025-0134, directa no proxy TLS), `unsafe-libyaml` (RUSTSEC-2024-0449, transitivo via serde_yaml).
- Todos **ignorados conscientemente** em `deny.toml` com razão e nota de follow-up. São `unmaintained`, não CVE. Governança correcta (não silenciados às cegas).

### SEC-05 — Chaves privadas DEMO em manifestos de exemplo — **BAIXA**
- `examples/gateway/ingress.yaml` e `httproute.yaml` embutem cert+chave PEM inline como demonstração do `kind: Ingress`/TLS.
- São chaves de exemplo/descartáveis (não segredos de produção), padrão comum em examples. **Nota:** nunca reutilizar; idealmente referir um Secret gerado em vez de embeber. Não é fuga de segredo.

---

## 3. Veredicto §16 — há bloqueio à publicação de produção?

**Integridade dos artefactos v0.1.0 — VERDE. Sem bloqueio.**
A assinatura minisign verifica contra a pubkey embutida, o SHA256SUMS é coerente e cobre também SBOM e extensão, o install.sh é fail-closed (cliente e pipeline), o SBOM SPDX é válido (402 pacotes, 401 deps com checksum), e a proveniência SLSA está cabeada. Nenhuma falha de integridade.

**Supply-chain / advisories — GATE VERMELHO, mas sem crítica explorável remota nos artefactos entregues.**
- As advisories high (quinn-proto 7.5) **não estão compiladas** nos binários (feature opcional desligada) → sem exposição.
- A exposição real é **SEC-01 (zstd, MÉDIA)**: memory-safety alcançável ao descomprimir uma imagem OCI maliciosa; bomba limitada por `LayerLimits`, mas a classe memory-safety não; **patch disponível por simples bump**.
- lru (BAIXA, TUI local) e não-mantidas (aceites em deny.toml) não bloqueiam.

**Conclusão:** NÃO há bloqueio crítico que obrigue a retirar a v0.1.0 (integridade intacta, high não-shipada). **MAS** a campanha v0.1.1 **não pode fechar o §16 como "produção-endurecida" sem resolver SEC-01** — bumpar `zstd`/`zstd-safe`/`zstd-sys` e repor o gate `cargo deny check advisories` a verde é o item obrigatório desta estabilização. SEC-02 (limpeza do lock quinn) e SEC-03 (lru) são follow-ups de menor prioridade.
