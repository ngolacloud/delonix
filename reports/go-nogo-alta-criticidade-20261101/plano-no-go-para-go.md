# Plano NO-GO → GO (alta criticidade)

> Fecha os 6 pontos do portão do [veredicto](README.md). Cada ponto tem: **estado
> actual** (baseline da avaliação), **critério de GO** (mensurável — como sabemos
> que fechou), **tarefas**, e **quem** pode fazê-las:
> `[agente]` executável por mim no repo · `[dono]` decisão/acção do dono
> (merge/release/commissão) · `[externo]` terceira parte independente.
>
> **Definição de GO global:** os 6 critérios verdes **E** uma re-execução desta
> avaliação na versão enviada confirma que nenhum desqualificador sobrevive.

## Ordem recomendada (porquê)

```
Onda 1 (base de verdade)   G6 re-medir ───┐
Onda 2 (confiança)         G1 segurança   ├─► G2 release validada
Onda 3 (segurança op.)     G3 upgrade/rollback   G5 escala/densidade
Onda 4 (tecto)             G4 HA/multi-nó  (ou decisão de excluir failover)
```

- **G6 primeiro:** toda a outra afirmação assenta em números actuais; a evidência
  hoje é de v0.x–v3.1.0/5.1.1 (`reports/.../stale-evidence.md`). Sem G6, G1/G2
  validam um alvo em movimento.
- **G1 antes de G2:** uma release "de confiança" exige que o pipeline que a produz
  seja ele próprio confiável (cadeia de fornecimento).
- **G4 por último:** é o maior investimento e pode ser **substituído por uma
  decisão de âmbito** (excluir failover e documentar o tecto single-node).

---

## G1 — Segurança assegurada externamente

**Estado actual.** Auditorias só internas (AI-adversarial); cadeia de fornecimento:
G-020 0/20 Actions SHA-pinned; sem `cargo-audit`/trivy no CI; scanner CVE sem feed
(`delonix-scanner/src/lib.rs:371`); assinatura/SLSA/SBOM "por desenho", não
verificados contra artefactos. Rootful/SELinux nunca avaliados.

**Critério de GO.** (a) Pentest externo da linha enviada com relatório e achados
HIGH/CRITICAL fechados; (b) 20/20 Actions SHA-pinned; (c) `cargo-audit` + scan de
imagem no CI a falhar o build em CVE conhecido; (d) scanner com feed real
(OSV/NVD/Alpine secdb) provado a apanhar um CVE conhecido; (e) assinatura +
provenance + SBOM **verificados contra os artefactos reais** da release; (f)
rootful e SELinux-enforcing avaliados (ou declarados fora de âmbito por escrito).

**Tarefas.**
- `[agente]` SHA-pin todos os `.github/workflows/*` (tag→SHA) + comentário com a versão legível.
- `[agente]` Adicionar `cargo-audit` (ou `cargo deny advisories` já existe — estender a CVE) e um passo de scan de imagem ao CI, a falhar o build.
- `[agente]` Ligar o scanner a um feed real (OSV/NVD/Alpine secdb), com teste que apanha um CVE conhecido plantado.
- `[agente]` Script que **verifica** assinatura/provenance/SBOM contra os assets de uma release candidata (fecha o "PASS por desenho").
- `[agente]` Bateria rootful + SELinux-enforcing (numa VM) e registo dos resultados; OU ADR a excluir esses modos.
- `[dono]`/`[externo]` **Comissionar o pentest externo** — não é auto-satisfazível; é o item que define "independente".

---

## G2 — Release tagueada e validada

**Estado actual.** Nada publicado na linha 0.x (`CHANGELOG` "[Não lançado]"); a
última legacy (v5.1.1) foi mal validada (pushes diretos a `main`, CI vermelho,
`release.yml` falhou 3× — `docs/RELEASES.md:78-99`).

**Critério de GO.** Uma tag cuja CI está **verde** e em que `cargo test` + `e2e.sh`
+ `chaos.sh` + `critest` correram **na própria tag** (não "na develop há semanas"),
com os artefactos assinados e verificados (liga-se a G1).

**Tarefas.**
- `[agente]` Endurecer `release.yml`/`gitflow.py`: gate que recusa taguear com CI não-verde e sem as suites corridas na tag.
- `[agente]` Fechar os gates de baseline que hoje falham (rustfmt/lang_ratchet) — ver [[delonix-merge-ci-pitfalls]] — para o "verde" ser real.
- `[dono]` Merge da linha estável → `main` e **cortar a tag** (acção do dono; o hook recusa push do agente).

---

## G3 — Upgrade/rollback

**Estado actual.** NÃO TESTADO (`release-verdict.md`); supervisores vivos correm o
código com que nasceram — sem hot-patch (`RELATORIO-PRE-PRODUCAO.md:116`); sem
rollback a meio de `apply` (`47_IAC_REVISAO.md:400`). Existe só
`e2e_state_upgrade.sh` (formato de estado em disco).

