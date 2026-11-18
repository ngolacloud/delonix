# COVERAGE_MATRIX.md — cobertura da superfície pública `delonix 0.1.2`

Cobertura separada por eixo (secção 5 do prompt mestre), contra o binário
entregue `0.1.2 @ b72edeed`. Fonte: `cli-tree.sh` (inventário), `e2e.sh`
(execução, `e2e-results.jsonl`), `cli_exec_ratchet.py` (execução sob asserção).

## Eixo 1 — caminhos canónicos (folhas invocáveis)

| Métrica | Total | Nota |
|---|---|---|
| Descobertos (parser real) | **366 folhas** / 413 comandos / 78 grupos | `cli-tree.sh --count` |
| Aplicáveis (perfil rootless+lab) | 366 | todos os grupos são alcançáveis no host |
| Com destino mapeado (gate) | **366** | `cli-tree.sh --gate` verde; 0 folhas órfãs |
| Executados sob asserção | **272 (74,3 %)** | `cli_exec_ratchet.py`, == baseline, `rc=0` |
| Ajuda verificada (`--help`) | **366 (100 %)** | a bateria checa o `--help` de cada folha por construção |
| Aprovados (perfil rootless) | ver §4 | 1735 PASS / 22 FAIL(triadas) / 35 SKIP |

> **Ressalva do numerador (documentada no arnês):** o ratchet **sub-conta** —
> uma folha invocada por dentro de uma função de shell não resolve a folha. Por
> isso `exec`, `logs`, `kill`, `cp`, `port` aparecem a 0 % embora a bateria os
> exercite (ex.: a regressão BUG-0008 corre `exec … id` e passa). O 74,3 % é
> piso, não tecto.

## Eixo 2 — cobertura de execução por grupo (top)

| Grupo | exec/folhas | % | Grupo | exec/folhas | % |
|---|---|---|---|---|---|
| net | 36/36 | 100 | vm | 33/39 | 84.6 |
| system | 20/20 | 100 | cluster | 19/22 | 86.4 |
| stack | 11/11 | 100 | network | 18/21 | 85.7 |
| secret | 9/9 | 100 | container | 27/35 | 77.1 |
| backup | 6/6 | 100 | manifest | 3/4 | 75.0 |
| provider | 6/6 | 100 | volume | 11/16 | 68.8 |
| systemcontainer | 6/6 | 100 | image | 16/28 | 57.1 |
| compose/gitops/serve/workload/mcp/config | 100 | | pod | 6/19 | 31.6† |
| oci-runtime | 0/5 | 0‡ | bucket | 0/3 | 0‡ |

† `pod`/`image` baixos: muitas folhas exercitadas por helper de shell (sub-contagem).
‡ `oci-runtime`/`bucket`: exercitados por `--help` mas sem execução com efeito
asserida no perfil rootless (oci-runtime exige bundle; bucket exige backend S3).

## Eixo 3 — contratos funcionais por família (secção 2 do prompt)

| Família | Cenários-chave exercitados | Veredicto rootless |
|---|---|---|
| Contentores/pods/OCI | ciclo de vida id+nome, sinais, TTY, exec, cp, logs, port, freezer, pod netns partilhada | ✅ PASS |
| VM / compute híbrido | CH create/start/snapshot/resize, inventário conjunto, namespaces | ✅ PASS (root curto) |
| Imagens/registry/segredos | build FROM+RUN, cache, digest, save/load, commit, login, secret cifrado | ✅ PASS |
| Rede/armazenamento | porta publicada, DNS interno, L3, IPAM multi-homed, volume bytes/mount, hosts | ✅ PASS |
| Kubernetes/protocolos | **cluster create→Ready (kind-mode)**, serve docker-api/cri, mcp negociação | ✅ PASS (KaaS); CRI-root ⛔ |
| IaC/workflows | plan/apply/diff/drift/delete idempotentes, GitOps, compose, wait, 33 Kinds | ✅ PASS |
| Operação/experiência | backup/restore, completion, man, dashboard(PTY), init, policy, version, events | ✅ PASS |

## Eixo 4 — aliases e parâmetros/variantes

- **Aliases:** `ps`/`container ls`, `images`/`image ls`, etc. — cobertos no
  inventário (classes do `cli-tree`); `ls -q/-a` tinha divergência (GAP-0019),
  **fechada pelo dono em `9ec12592`** durante a ronda.
- **Parâmetros:** casos positivo/negativo, default-quando-omitido, rejeição e
  limites são exercitados pela bateria (ex.: `--restart always:2` recusado;
  `--disk-size 0` recusado no parser; enums de `restartPolicy`
  Always/OnFailure/Never normalizados). Variantes condicionadas por build não se
  aplicam (um só artefacto de release).

## Eixo 5 — perfis (governação de gate, H07)

| Perfil | Descoberto | Aplicável | Mapeado | Executado | Aprovado |
|---|---|---|---|---|---|
| **pr-rootless** | 366 | 366 | 366 | 272 (+help 100%) | ✅ gates de merge APROVAM |
| **lab-ephemeral** | — | VM/KaaS/CRI/rootful | — | VM-CH ✅, KaaS ✅ | ◑ critest/rootful BLOCKED |
| **release** | — | critest/chaos/cargo-test | — | — | ⛔ fora de âmbito (sem tag) |

Evidência estruturada: `evidence.jsonl` (6 registos, validam contra
`docs/dev/evidence-schema.md`).
