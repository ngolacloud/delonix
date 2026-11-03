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
- `[agente]` ✅ **FEITO (mecanismo):** `scripts/e2e_upgrade_rollback.sh` — vN→vN+1→vN
  com container vivo + rede + marca de estado; 17 asserções VERDES (2026-11-02):
  sobrevivência (container `Up`, MESMO id após a troca do binário), gestão pelo
  motor novo (`exec`/`logs` sobre container nascido sob o antigo), reconciliação
  (`stop`/`start` preserva a camada gravável), e rollback. Param. `--old`/`--new`.
  **FALTA a compatibilidade CRUZADA de versões** (binário novo a ler estado de
  versão antiga): precisa de uma tag anterior compilada para `--old` — o harness
  já a mede sem mais mudanças. (O estado em disco entre versões fica em
  `e2e_state_upgrade.sh`.)
- `[agente]` ✅ **FEITO:** hot-reattach medido e documentado. Achados-chave:
  **(1)** `cp` in-place do binário em uso → **ETXTBSY** (kernel recusa); upgrade
  TEM de ser por **rename atómico** (dpkg/rpm/`install(1)` já o fazem).
  **(2)** containers vivos **sobrevivem** à troca do binário (supervisor fica no
  inode antigo, mapeado). **(3)** NÃO há hot-patch do supervisor vivo: ele corre
  o código com que nasceu até ser recriado — um fix no caminho de supervisão só
  chega a um container existente via recriação faseada.
- `[agente]` ✅ **FEITO:** runbook `docs/operations/upgrade-rollback.md` (procedimento
  rename-atómico, tabela do que sobrevive, rollback, validação, e a secção
  Limites: compat-cruzada por provar, sem hot-patch, sem rollback-a-meio-de-apply
  `47_IAC_REVISAO.md:400`).

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
- `[dono]` ✅ **DECIDIDO (2026-11-02): via PROVA — o Delonix quer SER HA.** Logo o
  critério é o primeiro bullet acima (overlay inter-nó E2E + control-plane k8s HA).
  Próximos passos: (1) correr `scripts/e2e_overlay_multinode.sh` contra 2 nós reais
  (2 VMs delonix ou 2 hosts SSH); (2) retomar a cadeia kubeadm HA (≥3 CP, etcd
  stacked, VIP), hoje bloqueada por [[delonix-cri-entrypoint-override-bug]] (fix em
  `fix/cri-command-entrypoint-override`, por integrar). O caminho "via âmbito"
  (ADR a excluir failover) fica DESCARTADO.
- `[agente]` **BANCADA CORRIDA EM 2 NÓS REAIS (2026-11-02) — ver
  `g4-overlay-forwarding.md`:** 2 VMs do golden k8s (virbr0, IPs roteáveis, pubkeys
  WG trocadas, binário 0.1.0 da linha atual). RESULTADO: **forwarding = 100% de
  perda** nos dois sentidos. CAUSA-RAIZ cravada: `ss -ulnp` no nó mostra NADA em
  UDP 51820/4789 — o overlay faz `wg-up`/VXLAN DENTRO do holder (slirp4netns), mas
  o `slirp_add_hostfwd` só liga portas publicadas, nunca 51820/4789 → o handshake
  WG do peer bate em porta fechada. Confirma (com dado) o `Partial` da matriz.
  Achado 2º: **`--subnet` ignorado** (overlay em 10.234/16 auto, cada nó
  independente), reproduzido em VMs reais. **DOIS bloqueios de SDN-core a fechar
  antes de "via prova" passar:** (1) hostfwd do slirp p/ 51820(WG)+4789(VXLAN)
  quando há overlay — ou um modo rootful com WG numa interface real; (2) honrar
  `--subnet` + coordenar o /16 entre nós. São mudanças com implicações de
  firewall/ADR → carecem de desenho do dono antes de implementar.
- `[agente]` ✅ **FEITO (bancada):** `scripts/e2e_overlay_multinode.sh` — a bancada
  multi-nó do caminho "via prova", topologia-agnóstica (`--via ssh A B`). Assere:
  identidade WireGuard em cada nó, criar o MESMO overlay (`--driver overlay --vni`)
  com o outro como `--peer <ip>=<pub>=<wg_ip>`, um container em cada nó na rede,
  **forwarding E2E** A→B e B→A pelos IPs-overlay, e **prova de caminho** (handshake
  WireGuard + peer). **MEDIDO e provado que uma só caixa NÃO corre isto** (`node
  init` ok, mas `network create --driver overlay` em rootful dá `DX-9301 timeout
  waiting for the netns holder`; rootless está atrás de slirp, sem inbound) — por
  isso o harness **recusa fingir** (exit 3) e imprime o runbook. Confere com a
  razão que a própria matriz já dá para marcar o overlay `Partial`. **FALTA**:
  correr contra 2 nós reais (SSH) — ou retomar a cadeia kubeadm HA bloqueada por
  [[delonix-cri-entrypoint-override-bug]] para o control-plane HA.
