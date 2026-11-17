# GAP_BACKLOG.md — 0.1.1 (@596663c6)

Funcionalidade ausente/divergente — não defeito de algo que já funcionava.
Numeração continua a de 0.1.0 (último GAP-0017). Lentes: Docker 29.x, Podman, k8s.

---

## Secção A — Paridade Docker/CLI (P1)

### GAP-0018 — Remoção não-variádica (`network/volume/secret/rmi` aceitam 1 só nome)
- **Current behaviour:** `network rm` (`bins/delonix/src/cmd/network.rs:608`), `volume rm` (`volume.rs:704`), `secret rm` (`secret.rs:104`), `rmi`/`image rm` (`image.rs:244`) usam `name: String`. Só `rm` de container (`container.rs:1607`, `ids: Vec<String>`) é variádico.
- **Expected:** Docker/Podman aceitam N nomes em todos (`docker volume rm a b c`).
- **Why it matters:** fricção diária e quebra de scripts portados do Docker.
- **Acceptance criteria:** os 4 comandos aceitam `Vec<String>` e removem todos; erro parcial não aborta os restantes. (Padrão já existe em `image.rs:291`.) Teste differential em `tests/cli-parity/`.
- **Dependencies:** nenhuma. **Complexity:** S. **Tier:** B.

### GAP-0019 — `vm ls` aceita só `-A`, não `-a` (inconsistente com `ps -a`)
- **Current behaviour:** `bins/delonix/src/cmd/vm.rs:1097` — `#[arg(short = 'A', long)]`; `ps` usa `#[arg(short, long)]` (`container.rs:1552`) → `-a`.
- **Expected:** `-a`/`--all` uniforme (Docker/Podman).
- **Acceptance criteria:** `delonix vm ls -a` funciona; `--all` mantém-se. Teste de help/parse.
- **Dependencies:** nenhuma. **Complexity:** XS. **Tier:** B.

### GAP-0020 — Docker API não serve a família `/networks` (bloqueia Testcontainers/kind)
- **Current behaviour:** `delonix serve docker-api --matrix` = 21/28 servidas; as 7 recusadas são `GET/POST/DELETE /networks`, `/networks/create`, `/networks/{id}` (`bins/delonix/src/cmd/dockerapi.rs`). Funcionalidade existe em `delonix network`.
- **Expected:** clientes do ecossistema (Testcontainers, kind, SDKs) chamam `/networks`.
- **Why it matters:** é o que mantém a Docker API em "experimental" (cf. `system features`). Desbloqueia portabilidade real.
- **Acceptance criteria:** matriz passa a 28/28; smoke de Testcontainers cria/inspecciona/remove rede via a API. (Relaciona-se com o antigo GAP-0004.)
- **Dependencies:** nenhuma. **Complexity:** L. **Tier:** B.

### GAP-0021 — `stats` sem stream nem `--no-stream` (semântica invertida vs Docker)
- **Current behaviour:** `container.rs:1701` — `stats` é sempre one-shot; não existe `--no-stream` nem `--stream`. `docker stats` faz stream por omissão e `--no-stream` é o one-shot.
- **Acceptance criteria:** aceitar `--no-stream` (no-op, compat de hábito) e, idealmente, `--stream` para amostragem contínua. Teste de parse.
- **Dependencies:** nenhuma. **Complexity:** S (no-op) / M (stream real). **Tier:** B.

### GAP-0022 — `logs --tail/--since/--timestamps` exigem o container corrido com `--log-cri`
- **Current behaviour:** verificação em runtime (`container.rs:7924-7929`, disparada em `:7964`), não clap. Sem `--log-cri` (`:1538`), as 3 flags falham com DX-1000.
- **Expected:** no Docker funcionam sempre.
- **Why it matters:** atrito grande — o utilizador tem de prever no `run` que vai querer `--tail` depois.
- **Acceptance criteria:** tornar o formato com timestamp por-linha o default de captura (ou gravar o timestamp lateral sempre), de modo a que `--tail/--since/--timestamps` funcionem em qualquer container. Teste e2e em logs.
- **Dependencies:** decisão de formato de log default. **Complexity:** M. **Tier:** B.

### GAP-0023 — `Pod.restartPolicy` default é `no` (Kubernetes usa `Always`)
- **Current behaviour:** `crates/contexts/delonix-compute/src/domain/pod.rs:16` (`default_restart() -> "no"`). Um Pod declarado sem política não auto-reinicia; um Pod k8s sim.
- **Expected:** paridade k8s (default `Always`), ou divergência documentada explicitamente.
- **Why it matters:** expectativa de quem vem de k8s; liga-se a GAP-0025 (liveness declarativa).
- **Acceptance criteria:** decisão registada (ADR) + default alinhado ou doc. Teste que fixa o default escolhido.
- **Dependencies:** acopla a GAP-0025. **Complexity:** S (+ decisão). **Tier:** B.

