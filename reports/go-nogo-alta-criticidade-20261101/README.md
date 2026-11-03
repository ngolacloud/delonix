# GO/NO-GO — Delonix vs Docker/Podman em produção de ALTA CRITICIDADE

> **Data:** 2026-11-01. **Âmbito:** decidir se o Delonix é GO ou NO-GO como
> substituto do Docker/Podman em ambientes de **alta criticidade computacional**
> onde os dois já são opções provadas. **Não** avalia uso em dev/CI/edge (ver §5).
>
> **Método.** Síntese de quatro varreduras à evidência do próprio repositório
> (segurança/auditorias, matriz de capacidades, defeitos/estabilização, maturidade
> operacional) cruzada com verificação pontual no código. **Aviso de método, e
> pesa no veredicto:** quase toda a evidência do repo é **auto-produzida** (revisão
> adversarial por agente, nunca pentest externo) e **datada** (medida em linhas
> v0.x–v3.1.0/5.1.1). Um comprador de alta criticidade exigiria medição
> independente — que é precisamente o item 1 do portão (§6).
>
> **Fontes:** `docs/COMPARACAO-DOCKER-PODMAN.md`, `docs/comparacao-medida.md`,
> `docs/AUDITORIA-E2E.md`, `docs/cri-conformance.md`,
> `docs/providers/capability-matrix.md`, `reports/capability-ceiling-20261027.md`,
> `reports/production-readiness/v3.1.0/*`, `reports/release-campaign-*`,
> `docs/RELATORIO-PRE-PRODUCAO.md`, `VERSIONING.md`, `MAINTAINERS.md`,
> `GOVERNANCE.md`, + verificação directa em `crates/adapters/delonix-oci/src/overlay.rs`.

## Veredicto

**NO-GO** como substituto do Docker/Podman em alta criticidade, **hoje**. GO
**condicional** para nichos single-node sem failover (§5). O portão para virar GO
está em §6 e no [plano](plano-no-go-para-go.md).

A barra de "alta criticidade onde Docker/Podman já são opções" é: fiabilidade
provada, HA/failover, garantia de segurança **externa**, suportabilidade e upgrade
seguro. O Delonix fica atrás do incumbente battle-tested em **todas**.

## Matriz de decisão

