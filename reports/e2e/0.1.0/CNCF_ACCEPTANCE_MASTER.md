# Delonix × Docker × Podman — Ficha mestra de aceitação CNCF

> Consolidação das medições de 2026-11-12 (develop @ `67ddef41`). Cruza o código
> do Delonix com o código oficial do BuildKit/buildah/compose-spec (via `gh`,
> literais confirmados em `file:path:linha`). Supersede os números de
> `CNCF_GAP_DOCKER.md`/`CNCF_GAP_PODMAN.md`/`BACKLOG.md` (2026-11-08) onde esta
> ronda os corrigiu. Doutrina: só o *efeito/contrato verificado* conta;
> implementado ≠ testado ≠ proposto.

## 0. Procedência

| Área | Fonte Delonix | Fonte de referência |
|---|---|---|
| 2 CRI | `reports/cri-conformance-0.1.1/03-full-suite-103.md` | `critest` v1.36.0 |
| 3 SLSA | `delonix-oci/build.rs`, `sign.rs` | `moby/buildkit` `solver/llbsolver/provenance/types/types.go`, `frontend/attestations/parse.go` |
| 4 SBOM | `delonix-scanner/lib.rs` (`sbom_spdx`) | `moby/buildkit` `exporter/containerimage/writer.go`, `frontend/attestations/sbom/` |
| 5 Build | `delonix-oci/build.rs`, `registry.rs`, `bins/delonix/src/cmd/build.rs` | `moby/buildkit` `cache/remotecache/*`, `exporter/containerimage/writer.go`; `containers/buildah` `pkg/cli/common.go`, `cmd/buildah/manifest.go` |
| 6 Compose | `bins/delonix/src/cmd/compose.rs` | `compose-spec/compose-go/schema/compose-spec.json`, `compose-spec/compose-spec/12-interpolation.md` |
| 8 Scan | `delonix-scanner/lib.rs` | `docker/scout-cli`, `containers/buildah` `--sbom` |

## 1. Scorecard das 8 áreas de valor CNCF (estado de hoje)

Legenda: ✗ ausente · ◑ parcial · ✔ feito. **Δ** = correcção vs backlog 2026-11-08.

| # | Área | Docker | Podman | **Delonix (medido)** | Est. | Δ |
|---|---|---|---|---|---|---|
| 1 | Linhagem de runtime | containerd+runc | crun+conmon | runtime Rust runc-compat, fora da cadeia containerd | ✗ | — |
| 2 | Conformância OCI/CRI | 100% | via containerd/cri-o | **CRI rootful-VM 103/103 limpo** · rootless 84/19 · interop k8s YAML em falta | ◑→forte | **↑** era 90/13 |
| 3 | Provenance SLSA | BuildKit v1/v0.2 | — | predicate não emitido; `sign.rs` cosign existe | ✗ | — |
| 4 | SBOM interoperável | BuildKit SPDX+in-toto | buildah CycloneDX/SPDX | **SPDX-2.3 já emitido**; falta wrapper in-toto + anexo OCI | ◑ forte | **↑** backlog subestimava |
| 5 | Build cloud-native | cache 6-backend, multi-arch, LLB | --manifest, --cache-to | cache local + cache-mount; `--platform` single; SBOM auto | ✗ | — |
| 6 | Compose Specification | Compose Spec | kube play | allowlist fail-closed, superfície comum; **sem interpolação `${VAR}`** | ◑ | reframe do alvo |
| 7 | Governança aberta | Moby aberta | LFX+PGP+LTS | repo privado, org própria | ✗ | — |
| 8 | Scanning maduro CI | Scout policy/compare | delega | OSV+staleness+`--fail-on`; sem policy/compare | ◑ | — |

**Já alinhado (não refazer):** rootless-first, daemonless, pods nativos,
`policy.json` runtime, cosign/sigstore (`sign.rs`), OSV (formato CNCF),
cache-mount, SBOM automático por build.

## 2. Medições-chave (com evidência)

### 2.1 Área 2 — CRI 103/103 (salto material)
`reports/cri-conformance-0.1.1/03-full-suite-103.md`: VM golden rootful+CNI,
`critest` v1.36.0 → **103 Passed | 0 Failed | 19 Skipped** (corrida única, sem
retries). Fecha `rshared` (ADR-0093), OOM (barreira cgroup), AppArmor,
HostNetwork/HostIpc, shared-PID. `rshared` atrás de privilégio (recusado em
rootless com `DX-1117`). **Falta da área 2:** interop k8s YAML (`kube play/generate`,
GAP-0015) — distinta da conformância CRI.

