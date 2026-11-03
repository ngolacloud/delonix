# G1f — rootful + SELinux-enforcing: avaliação e decisão de âmbito

**Data:** 2026-11-01. **Host:** Zorin OS 18.1 (base Ubuntu, LSM=AppArmor;
`/sys/fs/selinux` ausente). **Binário:** `target/release/delonix` @ feat/ipv6-phase1.
**Decisão:** ADR-0083.

Fecha o item (f) do portão G1 do plano NO-GO→GO: *«rootful e SELinux-enforcing
avaliados (ou declarados fora de âmbito por escrito)»*. Rootful foi **medido**
neste host (sudo sem password disponível); SELinux-enforcing foi **avaliado pelo
código** — este host não tem SELinux, pelo que uma medição real exigiria uma VM
Fedora/RHEL (não feita; ver ADR-0083 para a decisão de âmbito).

## Bateria rootful (medida, `sudo -E DELONIX_ROOT=<scratch>`)

| # | O que | Resultado |
|---|---|---|
| 1 | uid efectivo dentro do container | ✅ `uid=0(root) gid=0(root)` — root real, não userns |
| 2 | `/dev` por **mknod** (não bind), nós reais | ✅ `/dev/null` `crw-rw-rw- 1,3`, `/dev/zero` `1,5` |
| 3 | `--memory 64m` visível de dentro | ⚠️ `cat /sys/fs/cgroup/memory.max` → vazio de dentro do container; a cgroup do container fica em `system.slice` (root), não na vista exposta. **Aplicação do limite não confirmada de forma conclusiva** — carece de verificação dedicada (ler `memory.max` da cgroup do PID host-side) |
| 4 | **build `FROM scratch`** rootful | ❌ COPY+RUN correm, mas **packaging falha**: `DX-1410 image has no layers`. O rootless empacota o mesmo Dockerfile; é **específico de rootful+scratch** (o caminho de commit OCI rootful não lida com um base de zero layers mesmo depois de COPY/RUN acrescentarem conteúdo) |
| 4b | build rootful `FROM alpine:3.12` (base normal) | ✅ COPY+RUN+packaging ok (imagem criada) — isola o #4 como scratch-only |
| 5 | rede + `run -d`/`exec`/`rm -f` | ✅ container detached, `exec` com interface de rede, remoção limpa |
| 6 | `image ls` rootful | ✅ store rootful lista as imagens |

**Veredicto rootful:** o núcleo (root real, `/dev` por mknod, rede, ciclo de
vida, exec, store) **funciona**. Duas arestas registadas, nenhuma bloqueadora do
uso normal: **(#4)** `build FROM scratch` falha no packaging rootful — bug
distinto do caminho de commit (os três bugs de scratch corrigidos nesta linha
eram no arranque/rootless; este é no commit rootful); **(#3)** aplicação de
limite de memória rootful por confirmar com um teste dedicado.

## SELinux-enforcing (avaliado pelo código)

Este host não tem SELinux; a avaliação é do que o motor faz, não medida num
alvo enforcing.

- **Label de PROCESSO:** ✅ suportado. `apply_selinux` (delonix-linux `lib.rs:3388`)
  escreve o contexto em `/proc/thread-self/attr/exec` quando
  `selinux_active()` (`/sys/fs/selinux/enforce` existe) — a transição de domínio
  do processo do container (equivalente a `setexeccon`) é aplicada se o chamador
  passar um contexto.
- **RELABEL de VOLUMES (`:z`/`:Z`):** ❌ **não implementado**, e recusado
  fail-closed (`delonix-volume/src/lib.rs:1077-1081`, erro em `error.rs:95`). Um
  bind mount num host enforcing fica com o label do host; sem relabel para o
  domínio do container, o acesso é **negado** — o modo de falha típico em
  RHEL/Fedora enforcing.
- **Labeling de MOUNT (`context=`/`fscontext=`) e de ficheiro:** ❌ ausente.

**Veredicto SELinux:** parcial. O label de processo existe, mas sem relabel de
volumes nem labeling de mount um workload com binds não corre num host
enforcing. A peça em falta (relabel recursivo para o domínio do container,
via `libselinux`/`setfilecon` + `context=` por mount) é trabalho próprio, não um
ajuste — por isso **fora de âmbito para a linha 0.x** (ADR-0083), com o caminho
de suporte descrito lá.

## Reprodução

```
BIN=target/release/delonix
R=$(mktemp -d); sudo mkdir -p "$R"
# rootful core
sudo -E env DELONIX_ROOT="$R" $BIN container run --rm --net none alpine:3.12 id
sudo -E env DELONIX_ROOT="$R" $BIN container run --rm --net none alpine:3.12 \
  sh -c 'ls -l /dev/null /dev/zero'
# rootful FROM scratch (reproduz DX-1410 no packaging)
d=$(mktemp -d); sudo cp "$(command -v busybox)" "$d/busybox"
printf 'FROM scratch\nCOPY busybox /bin/sh\nRUN echo ok\n' > "$d/Dockerfile"
( cd "$d" && sudo -E env DELONIX_ROOT="$R" $BIN build -t rootscratch:1 . )
```
