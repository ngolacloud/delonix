# Execution PROMPTS — 0.1.0 backlog

Prompts ready to paste into a fresh session (each starts with `/escopo`). One per
workstream. A cross-cutting, **non-negotiable** rule: an item only closes once an
**automated regression test** exists that would fail if the defect came back — that
is what stops it reappearing in the next `report.md`. A fix without a test = not
closed.

GitFlow conventions: `fix/*` branches from `main` (propagates to `develop`); `feat/*`
and `chore/*` branch from `develop`. One worktree per workstream (`/home/walter/dlx-wt/<nome>`).
Tier: **B** = rootless (agent `delonix-builder-tester`); **D** = root/rootful-VM
(requires the owner/sudo/VM; the builder-tester CANNOT do it).

## Cross-cutting gates (MANDATORY — otherwise CI goes red)

Harness-alignment review (skill `delonix-backlog-driver`):

1. **Prompts that touch the CLI** (9 `build --sbom`, 10 `build --provenance`,
   11 `delonix scan`, 13 `delonix kube`, 15 `delonix generate systemd`) **MUST
   satisfy the repo's CLI gates**, or they fail CI:
   - new leaf/command → line in `docs/testing/folhas.tsv` + `scripts/cli-tree.sh --update`
     + `arch_fitness` (cli_context_lines) + regen of `dev_docs`;
   - new clap flag → hits `ARG_HELP_PENDING` (classify the help);
   - new user-facing text → `DX-NNNN` code + `pt.po` (3 strings) + order/digits;
   - `cli_exec_ratchet` + `folhas_gate` + `lang_ratchet` green.
   Each affected prompt must include this step in its done-criteria.

2. **GitFlow base for the conformance BUGs (BUG-0003/04/05/06, GAP-0001/0003):**
   the owner's workstream handles them in `feat/cri-conformance-rootful` (from develop),
   not in isolated `fix/` branches off main. **Prefer joining that workstream**
   (coordinate with the owner) over opening competing `fix/` branches on the same
   files. Bugs outside conformance (BUG-0001) follow a normal `fix/` off main.

3. **Do not touch the owner's dirty tree** — always use your own worktree; `reports/`
   by append only.

---

## Prompt 1 — BUG-0001 + M-TEST-03 · packaging `delonix-cri` [Tier B · `fix/` off main]

```
/escopo
Carrega a skill delonix-runtime-engineer e lê reports/e2e/0.1.0/BUG_BACKLOG.md
(BUG-0001) e referencia-correccoes.md.

Problema: `make install`/`make build` não geram o binário `delonix-cri` porque
`Makefile:44` PACKAGES lista o crate-biblioteca `delonix-cri` em vez do pacote
`delonix-cri-bin`. Instalação limpa falha com `Error 1`.

Tarefa (Tier B, rootless, agente delonix-builder-tester):
1. Em Makefile, trocar em PACKAGES `delonix-cri` → `delonix-cri-bin` (o bin
   continua a chamar-se `delonix-cri`; não tocar em BINARIES).
2. M-TEST-03: adicionar um gate que FALHA o build se algum binário de $(BINARIES)
   não aparecer em $(OUT_DIR) — p.ex. estender `require-built` e chamá-lo no fim
   de `build`, ou um scripts/check_binaries.py com test_*.py ao lado.

Teste de regressão (impede reaparecimento): o gate novo. Prova:
  - `git stash` da correcção do Makefile → `make build` deve FALHAR no gate.
  - com a correcção → `make install` numa árvore limpa instala os 5 binários sem
    passos manuais.

Done-criteria:
  [ ] `make install` limpo sem erro, 5 binários instalados
  [ ] `target/release/delonix-cri` produzido por `make build`
  [ ] gate de binários-em-falta existe e tem teste que o exercita
Fora de âmbito: renomear o bin; mexer no ADR-0086.
```

---

## Prompt 2 — M-TEST-02 · `stats` false-positive in the harness [Tier B · `chore/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê BACKLOG.md (M-TEST-02).

Problema: tests/cli-parity/frequent-verbs.sh:112 `same_behaviour "stats all"`
compara a saída de `delonix stats` vs `delonix container stats`. Como `stats` é
uma amostra volátil (CPU/mem), com um container vivo no host as duas chamadas
divergem → FAIL falso. Verificado: o handler é idêntico (bins/delonix/src/main.rs:217),
NÃO há bug de produto.

