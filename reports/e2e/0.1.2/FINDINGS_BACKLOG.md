# FINDINGS_BACKLOG.md — 0.1.2 (@b72edeed)

Achados **novos** desta ronda de regressão do CLI. Numeração continua a de 0.1.1
(último GAP-0027). Reproduzidos ao vivo, host rootless, 2026-11-18. Doutrina: só o
efeito verificado conta. **0 regressões de binário**; o que segue é 1 GAP de
consistência, 1 achado de robustez do arnês, e dívida de staleness da bateria.

---

## GAP-0028 — VM com o mesmo nome noutra namespace: coexiste (não conflito)

> **RESOLVIDO 2026-11-18 (`119221e7`) — reclassificado como staleness da bateria.**
> Investigação a fundo: o motor indexa o registo de VM por namespace
> (`vms_root` → `vms/<ns>/<nome>.json`, port do catálogo ADR-0086) e expõe `vm
> move-namespace` (#982/#999) — **VMs são objectos namespaced por desenho actual**.
> Provado ao vivo: `apply` em teamA e teamB ambos `ensured`, registos distintos
> `vms/teamA/nsvm.json` + `vms/teamB/nsvm.json`, cada ns vê a sua. O conflito(5)
> era a regra ANTIGA (ADR-0069, registo node-único `vms/<nome>.json`), superseded
> quando as VMs passaram a namespaced; o teste e2e é que ficou para trás. Fix: a
> secção passa a asserir a coexistência namespaced (não conflito). **Pendente ao
> dono:** reconciliar a cláusula de VM do ADR-0069 (drift de doc).

- **Comportamento (OBSOLETO — registo histórico do achado):** `stack apply` de uma `VirtualMachine` com o mesmo
  nome noutra namespace **é aceite** (`rc=0`, `vm/<nome>: ensured`), em vez de
  recusado como conflito.
- **Esperado:** paridade com o fix #824 (`3de5db5c`, "nome de container repetido
  noutra namespace é conflito, não 'já existe'") — que a 0.1.2 aplicou a
  **containers** mas **não** a `VirtualMachine`. A bateria `e2e.sh` já espera
  `exit 5` ("o mesmo nome noutra namespace é um conflito (5)") e observa `rc=0`.
- **Porquê importa:** inconsistência de modelo entre container e VM no mesmo
  motor; dois objectos VM com o mesmo nome em namespaces diferentes podem colidir
  em recursos nomeados por nome nu (cf. `boot.rs:398/:415`, nomes por prefixo sem
  tenant — ligado a D2 do PLATFORM_EVAL 0.1.1).
- **Reprodução:** `stack apply` do mesmo `VirtualMachine` em `teamA` e depois
  `teamB` → o segundo é `ensured`, não recusado.
- **Aceitação:** aplicar a mesma normalização/verificação de conflito de nome
  cross-namespace a VMs; `e2e.sh` "o mesmo nome noutra namespace é um conflito"
  passa a PASS. Teste em `delonix-compute`/`stack`.
- **Dependências:** reusa o caminho do #824. **Complexidade:** S. **Tier:** B (rootless).

---

## H-1 (achado de arnês) — `e2e.sh`/VM sensível ao comprimento de `DELONIX_ROOT`

> **RESOLVIDO 2026-11-18 (`119221e7`).** `ch_usable()` passa a exigir `ch_sockfits`
> (orçamento `len(DELONIX_ROOT)+50 <= 107`); quando o root é fundo demais, as 11
> secções CH viram **SKIP** com razão em `ch_missing` em vez de 6+ FAIL espúrios.
> Provado: root curto (21B) corre, root longo (121B) SKIP. O binário já guardava
> (`SUN_PATH_MAX=107`); isto fecha só a armadilha de medição no arnês.

- **Observado:** com `DELONIX_ROOT` longo (caminho-scratch da sessão), **toda** a
  secção de cloud-hypervisor da bateria falha com `error[DX-1506] … socket path …
  is 143 bytes, a UNIX socket path is limited to 107 bytes` — 6 FAILs espúrios na
  1ª corrida. O binário **diagnostica correctamente**; o problema é do arnês, que
  não acautela o limite AF_UNIX de 107 bytes ao escolher o root de isolamento.
- **Esperado:** o caminho do socket do VMM (`<DELONIX_ROOT>/vms/<nome>.sock`)
  devia ser robusto a roots longos (hash/encurtamento do nome, ou socket num
  `XDG_RUNTIME_DIR` curto), **ou** a bateria devia recusar/avisar quando o root
  escolhido mais o nome de VM mais longo excede 107 bytes — em vez de produzir
  FAILs que se lêem como regressão.
- **Impacto:** armadilha de medição — contaminou a 1ª corrida; qualquer runner de
  CI/lab com um root profundo (p. ex. caminhos de workspace de agente) herda-a.
- **Reprodução:** `DELONIX_ROOT=<caminho longo> delonix vm create x --backend
  cloud-hypervisor --disk d.qcow2` → DX-1506; com root curto → `✓ VM started`.
- **Aceitação:** o VMM socket sobrevive a `DELONIX_ROOT` longo (ou a bateria
  encurta/avisa). Documentado na skill E2E (preflight). **Tier:** B.

---

## Dívida de staleness da bateria `e2e.sh` (binário correcto, asserção envelhecida)

Não são defeitos do motor — a bateria é que carrega expectativas anteriores à
0.1.2. Cada uma chumba a bateria sem um achado por trás; devem reconciliar-se
para o gate voltar a verde num lab apropriado.

| Check da bateria | Observado (0.1.2) | Expectativa stale | Acção |
|---|---|---|---|
| `vm resize --disk-size 0 recusa (1)` / `CH: --disk-size 0 recusa (1)` | clap recusa a `rc=2` com mensagem útil | `rc=1` (recusa de domínio) | actualizar expectativa p/ `2` (recusa no parser é correcta) |
| `delete de um NetworkGateway recusado termina` / `… apaga-se sem provider` | `DX-4000 no such kind 'networkgateways'` (`rc=4`) | kind existente | o catálogo 0.1.2 tem `Gateway` + `Router.networkGateways` (campo); o teste usa um kind inexistente — corrigir o kind ou remover/xfail |
| `métricas: GET /metrics devolve OpenMetrics…` | `# TYPE delonix_build info` + `delonix_build_info{version="0.1.2"} 1` (OpenMetrics `info`) | grep por `^# TYPE delonix_build_info ` | ajustar o grep à linha `# TYPE delonix_build info` (tipo `info` é mais correcto; liga ao fix de telemetria 0.1.2) |
| `cluster kube generate: … is 4` | `DX-1000` (`rc=1`) p/ "no such pod or container" | `rc=4` (not-found) | decidir: mapear not-found→`4`, ou corrigir a expectativa p/ `1` |
| `cluster network profiles: a row per profile` | "no support level is reported" | linha por perfil | reconciliar formato de saída |
| `system monitor --no-stream: … counting the running containers` | `1 containers, — active connections (conntrack)` | "1 containers" exacto | grep tolera o sufixo de conntrack (`—` em rootless, medido-não-disponível) |

### Lacunas de ambiente (BLOCKED, não dívida) — precisam de VM rootful
CRI default-caps, landlock+tmpfs, mounts privilegiados `/sys` (on-top/rshared),
`net netns firewall/publish` (nft-no-netns), slirp-flaky, `vm --require` (probe
de capacidade libvirt). Todas mediriam num lab rootful + CNI; o host é rootless
sem sudo. → `evidence.jsonl` perfil `lab-ephemeral`.

---

## Carregado da 0.1.1 (confirmado ainda aberto em `b72edeed`, parte já fechado em `9ec12592`)
- **GAP-0019** (`vm ls` só `-A`) e **GAP-0027** (build-cache sem GC): observados
  abertos nesta ronda; **o dono fechou-os** em `9ec12592`
  (`fix/cli-ls-parity-and-buildcache-prune`) durante a campanha.
- GAP-0018/0020..0026 e KRN/NAT: ver `reports/e2e/0.1.1/` — fora do âmbito desta
  ronda de regressão (sem re-verificação dirigida).
