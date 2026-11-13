# CNCF Sandbox — Fase A readiness (construir a comunidade antes de candidatar)

> **O que esta ficha é.** O checklist accionável para levar o Delonix de
> «projecto open-source de uma pessoa» a «candidato credível ao CNCF Sandbox»,
> SEM candidatar prematuramente (uma candidatura solo sem tracção leva *defer* e
> queima a primeira impressão no TOC).
>
> **O que NÃO é.** A decisão de ceder a marca/domínio «Delonix» à Linux
> Foundation (Fase B) — essa é uma decisão de negócio do dono, não um item de
> checklist. Ver [GOVERNANCE.md](GOVERNANCE.md) e a secção «A decisão» no fim.

## Onde já estamos (não refazer)

| Requisito Sandbox | Estado | Prova |
|---|---|---|
| Repositório público | ✅ | `github.com/ngolacloud/delonix` (PUBLIC) |
| Licença OSI | ✅ | Apache-2.0 ([LICENSE](LICENSE)) |
| Código de Conduta | ◑ | Contributor Covenant 2.1 — falta linkar o CNCF CoC (item A4) |
| Governança documentada | ✅ | [GOVERNANCE.md](GOVERNANCE.md) (BDFL honesto + caminho multi-maintainer + escape-valve) |
| Supply-chain de releases | ◑ | o motor JÁ emite SBOM SPDX + proveniência SLSA + attestations OCI (campanha 2026-11) — falta *aplicá-lo às releases* (A1) |

O baseline administrativo está quase todo feito. A Fase A é sobre **sinais de
comunidade e vendor-neutrality**, que é o que o TOC pesa de facto.

---

## Checklist Fase A

Cada item: **o quê · porquê (critério CNCF) · como · feito quando**.

### A1 — Releases públicas, regulares, assinadas, com SBOM + proveniência
- **Porquê:** o TOC olha para «o projecto faz releases?» e para a maturidade de
  supply-chain. É o diferencial mais barato que temos — o motor já produz tudo.
- **Como:**
  1. Cadência: uma release menor a cada ~4–6 semanas, tag `vX.Y.Z` em `main`.
  2. Cada release assinada (`cosign` ou o `image sign` nativo) + **SBOM e
     proveniência anexados** usando o que a campanha entregou:
     `delonix build --provenance=max`, `delonix image sbom <ref> --attach`,
     `delonix image provenance <ref> --attach`.
  3. `scripts/release_verify.py` no pipeline (assinatura + SBOM + attestations).
  4. Notas de release geradas (CHANGELOG mantido).
- **Feito quando:** ≥2 releases públicas assinadas com attestations verificáveis
  por um terceiro (`cosign verify-attestation` / `oras discover`).

### A2 — DCO (Developer Certificate of Origin) em todos os PRs
- **Porquê:** a CNCF exige proveniência de contribuição (DCO ou CLA); DCO é o
  mais leve e o preferido.
- **Como:** adicionar o GitHub App «DCO» ao repo; exigir `Signed-off-by:` em cada
  commit; documentar em [CONTRIBUTING.md](CONTRIBUTING.md); `git commit -s`.
- **Feito quando:** o check DCO é obrigatório e passa em todos os PRs novos.

### A3 — `ADOPTERS.md` (sinal de uso real)
- **Porquê:** o TOC pergunta «quem usa isto?». Zero adopters é o ponto mais fraco
  de uma candidatura Sandbox.
- **Como:** criar `ADOPTERS.md` listando usos reais — a própria NgolaCloud
  (control-plane, workspace, mail) **conta** como adopter; idealmente ≥1 externo.
  Formato: organização · caso de uso · desde quando · contacto público.
- **Feito quando:** ≥2 adopters listados, ≥1 fora da NgolaCloud.

### A4 — Adoptar explicitamente o CNCF Code of Conduct
- **Porquê:** a CNCF requer o seu CoC (ou um compatível que o linke).
- **Como:** no [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), manter o Contributor
  Covenant mas **acrescentar** a referência ao CNCF CoC
  (`https://github.com/cncf/foundation/blob/main/code-of-conduct.md`) e o email
  de reporte de conduta.
- **Feito quando:** o CoC linka o da CNCF e nomeia um canal de reporte.