| Dimensão | Veredicto | Evidência-chave |
|---|---|---|
| **Segurança** | ⚠️ Honesta, **não assegurada externamente** | Engenharia fail-closed; fugas fail-open fechadas (asseveradas no CHANGELOG, não re-auditadas). Todas as auditorias são **AI-adversarial internas, nunca pentest externo**. Cadeia de fornecimento fraca: **G-020** 0/20 Actions SHA-pinned; sem `cargo-audit`/trivy no CI; scanner CVE **sem feed real** (`delonix-scanner/src/lib.rs:371`); assinatura/SLSA/SBOM "por desenho", não verificados contra artefactos (`supply-chain.md`). Whiteout OCI (achado #1) **verificado CORRIGIDO no código** (`overlay.rs:185-221`, `safe_rel` + canonicalização confinada) — o `AUDITORIA-E2E.md` é que está desactualizado. |
| **Fiabilidade** | ❌ **LAB/PILOT READY** | Veredicto formal auto-atribuído LAB READY; single-node rootless CLI = PILOT (`release-verdict.md`). **9 P1 abertos**; **sem release publicada** (`CHANGELOG` 0.x "[Não lançado]"); padrão **fail-open rc=0** histórico (anti-spoof/secrets/reaper — vários já corrigidos); `compose up` não reinicia serviço parado (DEF-003); `stack apply` diz "unchanged" com `command` alterado (DEF-002); CRI **84/104** (≈13/20 falhas são ambiente/rootless); migração ao vivo **NO-GO** (ADR-0031); densidade/escala **não medida**. |
| **Capacidades** | ⚠️ 181/271 (66,8%), tecto 247 — re-medido no commit atual 2026-11-02 (era 180/270@5.1.1: `net.ipv6` passou a `supported`, ADR-0082) | ≥ Docker/Podman em ciclo-de-vida single-node/pods/CRI/rede-rootless; **único** em container+microVM+CRI num motor. Falta: **orquestração multi-nó + HA** (VM HA/replicação = `requires-external-component`; overlay inter-nó não provado E2E), compose completo (`profiles`/`extends`/multi-ficheiro), log drivers, BuildKit (`ssh`/`cache`/multi-arch push), **GPU/CDI por provar em HW real**, `--format` Go-template. |
| **Operacional** | ❌ Imaturo | Single-node por desenho (frota → `delonix-paas`, ADR-0010); **sem HA, sem Helm/Terraform/Ansible, sem SLA**; **1 mantenedor** (BDFL; ~2735 commits de uma pessoa, `MAINTAINERS.md`/`GOVERNANCE.md`); pré-1.0 (minors podem partir CLI/API/schema, `VERSIONING.md`); **upgrade/rollback NÃO TESTADO** (`release-verdict.md`), sem hot-patch de supervisores vivos (`RELATORIO-PRE-PRODUCAO.md:116`); observabilidade forte-no-desenho mas limitada (daemonless; tracing do motor não ligado; `/metrics` do mgmt congelado por ADR-0041). |

## Desqualificadores (cada um basta sozinho)

1. **Upgrade/rollback não testado** + impossível corrigir supervisores em execução sem recriar containers.
2. **Sem HA / multi-nó não provado E2E** — se o ambiente exige failover, não existe.
3. **Sem garantia de segurança independente** — nenhum pentest externo; cadeia de fornecimento não-pinada/não-verificada (o pipeline que produz releases "de confiança" é o elo mais fraco).
4. **Pré-release** (0.1.0 não lançada), **1 mantenedor**, **sem SLA**, breaking-changes permitidos em 0.x.
5. **Escala/densidade não medida** — o caminho de attach faz dumps `nft` que crescem com o nº de containers; tudo medido em nó vazio.

## A favor — genuíno e medido

Não é "Docker com menos features". Diferenciais **reais e medidos**: reconfiguração
a quente sem mudar PID; daemonless ~2,3–3,5× menos latência no default (91 vs
208/268 ms); **rede rootless governada** (incl. IPv6 dual-stack por omissão com
política v6 por-workload — único dos três, ver `docs/comparacao-medida.md` §IPv6);
container+microVM+CRI num só motor (kubelet real, `kindest/node` a `Ready`);
segurança mais estrita por omissão (NNP sempre + check fail-closed); tectos de
recursos obrigatórios (Docker/Podman deixam memória sem tecto).

## §5 — Onde SERIA GO (condicional)

Single-node, **sem requisito de failover**, imagens confiáveis, operador que aceita
pré-1.0 e auto-suporte: **dev/CI, edge single-node, labs rootless-first,
nó-k8s-sem-Docker**. Aí é PILOT credível e por vezes superior ao incumbente.

## §6 — Portão para virar GO em alta criticidade

Os 6 pontos cujo fecho vira o veredicto. Plano detalhado em
[plano-no-go-para-go.md](plano-no-go-para-go.md).

1. **Segurança externa** — pentest externo da linha atual + SHA-pin dos Actions + feed CVE real + avaliar rootful/SELinux.
2. **Release validada** — tag com CI verde + e2e/chaos/critest corridos NA tag (não o processo da v5.1.1).
3. **Upgrade/rollback** — testados E2E + história de hot-patch de supervisores vivos.
4. **HA/multi-nó** — provado E2E, OU failover excluído de âmbito e tecto single-node documentado.
5. **Escala/densidade** — medida até um tecto declarado com SLOs.
6. **Evidência actual** — re-medir o stale na versão que efetivamente envia.