Tarefa (Tier B): tornar o caso determinístico — ou remover `stats` do conjunto
`same_behaviour` (mantendo o `same_help "stats"` que já prova o alias), ou
normalizar a saída (filtrar colunas voláteis) antes de comparar.

Teste de regressão: correr `tests/cli-parity/run.sh fast target/release/delonix`
COM um container vivo (`delonix run -d busybox sleep 60`) deve dar 50/50, não 49/1.

Done-criteria:
  [ ] run.sh fast = 0 falhas mesmo com container vivo
  [ ] o alias stats continua coberto por same_help
Fora de âmbito: mexer no código da CLI (não há bug lá).
```

---

## Prompt 3 — BUG-0004 (+ unblocks GAP-0001, GAP-0003) · host-namespaces [Tier D · `fix/` off main]

```
/escopo
Carrega delonix-runtime-engineer; lê BUG_BACKLOG.md (BUG-0004), GAP_BACKLOG.md
(GAP-0001, GAP-0003) e referencia-correccoes.md. ESTE é Tier D: validação em
rootful-VM (KVM) — coordena com o dono para root/VM.

Problema: HostNetwork e HostIpc (true/false) falham no critest mesmo em
rootful-VM. O flag é registado (delonix-cri/.../lifecycle.rs:38-43,86-92) e
propagado (delonix-linux/src/launch_spec.rs:43) mas NÃO há join de IPC-namespace
do host no delonix-linux (sem setns IPC). PodPID (GAP-0001, lifecycle.rs:1930) e
port-forward host-net (GAP-0003, streaming.rs:227) dependem disto.

Tarefa: ligar a partilha de host netns/ipc no caminho Linux; depois reavaliar a
recusa de shared-PID para pods host-net/CNI e o ramo host-net do port-forward.

Teste de regressão (impede reaparecimento): adicionar ao CI/gate um alvo que
corre critest em rootful-VM com
  FOCUS='NamespaceOption|PodPID|portforward in host network'
e exige 0 falhas nesses specs. Guardar o baseline esperado.

Done-criteria (medir em rootful-VM, comparar com reports/cri-conformance-0.1.1/):
  [ ] HostNetwork true/false e HostIpc true/false PASSAM (4 specs)
  [ ] PodPID PASSA (GAP-0001)
  [ ] port-forward in host network PASSA (GAP-0003)
  [ ] rootful-VM sobe de 90/13 para ≥97/≤6
  [ ] o default rootless NÃO regride (84/19 mantém-se)
Fora de âmbito: AppArmor, OOM, rshared (prompts próprios).
```

---

## Prompt 4 — BUG-0003 · AppArmor enforcement [Tier D · `fix/` off main]

```
/escopo
Carrega delonix-runtime-engineer; lê BUG_BACKLOG.md (BUG-0003). Tier D (rootful-VM).

Problema: 4 specs AppArmor falham com root real. Infra existe:
delonix-linux/src/lib.rs:3370 apply_apparmor (aa_change_onexec), run_host.rs:58-131
ensure_apparmor (apparmor_parser), delonix-cri/.../lifecycle.rs:1664-1705 parse.
O perfil pedido pelo pod não é imposto (escritas que deviam ser bloqueadas passam).

Tarefa: garantir que (a) o perfil instalado pela suite é carregado e (b)
apply_apparmor o aplica no exec do container no caminho rootful. Confirmar a
transição de perfil efectiva.

Teste de regressão: alvo CI em rootful-VM com FOCUS='AppArmor' exigindo 0 falhas;
mais um teste de integração que cria um container com perfil que nega escrita e
verifica EACCES numa escrita.

Done-criteria:
  [ ] 4 specs AppArmor PASSAM em rootful-VM
  [ ] escrita bloqueada por perfil → EACCES (teste de integração)
  [ ] rootless não regride
Fora de âmbito: host-ns, OOM, rshared.
```

---

## Prompt 5 — BUG-0005 · mount propagation `rshared` [Tier D · `fix/` off main]

```
/escopo
Carrega delonix-runtime-engineer; lê BUG_BACKLOG.md (BUG-0005). Tier D.

Problema: propagação bidireccional `rshared` (host↔container) não funciona.
Ponto-chave: delonix-linux/src/lib.rs:2504-2514 (raiz MS_PRIVATE por omissão,
só muda com wants_propagation()); mount() em :1505-1508; mapa CRI
lifecycle.rs:3357-3361 (PropagationBidirectional → rshared).

