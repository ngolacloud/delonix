# `delonix container run` — defeitos de paridade e prompts de correcção

**Fonte:** teste funcional ao vivo, parâmetro a parâmetro, de `delonix container run`
contra o comportamento Docker/Podman.
**Ambiente:** delonix 0.1.1 (commit `64ec5a498`), host rootless · daemonless,
cgroup2 delegado, imagens `alpine:3.19` + `busybox`. Data: 2026-11-19.
**Método:** efeito observado (não só `--help`) — leitura de cgroups no host via
`/proc/<pid>/cgroup`, inodes `/proc/self/ns/*`, `/proc/self/status`
(CapEff/Seccomp/NoNewPrivs), probes HTTP/TCP nativas e `curl`/`wget` ponta-a-ponta.

**Veredicto:** ~95 flags exercitadas; núcleo em forte paridade (e por vezes mais
seguro que o Docker). 4 defeitos onde a flag é aceite mas inoperante/quebrada.

Ordem de ataque (severidade): segurança → funcionalidade → paridade → DX.

---

## Notas transversais (para qualquer destas sessões)

- Host **rootless e sem sudo**; validar com os arneses rootless.
- `/tmp` e o scratchpad são **efémeros entre turnos** — guardar artefactos fora de `/tmp`.
- Testes do binário: `cargo test -p delonix-bin --bins` (o pacote `delonix-bin` não tem lib).
- Processo **develop-GitFlow**; o `push`/`merge` de PR é do dono.
- Não enfraquecer defaults de segurança ao corrigir (1, 3 e 4 tocam em hardening).

---

## BUG-1 — `--knows` / `--knows-none` não aplicam isolamento de DNS  _(Alta · segurança)_

**Sintoma.** `run --knows-none` e `run --knows X` são aceites e persistidos mas não
isolam: o container continua a resolver e a ligar a todos os pares da rede.

**Evidência.** Em rede dedicada:
```
run --net N --knows-none alpine nslookup srv  → Address 10.239.102.249 (resolveu)
                                   wget http://srv/ → SRVBODY (ligou)
run --net N --knows srv ...  → srv=OK  mas  srv2 também respondeu (fuga K2LEAK)
```

**Causa-raiz.** O campo `dns_knows` É escrito no record
(`crates/contexts/delonix-compute/src/app/run.rs:583-586`,
`crates/contexts/delonix-compute/src/app/record.rs:647`) mas **nenhum resolvedor o
consome**. O resolvedor de nomes de containers
(`crates/contexts/delonix-networking/src/domain/dns.rs` e
`crates/adapters/delonix-sdn/src/`) não filtra as respostas pela allowlist.

**Correcção.** Ao responder A/AAAA, filtrar pela `dns_knows` do container que faz a
query: `None` = resolve tudo (default); `Some([])` = resolve nada (`--knows-none`);
`Some([...])` = só esses nomes + o próprio.

**Done when.**
1. `--knows-none` → `nslookup <par>` devolve NXDOMAIN e `wget` falha.
2. `--knows A` resolve `A` mas devolve NXDOMAIN para `B`.
3. Teste e2e/regressão que PROVE o isolamento (não um teste cego — ver a armadilha
   `nft ip-vs-inet`, onde harnesses contavam na família errada e não viam a fuga).

**Gates.** Sem flag nova (sem pt.po novo). Não regredir a resolução normal (sem `--knows`).
Gates rootless. **Agente:** general-purpose, tier alto (resolvedor/SDN).

---

## BUG-2 — `--pod <name>` não junta o netns do pod  _(Alta · funcionalidade)_

**Sintoma.** `run --pod <name>` dá ao container IP próprio em vez de partilhar o do
pod, e um pod inexistente é aceite sem erro.

**Evidência.** Pod `vp` com IP `10.239.0.2` servindo em `:8080`:
```
run --net N --pod vp busybox …  → hostname -i = 10.239.9.136  (≠ 10.239.0.2)
                                   wget 127.0.0.1:8080 → Connection refused (SEPARATE-NS)
run --pod naoexistexyz busybox echo joined  → rc=0  (devia falhar)
```

**Causa-raiz.** Join imperativo em `bins/delonix/src/cmd/container.rs:3416-3424`;
handling do netns/erro em `container.rs:3931-4058`. A guarda "pod does not exist"
(`:4056`, `joined.is_err() && !pod::exists`) não dispara — o join "tem sucesso" sem
ligar ao netns. O caminho vizinho `--net container:<name>` (`container.rs:3931`)
funciona e faz o setns correcto.

