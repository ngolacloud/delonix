# CNCF gap analysis — Delonix × Docker (8 áreas de valor)

> Leitura do código oficial do Docker/Moby (via `gh`: `moby/moby`,
> `moby/buildkit`, `docker/scout-cli`, `docker/compose`, `compose-spec/compose-spec`,
> linhagem `opencontainers/runc` + `containerd/containerd`) cruzada com o código
> do Delonix (`delonix-scanner`, `delonix-oci/sign.rs`, `build.rs`, CRI) e os
> testes de 2026-11-08. Objectivo: o que falta para **100% nas 8 áreas de valor
> CNCF**. Imparcial: inclui onde o Delonix **já** está alinhado.

## Onde o Delonix já está alinhado (não inflacionar o gap)
- **CVE via OSV** — `delonix-scanner` consome feeds **OSV** (formato aberto CNCF), offline. ✔
- **Assinatura cosign/sigstore** — `delonix-oci/src/sign.rs` é compatível com **cosign** (ECDSA-P256, artefacto `.sig`, simplesigning). ✔
- **Rootless-first** — paridade filosófica com BuildKit/Moby rootless. ✔

## Matriz das 8 áreas → caminho para 100%

| # | Área de valor CNCF | Docker (evidência) | Delonix hoje | Itens para 100% | Estado |
|---|---|---|---|---|---|
| 1 | **Linhagem de runtime** | containerd (CNCF *graduated*) + runc (ref. OCI) | runtime Rust runc-compat, fora da cadeia containerd | GAP-0012 | ✗ |
| 2 | **Conformância OCI/CRI** | containerd CRI 100%; Moby ref. OCI image/dist | CRI 84/19 rootless · 90/13 rootful-VM; bugs OCI | BUG-0003/04/05/06 · GAP-0001/0003 · GAP-0012 | ◑ |
| 3 | **Provenance SLSA no build** | BuildKit SLSA v0.2/v1.1 (min/max, builder-id, reproducible) | não gera provenance no build | **GAP-0008** | ✗ |
| 4 | **SBOM interoperável (in-toto/SPDX)** | BuildKit SBOM SPDX+in-toto, scanner plugável (syft) | scanner gera SBOM p/ CVE, não anexa SPDX in-toto à imagem | **GAP-0009** | ◑ |
| 5 | **Build cloud-native** | cache distribuído (registry/S3/Azure/GHA), multi-plat, frontends LLB | COPY/`--link` ok; sem cache dist./multi-plat/frontends | **GAP-0010** | ✗ |
| 6 | **Compose como especificação aberta** | Compose Specification comunitária | compose próprio, lacunas (DEF-003) | GAP-0005 + **GAP-0011** | ◑ |
| 7 | **Governança aberta** | Moby governança aberta; doou runc/containerd | repo privado, org própria | **GAP-0014** | ✗ |
| 8 | **Scanning maduro em CI** | Scout: policy, `compare`, gating CI (`--exit-code`) | scanner OSV novo, sem policy/compare/CI | **GAP-0013** | ◑ |

Legenda: ✗ ausente · ◑ parcial · ✔ feito. **100% = todas a ✔.**

## Prioridade (lente CNCF)
1. **Supply-chain de build** (áreas 3+4) — GAP-0008, GAP-0009. Aditivo: `sign.rs`
   (cosign) e `delonix-scanner` (SBOM/OSV) já dão metade do caminho; falta
   emitir **in-toto/SPDX** + **SLSA provenance** e anexar como attestation OCI.
2. **Conformância comprovada** (áreas 1+2) — GAP-0012 + os BUGs CRI já abertos.
3. **Compose Spec** (área 6) — GAP-0005 + GAP-0011.
4. **Scanning em CI** (área 8) — GAP-0013.
5. **Build avançado** (área 5) — GAP-0010.
6. **Governança** (área 7) — GAP-0014 (decisão do dono, não só código).

---

## Fichas dos GAPs novos (formato backlog)