### A5 — Roadmap público
- **Porquê:** o TOC quer ver direcção e que o projecto não é um despejo de código.
- **Como:** `ROADMAP.md` (ou GitHub Projects público) com os marcos — reusar o
  mapa das **8 áreas de valor CNCF** já medido
  (`reports/cncf-acceptance-2026-11-12/report.md`): supply-chain ✅, interop k8s
  ✅, falta build cloud-native (área 5), linhagem de runtime (área 1).
- **Feito quando:** `ROADMAP.md` público, datado, ligado ao README.

### A6 — Um segundo maintainer de outra organização ⭐ (o sinal mais forte)
- **Porquê:** é a prova nº 1 de que o projecto **não é cativo de um fornecedor**.
  Um projecto de 1 maintainer/1 org é o motivo mais comum de *defer* no Sandbox.
- **Como:** o [GOVERNANCE.md](GOVERNANCE.md) já descreve como adicionar um
  maintainer (proposto em aberto, janela de objecção). Angariar alguém com track
  record real (reviews/PRs), idealmente de fora da NgolaCloud; adicionar a
  `MAINTAINERS.md` + `.github/CODEOWNERS` para a área que passa a possuir.
- **Feito quando:** ≥2 maintainers em `MAINTAINERS.md`, ≥1 de outra org, com
  ownership real (não cosmético).

### A7 — Contribuidores e canais de comunidade abertos
- **Porquê:** sinal de actividade e de que há *onde* a comunidade acontece.
- **Como:** GitHub Issues + Discussions ligados; `good-first-issue`/`help-wanted`
  etiquetados; um canal público (Slack CNCF `#delonix` quando aceite, ou um
  Matrix/Discord entretanto); SLA informal de resposta documentado em
  CONTRIBUTING.
- **Feito quando:** ≥3 PRs/issues de contribuidores externos fechados; canal
  público ligado no README.

### A8 — Security disclosure madura
- **Porquê:** critério de maturidade; já temos [SECURITY.md](SECURITY.md).
- **Como:** confirmar email/processo de disclosure privado, janela de resposta, e
  (quando houver 2º maintainer) uma *security list* com ≥2 pessoas.
- **Feito quando:** SECURITY.md nomeia processo + ≥2 contactos.

---

## Scorecard de prontidão

| Item | Peso TOC | Estado |
|---|---|---|
| A1 releases assinadas + SBOM/provenance | alto | ◑ motor pronto; falta pipeline |
| A2 DCO | médio | ☐ |
| A3 ADOPTERS.md | **alto** | ☐ |
| A4 CNCF CoC | baixo | ◑ |
| A5 ROADMAP público | médio | ☐ (medição já existe, falta publicar) |
| A6 2º maintainer de outra org | **crítico** | ☐ |
| A7 contribuidores/canais | alto | ☐ |
| A8 security disclosure | baixo | ◑ |

**Prontos para candidatar quando:** A1, A3, A5, A6, A7 fechados (os de peso
alto/crítico). A4/A8 são rápidos; A2 é mecânico.

---

## A decisão (Fase B — não é um item de checklist)

Mesmo com a Fase A toda verde, **a admissão exige transferir a marca «Delonix»,
o nome e os domínios do projecto para a Linux Foundation** (regra CNCF desde
2021). Isso é ceder o controlo exclusivo:

- **Queres o Delonix como produto da NgolaCloud** (controlo, monetização directa
  sob a marca) → fica um projecto open-source teu, excelente, **não-CNCF**. É uma
  escolha legítima; a Fase A fortalece-o na mesma.
- **Queres o selo CNCF** (adopção, neutralidade, ecossistema) → doas o IP e abres
  a governança. O teu GOVERNANCE.md já antecipa isto.

A Fase A **não compromete** nenhuma das duas — torna o projecto melhor em ambos
os caminhos. A decisão da Fase B pode esperar até a Fase A estar feita.

## Diferenciação (o «porquê CNCF», que a candidatura vai exigir)

Guardado aqui porque o TOC pergunta sempre «porquê um runtime novo?»:
container **e** microVM num só binário rootless/daemonless (ao contrário de
Kata = containerd+QEMU), kernel-native em Rust fora da cadeia containerd, e o
ângulo de **cloud soberana** para contextos de baixa conectividade/soberania de
dados. É um diferencial de *missão* além do técnico — algo que o CNCF valoriza.
