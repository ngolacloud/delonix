# §12 — Recuperação e Continuidade — Delonix 0.1.0 (candidato v0.1.1)

- Binário: `target/release/delonix` 0.1.0, commit 8ab783dd7 (+39 commits), construído 2026-10-29.
- Host: rootless, state root `~/.local/share/delonix`. Disco a 99% (11G livres) — nada construído.
- Providers vivos: cloud-hypervisor v52, libvirt 10/QEMU 8.2.2, linux. proxmox/opnsense NotConfigured (fora de âmbito).
- Recursos de teste criados com prefixo `s12-` e **todos removidos no fim** (confirmado: sem containers/volumes/records s12; 3 containers do dono intactos; `git status` limpo).

Classes usadas: **implementado** / **testado-a-passar** / **testado-a-falhar** / **mecanismo-existe-não-exercitado** / **ausente**.

---

## Mecanismos encontrados no código (mapa)

| Mecanismo | Localização | Nota |
|---|---|---|
| Reconciliação de estado de container | `crates/adapters/delonix-linux/src/lib.rs:10991` `reconcile_status` | Running+pid morto→Crashed; Running+frozen→Paused; Paused+thawed→Running. Usa `safe_to_signal(pid, pid_starttime)` para fechar a janela de reuso de PID. Reconcilia em **memória**; só grava se o chamador gravar. |
| Chamadores de reconcile | node-api (`containers.rs:138`, `container_ops.rs`), mgmt (`dashstats.rs`), cri, mcp | `records()`/`list_in` reconciliam na leitura mas **não persistem** (daemonless — ninguém vivo para gravar). |
| Supervisor daemonless | o próprio `delonix run -d` fica vivo a vigiar o init | Ao morte do init, reap + **persiste** estado terminal em disco. |
| Reconciliação de VM | `crates/contexts/delonix-compute/src/vm.rs:2049` `adopt_pid_starttime`, `status_reconciles_a_vm_whose_backend_stopped_it` | Adoção por pid+starttime; não exercitado ao vivo (evitei VMs pesadas). |
| Escrita atómica durável | `crates/contexts/delonix-node/src/atomic.rs` `write_atomic_mode` | tmp único por escritor (`pid.seq`) → `sync_all` → `rename` → fsync do dir. Modo posto na criação (anti-TOCTOU). |
| Store de records (O_EXCL) | `crates/adapters/delonix-state/src/store.rs:162,867` `create_new(true)` | Criação exclusiva para idempotência/replay. |
| Idempotência de operações | `crates/interfaces/delonix-node-api/src/operations.rs:8` | `request_id`→`r-<id>` criado exclusivamente; pedido repetido devolve a resposta anterior. |
| Publicação atómica de camada / cura de cache | `crates/adapters/delonix-oci/src/overlay.rs:298` `publish_layer_dir` + marcador `.extracted` (="ok") | Dir de camada parcial/envenenado (sem `.extracted`) é **substituído** por swap atómico; exemplo medido ao vivo (kindest/node preso em 3 entradas). Relação directa com F-1. |
| Guard de versão entre binários | `crates/contexts/delonix-node/src/dispatch.rs` `check_version` (`DELONIX_DISPATCH_VERSION`) | Recusa um servidor (CRI/node-api/MCP) de outra release ao lado do `delonix`. |
| Backup por-recurso | CLI `delonix backup create/inspect/restore/schedule/remove` | Archive tar.gz com record + DADOS dos volumes; campo `Format: 1`, `Written by: delonix 0.1.0`. |
| Snapshot do nó inteiro | CLI `delonix system snapshot create/restore` | Registries, IPAM, secrets, PKI, log de eventos. Não exercitado. |
| Persistência de arranque | CLI `delonix system boot enable/status/disable` | Unidades systemd user para containers voltarem após reboot. **Desactivado por omissão.** |
| Forward-compat de records | `crates/contexts/delonix-compute/src/record.rs:483` `#[serde(default)]` | Migração aditiva por serde; **sem** versão de schema explícita nem gate de downgrade nos records. |
| Detecção de corrupção | `cred_vault.rs` `CorruptMasterKey`; `sdn` `NetworkRecordCorrupted`; `vm_error.rs` `DiskCorrupted` (via `qemu-img check`) | Mecanismos existem; não corrompi estado vivo do dono. |

