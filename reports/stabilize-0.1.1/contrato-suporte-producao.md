# Delonix 0.1.x — Contrato de suporte para produção (§3)

**Candidato:** `delonix 0.1.0`, catálogo de capacidades `1.4.0`, commit
`8ab783dd` (= `origin/develop`). Linha de correcção: `v0.1.1`.
**Data da medição:** 2026-10-29. **Fonte de verdade viva:** `delonix provider ls`
(medido neste host), `delonix provider describe <id> -o json` e
`delonix provider matrix` (vista declarada) — este documento é a **camada de
classificação de suporte** por cima dessa matriz, não uma cópia paralela.

> **0.x — contrato NÃO estável.** Esta é uma linha pré-1.0. As superfícies
> (CLI, JSON, gRPC/node.v1, schemas `*.delonix.io`, estado persistido) **podem
> mudar entre versões menores** com nota de migração; não há a garantia de
> estabilidade de uma 1.0. Ver `docs/cli-stability.md` e `VERSIONING.md`. Uma
> capacidade marcada SUPORTADA garante o *comportamento no âmbito validado*, não
> a imutabilidade do contrato.

## 1. Âmbito validado (onde estas classificações valem)

| Eixo | Validado |
|---|---|
| SO / kernel | Linux `7.0.0-38-generic` |
| Arquitectura | `x86_64` (binários `aarch64` e `x86_64-v3` publicados, **não** re-validados aqui) |
| Modo | **rootless-first** (daemonless), `cgroup2` delegado = sim; rootful suportado mas menos exercitado nesta medição |
| Engine | Delonix 0.1.0, state root `~/.local/share/delonix` |
| cloud-hypervisor | **v52.0** — disponível, saúde Ok |
| libvirt / QEMU | **libvirt 10.0.0 / QEMU 8.2.2** — disponível, saúde Ok |
| proxmox | **NotConfigured** neste host (sem alvo PVE vivo) |
| opnsense (gateway) | **NotConfigured** neste host (sem appliance) |
| linux/storage | Ok para volumes locais; **shares de rede (nfs/cifs/webdav) precisam de root** |

Fora deste âmbito (outro SO/arch, outras versões de provider, outro alvo
PVE/OPNsense do operador) o suporte é **não demonstrado** até re-validação.

## 2. Classes de suporte e critério

| Classe | Critério (regra aplicada) |
|---|---|
| **SUPORTADA PARA PRODUÇÃO NO ÂMBITO VALIDADO** | estado `supported` com evidência de **comportamento exercitado** — `live:` / `e2e:` / `chaos:` / `check:` (bateria) — que o gate `every_supported_capability_cites_evidence_that_exists` confirma existir |
| **IMPLEMENTADA, NÃO VALIDADA** | estado `supported` cuja única evidência é `test:` (teste unitário / mock), sem prova de comportamento end-to-end |
| **EXPERIMENTAL** | estado `partial` — implementada com um limite escrito, ou sem prova ao vivo |
| **NÃO SUPORTADA** | estado `not-implemented` (ausente) |
| **BLOQUEADA POR PRÉ-REQUISITO** | `requires-external-component`, `unavailable-on-host`, ou provider `NotConfigured`/sem infra neste âmbito |
| *(não-aplicável)* | `unsupported-by-provider` — a capacidade não pertence àquele provider (ex.: VMs no provider `linux`). **Não é uma lacuna**; é o denominador que a métrica N2 (270 células) exclui de propósito. |

**Nota metodológica honesta:** das 170 células em estado `supported` dos quatro
providers de compute/rede/storage, a distribuição de evidência é **live 63 ·
check 48 · e2e 35 · chaos 18 · test 6**. Só **6** são unit-only (→ IMPLEMENTADA,
NÃO VALIDADA). A métrica N2 publicada (**180/270**) inclui ainda 10 células
declaradas-supported do `opnsense`, que **neste host são BLOQUEADAS** (ver §4).

## 3. Classificação por domínio (4 providers descritíveis — 467 células)