### GAP-0008 — Provenance SLSA no `delonix build` (área 3)
- **Current:** `build.rs` constrói a imagem (replica COPY/`--link` do BuildKit) mas não emite proveniência.
- **Expected (CNCF):** BuildKit emite SLSA provenance (v0.2/v1.1, min/max) como in-toto attestation anexada à imagem.
- **Target 100%:** `delonix build --provenance[=mode=min|max]` produz um predicate SLSA v1.1 (builder.id, materials, invocation) embrulhado em in-toto e anexado ao índice OCI.
- **Component:** `crates/adapters/delonix-oci/src/build.rs` + `.../sign.rs` (reutilizar) + `image_ops.rs`. **Compl.:** L. **Dep.:** GAP-0009 (partilham o anexo de attestations).

### GAP-0009 — SBOM SPDX in-toto anexado à imagem (área 4)
- **Current:** `delonix-scanner` extrai SBOM (apk/dpkg) para match de CVE, mas não o publica como attestation da imagem.
- **Expected (CNCF):** BuildKit anexa SBOM **SPDX** embrulhado em **in-toto**, scanner plugável.
- **Target 100%:** `delonix build --sbom` emite SPDX JSON (a partir do scanner existente) em in-toto attestation, anexada ao índice OCI; `delonix image sbom <ref>` exporta SPDX.
- **Component:** `crates/adapters/delonix-scanner` (emissor SPDX) + `delonix-oci` (anexo). **Compl.:** M. **Dep.:** —.

### GAP-0010 — Build cloud-native: cache distribuído + multi-plataforma (área 5)
- **Current:** build local; sem cache exportável nem imagens multi-arch nativas.
- **Expected (CNCF):** BuildKit: `--cache-to/--cache-from` (registry), `--platform` multi-arch, frontends.
- **Target 100%:** `delonix build --platform linux/amd64,linux/arm64` produz índice multi-arch; `--cache-to/--cache-from type=registry` round-trip.
- **Component:** `crates/adapters/delonix-oci/src/build.rs` + `registry.rs`. **Compl.:** XL. **Dep.:** —. (Frontends LLB ficam fora de âmbito — decisão arquitectural.)

### GAP-0011 — Convergência para a Compose Specification (área 6)
- **Current:** compose próprio com lacunas (DEF-003, ver GAP-0005).
- **Expected (CNCF):** aderir à Compose Specification (`compose-spec/compose-spec`).
- **Target 100%:** suite de conformância contra a spec; directivas comuns verdes; divergências documentadas.
- **Component:** `delonix compose`. **Compl.:** M. **Dep.:** GAP-0005.

### GAP-0012 — Conformância OCI/distribution/CRI comprovada + posição de runtime (áreas 1+2)
- **Current:** runtime próprio runc-compat; CRI parcial; sem passagem publicada das suites OCI.
- **Expected (CNCF):** conformância demonstrável (containerd passa CRI 100%; suites OCI image/distribution/runtime).
- **Target 100%:** CRI ≥95% em rootful-VM (fecha com BUG-0003/04/05/06 + GAP-0001/0003); correr e publicar **oci image-spec/distribution-spec/runtime-spec conformance**; documentar a decisão «runtime próprio vs adoptar containerd» num ADR.
- **Component:** cross (`delonix-cri`, `delonix-oci`, `delonix-linux`). **Compl.:** XL. **Dep.:** BUGs CRI.

### GAP-0013 — Scanner maduro em CI: policy + compare + gating (área 8)
- **Current:** `delonix-scanner` faz SBOM+CVE (OSV), sem policy/compare/CI.
- **Expected (CNCF):** Scout: policy engine, `compare` entre imagens, `--exit-code --only-severity` para gating.
- **Target 100%:** `delonix scan <ref> --exit-code --only-severity critical,high`; `delonix scan compare A B`; política declarável (kind).
- **Component:** `delonix-scanner` + CLI `scan`. **Compl.:** M. **Dep.:** —.

### GAP-0014 — Governança aberta do projecto (área 7)
- **Current:** repo privado, org própria.
- **Expected (CNCF):** governança aberta (Moby), SECURITY.md, processo de contribuição, licença aberta.
- **Target 100%:** decisão do dono — tornar público com governança documentada, SECURITY.md, CODEOWNERS, política de release. **Não é só código.**
- **Component:** repo/projecto. **Compl.:** M. **Dep.:** decisão estratégica.