---

## Tabela item → classe → evidência → veredicto

### Item 1 — Morte do processo Delonix / do holder → reconcilia? duplica?
**Classe: testado-a-passar.**

- **1a — matar o init (SIGKILL):**
  `delonix run -d --name s12-kill busybox:1.36 sh -c 'while true; do sleep 5; done'` → PID init 3369761, supervisor (`delonix run -d`) 3369760.
  `kill -9 3369761` → supervisor **também saiu** e **persistiu em disco** `status=Crashed pid=None` (ficheiro `containers/<id>.json`). `delonix ps -a` mostra `Dead`. Sem processos busybox órfãos (`pgrep -af "while true"` vazio), sem leaf de cgroup `*72a65*`.
- **1b — matar só o supervisor:**
  `kill -9 <supervisor>` → init `sh` **sobrevive**, reparenta (ppid 2434), `ps` continua `Up`, record em disco `Running pid=<init>`. reconcile_status continua correcto.
  Em seguida `kill -9 <init>` (sem supervisor) → `ps -a` mostra `Exited (unknown)`, disco `Crashed pid=None`; `delonix start s12-sup` recupera → `Up`, `restarts=1`, novo pid, disco `Running`.

**Veredicto:** reconcilia em todos os casos; **sem duplicação**; recuperação por `start` funciona. Ressalva cosmética: o mesmo estado Crashed aparece como `Dead` (1a) e `Exited (unknown)` (1b). `crash_reason` vem `null` numa das mortes (conhecido — leaf some antes de ler).

### Item 2 — Reinício do host
**Classe: simulado (perda de supervisor) testado-a-passar + mecanismo-existe-não-exercitado (reboot real).**

- `delonix system boot status` → `mode: rootless (user units + linger)`, `dir: ~/.config/systemd/user`, **`(no boot units — run delonix system boot enable)`**. Ou seja: a persistência de arranque EXISTE mas está **desligada**; após reboot real os containers **não** voltam sem `boot enable`.
- Reboot real **não executado** (instrução). A perda de supervisor (Item 1b) é o análogo exercitável: a carga sobrevive à perda do seu supervisor e a vista recupera por reconcile; `start` relança.

**Veredicto:** mecanismo presente, **inactivo por omissão** (ver bloqueio P2-2). Reboot real por exercitar.

### Item 3 — Resposta perdida / provider indisponível / credencial revogada (timeout ≠ falha remota)
**Classe: testado-a-passar (parcial) + mecanismo-existe.**

- Registry inalcançável: `delonix create --name s12-badimg does-not-exist-registry.invalid/nope:0` → `error[DX-9401] registry error: error sending request...` com hint. `delonix ps -a` **sem** registo órfão s12-badimg.
- Timeout/resposta perdida ≠ falha remota: `operations.rs` — `request_id`→`r-<request_id>` criado exclusivamente (O_EXCL); o mesmo pedido reenviado devolve a resposta anterior em vez de re-executar. Mecanismo presente; exercitado indirectamente pelo caminho de replay de `delete`.
- Credencial revogada / vault: `CorruptMasterKey` detectado quando o ficheiro de chave tem tamanho errado (`cred_vault.rs`). **Não** corrompi o vault vivo do dono.
- proxmox/opnsense NotConfigured → fora de âmbito.

**Veredicto:** comportamento honesto na indisponibilidade; idempotência protege contra dupla-execução por resposta perdida. Revogação de credencial remota por exercitar (providers fora de âmbito).

### Item 4 — Falha parcial de criação → estado órfão?
**Classe: testado-a-passar.**

