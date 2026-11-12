# Plano de resolução dos gaps CNCF — Delonix

> Deriva de `CNCF_ACCEPTANCE_MASTER.md` (2026-11-12). Ordenação por **retorno
> para a admissão CNCF**, não por área numérica. Cada onda declara dependências,
> esforço, reutilização de código existente e critério de «feito» (efeito
> verificável, não «compila»). GAP IDs alinhados com `BACKLOG.md`.

## Princípio de ordenação

1. **Admissão primeiro, qualidade técnica depois.** A área 7 (governança) é o
   único bloqueador *de entrada*; corre em paralelo a tudo porque é decisão do
   dono, não código.
2. **Maior ROI de código = supply-chain** (3+4): um único primitivo (anexo
   in-toto/OCI) destranca quase duas áreas reutilizando `sbom_spdx`+`sign.rs`+
   `registry.rs`.
3. **Barato antes de caro.** Área 8 (policy sobre OSV já existente) e área 2
   (kube interop) antes da área 5 (build multi-arch, XL).
4. **Não degradar invariantes** (rootless-first, fail-closed, contratos
   CLI/JSON/schema) — cada onda testa comportamento + nota de migração.

## Onda 0 — Governança (área 7) — DESTRANCA A ADMISSÃO · paralela

Decisão do dono, não engenharia de motor. Sem isto, nenhuma onda abaixo muda o
veredicto de entrada.

| Item | Acção | Done |
|---|---|---|
| GAP-0014a | Tornar o repo **público** (Apache-2.0 limpo, headers, DCO/CLA) | repo público + LICENSE/NOTICE auditados |
| GAP-0014b | Governança neutra documentada (`GOVERNANCE.md`+`MAINTAINERS.md` com papéis reais, processo de decisão) | docs com ≥2 maintainers e processo |
| GAP-0014c | Releases assinadas + reproduzíveis (`release_verify.py`, cosign/PGP, SBOM anexado) | release v0.1.x assinada e verificável |
| GAP-0014d | Sinais de comunidade (security policy viva, roadmap público, CoC enforcement) | SECURITY.md + roadmap públicos |

> **Gate de admissão:** até a Onda 0 fechar, o estado continua **NO-GO** para
> Sandbox, independentemente do resto.

## Onda 1 — Âncora supply-chain (áreas 3+4) — MAIOR ROI DE CÓDIGO

### 1A · GAP-0009 — anexo in-toto/OCI de attestations · **Compl. M** · dep. —
O pivô. Entrega a área 4 quase inteira e metade da 3 de uma vez.
- **Reutiliza:** `sbom_spdx()` (SPDX pronto), `sign.rs` (chave+assinatura),
  `registry.rs::push_signature` (push de artefacto OCI no mesmo repo).
- **Implementar:** embrulhar o SPDX num **in-toto Statement cru** (`_type:
  https://in-toto.io/Statement/v1`, `predicateType: https://spdx.dev/Document`);
  publicar manifest `artifactType=application/vnd.docker.attestation.manifest.v1+json`
  com `subject=<imagem>`, layer `application/vnd.in-toto+json` + annotation
  `in-toto.io/predicate-type`; referenciar no index com `platform=unknown/unknown`
  + annotations `vnd.docker.reference.type=attestation-manifest`/`.digest`.
- **CLI:** `delonix build --sbom`; `delonix image sbom <ref>` exporta SPDX.
- **Done:** `cosign verify-attestation` / `docker buildx imagetools inspect` lê a
  attestation anexada; round-trip `push→pull→verify` contra registry local.

### 1B · GAP-0008 — provenance SLSA no build · **Compl. L** · dep. 1A
- **Reutiliza:** o anexo da 1A; `build.rs` já conhece base/materials/steps.
- **Implementar:** instrumentar o build para emitir predicate
  `https://slsa.dev/provenance/v1` (`buildType`, `buildDefinition{externalParameters,
  resolvedDependencies com checksum}`, `runDetails{builder.id, metadata}`); flag
  `--provenance mode=min|max` (default max, como BuildKit). `byproducts` pode
  ficar vazio (o próprio BuildKit não o preenche).
- **Done:** imagem construída traz provenance v1 verificável; `mode=min` omite
  valores de build-args/secrets, `mode=max` inclui Dockerfile+sourcemaps.

## Onda 2 — Scanning maduro (área 8) · **Compl. M** · dep. — (1A ajuda GAP-0017)

- **GAP-0013** — `policy` declarativa + `scan compare` + gating CI. Alavanca o
  OSV+`--fail-on` já existentes; adicionar ficheiro de policy (severidade por
  pacote/CVE allowlist) e `delonix scan compare <imgA> <imgB>` (delta de CVEs).
- **GAP-0017** (lente Podman) — trust de assinatura por policy: estender
  `delonix-security/policy.rs` com regras por registo/repo (sigstore+chave),
  aplicadas no `pull`/`run`. Reusa o formato de attestation da Onda 1.
