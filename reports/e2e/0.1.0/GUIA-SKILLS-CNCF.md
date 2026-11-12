# Guia — usar as skills para levar o Delonix à CNCF

Como cada skill/agente faz o seu trabalho, em que ordem se encadeiam, e o caminho
até **submeter o Delonix à CNCF (Sandbox)**. Público-alvo: quem conduz sessões de
engenharia no `delonix-runtime`.

> Verdade incómoda, dita já: **100% nas 8 áreas técnicas é necessário mas não
> suficiente** para a CNCF. A CNCF Sandbox também pesa **governança aberta,
> neutralidade de fornecedor e adopção**. Por isso o plano tem duas pistas:
> *técnica* (BUGs/GAPs) e *projecto* (GAP-0014).

## 1. Catálogo — cada skill/agente e o seu papel

### Skills de condução (user-level, `~/.claude/skills/`)
| Skill | Papel | Quando invocar |
|---|---|---|
| `delonix-runtime-engineer` | **Contrato de entrada** — fronteiras do produto, comandos verificados, regras de delegação, referências por domínio | **Sempre primeiro**, em qualquer trabalho no motor |
| `delonix-backlog-driver` | **Condutor** — escolhe o próximo item (dependências→tier→peso CNCF→WIP), dispara o prompt certo, revê alinhamento | "qual é o próximo", "implementar o backlog", "validar prompts" |
| `delonix-rust-reviewers` | Painel Rust de elite (unsafe/ABI/perf/linking/libc) | rever PRs/crates/diffs Rust |
| `escopo` | Abre o contrato da sessão (1 objectivo, ≤3 critérios, fora-de-âmbito) | ao arrancar cada item |
| `fila` | Uma fila de prompts prontos, com WIP cap | "o que está aberto", "próxima" |
| `fechar` | Fecha a sessão/consolida com registo | critérios cumpridos; "fecha isto" |

### Skills do harness (no repo, `.ai/skills/`)
| Skill | Papel |
|---|---|
| `delonix-runtime-e2e-quality` | **Produz** os backlogs (`reports/e2e/<versão>/`): só o *efeito* verificado conta |
| `delonix-bug-triage` | **Groома** findings → fila deduplicada e ordenada (P0–P4) |
| `delonix-architecture-review` | Revê mudança/desenho contra as camadas do motor (ADRs) |
| `delonix-security-review` | Revê risco de isolamento/privilégio/segredos/supply-chain/MCP |
| `delonix-release` | Leva «código pronto» → «humano publica em segurança» |

### Agentes do harness (no repo, `.ai/agents/`, por tier)
| Agente | Tier | Papel |
|---|---|---|
| `delonix-auditor` | A (read-only) | «como funciona X / onde vive Y», com evidência file:line |
| `delonix-builder-tester` | B (rootless) | build/test/gates + edições pequenas; **nunca root** |
| `delonix-gatekeeper` | B | corre e interpreta os gates (arch_fitness, folhas, contract, version, lang…) + baselines |
| `delonix-security-reviewer` | A | classifica risco em diffs de namespaces/cgroups/caps/seccomp/mounts/KVM |
| `delonix-release-manager` | C (propõe) | valida prontidão de release; **humano publica** |

Tiers (de `.ai/agents/permission-tiers.md`): **A** read-only · **B** rootless ·
**C** aprovação humana · **D** privilegiado/root (rootful-VM; nunca shell root).

## 2. O fluxo end-to-end (quem chama quem)

```
  medir            triar           enfileirar      conduzir            executar         verificar          fechar
┌───────────┐   ┌───────────┐   ┌─────────┐   ┌───────────────┐   ┌───────────┐   ┌─────────────┐   ┌─────────┐
│ e2e-      │ → │ bug-      │ → │  fila   │ → │ backlog-driver│ → │ escopo +  │ → │ teste de    │ → │ fechar  │
│ quality   │   │ triage    │   │         │   │ (ordem+revê)  │   │ agente    │   │ regressão   │   │         │
└───────────┘   └───────────┘   └─────────┘   └───────────────┘   └───────────┘   └─────────────┘   └─────────┘
 reports/e2e     BACKLOG.md       prompts        Prompt N +          worktree         CI verde,         matriz
                                                 checklist           + tier certo      não reaparece     8 áreas++
```

Regra transversal **não-negociável**: um item só fecha com **teste de regressão
automatizado** — é o que o impede de voltar ao próximo `report.md`.

## 3. Playbook — "quero X, uso Y"