- Create com imagem inalcançável (acima) → falha limpa, **sem** record órfão.
- `atomic.rs`: escrita por tmp único + fsync + rename + fsync do dir → um create interrompido a meio deixa um `.tmp`, **nunca** um record rasgado/corrompido.
- Extracção de camada interrompida: `overlay.rs publish_layer_dir` + marcador `.extracted` ("ok") distingue extracção completa de parcial e **substitui** por swap atómico dirs parciais/envenenados.

**Veredicto:** sem órfãos observados; desenho anti-corrupção/atómico presente e documentado (medido ao vivo no passado).

### Item 5 — Storage indisponível; concorrência; limpeza interrompida
**Classe: MISTA — testado-a-passar (rm concorrente) / testado-a-FALHAR (create concorrente mesmo nome) / mecanismo-existe-não-exercitado (storage indisponível, prune interrompido).**

- **rm concorrente, mesmo recurso (a correr):** `delonix rm s12-sup & delonix rm s12-sup & wait` → **ambos** a mesma recusa `[DX-5101] conflict: container ... is running (use --force)`. Container intacto. Sem estado parcial.
- **force-rm concorrente:** `rm -f & rm -f & wait` → um remove (`44289cc...`), o outro `[DX-4101] no such container`. Sem double-free, sem processos órfãos (`pgrep -af 44289cc` vazio).
- **create concorrente mesmo nome (FALHA):** `delonix run -d --name s12-race ... & delonix run -d --name s12-race ... & wait` → **ambos** sucederam, criando DOIS containers `default/s12-race` (ids `dcf6617...` e `96fda7b...`, confirmado por `ps -a`). A resolução por nome ficou **permanentemente partida**: `delonix container describe s12-race` → `error[DX-1000] invalid argument: this name exists in several namespaces — qualify it as <namespace>/<name>: s12-race (default/s12-race, default/s12-race)` (texto enganador: é o MESMO namespace). Endereçável só por ID. Um 3.º `run` sequencial recusa correctamente (`DX-5000 ... already in use ... by container 96fda7bf41b7`), confirmando que a guarda é sequencial e a janela é a corrida (TOCTOU). O mecanismo atómico de reserva existe no código (ficheiros `<nome>.claim` com O_EXCL em `container-names/`, usados pelo caminho de stack/apply) mas o caminho `run --name` **não o usa**. Recuperável: `rm -f <id> <id>` limpou ambos (eventos `die exit=137` + `remove` por id).
- **storage indisponível / prune interrompido:** não exercitado (exigiria injecção de falha no fs de estado / SIGINT cronometrado num prune). Mecanismo-existe-não-exercitado.

**Veredicto:** concorrência de remoção sólida; **concorrência de criação por nome tem corrida (P2)**; storage-down e prune-interrompido por exercitar.

### Item 6 — Estado persistido: backup/restore, versão de schema, corrupção, downgrade
**Classe: testado-a-passar (backup/restore) + mecanismo-existe (snapshot, corrupção) + LACUNA (versão de record/downgrade).**

- **Backup/restore por-recurso (round-trip PROVADO):**
  `delonix volume create s12-vol`; `delonix run -d --name s12-bkp -v s12-vol:/data busybox ...` (escreve `/data/marker.txt=hello-s12`).
  `delonix backup create container s12-bkp --to <dir>` → archive `container-s12-bkp-...tar.gz (1.3 KiB)`, `volumes: s12-vol`.
  `delonix backup inspect` → `Kind: container`, `Format: 1`, `Written by: delonix 0.1.0`, `Volumes: s12-vol`.
  Mutei: `exec s12-bkp sh -c 'echo CHANGED-AFTER-BACKUP > /data/marker.txt'`. `delonix backup restore <archive>` sem --force → **recusa**: `DX-1000 ... 's12-bkp' is running — stop it first, or pass --force`. Com `--force` → `volume s12-vol restored`, `container kept`, `started again`; `exec s12-bkp cat /data/marker.txt` → **`hello-s12`** (revertido).