Tarefa: corrigir a decisão de propagação da raiz para que um mount `rshared`
propague nos dois sentidos.

Teste de regressão: alvo CI em rootful-VM FOCUS='Mount Propagation' = 0 falhas;
teste de integração: mount feito no host após arranque aparece no container e
vice-versa.

Done-criteria:
  [ ] spec `rshared` PASSA; `rprivate`/`rslave`/readonly continuam a passar
  [ ] teste bidireccional verde
Fora de âmbito: outros specs.
```

---

## Prompt 6 — BUG-0006 · OOMKilled / exit 137 [Tier D · `fix/` off main]

```
/escopo
Carrega delonix-runtime-engineer; lê BUG_BACKLOG.md (BUG-0006). Tier D.

Problema: container morto por OOM não reporta reason=OOMKilled / exit 137 mesmo
em rootful-VM. Infra completa: delonix-linux/src/lib.rs:146 died_of_oom (lê
memory.events.local), delonix-model/src/records.rs:227-230 (137),
delonix-cri/.../lifecycle.rs:1022 exit_reason.

Tarefa: confirmar que `memory.max` é aplicado ao cgroup do container na VM e que
memory.events.local é lido no cgroup certo; corrigir o elo partido.

Teste de regressão: alvo CI em rootful-VM FOCUS='OOM' = 0 falhas; teste de
integração que rebenta o limite de memória e assere exit 137 + OOMKilled.

Done-criteria:
  [ ] spec OOM PASSA em rootful-VM
  [ ] teste de integração OOM verde
Fora de âmbito: outros specs.
```

---

## Prompt 7 — GAP-0002 · broaden the `differential` suite [Tier B · `feat/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê GAP_BACKLOG.md (GAP-0002). Tier B.

Problema: tests/cli-parity differential só cobre 2 fluxos (container, pod). 76
comandos ficam só em reconhecimento de parser e 36 por-fazer — a afirmação de
paridade é parcial.

Tarefa: acrescentar fluxos differential (correm nos 3 motores docker/podman/
delonix) para: volumes (create/inspect/rm round-trip + recusa-em-uso), networks
(create/connect/rm), images (pull/tag/save/load). Seguir o padrão de
differential/flow1_interactive.py e flow2_pod.py.

Teste de regressão: os fluxos novos entram no `run.sh differential` e no CI; um
comando que regrida passa a falhar o differential.

Done-criteria:
  [ ] ≥3 fluxos novos verdes nos 3 motores
  [ ] matriz-cli-parity.csv actualizada (build_matrix.py) — menos «por-testar»
Fora de âmbito: implementar comandos em falta (isso é GAP-0007).
```

---

## Prompt 8 — M-DOC-01 + M-DOC-04 · real numbers and the rootful-VM path [Tier B · `chore/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê BACKLOG.md (M-DOC-01, M-DOC-04). Correr
DEPOIS dos prompts 3–6 fecharem, para publicar números verdadeiros.

Tarefa:
1. M-DOC-01: substituir qualquer menção a «~104/104» / «84/20» pela medição real
   corrente (rootless e rootful-VM) em docs/README/relatórios públicos. Citar
   reports/cri-conformance-0.1.1/ e reports/e2e/0.1.0/.
2. M-DOC-04: documentar o modo rootful-VM como caminho oficial para cargas que
   exigem host-namespaces/AppArmor/OOM (materializa «rootless-first, rootful onde
   exigir»).

Teste de regressão: um gate de docs (docs_cli_gate / dev_docs_site.py --check)
que falha se os números publicados divergirem do último report.md.

Done-criteria:
  [ ] nenhuma menção a números desactualizados nas docs
  [ ] secção rootful-VM publicada
Fora de âmbito: correcções de código.
```

---

# CNCF PROMPTS — 8 value areas (GAP-0008..0014)

Rationale: `CNCF_GAP_DOCKER.md`. Supply-chain first (highest weight). Already
aligned (do not redo): OSV in the scanner, cosign/sigstore in `sign.rs`, rootless.

## Prompt 9 — GAP-0009 · SPDX in-toto SBOM on the image [CNCF 4 · Tier B · `feat/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê CNCF_GAP_DOCKER.md (GAP-0009) e
GAP_BACKLOG.md. Tier B (rootless).

Problema: delonix-scanner já extrai SBOM (apk/dpkg do CAS), mas não o publica
como attestation da imagem. BuildKit anexa SBOM SPDX embrulhado em in-toto.

