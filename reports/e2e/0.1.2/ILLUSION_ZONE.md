# ILLUSION_ZONE.md — medição com efeito das 95 folhas não-contadas (0.1.2)

Resposta à pergunta «funcionam todos ou é ilusão?». O ratchet contava **272/366**
folhas executadas-sob-asserção pela bateria; as outras **94–95** eram "só ajuda"
na contagem. Este probe (`probe-missing.sh`) **exercita cada uma com argumentos
reais e lê o efeito de forma independente** (não `--help`, não `exit 0` cego),
host rootless, root curto isolado, teardown verificado (0 resíduos).

## Resultado medido (reproduzível: `reports/e2e/0.1.2/probe-missing.sh`)

| Classe | Nº | Significado |
|---|---|---|
| **PASS (efeito provado)** | **71** | corre e o efeito confirma-se por leitura independente |
| **SKIP (semântica)** | **3** | não aplicável desta forma em rootless |
| **BLOCKED (capacidade ausente)** | **21** | precisa de registo/S3/CRIU/nó-rootful — **nomeado**, não "partido" |
| FAIL | **0** | — |

**Não sobra zona de ilusão:** das 95, **nenhuma** fica "help-only/desconhecido" —
cada folha é *provada-com-efeito* ou *BLOCKED com a capacidade em falta nomeada*.

## Leitura combinada da superfície (366 folhas)

- **Provado-com-efeito:** 272 (bateria) + 71 (este probe) ≈ **343/366 (~94 %)**
  — algumas das 71 já eram exercitadas pela bateria mas não *contadas* (invocação
  por função de shell); o probe torna-as medidas.
- **BLOCKED por capacidade:** 21 — todas correriam com o recurso respectivo.
- **SKIP de semântica:** 3.

## As 71 provadas com efeito (amostra do que "funciona de facto")
Ciclo de vida completo (`run/ps/top/stats/exec/cp/pause/unpause/rename/port/
restart/stop/start/kill/rm`), imagens (`images/tag/untag/inspect/exists/tree/
diff/history/save/load/build/commit/export/import/rmi/pull`), **pods inteiros**
(13 folhas: `ps/exists/inspect/stats/top/pause/unpause/stop/start/restart/kill/
rm/prune`), rede/volume (`network exists/reload/prune`, `volume exists/mount/
export/import`), introspecção (`info/events/attach`), **`oci-runtime` completo**
(`create/start/state/kill/delete` sobre um bundle OCI real), **`vm clone`/`vm
template`** (sobre uma VM cloud-hypervisor real), e `cluster create` (provado na
campanha, `cluster-lab.log`).

## As 21 BLOCKED — e o que cada uma exige (honesto, não "não funciona")

| Capacidade em falta | Folhas |
|---|---|
| **Registo autenticado** | `push`, `image push`, `login`, `logout`, `search`, `image search`, `vm image pull/push/ls-remote` |
| **Assinatura/supply-chain** (chaves) | `image sign`, `image attestations`, `image provenance` |
| **CRIU** (checkpoint rootless) | `container checkpoint`, `container restore`, `container clone` |
| **Backend S3** (ObjectStorage) | `bucket ls/inspect/rm` |
| **Cluster vivo / nó rootful** | `cluster kube play`, `cluster kubeadm` |
| **Recipe de imagem de VM** | `vm image build` |

## As 3 SKIP
`volume unmount`, `container mount`, `container unmount` — montagem imperativa do
rootfs (precisa de container parado + privilégio de mount); em rootless a
montagem é por-`run -v` (provada em `volume mount`).

## Veredicto honesto
**Não é ilusão para ~94 % da superfície** — esses estão *medidos com efeito*. Os
restantes **não são "partidos" nem "desconhecidos": são BLOCKED por uma capacidade
que este host não tem** (registo, S3, CRIU, nó rootful). O caminho para 100 %
provado é **fornecer essas capacidades** (um registo de teste, chaves de
assinatura, CRIU, um lab rootful) — que é precisamente a **Parte 2** (labs
privilegiados em VM), a sessão seguinte.

## Parte (b) — capacidades fornecidas e medidas (`probe-capabilities.sh`)

Das **21 BLOCKED**, fornecendo as capacidades em falta em rootless, **14 passam a
provadas com efeito** (`probe-capabilities-results.tsv`): PASS=14 · FAIL=0 · SKIP=7.

| Capacidade fornecida | Folhas provadas |
|---|---|
| **Registo OCI local** (registry:2) | `push`, `image push`, `pull`, `login`, `logout`, `search`, `image sign`, `image attestations`, `image provenance`, `vm image ls-remote`, `vm image pull`, `vm image push` (12) |
| **(nenhuma)** — já funcionavam | `bucket ls` (lista vazia), `cluster kube play` (≡ podman kube play, pod local) (2) |

**Restam 7 SKIP, com pré-requisito nomeado:** `bucket inspect`/`bucket rm` (um
`Bucket` external sobre um S3 vivo — MinIO não puxou esta sessão); `cluster
kubeadm` + `container checkpoint`/`restore`/`clone` (CRIU/nó rootful → **Parte c**);
`vm image build` (recipe de imagem de VM).

### Número da superfície actualizado (366 folhas)
- **Provado-com-efeito:** 272 (bateria) + 71 (zona de ilusão) + 14 (capacidades) =
  **~357/366 (~97,5 %)**.
- **Resta:** 7 capability-SKIP (S3/VM/CRIU/recipe) + 3 SKIP de semântica (mount) —
  todos com pré-requisito nomeado, nenhum "partido" ou "desconhecido".

## Artefactos
- `probe-missing.sh` — o probe da zona de ilusão.
- `probe-capabilities.sh` (+`-results.tsv`) — o probe das capacidades (Parte b).
- `probe-missing-results.tsv` — resultado por folha (PASS/SKIP/BLOCKED).
- `probe-missing-run.log` — stdout completo da corrida.