**Correcção.** Alinhar o `--pod` para reutilizar o mesmo mecanismo de setns do
`--net container:` (entrar no netns do infra/membro do pod), e fazer a guarda de
pod inexistente abortar com erro claro.

**Done when.**
1. `run --pod vp` → `hostname -i` == IP do pod e `wget 127.0.0.1:<porta-do-pod>`
   responde (`JOINED-NS`).
2. `run --pod inexistente` → erro claro, `rc≠0`.
3. Teste cobrindo join bem-sucedido + pod ausente.

**Gates.** Paridade Podman `run --pod`. Não regredir `--net container:`. Gates rootless.
**Agente:** general-purpose, tier alto.

---

## BUG-3 — `--pid host` / `--host-pid` falham em rootless não-privilegiado  _(Média · paridade)_

**Sintoma.** Falham no arranque; funcionam com `--privileged`. O Docker permite
`--pid=host` sem privilégio.

**Evidência.**
```
run --pid host busybox …  → delonix: could not bind '/proc/sys' for read-only:
                              EINVAL: Invalid argument; aborting
run --pid host --privileged busybox …  → 905 pids do host visíveis  ✓
```

**Causa-raiz.** O hardening remonta `/proc/sys` read-only
(`crates/contexts/delonix-compute/src/app/run.rs:615`
`default_readonly_paths: ["/proc/sys"]`, aplicado em
`crates/adapters/delonix-linux/src/lib.rs` ~1625-1660 / 2034-2047). Com o PID-ns do
host partilhado (`host_pid` em `bins/delonix/src/cmd/container.rs:136/1369-1377`), o
`/proc` é o do host e o utilizador rootless não pode remontar `/proc/sys` ro → EINVAL.

**Correcção.** Quando `host_pid`, omitir `/proc/sys` dos `default_readonly_paths` (ou
tornar esse remount best-effort com aviso — nunca abortar).

**Done when.**
1. `run --pid host busybox sh -c 'ls /proc | grep -c "^[0-9]"'` → >50 em rootless sem
   `--privileged`.
2. `--host-pid` idem.
3. Container normal (sem `--pid host`) mantém `/proc/sys` ro (não-regressão do hardening).

**Gates.** Não enfraquecer o default. Gates rootless + segurança. **Agente:**
general-purpose, tier alto (namespaces/mounts — considerar o painel de revisores Rust).

---

## BUG-4 — `--wait` ignora a probe definida na CLI  _(Média · DX)_

**Sintoma.** `--wait` + `--health-cmd`/`--health-http`/`--health-tcp` é rejeitado,
mesmo quando a probe (sozinha) funciona e atinge `healthy`.

**Evidência.**
```
run -d --health-http :8080/ --wait busybox <srv>  →
  error[DX-1000]: --wait: image 'busybox' declares no HEALTHCHECK — nothing to wait for
(idêntico para --health-tcp e --health-cmd; as probes SEM --wait → healthy)
```

**Causa-raiz.** A validação do `--wait` em
`bins/delonix/src/cmd/container.rs:4242-4252` só considera `--no-healthcheck` e o
HEALTHCHECK da imagem, ignorando as probes da CLI. O caminho em
`container.rs:6687` já considera `--health-cmd`+imagem — alinhar a condição de `:4252`
para aceitar qualquer probe efectiva (CLI `--health-cmd/http/tcp` OU imagem) antes de
rejeitar.

**Done when.**
1. `run -d --health-http :8080/ --wait --wait-timeout 40 busybox <srv>` bloqueia e
   retorna `rc=0` com estado `healthy`.
2. Idem `--health-tcp` e `--health-cmd`.
3. `--wait` sem probe nenhuma mantém o erro claro actual.

**Gates.** Mensagem de erro consistente. Sem pt.po novo (flags existentes). Gate de
teste. **Agente:** general-purpose, tier médio (lógica localizada no CLI).

---

## Divergências seguras (NÃO são bugs — não corrigir)

- **`-p` safe-by-default**: liga a `127.0.0.1`, não a `0.0.0.0` (mais seguro que Docker;
  `0.0.0.0:p:p` alarga explicitamente). Comportamento intencional.
- **`--sysctl` recusa `net.*` não-namespaced** com "not namespaced; refused (affects
  the host)". Chaves namespaced (`kernel.shmmax`) aplicam. Conservador, do lado seguro.
- **`/sys/fs/cgroup` não é montado dentro do container** (rootless; Docker monta-o ro).
  Os limites SÃO aplicados — verificado no cgroup do host.
- **SKIP de host:** `--gpus all` exige CDI NVIDIA (`--gpus dri` funciona);
  `--no-userns` não é possível em rootless (userns obrigatório para o mapeamento).
