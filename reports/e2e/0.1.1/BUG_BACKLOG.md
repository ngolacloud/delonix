# BUG_BACKLOG.md — 0.1.1 (@596663c6)

Defeitos de algo que deveria já funcionar. Reproduzidos ao vivo em host rootless,
2026-11-17 (ver `report.md`). Numeração continua a de 0.1.0 (último BUG-0006).

## Estado (2026-11-17) — os 3 RESOLVIDOS (não committado; entregar ao dono)

| Bug | Ficheiros | Prova ao vivo | Teste de regressão |
|-----|-----------|---------------|--------------------|
| **BUG-0007** ✅ | `dockerapi.rs` (resolve `Image.id`) | `curl /containers/json` → `ImageID: sha256:…`, `Image` mantém `repo:tag` | assert em `tests/compat/docker_api_smoke.py` (corre no CI; docker-py ausente no host local) |
| **BUG-0008** ✅ | `delonix-linux/src/lib.rs` (exec + init, `setgroups` incondicional) | `exec … id` → `uid=0(root) gid=0(root)`, sem 65534 | assert em `scripts/e2e.sh` (secção exec) |
| **BUG-0009** ✅ | correcção de RAIZ: `canonical_restart_policy` em `delonix-compute/domain/pod.rs` + carimbo em `bins/.../pod.rs` (reverti o penso em `reconcile.rs`) | `apply`+`drift` com `Always`/`OnFailure`/`Never` → zero drift | teste unitário `restart_policy_maps_k8s_to_canonical_and_passes_delonix_through` (verde) |

Compilação verde; **178 testes** passam em `delonix-compute` (124) + `delonix-stack` (54), zero regressões. Branch de trabalho: árvore `feat/init-templates-prod-hardening` (edits isolados dos ficheiros do dono), **não committado**.

## P1

### BUG-0007 — `ImageID` da Docker API devolve o nome `repo:tag` em vez do digest
- **Component:** `bins/delonix/src/cmd/dockerapi.rs:2777` (`"ImageID": c.image`, dentro de `containers_json()` :2765-2796). O digest real está em `delonix_oci::Image.id` (cf. `bins/delonix/src/cmd/image.rs:122`), resolvível via `state.images` (já usado em `dockerapi.rs:2898` para `image_size`).
- **Owner:** unassigned
- **Impact:** em `GET /containers/json`, `ImageID` sai igual a `Image` (ex. `"alpine:3.24"`), quando o Docker devolve sempre um digest `sha256:…`. O cliente `docker` oficial renderiza a coluna IMAGE truncada (`3.24`, `16`); ferramentas que correlacionam containers↔imagens por ImageID (Testcontainers, dashboards) partem.
- **Reproduction:** `delonix serve docker-api --addr unix:///tmp/d.sock &`; `curl -s --unix-socket /tmp/d.sock http://d/containers/json | jq '.[].ImageID'` → devolve `"alpine:3.24"` em vez de `sha256:…`.
- **Acceptance criteria:** `.ImageID` começa por `sha256:`; `Image` mantém `repo:tag`; `docker ps` via `DOCKER_HOST` mostra a coluna IMAGE completa. Teste de regressão no smoke docker-api (`tests/compat/`).
- **Related tests:** `tests/compat/docker_api_smoke.py` (estender para asserir o formato de ImageID).
- **Dependencies:** nenhuma. **Complexity:** XS. **Tier:** B (rootless).

### BUG-0008 — `exec`/init herdam os grupos suplementares do host → `groups=65534(nobody)` repetido
- **Component:** `crates/adapters/delonix-linux/src/lib.rs:10325-10332` (exec: `setgroups` só dentro de `if !container.group_add.is_empty()`) e `:4885-4893` (init do container, mesma lacuna). Contraste: o ramo com `--user` repõe sempre (`:10333-10342`, `:4803-4806`). gid_map em `write_userns_maps` `:3024-3083`.
- **Owner:** unassigned
- **Impact:** quando não há `--group-add`, o processo do container **herda os ~13 grupos suplementares do processo `delonix`** (host), não-mapeados no userns, que o kernel projecta todos para o overflow gid 65534. `id` dentro do container mostra `groups=65534(nobody),65534(nobody),…` ×13. É ruído de correcção **e** fuga de contexto de grupos do host para dentro do container (defesa-em-profundidade).
- **Reproduction:** `delonix run -d --name t alpine:3.24 sleep 300; delonix exec t id` → fila de `65534(nobody)`.
- **Acceptance criteria:** `exec … id` sem `--group-add` mostra `groups=` limpo (só o gid primário, ou vazio); nenhum 65534 herdado. Teste de regressão que corre `id` e asserir a ausência de 65534 repetido.
- **Related tests:** adicionar a `scripts/e2e.sh` (secção exec).
- **Dependencies:** nenhuma. **Complexity:** S. **Tier:** B (rootless).

### BUG-0009 — `drift` reporta falso-positivo por casing de `restartPolicy` (`Always` vs `always`)
- **Component:** normalização para minúsculas em `crates/contexts/delonix-compute/src/domain/pod.rs` / `launch.rs:189` (`should_restart`) vs comparação literal em `crates/contexts/delonix-stack/src/app/reconcile.rs:731` (`same_value`), consumida por `bins/delonix/src/cmd/drift.rs:113` (`rows_for_with`) e `diff.rs:34`.
- **Owner:** unassigned
- **Impact:** um manifesto com `restartPolicy: Always` (a forma **canónica Kubernetes**) gera drift permanente e espúrio: `LAST-APPLIED=Always` vs `OBSERVED=always`. Mina a confiança no `drift`/`diff` e faria `drift --correct`/GitOps re-aplicar sem necessidade.
- **Reproduction:** `apply` de um Pod com `restartPolicy: Always`, depois `delonix drift -f manifesto.yaml` → reporta drift em `restartPolicy` sem qualquer alteração ter ocorrido.
- **Acceptance criteria:** `apply` seguido de `drift` sobre um manifesto inalterado com `restartPolicy: Always` reporta **zero** drift. Normalizar valores de enum nos três lados (desired/last-applied/observed) antes de carimbar/comparar. Teste de regressão em `drift.rs` (tabela de casos de casing).
- **Related tests:** testes unitários de `drift`/`reconcile` (cf. `drift.rs:449`).
- **Dependencies:** nenhuma. **Complexity:** S. **Tier:** B (rootless).

## Nota de paridade (não-bug, documentar ou alinhar)
- Estado após SIGKILL é `Dead` (Docker: `Exited (137)`). O exit code normal **é** exposto (`describe` → `Status: Exited (N)`); só o caso morte-por-sinal diverge. Ver GAP_BACKLOG (menores).