| Quero… | Faço |
|---|---|
| Começar qualquer trabalho | Carregar `delonix-runtime-engineer` |
| Saber o próximo item / avançar o backlog | `delonix-backlog-driver` → dá o Prompt N e a vaga |
| Executar um item | Colar o Prompt N (abre com `/escopo`); 1 item = 1 worktree = 1 branch |
| Correr build/test/gates sem root | agente `delonix-builder-tester` (Tier B) |
| Mexer em AppArmor/host-ns/OOM/mount (root) | **rootful-VM** (Tier D), coordenar com o dono |
| Entender código antes de mexer | agente `delonix-auditor` (Tier A) |
| Perceber porque um gate está vermelho | agente `delonix-gatekeeper` |
| Rever segurança de um diff | `delonix-security-review` / agente security-reviewer |
| Rever qualidade Rust | `delonix-rust-reviewers` |
| Medir conformância/paridade de novo | `delonix-runtime-e2e-quality` |
| Preparar (não publicar) release | `delonix-release` / agente release-manager (Tier C) |
| Consolidar e fechar | `fechar` |

## 4. Caminho para a CNCF — duas pistas

### Pista técnica: 100% nas 8 áreas (ver `BACKLOG.md`)
Ordem recomendada pela `delonix-backlog-driver`:
1. **Vaga 1** — BUG-0001 + M-TEST-02 (Tier B): instalação limpa + harness honesto.
2. **Vaga 2** — BUG-0004→GAP-0001/0003, BUG-0003/05/06 (Tier D, rootful-VM):
   conformância CRI ≥95%.
3. **Vaga 3** — GAP-0009→0008→0017, GAP-0013 (Tier B): supply-chain
   (SBOM SPDX in-toto, SLSA, trust por policy, scanner CI) — **maior peso CNCF**.
4. **Vaga 4** — GAP-0015 (interop k8s), GAP-0011 (Compose Spec), GAP-0016 (systemd).
5. **Roadmap** — GAP-0012 (conformância publicada), GAP-0010 (multi-arch).

Marco técnico: a matriz das 8 áreas toda a ✔.

### Pista de projecto: prontidão CNCF Sandbox (GAP-0014)
A CNCF Sandbox exige, além do código:
- **Open-source com licença aprovada** — ✔ o repo tem `LICENSE` (Apache-2.0).
- **Governança pública** — ✔ existem `GOVERNANCE.md`, `MAINTAINERS.md`,
  `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODEOWNERS`.
- **Repo público** — ✗ hoje é privado → **tornar público**.
- **Releases assinadas + SBOM/proveniência** — liga à pista técnica (GAP-0008/09).
- **Neutralidade e adopção** — mostrar utilizadores/adoptantes e roadmap público.
- **CNCF Code of Conduct** adoptado; **DCO** nos commits.

Marco de projecto: repo público + governança + releases assinadas + 1ª submissão
ao CNCF TOC (Sandbox).

## 5. Checklist de submissão (fechar antes de propor à CNCF)
- [ ] Matriz das 8 áreas 100% ✔ (`BACKLOG.md`)
- [ ] CRI ≥95% publicado + suites OCI image/distribution/runtime (GAP-0012)
- [ ] Imagens com SBOM SPDX in-toto + SLSA provenance verificáveis (GAP-0008/09)
- [ ] `delonix kube play/generate` (interop k8s) a funcionar (GAP-0015)
- [ ] Repo **público**, Apache-2.0, DCO, CoC CNCF, governança e maintainers claros
- [ ] Releases assinadas (PGP/sigstore), CHANGELOG, política de versões
- [ ] Documentação de utilizador + página de adopção/roadmap públicos
- [ ] Decisão ADR sobre runtime próprio vs containerd (GAP-0012)

## 6. Regras de ouro (nunca quebrar)
- **Não tocar a árvore suja do dono** (committa em paralelo) — worktree próprio;
  `reports/` só por acrescento.
- **Tiers:** nunca root a um agente Tier B; itens Tier D só via rootful-VM.
- **Distinguir** implementado / testado / proposto / bloqueado — sem teste de
  regressão, não está fechado.
- **Standards interoperáveis** no supply-chain (SPDX, in-toto, SLSA, OSV, sigstore)
  — nunca formato proprietário.
- **Reconfirmar `git status`** antes de qualquer operação de branch.

---
Fontes: `report.md`, `BUG_BACKLOG.md`, `GAP_BACKLOG.md`, `BACKLOG.md`,
`PROMPTS.md`, `CNCF_GAP_DOCKER.md`, `CNCF_GAP_PODMAN.md`; skills em
`~/.claude/skills/` e `.ai/{skills,agents}/`.
