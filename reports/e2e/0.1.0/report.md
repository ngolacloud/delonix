# Delonix Runtime — Relatório de validação (paridade CLI + conformância CRI + instalação)

> Âmbito focado, não um E2E completo. Esta ronda mede três eixos que sustentam
> a comparação imparcial Delonix × Docker × Podman: (1) instalação a partir da
> árvore, (2) paridade de CLI/comportamento contra docker e podman, (3)
> conformância CRI real (critest), em rootless e em rootful-VM. Os grupos não
> exercitados ficam marcados BLOCKED/SKIPPED com motivo — nunca omitidos.

## Executive Summary

| | |
|---|---|
| Version | `delonix 0.1.0` |
| Commit | `1898466bb` (develop; +241 commits since v0.1.0) |
| Date | 2026-11-08 |
| Environment | Host partilhado Linux 7.0.0, x86_64, cgroup v2; **sob carga** (QEMU cri-rootful + cluster kind + Ceph + Odoo do dono a decorrer em paralelo) |
| Engines comparados | Docker 29.9.2 (rootful, daemon), Podman 4.9.3 (rootless), Delonix 0.1.0 (rootless) |
| Modo | **rootless-first**; CRI também medido em **rootful-VM** (KVM, root real) onde a suite o exige |
| Suites | `cri-tools critest` v1.36.0; harness `tests/cli-parity` (fast + differential) |

| Eixo | Resultado |
|---|---|
| Instalação `make install` | **FALHA → contornada** (bug de empacotamento B1); binário 0.1.0 instalado |
| Paridade `fast` (parser/TAB/erros, rootless) | **49 OK / 1 FAIL** |
| Paridade `differential` (docker×podman×delonix) | **PASS** (flow container + flow pod) |
| Matriz estática CLI (198 comandos, Nov 6) | 63 passou · 76 reconhecido-por-testar · 36 por-fazer · 14 fora-âmbito · 9 divergência |
| CRI critest **rootless** | **84 ✓ / 19 ✗ / 19 skip** (Ran 103/122, 616 s) |
| CRI critest **rootful-VM** | **90 ✓ / 13 ✗ / 19 skip** (Ran 103/122, 389 s) |

## 1. Instalação (eixo Core Runtime / build)

`make install` abortou: `error: target/release/delonix-cri is missing`. Causa:
`PACKAGES` no `Makefile` (linha 44) lista o **crate-biblioteca** `delonix-cri`,
não o pacote-binário `delonix-cri-bin` que produz o executável `delonix-cri`
(ADR-0086 moveu o bin; `scripts/critest.sh` foi corrigido na sessão do dono, o
`Makefile` não). Contornado com
`cargo build --release -p delonix-cri-bin --bin delonix-cri` + novo `make install`.
→ **BUG-0001**.

## 2. Paridade de CLI e comportamento

### 2.1 Tier `fast` (parser, TAB, erros — rootless, sem motor)

`tests/cli-parity/run.sh fast target/release/delonix` → **49 OK / 1 FAIL**.
Todos os aliases de topo (`ps`, `logs`, `images`, `rmi`, `cp`, `port`, `info`,
`events`, `wait`…) alinham com a forma longa `container|image|system …`. A única
falha — `same_behaviour "stats all"` — **NÃO é bug do produto**: verificado ao
vivo, `delonix stats` e `delonix container stats` são **byte-idênticos**
(partilham a struct `StatsArgs` e o mesmo handler, `bins/delonix/src/main.rs:217`
→ `cmd::container::run`). O FAIL é do próprio harness: `same_behaviour` compara
uma **amostra volátil** de `stats` (CPU/mem) e, com um container vivo no host
(o Odoo do dono) entre as duas chamadas, os valores divergem. → **MELHORIA
M-TEST-02** (não BUG). Com o host sem containers vivos, `diff` = idênticos.

### 2.2 Tier `differential` (docker × podman × delonix, motores reais)

`tests/cli-parity/run.sh differential` → **PASS**. Prova paridade comportamental
real, não só de parser:

