# Finding — `HOME=/root` para utilizador não-root sem `/etc/passwd` (CRI)

**Área CNCF:** H1 (conformidade / interop Sonobuoy)
**Severidade:** alta — parte workloads client-go não-root (o agregador Sonobuoy
morre com `EACCES` ao resolver `~/.kube/config`).
**Estado:** CORRIGIDO (crate verde + arch_fitness delta-zero; prova live abaixo).

## Sintoma medido (antes)

Com `--mode quick`, o agregador Sonobuoy corre non-root (uid próprio da imagem) e
morre ao escrever/ler `~/.kube/config`:

```
... open /root/.kube/config: permission denied (EACCES)
```

Workaround que mascarava o bug: forçar `HOME=/tmp` no pod do Sonobuoy. Esta
correcção remove a necessidade desse workaround.

## Causa-raiz

[`apply_env`](../../../crates/adapters/delonix-linux/src/lib.rs) — o init do
container (PID 1 dentro dos namespaces) montava um ambiente limpo e fixava
**`HOME=/root` para QUALQUER utilizador**, independentemente do `runAsUser`:

```rust
std::env::set_var("HOME", "/root");   // ← fixo, mesmo para uid != 0
```

Um processo não-root (Kubernetes `runAsUser: 1000` contra uma imagem cujo
`/etc/passwd` só conhece `root`) recebia então `HOME=/root` — um directório
`0700` pertencente ao `root` onde não pode escrever. Todo o tooling client-go que
resolve `~/.kube/config` rebenta com `EACCES`. runc/OCI, pelo contrário, derivam
`HOME` da entrada do utilizador em `/etc/passwd` e usam `/` por omissão.

## Correcção

`crates/adapters/delonix-linux/src/lib.rs`:

1. Novo `passwd_home(uid)` (+ metade pura `parse_passwd_home(text, uid)`): lê o
   `/etc/passwd` do container (pós-`pivot_root`) e devolve o campo *home* (6.ª
   coluna) da entrada do uid; `None` se o ficheiro faltar ou o uid não tiver
   entrada.
2. `apply_env(hostname, env, uid)` passa a resolver
   `HOME = passwd_home(uid).unwrap_or("/")` — **nunca `/root` fixo** para um
   não-root sem entrada. Um `HOME` explícito no ENV (imagem/stack/CLI) continua a
   ganhar, porque é aplicado por último.
3. Ambos os sítios de chamada passam o uid efectivo: caminho `run`
   (`run_uid.unwrap_or(0)`) e caminho `exec` (utilizador do `-u`, senão o do
   container).

Paridade runc: uid presente em `passwd` → o seu *home* (root mantém `/root`);
uid sem entrada → `/`.

## Prova medida

### Unitária (crate `delonix-linux`)

`parse_passwd_home_honours_the_entry_and_defaults_to_root_dir` cobre: uid em
passwd → *home*; **uid 65532 sem entrada → `None` (⇒ `/`, não `/root`)**; passwd
vazio → `None`; linhas malformadas ignoradas.

```
$ cargo test -p delonix-linux
test result: ok. 258 passed; 0 failed; 0 ignored; ...
```

### Gate `arch_fitness` — delta-zero

Medido com e sem a alteração a `lib.rs` (restante árvore igual): **saída
byte-a-byte idêntica**. Todos os FAIL (`env_writes 25`, `cli_context_lines`,
`library_prints`, `raw_error_variant_matches`, `cmd/distroless`, `cmd/help`) são
dívida pré-existente / de edições concorrentes do dono, não desta correcção. O
`git diff` desta alteração a `env::set_var`/`remove_var` é 1:1 (substitui a linha
`HOME`), logo contributo **zero** para `env_writes`.

### Runtime directo (binário `target/release/delonix` 0.1.2 com a correcção)

Prova decisiva do EXACTO caminho `apply_env` (idêntico no CLI `run` e no CRI —
ambos derivam `HOME` de `run_uid` via `apply_env`). `busybox:1.36` não tem
entrada para uid 1000 em `/etc/passwd` (`grep -c ":1000:" → 0`; `root` → `/root`):

```
$ delonix container run --rm --user 1000 busybox:1.36 sh -c 'echo HOME=$HOME; id'
HOME=/                       # ← uid 1000 sem entrada → "/" (ANTES: /root)
uid=1000 gid=1000 groups=1000

$ delonix container run --rm busybox:1.36 sh -c 'echo HOME=$HOME'
HOME=/root                   # ← root COM entrada em passwd → /root (sem regressão)
```

**Porque isto resolve o Sonobuoy:** o agregador corre non-root e o client-go
tenta `~/.kube/config`. Com `HOME=/root` (0700, do `root`) o processo non-root
nem consegue _atravessar_ o directório → `stat` devolve `EACCES` → fatal. Com
`HOME=/` (0755, atravessável) o `stat` de `/.kube/config` devolve `ENOENT` —
que o client-go trata como «sem kubeconfig» e cai, limpo, na config
*in-cluster* (service-account token). É exactamente a semântica do runc.

### Live (cluster Delonix-CRI — integração kubeadm + Sonobuoy)

Medido 2026-11-20. Golden image `delonix-vm-k8s-homefix:1.36` reconstruída a
embeber os binários corrigidos (`delonix-cri` sha256 `7a5166fc…`); cluster de
1 nó `delonix cluster kubeadm --name homefix --control-plane 1 --workers 0`.

**Critério 1 — HOME de um `runAsUser:1000` in-cluster:**

```
$ kubectl run h --image=busybox:1.36 --restart=Never --rm -i \
    --overrides='{"spec":{"securityContext":{"runAsUser":1000}}}' \
    --command -- sh -c 'echo $HOME'
/
```

**Critério 2 — Sonobuoy `--mode quick` SEM o workaround `HOME=/tmp`:**

O agregador (`pod/sonobuoy`, que corre non-root) manteve-se `1/1 Running`,
**0 restarts** — já NÃO rebenta com `EACCES` em `/root/.kube/config`. Resultado:

```
$ sonobuoy results <tarball>
Plugin: e2e
Status: passed
Total: 7586
Passed: 8
Failed: 0
Skipped: 7578

Plugin: systemd-logs
Status: passed
Passed: 1
Failed: 0
```

e2e **passed, Failed 0**, sem o workaround. (As «Errors detected in files» da
saída completa são ocorrências informativas nos logs dos componentes, não
falhas de teste — `Failed: 0` é a contagem autoritativa.)

## Execução live (reproduzir)

```sh
# binário com a correcção
cargo build -p delonix-bin --release
CRI=$(pwd)/target/release/delonix

# cluster de 1 nó (sobe em VM; sem sudo no host)
delonix cluster kubeadm --control-plane 1 --workers 0

# Critério 1 — HOME de um não-root sem entrada em passwd → "/"
kubectl run h --image=busybox:1.36 --restart=Never --attach --rm \
  --overrides='{"spec":{"securityContext":{"runAsUser":1000}}}' \
  --command -- sh -c 'echo $HOME'
# esperado: /

# Critério 2 — Sonobuoy quick SEM workaround HOME=/tmp
sonobuoy run --mode quick --wait
sonobuoy status          # e2e: Passed, Failed 0
sonobuoy retrieve && sonobuoy results <tarball>
```