- **Done:** `delonix scan --policy p.json` falha o CI conforme regras; `run`
  recusa imagem não-assinada sob policy.

## Onda 3 — Interop Kubernetes (completa a área 2) · **Compl. L** · dep. —

- **GAP-0015** — `delonix kube play <k8s.yaml>` corre Pod/Deployment em
  `apiVersion: v1`/`apps/v1`; `delonix kube generate <ref>` emite YAML k8s válido.
  Maior diferencial CNCF do Podman (k8s é o centro do ecossistema). Conversor
  k8s↔envelope em `delonix-compute`.
- **Done:** round-trip `generate → play` de um pod de 2 membros; YAML aceite por
  `kubectl --dry-run=client`.

> A conformância CRI do motor (área 2) **já está** em 103/103 — esta onda fecha a
> dimensão de *interop*, não a de conformância.

## Onda 4 — Convergência Compose (área 6) · **Compl. M–L** · dep. —

- **Interpolação `${VAR}`/`.env`** (o maior gap, normativo) — passe de
  substituição antes do parse: `$V`, `${V}`, `${V:-def}`, `${V-def}`, `${V:?err}`,
  `${V?err}`, nested, escape `$$`; auto-load de `./.env`. Novo módulo antes de
  `serde_yaml`.
- **GAP-0005** — `compose up` reinicia serviço parado (semântica de ciclo de vida).
- **GAP-0011** — implementar a cauda longa de ~35 atributos por procura
  (prioridade: `pull_policy`, `cgroup_parent`, `pids_limit`, `oom_*`, `gpus`,
  `blkio_config`); `include` top-level.
- **Reframe do alvo (M-DOC):** o critério de «100%» da área 6 passa a **validar
  contra `compose-spec.json` + paridade `compose-go`** (não há suite). Corrigir
  `BACKLOG.md`.
- **Done:** ficheiros Docker Compose reais com `${VAR}` sobem sem edição; `up`
  reinicia parados; schema-validation verde.

## Onda 5 — Build cloud-native (área 5) · **Compl. L–XL** · dep. —

Menor peso de *admissão* (é tier Incubating, não Sandbox); fazer depois.

- **GAP-0010a** — cache distribuído `--cache-to/--cache-from type=registry` +
  `mode=min|max`. Reusa `registry.rs` (push/pull de blobs + mount cross-repo U6);
  falta a tag de cache + serialização dos modos. **Compl. L.**
- **GAP-0010b** — `--platform a,b` → **emitir `ImageIndex`**. `registry.rs` já
  *descodifica* index; falta a direcção de *escrita* (montar index de N builds
  por arch). **Compl. XL.**
- **GAP-0016** — `delonix generate systemd/Quadlet` p/ workloads do utilizador
  (reusa `serve.rs`). **Compl. M.**
- **Done:** `--cache-to/from` round-trip entre dois builds; índice multi-arch
  `imagetools inspect` mostra amd64+arm64; unit systemd funcional.

## Onda 6 — Linhagem de runtime (área 1) · **Compl. XL** · dep. BUGs CRI (fechados)

- **GAP-0012** — ADR de linhagem (posicionar o runtime Rust face a containerd/runc;
  justificar o caminho próprio) + correr suites OCI image/runtime/distribution
  para prova documentada. A conformância CRI (103/103) já é insumo.
- **Done:** ADR publicada + relatório de suites OCI verdes anexado.

## Mapa de ondas → áreas → veredicto

| Onda | Áreas | Esforço | Move o veredicto de admissão? |
|---|---|---|---|
| 0 Governança | 7 | decisão | **SIM — destranca o NO-GO** |
| 1 Supply-chain | 3+4 | M→L | sinal técnico forte p/ Sandbox |
| 2 Scanning | 8 | M | reforça supply-chain |
| 3 Kube interop | 2 | L | k8s = centro CNCF |
| 4 Compose | 6 | M–L | correcção visível |
| 5 Build | 5 | L–XL | tier Incubating |
| 6 Linhagem | 1 | XL | narrativa/prova |

**Sequência recomendada:** Onda 0 em paralelo (dono) **+** Ondas 1→2→3 (código,
maior retorno) → 4 → 5 → 6. A admissão depende da 0; a *competitividade* técnica
sobe monotonicamente com 1→6.

## Como executar (harness)

Cada GAP vira um prompt de sessão conduzido pela skill `delonix-backlog-driver`
(escolhe o próximo item por dependência→tier→peso CNCF→WIP cap) e validado pela
skill `delonix-runtime-e2e-quality` antes de fechar. Um worktree por onda a
partir de `develop`; contratos em `.claude/sessions/`. As Ondas 1–2 (supply-chain)
tocam `delonix-oci`+`delonix-scanner`+`delonix-security` — declarar ownership
desses crates para evitar edições concorrentes.
