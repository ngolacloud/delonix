# QA re-verificação v0.1.1 — DEF-003 / OOM / CRI

- Data: 2026-10-31
- Binário sob teste: `/home/walter/dlx-wt/stabilize-0.1.1/target/debug/delonix` → `delonix 0.1.0`, commit `b9925e06e (+52 commits since v0.1.0)`, built 2026-10-31. (NÃO foi construído nada.)
- Root isolado: `DELONIX_ROOT=/home/walter/dlx-wt/qa-def` (criado para o teste; containers do dono no root por omissão intocados).
- Host: Linux 7.0, cgroup2 com memory/cpu/pids delegados (io/cpuset não). alpine:3.20 puxado para o root isolado (pull ~1s, 3.5 MiB).

## Veredicto resumido

| Defeito | Repro (comando exacto) | Observado | Veredicto |
|---|---|---|---|
| **DEF-003** `compose up` não reconcilia serviço parado | `delonix compose up -d` (2 serviços alpine, rede bridge do projeto) | `compose up -d` **pendura** após "network ... already exists" e **nunca cria os containers** (timeout 120s, exit 124). Mesma falha com `container run --net <bridge>` (exit 124). `--net host` funciona. O passo "stop + re-up" nunca é alcançável. | **BLOQUEADO-POR-AMBIENTE** (networking bridge/ingress rootless congestionado no host) |
| **OOM** limite de memória efectivo + kill | `delonix container run -d --memory 32M alpine sh -c 'tail /dev/zero'` | Container `Dead (OOMKilled)`, `Crash reason: oom_killed`, `Memory: 32M`. Prova do VALOR do limite: 50M→tmpfs sob 32M → **morto (exit 137)**; mesmo workload sob 128M → **sucesso (RC=0)**. | **NÃO REPRODUZ / FUNCIONA** (limite realmente imposto pelo cgroup v2) |
| **CRI** sanidade básica | `delonix serve cri --addr unix://.../cri.sock` | Recusa com `DX-6000`: `delonix-cri` não está ao lado do `delonix` na árvore v0.1.1; a cópia no PATH (`~/.local/bin/delonix-cri`) é **5.1.1** e, forçada com `DELONIX_SERVER_FROM_PATH=1`, aborta por mismatch de versão (`delonix-cri 5.1.1 vs delonix 0.1.0`). | **BLOQUEADO-POR-AMBIENTE** (binário `delonix-cri` da linha v0.1.1 não construído; proibido construir) |

---

## 1. DEF-003 — `compose up` não reconcilia serviço parado

### compose.yaml usado
```yaml
name: qa-def003
services:
  qa-web:   { image: alpine:3.20, command: ["sleep","100000"] }
  qa-cache: { image: alpine:3.20, command: ["sleep","100000"] }
```

### Comandos e saída
```
$ DELONIX_ROOT=/home/walter/dlx-wt/qa-def delonix compose up -d
network/qa-def003_default: already exists, nothing to do
<pendura>                       # timeout 120s -> UPEXIT=124
$ delonix compose ps
SERVICE   NAME   IMAGE   STATUS   # vazio, nenhum container criado
```

`compose up --dry-run` resolve correctamente o projeto (Network bridge `qa-def003_default` 10.209.0.0/16 + 2 serviços). O dry-run é instantâneo; o `up` real pendura.

### Isolamento da causa
- `delonix container run -d --net qa-def003_default alpine sleep ...` → **pendura, exit 124** (nem chega a lançar o `slirp4netns` próprio). Portanto o hang é no attach à rede bridge/ingress, não no compose em si.
- `delonix container run -d --net host alpine sleep ...` (qa-smoke) → **arranca, `Up` 5+ min**. Containers host-net funcionam.
- `network_mode: host` no compose é **recusado** (`DX-1000`), por isso o compose usa sempre a rede bridge do projeto → caminho sempre bloqueado neste host.
- Durante o hang, os **próprios comandos do dono** estão presos no host: `delonix container start travel-pg` (pids 205966/205990) e `delonix container restart stage-demo-ui` (pids 2723932/2723937) persistem em todas as amostras; host também corre kind+ceph+kubelet. → o layer de networking rootless está congestionado a nível de host.