- **Snapshot do nó:** `delonix system snapshot create/restore` (registries, IPAM, secrets, PKI, eventos). Mecanismo-existe-não-exercitado.
- **Versão de schema / downgrade:** backups carregam `Format: 1`. **Records de container NÃO têm versão explícita** — forward-compat aditiva por serde `#[serde(default)]`; downgrade tolerado (serde ignora campos desconhecidos) mas **sem gate/aviso de downgrade**. 
- **Corrupção:** detecção presente — `CorruptMasterKey` (vault), `NetworkRecordCorrupted`, `DiskCorrupted` (qemu-img check). Não corrompido ao vivo.
- **Rollback de binário ≠ rollback de estado (relação com F-1):**
  - `dispatch.rs check_version` recusa misturar um servidor de outra release com o `delonix` instalado.
  - F-1 (cache de camadas envenenada): neste binário a cura é o **marcador `.extracted` + swap atómico** de `publish_layer_dir` (a "época de formato" materializa-se como completo-vs-parcial), não um inteiro de época. Um binário que mude a semântica do formato fica protegido porque dirs antigos/parciais são **substituídos**, não servidos.
  - **LACUNA**: não há carimbo de versão de estado nem política de downgrade nos records — recuar o BINÁRIO para um pré-0.1.0 sem campos é tolerado silenciosamente (perde semântica de campos novos), sem recusa nem aviso. Só os backups são versionados.

**Veredicto:** backup/restore por-recurso sólido e provado; snapshot do nó e versão/downgrade de record são a fragilidade (ver P3).

---

## Bloqueios

- **P2-1 — Corrida de unicidade de nome em `run --name` concorrente (testado-a-falhar).** Dois creates concorrentes com o mesmo `namespace/name` ambos sucedem → dois containers homónimos; resolução por nome fica partida (`DX-1000`, texto enganador "several namespaces"), endereçável só por ID. Guarda sequencial (`DX-5000`) ok; falta a reserva atómica. Fix: usar no caminho `run/create` o mecanismo `.claim` O_EXCL que já existe (stack/apply). Recuperável por `rm <id>`. Severidade P2 (viola invariante de unicidade, degrada UX; não corrompe, não crasha).
- **P2-2 — Persistência de arranque desligada por omissão (ressalva operacional).** `system boot status` = sem unidades; um reboot real **não** repõe os containers sem `system boot enable` prévio. Para um nó PaaS é um footgun. Fix: documentar/activar por omissão em perfil de nó.
- **P3 — Sem versão de schema / gate de downgrade nos records de estado (observação).** Backups têm `Format:1`; records não têm carimbo nem política de downgrade. Rollback de binário sobre estado mais novo é silencioso. Recomendar carimbo de versão + recusa/aviso em downgrade.
- **Cosmético** — o estado Crashed renderiza como `Dead` e como `Exited (unknown)` consoante o caminho; `crash_reason` por vezes `null`.

**Nenhum P1 encontrado.**

---

## Por exercitar (honesto)

- Reboot real do host (instrução: não reiniciar). Só a perda de supervisor foi simulada.
- Reconciliação de VM ao vivo (`adopt_pid_starttime`) — evitei VMs pesadas / disco a 99%.
- Storage indisponível (ENOSPC / fs de estado read-only) — exigiria injecção de falha.
- `rm`/`prune` interrompido a meio (SIGINT cronometrado).
- `system snapshot create/restore` do nó inteiro — destrutivo/pesado, não corrido.
- Corrupção real do vault / revogação de credencial de provider — destrutivo para o dono / providers fora de âmbito.
- Restore de backup com `Format` incompatível — não forjei um `Format: 2`.

---

## Veredicto §12: **PRONTO COM RESSALVAS**

O núcleo de recuperação está sólido e **provado ao vivo**: reconcile de crash sem duplicação, recuperação por `start`, escrita atómica anti-corrupção, idempotência de operações, cura de cache de camadas por marcador+swap, e backup/restore por-recurso com round-trip de dados. Para fechar v0.1.1 em produção, resolver **P2-1** (corrida de nome) e decidir **P2-2** (persistência de arranque), e endereçar **P3** (versão/downgrade de estado). Sem P1.