**Critério de GO.** (a) Teste E2E: instalar vN, correr cargas, actualizar para
vN+1 **com containers/VMs vivos**, confirmar que sobrevivem e reconciliam; (b)
rollback para vN provado (binário anterior + `netns down/up` numa janela) sem perda
de estado; (c) procedimento escrito para corrigir um supervisor vivo (ou ADR que
assuma o limite e o runbook de "recriar em janela").

**Tarefas.**
- `[agente]` Harness `e2e_upgrade_rollback.sh`: vN→vN+1→vN com containers, VM, stack e rede ativas; asserções de sobrevivência + reconciliação.
- `[agente]` Medir e documentar o hot-reattach de supervisores (o que sobrevive a um upgrade do binário vs o que exige recriação).
- `[agente]` Runbook de upgrade/rollback em `docs/` + o rollback-a-meio-de-apply (hoje "não há").

---

## G4 — HA / multi-nó

**Estado actual.** Single-node por desenho; frota delegada ao `delonix-paas`
(ADR-0010). Overlay inter-nó (VXLAN/WireGuard) não provado E2E; VM HA/replicação =
`requires-external-component`; migração ao vivo NO-GO (ADR-0031); kubeadm HA offline
não executado nas campanhas. Há trabalho feito (5 VMs + 3 CP etcd stacked + HAProxy
provado numa sessão anterior — ver [[delonix-kubeadm-ha-offline-findings]]).

**Critério de GO (um dos dois).**
- **Via prova:** overlay inter-nó forwarding E2E entre ≥2 nós + um control-plane k8s HA (≥3 CP, etcd stacked, VIP) a sobreviver à queda de um CP, tudo sobre o Delonix; OU
- **Via âmbito:** ADR explícito que exclui failover do produto, documenta o tecto single-node, e aponta a frota para `delonix-paas`/k8s externo — tornando "alta criticidade COM HA" um não-objetivo honesto em vez de uma lacuna.

**Tarefas.**
- `[dono]` **Decisão de âmbito primeiro:** o Delonix quer ser HA, ou ser o *nó* e delegar HA? Isto muda todo o resto do trabalho.
- `[agente]` (se via prova) Bancada multi-nó: 2+ VMs, overlay forwarding E2E com contadores; retomar a cadeia kubeadm HA bloqueada por [[delonix-cri-entrypoint-override-bug]].
- `[agente]` (se via âmbito) Escrever o ADR + a secção "tecto single-node" no handbook.

---

## G5 — Escala / densidade

**Estado actual.** Não medida. Única amostra: chaos `scale(30)` com teardown limpo.
O caminho de attach faz ~20-25 `fork`+`exec` + 2 dumps `nft` que crescem com o nº
de containers/namespaces; todos os números foram tirados em nó vazio
(`comparacao-medida.md:207-230`).

**Critério de GO.** Tecto de densidade declarado (ex. "N containers/nó com p99 de
attach < X ms e teardown estável"), medido num nó dedicado, com a curva de latência
vs densidade (não um ponto em nó vazio) e as SLOs correspondentes.

**Tarefas.**
- `[agente]` Bench de densidade: rampa 1→50→200→… containers/namespaces, medir attach p50/p99 e os dois dumps `nft` em cada patamar, até degradar.
- `[agente]` Otimizar o que a curva mostrar (ex. os dumps `nft` lineares — candidato a cache/índice) OU declarar o tecto honestamente.
- `[agente]` SLOs escritas + a curva publicada no relatório.

---

## G6 — Re-medir a evidência stale

**Estado actual.** CRI/bench/E2E/Docker-API medidos em v0.x–v3.1.0/5.1.1; "quase
nenhum número publicado foi medido em 3.x+" (`stale-evidence.md`). O veredicto
assenta nisto com aviso.

**Critério de GO.** `critest`, `bench.sh`, `e2e.sh`, `chaos.sh` e a matriz de
capacidades **re-corridos na versão enviada**, com as datas e o commit a bater
certo; os documentos de comparação actualizados a partir dessas corridas.

**Tarefas.**
- `[agente]` Re-correr as cinco suites na linha atual (nó ocioso/dedicado para o bench, como a própria bancada exige).
- `[agente]` Actualizar `capability-matrix.md`, `cri-conformance.md`, `comparacao-medida.md` com os números novos + datas.
- `[agente]` Marcar no `stale-evidence.md` o que passou a fresco.

---

## O que posso começar já (sem merge/release, sem terceiros)

Executável por mim de imediato, de maior para menor alavanca:
1. **G1 (b,c,d,e):** SHA-pin dos Actions, `cargo-audit`+scan no CI, feed CVE real, verificador de artefactos. *(a cadeia de fornecimento é o elo mais fraco e é todo in-repo.)*
2. **G6:** re-correr as suites e refrescar os números — destrava a confiança em tudo o resto.
3. **G5:** bench de densidade (dá o primeiro tecto de escala que nunca existiu).
4. **G3:** harness de upgrade/rollback.

Fica para o dono/terceiros: o **pentest externo** (G1a), a **tag/release** (G2), e
a **decisão de âmbito de HA** (G4). Diz-me por qual das quatro começo.