### Conclusão
O passo central do DEF-003 (parar um serviço e verificar se o `up` o reconcilia) **não é exercitável** porque o `compose up` nunca consegue levantar os serviços neste host (rede bridge/ingress rootless congestionada; `slirp4netns` existe em `/usr/bin`). **BLOQUEADO-POR-AMBIENTE.** Precisa de um host rootless com o layer de ingress livre (ou um teste host-net) para re-verificar o DEF-003 no HEAD.

## 2. OOM — limite de memória efectivo + kill

```
$ delonix container run -d --name qa-oom --memory 32M alpine:3.20 sh -c 'tail /dev/zero'
45cff32ea18f1ed6
$ delonix container describe qa-oom
Status:         Dead (OOMKilled)
Crash reason:   oom_killed
  Memory:       32M

# Prova de que o VALOR do limite é imposto (não só registado):
$ delonix container run --rm --memory 32M  alpine sh -c 'dd if=/dev/zero of=/dev/shm/x bs=1M count=50' -> exit 137 (SIGKILL/OOM)
$ delonix container run --rm --memory 128M alpine sh -c 'dd if=/dev/zero of=/dev/shm/x bs=1M count=50' -> RC=0 (sucesso)
```

- Alocação retida (`tail /dev/zero`) sob 32M → OOMKilled, estado `Dead (OOMKilled)`, `oom_killed`.
- 50 MiB para tmpfs (contabilizado no cgroup v2) **morre a 32M** e **sobrevive a 128M** → o limite é realmente aplicado pelo kernel, com o valor configurado.

**Veredicto: NÃO REPRODUZ / FUNCIONA.** memory delegado neste host; o limite é imposto e o OOM-kill ocorre.

## 3. CRI — sanidade básica

```
$ delonix serve cri --addr unix:///home/walter/dlx-wt/qa-def/cri.sock
error[DX-6000] unavailable: 'delonix-cri' is not next to delonix in .../target/debug, and a copy
found on the PATH would be from another install — install it with `install.sh --with-cri`, or set
DELONIX_SERVER_FROM_PATH=1 ...

$ DELONIX_SERVER_FROM_PATH=1 delonix serve cri --addr unix://.../cri2.sock
warning: serving with '/home/walter/.local/bin/delonix-cri' (DELONIX_SERVER_FROM_PATH=1)
ERROR delonix_cri: this delonix-cri is version 5.1.1 but delonix is 0.1.0 — install the delonix-cri
of the same release next to delonix   (exit 2)
$ crictl --runtime-endpoint unix://.../cri2.sock version  -> fatal: no such file or directory (socket nunca criado)
```

- Infra do host presente: `crictl`, `critest`, `containerd`, `slirp4netns`.
- Falta: o binário **`delonix-cri` da linha v0.1.1** ao lado do `delonix` em `target/debug` (não existe; proibido construir). A única cópia no PATH é **5.1.1** e o guardrail de versão recusa-a mesmo com `DELONIX_SERVER_FROM_PATH=1`.

**Veredicto: BLOQUEADO-POR-AMBIENTE.** O que falta exactamente: construir/instalar `delonix-cri` da mesma release (v0.1.1) ao lado do `delonix` (`install.sh --with-cri`). Só então faria sentido `crictl version/info` e um subconjunto do `critest`.

---

### Notas de ambiente
- Nenhum recurso do dono foi tocado; todo o trabalho em `DELONIX_ROOT=/home/walter/dlx-wt/qa-def` com recursos prefixados `qa-`.
- Recursos qa-* e o root isolado limpos no fim da sessão.