Tarefa: (1) emissor SPDX JSON a partir do SBOM do scanner; (2) embrulhar em
in-toto attestation; (3) `delonix build --sbom` anexa-a ao índice OCI; (4)
`delonix image sbom <ref>` exporta o SPDX. Reutilizar o caminho de attestation
de delonix-oci/sign.rs.

Teste de regressão: teste que constrói com --sbom, lê o índice OCI, valida que o
predicate é SPDX válido in-toto, e que `cosign`/um verificador terceiro o lê.
Entra no CI.

Done-criteria:
  [ ] SPDX in-toto anexada e verificável por ferramenta externa
  [ ] `delonix image sbom` exporta SPDX conforme
Fora de âmbito: SLSA provenance (GAP-0008, prompt próprio).
```

## Prompt 10 — GAP-0008 · SLSA provenance on the build [CNCF 3 · Tier B · `feat/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê CNCF_GAP_DOCKER.md (GAP-0008). Tier B.
Correr DEPOIS do Prompt 9 (partilham o anexo de attestations).

Problema: delonix build não emite proveniência. BuildKit emite SLSA v0.2/v1.1
(mode min/max, builder-id, reproducible) como in-toto.

Tarefa: `delonix build --provenance[=mode=min|max]` produz predicate SLSA v1.1
(builder.id, materials, invocation, metadata) em in-toto, anexado ao índice OCI.

Teste de regressão: teste que constrói com --provenance=max, valida o predicate
contra o schema SLSA v1.1 e confirma materials/builder.id; no CI.

Done-criteria:
  [ ] SLSA v1.1 in-toto anexada, modos min/max
  [ ] validada contra o schema slsa.dev e por verificador terceiro
Fora de âmbito: cache distribuído, multi-arch (GAP-0010).
```

## Prompt 11 — GAP-0013 · Scanner policy + compare + CI gating [CNCF 8 · Tier B · `feat/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê CNCF_GAP_DOCKER.md (GAP-0013). Tier B.

Problema: delonix-scanner faz SBOM+CVE (OSV) mas sem policy/compare/gating — o
que Docker Scout oferece para pipelines.

Tarefa: (1) `delonix scan <ref> --exit-code --only-severity critical,high`
(sai !=0 se encontrar); (2) `delonix scan compare A B` (diff de CVEs); (3)
política declarável (um kind) avaliada no scan.

Teste de regressão: teste com imagem de CVE conhecida → exit-code dispara; outra
limpa → 0; compare detecta CVE novo. No CI.

Done-criteria:
  [ ] gating por severidade com exit-code
  [ ] compare entre imagens
  [ ] política declarável avaliada
Fora de âmbito: trocar o backend OSV.
```

## Prompt 12 — GAP-0011 · Converge on the Compose Specification [CNCF 6 · Tier B · `feat/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê CNCF_GAP_DOCKER.md (GAP-0011) e GAP-0005.
Tier B. Fechar DEF-003 primeiro (GAP-0005).

Tarefa: alinhar `delonix compose` com a Compose Specification
(compose-spec/compose-spec); mapear directivas comuns; documentar divergências.

Teste de regressão: suite de conformância que corre um conjunto de compose.yaml
da spec e exige comportamento esperado; no CI. Uma directiva que regrida falha.

Done-criteria:
  [ ] suite de conformância Compose Spec verde nas directivas comuns
  [ ] divergências documentadas (não silenciosas)
Fora de âmbito: directivas Swarm-only.
```

## Prompt 13 — GAP-0015 · Kubernetes YAML interop `kube play`/`generate` [CNCF 2 (Podman) · Tier B · `feat/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê CNCF_GAP_PODMAN.md (GAP-0015). Tier B.

Problema: Delonix usa manifests próprios (compute.delonix.io/v1beta1); não corre
nem gera YAML Kubernetes standard de workloads. Podman tem `kube play`/`generate`.

Tarefa: (1) `delonix kube play <manifest.yaml>` corre workloads em apiVersion:v1/
apps/v1 (Pod no mínimo), convertendo para o envelope interno; (2) `delonix kube
generate <ref>` emite YAML k8s válido de um pod/container existente.

Teste de regressão: round-trip no CI — generate de um pod → play do YAML gerado
→ pod equivalente a correr; e play de um Pod k8s canónico. Falha se divergir.