| Domínio | SUPORTADA | EXPERIMENTAL | IMPL-NÃO-VALID. | NÃO-SUPORT. | BLOQ-PRÉ-REQ | (não-aplic.) |
|---|---:|---:|---:|---:|---:|---:|
| vm-compute | 61 | 8 | 3 | 9 | 1 | 46 |
| network | 25 | 9 | 0 | 9 | 1 | 50 |
| protection | 21 | 2 | 0 | 0 | 0 | 13 |
| containers | 11 | 1 | 0 | 0 | 0 | 36 |
| console | 9 | 1 | 0 | 0 | 0 | 6 |
| metrics | 8 | 2 | 2 | 4 | 0 | 0 |
| system-containers | 7 | 4 | 0 | 0 | 0 | 45 |
| firewall | 6 | 5 | 0 | 2 | 0 | 5 |
| storage | 6 | 1 | 0 | 2 | 5 | 1 |
| inventory | 4 | 5 | 1 | 6 | 0 | 0 |
| mobility | 4 | 0 | 0 | 1 | 6 | 5 |
| access | 2 | 4 | 0 | 0 | 0 | 2 |
| **TOTAL** | **164** | **42** | **6** | **33** | **13** | **209** |

SUPORTADA por provider: proxmox 47 · linux 44 · libvirt 37 · cloud-hypervisor 36.
(TSV célula-a-célula: `scratchpad/support-contract-cells.tsv`.)

## 4. Sobreposição crítica: disponibilidade de host (o que muda o GO)

A classe SUPORTADA acima assenta em evidência que **existe** no repositório. Para
**produção no âmbito do operador** sobrepõe-se a disponibilidade real:

| Provider / kind | Host desta medição | Efeito no suporte de produção |
|---|---|---|
| cloud-hypervisor compute | disponível, Ok | SUPORTADA **demonstrável aqui** |
| libvirt compute | disponível, Ok | SUPORTADA **demonstrável aqui** |
| linux compute / network | disponível, Ok | SUPORTADA **demonstrável aqui** |
| linux storage (local) | Ok | SUPORTADA **demonstrável aqui** |
| linux storage (nfs/cifs/webdav) | precisa de root | **BLOQUEADA POR PRÉ-REQUISITO** (3 células) |
| **proxmox compute+network** (47 SUPORTADA) | **NotConfigured** | evidência `live:` existe (provada contra um PVE de referência, `pve92`), mas **requer configurar + re-validar contra o alvo PVE do operador** antes de afirmar produção |
| **opnsense gateway** (10 declaradas) | **NotConfigured** | **BLOQUEADA POR PRÉ-REQUISITO** (precisa do appliance OPNsense vivo) |

## 5. Política de funcionalidades experimentais

As 42 células EXPERIMENTAL e as 6 IMPLEMENTADA-NÃO-VALIDADA **não devem ser
operadas em produção** sem aceitação explícita de risco. Quando uma experimental
possa afectar **segurança, integridade ou disponibilidade** (ex.: hotplug
cpu/mem em proxmox «mock-tested, never run against a guest»; `async-operations`
sem job handle; firewall `read back live, not measured on a packet`), a sua
activação tem de ser **explícita** e registada. O tecto honesto da matriz é
**247/270** — 23 células (bucket D) **nunca** devem virar SUPORTADA por desenho/
segurança/proveniência (ver `reports/capability-ceiling-20261027.md`).

## 6. Reflexo nas interfaces

A classificação é derivada da matriz em CÓDIGO, logo já se reflecte na CLI
(`provider ls`/`describe`/`matrix`) e nos docs gerados. **Pendente de alinhamento
na linha v0.1.1:** a linha geradora da matriz ainda diz «engine 5.1.1» —
cosmético, mas deve passar a 0.1.x. Desktop/console/REST/gRPC consomem o mesmo
catálogo; a sua conformidade com esta classificação é item da Sprint 3 (§11).

## 7. O que falta para fechar a decisão (não é GO ainda)

1. Re-validar os bloqueios restantes no HEAD: DEF-003 (compose up não reconcilia),
   OOM CRI, ramos CRI entrypoint/flannel (§10).
2. Re-validação por provider indisponível: proxmox contra alvo do operador;
   opnsense com appliance (§8).
3. Recuperação/continuidade (§12), instalação/upgrade em ambiente limpo (§13),
   testes sustentados (§15), segurança de artefactos (§16).
4. Integrar o fix F-1 (cache de camadas) na v0.1.1 — único P1 de integridade
   fechado nesta campanha (código+unit+live A/B).