### GAP-0024 — Menores de paridade (agrupados)
- `compose -f` só por subcomando (`compose.rs:456-532`), não global como `docker compose -f x up`.
- `config set` exige `KEY VALUE`; aceitar `key=value` (`config.rs:81`).
- `pod create` só declarativo (`pod.rs:120`); falta forma imperativa `--image/--name` (podman).
- `volume inspect` mostra `created: unix=<epoch>` — data legível.
- Estado `Dead` após SIGKILL → mapear para `Exited (137)` (paridade; ver BUG_BACKLOG nota).
- **Complexity:** XS cada. **Tier:** B.

---

## Secção B — Arquitectura (P1/P2)

### GAP-0025 — Build/commit achatam layers em rootless: sem partilha de base, amplificação de disco
- **Current behaviour:** bifurcação por `runtime::is_rootless()` em `bins/delonix/src/cmd/build.rs:1357` e `container.rs:~6425`. Via rootless → `commit_flat_rootfs` (`crates/adapters/delonix-oci/src/build.rs:1115-1180`, `diff_ids` de 1 elemento `:1143`, `layers: vec![layer]` `:1161`) empacota o rootfs **inteiro** como 1 layer. Via root/overlay → `commit_upper` + `build_image` (`:868`, `base.layers.clone(); push(delta)` `:876`) empilha correctamente. Formato interno **já suporta** multi-layer (`Image.layers: Vec<String>`, `image.rs:115`); CAS deduplica por digest (`cas.rs:69`). Cache de build rootless (snapshot-chain) existe mas só para cache (`build.rs:616`).
- **Evidência medida:** 2 imagens `FROM alpine:3.24` → 3 digests de layer distintos (`040a…`, `853b…`, base `881f…`): zero partilha. `system df`: `images 12.2 GiB / layers ≥20.3 GiB`.
- **Why it matters:** cada imagem carrega uma cópia completa e achatada da base → amplificação de disco (crítico com 3 GiB livres), push/pull sem reutilização de layers comuns, cache de registo ineficiente. É o risco de densidade/produção nº1.
- **Acceptance criteria:** em rootless, `history` de `FROM alpine + RUN` mostra ≥2 layers; o digest da layer-base **iguala** o do `alpine:3.24`; duas imagens da mesma base partilham o blob da base no CAS. Caminho sugerido: capturar um diff por step a partir da snapshot-chain já calculada, ou `fuse-overlayfs` como snapshotter, em vez de `pack_rootfs_tar` do rootfs completo (hoje deita fora a lineage, cf. `build.rs:1470`).
- **Dependencies:** nenhuma (formato já suporta). **Complexity:** XL. **Tier:** B.

### GAP-0026 — Camada declarativa cega ao estado de runtime (sem reconciliação de liveness)
- **Current behaviour:** `apply`/`plan`/`diff`/`drift` comparam **só configuração carimbada**. `Facts.running` (`crates/contexts/delonix-stack/src/app/reconcile.rs:663`) só modula Reboot-vs-Update, nunca aciona restart. Pod idempotente por *presença* (`bins/delonix/src/cmd/pod.rs:439-459`: "already exists, nothing to do" não lê Status); container idem (`container.rs:2634`). `drift` compara stamp vs observado de config (`drift.rs:113`, frase `:302`); `diff` OBSERVED=`a.fields` de config (`diff.rs:34`). Não há control-loop declarativo. O self-healing existe mas desligado: supervisor por-container (`crates/contexts/delonix-compute/src/domain/launch.rs:189` `should_restart`, `:149` `start`), `health_monitor_loop` (`container.rs:8553`), `rm_watcher` (`container.rs:4387`), detecção de morte read-only (`reconcile_status`, `container.rs:4334`), persistência de boot via systemd (`cmd/boot.rs`).
- **Evidência medida:** matar o container de um Pod declarado → `0/1 Stopped`; `drift` diz "no drift"; `diff` DESIRED=OBSERVED; re-`apply` "already exists, nothing to do" (não reinicia). COM `restartPolicy: Always` o supervisor recupera (Stopped→Running ~3 s) — logo o gap é de **observação/reconciliação declarativa**, não de ausência de self-healing.
- **Acceptance criteria:** (a) `drift`/`diff` passam a reportar um membro de Pod/stack caído (Running/Stopped como facto comparável); (b) opcionalmente, um "ensure-running" (generalização do `rm_watcher`/supervisor, independente do parentage) que, governado por política, reinicie o que foi declarado. Teste: matar container de Pod declarado → `drift` reporta-o e a reconciliação recupera-o.
- **Dependencies:** acopla a GAP-0023 (default restartPolicy). **Complexity:** XL. **Tier:** B.

### GAP-0027 — `build-cache` (17 GiB) sem GC; `system prune` não o toca
- **Current behaviour:** `system prune` remove containers/imagens/blobs CAS/cgroups/directórios órfãos, mas **não** o build-cache; só apagando `~/.local/share/delonix/build-cache` à mão (confirmado por `system df`).
- **Why it matters:** é o maior consumidor de disco (17 GiB medidos) e não tem recuperação por comando.
- **Acceptance criteria:** `delonix system prune --build-cache` (ou `delonix build prune`) recupera o cache por LRU/idade, com `--auto`/`--threshold` para timer. Teste que cria cache, corre o GC e mede a recuperação.
- **Dependencies:** nenhuma. **Complexity:** M. **Tier:** B.
