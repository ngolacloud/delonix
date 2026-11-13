# Delonix — aceitação CNCF: medição + plano (report)

> **Campanha:** aferir se a CNCF aceitaria a entrada do Delonix, comparando com
> Docker e Podman, e desenhar o plano de fecho dos gaps.
> **Data:** 2026-11-12 · **Base:** develop @ `67ddef41`.
> **Método:** código do Delonix cruzado com o código oficial de
> BuildKit/buildah/compose-spec (via `gh`, literais em `file:path:linha`).
> Doutrina: só o contrato/efeito verificado conta.
>
> **Detalhe:** `CNCF_ACCEPTANCE_MASTER.md` (scorecard+evidência) e
> `CNCF_RESOLUTION_PLAN.md` (plano por ondas), em `reports/e2e/0.1.0/`.

## 1. Veredicto

- **Tecnicamente**: competitivo para **CNCF Sandbox** numa dimensão forte
  (conformância CRI **103/103 limpo**, rootful+CNI) e a meio-caminho em
  supply-chain (cosign + OSV + SPDX já existentes).
- **Admissão**: **NO-GO** até fechar a **governança (área 7)** — repo privado e
  org de dono único. Os critérios Sandbox são sobretudo não-técnicos (público,
  governança neutra, IP sem dono único); a qualidade do motor não compensa isso.
- **Conclusão**: a engenharia não é o gargalo de *entrada*; a decisão de abertura
  é. As ondas de código elevam a *competitividade* (rumo a Incubating), não
  destrancam a admissão — isso é a Onda 0.

## 2. Scorecard das 8 áreas (hoje)

Legenda: ✗ ausente · ◑ parcial · ✔ feito. **Δ** = correcção vs backlog 08-Nov.

| # | Área | Docker | Podman | **Delonix (medido)** | Est. | Δ |
|---|---|---|---|---|---|---|
| 1 | Linhagem de runtime | containerd+runc | crun+conmon | runtime Rust runc-compat, fora da cadeia containerd | ✗ | — |
| 2 | Conformância OCI/CRI | 100% | containerd/cri-o | **CRI 103/103 limpo**; interop k8s YAML em falta | ◑→forte | **↑** era 90/13 |
| 3 | Provenance SLSA | BuildKit v1/v0.2 | — | predicate não emitido; cosign existe | ✗ | — |
| 4 | SBOM interoperável | SPDX+in-toto | CycloneDX/SPDX | **SPDX-2.3 já emitido**; falta wrapper in-toto+anexo | ◑ forte | **↑** subestimado |
| 5 | Build cloud-native | cache 6-backend, multi-arch | --manifest, --cache-to | cache local+mount; `--platform` single | ✗ | — |
| 6 | Compose Spec | Compose Spec | kube play | allowlist fail-closed; **sem interpolação `${VAR}`** | ◑ | reframe do alvo |
| 7 | Governança | Moby aberta | LFX+PGP+LTS | repo privado, org própria | ✗ | — |
| 8 | Scanning CI | Scout policy/compare | delega | OSV+staleness+`--fail-on`; sem policy/compare | ◑ | — |

**Correcções materiais desta ronda:** (2) CRI subiu 90/13 → **103/103** (fecho de
`rshared`/OOM/AppArmor/HostNS/shared-PID); (4) o **SPDX-2.3 já é gerado**
(`sbom_spdx`), só falta embrulhar+anexar; (6) **não existe suite de conformância
Compose** — o alvo é validar contra o JSON Schema + paridade `compose-go`.

## 3. Critérios de admissão Sandbox

| Critério | Delonix |
|---|---|
| Licença OSI (Apache-2.0) | ✔ |
| **Repositório público** | **✗ privado** |
| Governança neutra / IP sem dono único | ◑ |
| Código de conduta | ✔ |
| Adopção/comunidade | ✗ |
| Diferenciação cloud-native | ✔ (Rust rootless/daemonless + microVMs) |

## 4. Plano de fecho (por retorno-para-admissão)

| Onda | Áreas | Esforço | Papel |
|---|---|---|---|
| **0 Governança** | 7 | decisão do dono | **destranca o NO-GO** — paralela |
| **1 Supply-chain** | 3+4 | M→L | **pivô**: GAP-0009 (anexo in-toto/OCI) → GAP-0008 (SLSA) |
| **2 Scanning** | 8 | M | policy+compare sobre OSV existente |
| **3 Kube interop** | 2 | L | `kube play/generate` (diferencial Podman) |
| **4 Compose** | 6 | M–L | interpolação `${VAR}` + GAP-0005 (up restart) |
| **5 Build** | 5 | L–XL | cache registry + índice multi-arch + systemd |
| **6 Linhagem** | 1 | XL | ADR + suites OCI |

**Sequência:** Onda 0 em paralelo (dono) + Ondas 1→2→3→4→5→6 (código). Ponto de
maior ROI: **Onda 1A (GAP-0009)** — `Compl. M`, reutiliza `sbom_spdx`+`sign.rs`+
`registry.rs`, move as áreas 3 e 4 ao mesmo tempo.

## 5. Estado de execução

| Onda | Item | Estado |
|---|---|---|
| 1A | GAP-0009 anexo in-toto/OCI | **EM ARRANQUE** (ficha accionável + worktree) |
| restantes | — | planeadas |

> Execução conduzida pela skill `delonix-backlog-driver`; validação por
> `delonix-runtime-e2e-quality` antes de fechar cada onda. Um worktree por onda a
> partir de `develop`.