- `[agente]` ✅ **OVERLAY INTER-NÓ: PROVADO E2E (2026-11-02).** Três fixes (WG
  hostfwd ADR-0084 + `--subnet` honrado + firewall `dlxinput` aceita a VXLAN de
  entrada `iifname "wgo*" udp dport 4789`): ping container-a-container entre 2 VMs
  reais a **0% de perda** nos dois sentidos. `net.overlay.{vxlan,encrypted}`
  Partial→Supported na matriz (183/271). Ver `g4-overlay-forwarding.md`.
- `[agente]` **CONTROL-PLANE k8s HA — estado (2026-11-03):** o blocker do CNI (bug
  CRI de override de entrypoint) está **RESOLVIDO e verificado nesta árvore** —
  o fix está em `delonix-cri` (`runtime_svc/lifecycle.rs`: `command`→`o.entrypoint`
  quando não vazio), tem teste `cri_command_overrides_the_image_entrypoint`
  (PASSA), e `cargo build -p delonix-cri` compila. A mecânica HA (3 CP + etcd
  stacked + HAProxy) já tinha sido provada noutra sessão
  ([[delonix-kubeadm-ha-offline-findings]]). **ACHADO:** o turnkey `delonix cluster
  create` (kind-mode) **recusa `--control-planes > 1`** (DX-1000: "delonix does not
  provision an LB/VIP yet") — logo a HA EXIGE o caminho kubeadm-em-VMs + um
  LB/VIP externo (HAProxy/keepalived). **FALTA** o re-demo end-to-end (3 CP +
  etcd + VIP + CNI + sobreviver à queda de um CP) — são 3-5 VMs k8s + LB, que
  NÃO corri aqui: disco a 95% (46G) com as BDs vivas do dono = risco de
  exaustão; precisa de um **nó dedicado**. Candidato de produto: o `cluster
  create` provisionar o LB/VIP para desbloquear HA turnkey.
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
- `[agente]` ✅ **FEITO (arnês):** `scripts/bench_density.sh` — rampa de patamares
  (`--tiers 1,10,25,50,100`), attach p50/p99 por patamar + tamanho dos dois dumps
  `nft` (`list sets`, `-a list chain fwdeny`); emite a CURVA em JSON (contrato
  `publishable` do `bench.sh`), herda o juízo de bancada do `bancada.sh` (recusa
  acima do limiar), marca NÃO PUBLICÁVEL quando há containers alheios, e limpa
  tudo o que cria (prefixo `benchdens-`). Provado ponta-a-ponta em rootless (curva
  + cleanup + JSON). As colunas `nft` enchem em **rootful** (tabela `inet dlxing`
  visível do host); em rootless ficam `null` por desenho (o holder não se
  `nsenter`a de fora). **Falta correr num nó DEDICADO** (esta caixa tem as BDs do
  dono → não publicável) para produzir a curva a sério.
- `[agente]` Otimizar o que a curva mostrar. Alvo já localizado por inspeção:
  `drop_from_every_ns_set` (`infra.rs:3947`) faz `nft list sets` a cada
  attach/detach, e há 3 `nft -a list chain fwdeny` que varrem a cadeia inteira —
  candidatos a O(N)→O(1). **NÃO otimizar antes da curva** (medir-primeiro).
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
- `[agente]` ✅ **FEITO (matriz):** `capability-matrix.md` re-gerada do binário atual
  (2026-11-02): **181/271 (66,8 %)**, engine 0.1.0 — era 180/270@5.1.1; `net.ipv6`
  passou a `supported` (ADR-0082). Drift fechado: o teste `the_published_matrix_is_
  the_generated_one` volta a VERDE. README deste relatório + `stale-evidence.md`
  actualizados com data/commit.
- `[agente]` **FALTA (nó dedicado/infra):** re-correr `bench.sh` (recusa-se em nó
  carregado — por desenho), `bench_density.sh` (curva — nó ocioso), `critest`
  (k8s/CRI) e `chaos.sh`/`e2e.sh` (criam VMs/kind). Esta caixa tem carga alheia
  (BDs do dono) → não publicável. Actualizar `cri-conformance.md`/`comparacao-medida.md`
  quando houver essas corridas.
- `[agente]` ✅ **FEITO:** `stale-evidence.md` marca a matriz como FRESCA e lista o
  que continua STALE (e porquê).

---

## O que posso começar já (sem merge/release, sem terceiros)

Executável por mim de imediato, de maior para menor alavanca:
1. **G1 (b,c,d,e):** SHA-pin dos Actions, `cargo-audit`+scan no CI, feed CVE real, verificador de artefactos. *(a cadeia de fornecimento é o elo mais fraco e é todo in-repo.)*
2. **G6:** re-correr as suites e refrescar os números — destrava a confiança em tudo o resto.
3. **G5:** bench de densidade (dá o primeiro tecto de escala que nunca existiu).
4. **G3:** harness de upgrade/rollback.

Fica para o dono/terceiros: o **pentest externo** (G1a), a **tag/release** (G2), e
a **decisão de âmbito de HA** (G4). Diz-me por qual das quatro começo.