Done-criteria:
  [ ] `kube play` corre um Pod k8s canónico
  [ ] `kube generate` emite YAML k8s válido (round-trip verde)
Fora de âmbito: Deployment/StatefulSet completos (fase 2).
```

## Prompt 14 — GAP-0017 · Signature trust by policy [CNCF 4/8 (Podman) · Tier B · `feat/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê CNCF_GAP_PODMAN.md (GAP-0017). Tier B.
Correr DEPOIS do Prompt 9 (formato de attestation).

Problema: `sign.rs` verifica cosign por chave explícita; `policy.json`
(delonix-security/domain/policy.rs) é de admissão/runtime, não trust de assinatura
por registo. Podman: containers-policy.json (sigstore+GPG, trust por registo/repo).

Tarefa: estender o `policy.json` com regras de trust de assinatura por registo/
repo (sigstore + chave); `delonix container run`/`image pull` recusa imagem fora
da policy, reutilizando sign.rs.

Teste de regressão: imagem assinada-conforme → corre; não-assinada/fora da policy
→ recusada com erro accionável. No CI.

Done-criteria:
  [ ] trust declarativo por registo/repo no policy.json
  [ ] run/pull recusa imagem fora da policy
Fora de âmbito: emissão de assinaturas (já em sign.rs).
```

## Prompt 15 — GAP-0016 · Quadlet/systemd for workloads [CNCF 5 (Podman) · Tier B · `feat/` off develop]

```
/escopo
Carrega delonix-runtime-engineer; lê CNCF_GAP_PODMAN.md (GAP-0016). Tier B.

Problema: `delonix serve` gera unit só para o node-api (cmd/serve.rs); não há
unidades systemd para containers/pods do utilizador. Podman: Quadlet/generate systemd.

Tarefa: `delonix generate systemd <container|pod>` emite uma unit funcional
(reutilizar a mecânica de serve.rs); opcionalmente um formato Quadlet declarativo.

Teste de regressão: gerar unit de um container, instalar num systemd de teste
(user), `systemctl start` → container a correr; `stop` → parado. No CI (se houver
systemd no runner) ou teste de geração+lint da unit.

Done-criteria:
  [ ] unit systemd funcional gerada p/ container e pod
  [ ] start/stop via systemd comprovados (ou unit validada)
Fora de âmbito: socket-activation avançada.
```

## CNCF roadmap (larger prompts / decision)

- **GAP-0012** (CNCF 1+2, XL) — run+publish oci image-spec/distribution-spec/
  runtime-spec conformance; CRI ≥95% (consumes BUG-0003/04/05/06); **ADR**
  «own runtime vs adopting containerd». Open only after the CRI BUGs.
- **GAP-0010** (CNCF 5, XL) — `--platform` multi-arch + `--cache-to/--cache-from
  type=registry`; its own campaign.
- **GAP-0014** (CNCF 7) — **owner's decision**: go public + governance
  (SECURITY.md, CODEOWNERS, release policy). Not just code.

---

## Parity roadmap (larger prompts — open when prioritised)

- **GAP-0004** (full Docker Engine API, XL) — multi-phase campaign per endpoint
  family; measure against a real client. Its own prompt per family.
- **GAP-0005** (Compose / DEF-003) — close DEF-003 + directives matrix.
- **GAP-0006** (contexts / `system connection`) — **ADR-0010 decision first**:
  confirm WONT_FIX or reopen the scope; not code debt until decided.
- **GAP-0007** (36 unimplemented commands) — depends on GAP-0002 to measure; then
  one prompt per family (image/volume/network prune …).

## Firing order (respects dependencies and the `fila`'s WIP cap)

Parity/CRI (test round):
1. Prompt 1 (BUG-0001 + gate) · Prompt 2 (M-TEST-02) — Tier B, independent.
2. Prompt 3 (BUG-0004 → GAP-0001/0003) — Tier D, biggest lever (+7 specs).
3. Prompts 4, 5, 6 (AppArmor, rshared, OOM) — Tier D, parallelisable.
4. Prompt 7 (differential) — Tier B, any time.
5. Prompt 8 (docs) — only after 3–6.

CNCF (supply-chain first):
6. Prompt 9 (SBOM in-toto) → Prompt 10 (SLSA provenance) — Tier B.
7. Prompt 11 (scanner CI) — Tier B.
8. Prompt 12 (Compose Spec) — after GAP-0005.
9. GAP-0012 / GAP-0010 / GAP-0014 — roadmap/decision.