- **Flow 1 — container interactivo:** os três motores devolvem resultados
  **idênticos** — `run --rm -it` (marker + `test -t 0` + exit 37), `ps -a`
  (`--rm` limpou), `run -i` sem TTY, stdout/stderr em streams separados,
  `stats` single-sample (sem stream).
- **Flow 2 — pod de dois membros:** `delonix` e `podman` OK (comms localhost,
  listar membros, inspect/describe, stop/start/restart, rm). `docker` omitido
  (não tem pods). Divergência **deliberada**: podman junta membros com
  `run --pod` imperativo; delonix é **declarativo-first** (`pod create -f`,
  `kind: Pod`) e devolve *erro accionável* ao `run --pod` imperativo.

> Cobertura: o `differential` só exercita 2 fluxos; volumes/networks/images
> ficam ao nível de reconhecimento de parser («por-testar»). → **GAP-0002**.

## 3. Conformância CRI (critest v1.36.0)

### 3.1 Rootless (baseline fresco do dono, mesmo commit) — 84/19

19 falhas: classificadas como 17 de ambiente/arquitectura-rootless + 1
divergência deliberada (seccomp `nil`) + 1 feature (PodPID).

### 3.2 Rootful-VM (KVM, root real) — 90/13

Subiu 84→90: *flipam* como previsto `mount rprivate/rslave`, `mount readonly`
não-recursivo e 2 dos 6 AppArmor. **Mas a projecção de ~104/104 NÃO se
confirma** — restam **13 falhas com root real**:

| # | Spec falhado em rootful-VM | Classe | Item |
|---|---|---|---|
| 1 | AppArmor — enforce `apparmor_profile` (blocking writes, field) | BUG | BUG-0003 |
| 2 | AppArmor — permissive deprecated profile | BUG | BUG-0003 |
| 3 | AppArmor — prefer new field, blocking writes | BUG | BUG-0003 |
| 4 | AppArmor — prefer new field, work with profile | BUG | BUG-0003 |
| 5 | Security Context HostNetwork = true | BUG | BUG-0004 |
| 6 | Security Context HostNetwork = false | BUG | BUG-0004 |
| 7 | Security Context HostIpc = true | BUG | BUG-0004 |
| 8 | Security Context HostIpc = false | BUG | BUG-0004 |
| 9 | Mount Propagation `rshared` (host↔container) | BUG | BUG-0005 |
| 10 | Container OOM — exitCode 137 / OOMKilled | BUG | BUG-0006 |
| 11 | Streaming port-forward in host network | GAP | GAP-0003 |
| 12 | Security Context PodPID (`shareProcessNamespace`) | GAP | GAP-0001 |
| 13 | SeccompProfilePath — `nil` = unconfined | Divergência deliberada | (flag conformance-only) |

**Correcção de narrativa (M1):** dos 13, **1** é divergência deliberada; os
outros **12 são trabalho real de engenharia** (AppArmor, host-namespaces, OOM,
rshared, PodPID), não «ambiente». O número honesto a publicar é
**rootless 84/19 · rootful-VM 90/13**.

## 4. Grupos não exercitados nesta ronda (honestidade de cobertura)

BLOCKED/SKIPPED com motivo: **VM/microVM, Storage (Ceph), Kubernetes, IaC,
GitOps, PaaS, MCP, Providers** — não re-testados aqui; o host estava saturado
com as cargas do dono e o âmbito desta ronda era paridade+CRI+instalação. O
baseline CRI rootless **não foi re-corrido** por ser determinístico no mesmo
commit e pela saturação do host (o dono corria o critest-rootful-VM em
simultâneo). Backlogs em `BUG_BACKLOG.md`, `GAP_BACKLOG.md`, `BACKLOG.md`.

## 5. Procedência da evidência

- Paridade fast/differential: corridas nesta sessão contra `target/release/delonix`
  (0.1.0 @ 1898466b) com docker/podman reais do host.
- CRI rootless/rootful-VM: baselines do dono de 2026-11-08 em
  `reports/cri-conformance-0.1.1/` (`00-baseline-develop.md`, `rootful-vm.out`),
  mesmo commit — validados e re-classificados aqui.