### 2.2 Áreas 3+4 — supply-chain: o anexo in-toto/OCI é o pivô
- **Delonix já tem:** SPDX-2.3 (`sbom_spdx`, `delonix-scanner/lib.rs:605`),
  assinatura cosign (`sign.rs`), push de artefacto OCI no mesmo repo
  (`registry.rs::push_signature`), leitura/resolução de digests.
- **Falta (partilhado por 3 e 4):** embrulhar em in-toto Statement **cru** (não
  DSSE) e anexar à imagem via o mecanismo do BuildKit:
  - layer `application/vnd.in-toto+json` + annotation `in-toto.io/predicate-type`
    (`writer.go:633,639`);
  - manifest `artifactType=application/vnd.docker.attestation.manifest.v1+json`,
    `subject=<imagem>` (`writer.go:51,675`);
  - descriptor no index `platform=unknown/unknown` + annotations
    `vnd.docker.reference.type=attestation-manifest` /
    `vnd.docker.reference.digest=<digest>` (`util/attestation/types.go:4-7`).
- **Falta (só área 3):** gerar o predicate SLSA durante o build — `predicateType
  https://slsa.dev/provenance/v1` (default), `buildType`, `buildDefinition`/
  `runDetails`, flag `--provenance mode=min|max` (default max). O `build.rs`
  constrói mas não instrumenta materials/invocation.

### 2.3 Área 5 — build cloud-native (a mais fraca)
- **Cache:** só local (`build.rs:623`) + cache-mount (`build.rs:24-47`). **Sem**
  `--cache-to/--cache-from` (BuildKit tem 6 backends:
  `registry·local·inline·gha·s3·azblob`, `buildkitd/main.go:943-956`, `mode=min|max`;
  buildah tem cache por registry). Delonix abaixo até do buildah aqui.
- **Multi-arch:** `--platform linux/<arch>` **único** (`build.rs:213-228`), cross
  via binfmt/qemu do host (`build.rs:1122`). **Não** aceita lista nem **emite
  `ImageIndex`** (BuildKit monta-o automático, `writer.go:246-313`; `registry.rs`
  só *lê* index). 
- **LLB frontends:** fora de âmbito (decisão arquitectural — parser Dockerfile próprio).
- **Host/ops:** falta `generate systemd/Quadlet` p/ workloads (GAP-0016).

### 2.4 Área 6 — Compose (reframe do alvo)
- **Não existe suite de conformância Compose** (≠ `critest`). Conformidade =
  **JSON Schema `compose-go/schema/compose-spec.json`** + paridade com o parser
  `compose-go`. O alvo «suite verde» do backlog é inválido.
- Delonix: allowlist **fail-closed** (recusa com razão, nunca ignora), 94 chaves
  rastreadas (`SPEC_SERVICE_KEYS`), 3 condições `depends_on`, `extends`,
  `profiles`, `x-`, secrets/configs, deploy/resources, `version` obsoleto tratado.
- **Gaps:** interpolação `${VAR}`/`.env` **ausente** (normativa, `12-interpolation.md`)
  — o maior; `include` recusado (workaround `-f`); cauda longa de ~35 atributos
  recusada; `up` não reinicia serviço parado (GAP-0005/DEF-003); `develop.watch`/
  `models`/`jobs` ausentes (opcional/nicho).

### 2.5 Área 8 — scanning
OSV (array+ZIP, `lib.rs:895-1015`), `db_is_stale` (diferencial a favor),
`--fail-on` por severidade (gate CI primitivo). **Falta:** `policy` declarativa,
`compare` entre imagens, trust-por-policy (GAP-0017, lente Podman).

## 3. A pergunta de admissão CNCF

A tabela técnica **não** é o critério de entrada. O nível **Sandbox** avalia
sobretudo o não-técnico:

| Critério Sandbox | Delonix |
|---|---|
| Licença OSI (Apache-2.0) | ✔ LICENSE presente |
| **Repositório público** | **✗ privado** |
| Governança neutra / IP sem dono único | ◑ ficheiros existem; org e controlo do dono |
| Código de conduta | ✔ |
| Adopção/comunidade (sem exigir escala) | ✗ |
| Diferenciação cloud-native | ✔ runtime Rust rootless/daemonless + microVMs |

**Veredicto:** tecnicamente o Delonix já é competitivo para Sandbox numa
dimensão forte (CRI 103/103 — prova que muitos candidatos não têm) e tem
meio-caminho em supply-chain. **O bloqueador de admissão é a área 7
(governança): enquanto o repo for privado e a org for de dono único, a entrada
é NO-GO independentemente da qualidade do motor.** A engenharia não é o gargalo;
a decisão de abertura é.
