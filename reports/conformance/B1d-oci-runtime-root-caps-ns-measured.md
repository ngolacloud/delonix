# B1d — `delonix oci-runtime` a root: modelo de capabilities OCI (5 conjuntos) + validação de namespace

Medido 2026-11-21 na `h3-root-vm` (qemu/KVM, Ubuntu 26.04, cgroup v2 unified,
uid 0, `cap_last_cap=40`), com um `delonix` **release** construído do working
tree (caps 5-conjuntos + validação ns + trabalho não-committado de seccomp-args
do dono). Suite: `runtime-tools@v0.9.1-...bd3b0bd`, via
`scripts/oci-runtime-rt4-wrapper.sh`.

## Resultado

| binário | ok | not ok |
|---|---|---|
| delonix 0.1.2 (B1c, ANTES — guard ambient) | **19** | 31 |
| delonix 0.1.2 (ESTE run — 5 conjuntos + raise ambient + ns path/type) | **21** | 58 |
| runc 1.5.3 (referência, B1c) | 53 | 53 |

(O total de `not ok` sobe porque mais casos passam a CRIAR o container e a
emitir asserções de lifecycle que antes nem corriam — o número não é monótono,
como documentado.)

## O que mudou e porquê (+2, sem regressão)

Os 19 originais mantêm-se **todos**: `linux_ns_path_type` (14 asserções: 7
negativas por recusa de path-de-tipo-errado + 7 "matches") continua 14/14 —
preservado pela nova regra «qualquer namespace com `path` é recusado» (o engine
cria namespaces frescos, não faz setns por path), mais os 5 casos de
"operation MUST error if no container ID".

Os **+2** vêm directamente de o guard ambient ter deixado de bloquear o
`create`:

- `kill_no_effect` — cria um container e mata-o antes do `start`; só precisava
  de o `create` passar (a config default traz ambient).
- `misc_props` — cria um container com uma annotation desconhecida e exige que
  NÃO haja erro; idem, só precisava do `create`.

Ambos falhavam antes em `create` com `error[DX-1000] … process.capabilities.ambient
— ambient capabilities are not raised by this engine`.

## O modelo de 5 conjuntos funciona end-to-end (prova independente)

A suite não consegue observar as ~205 asserções de capability do `runtimetest`
porque o `oci-runtime start` corre o container **detached** (log próprio) em vez
de ligar o stdout aos fds de `create` (modelo runc de fd herdado); o TAP do
`runtimetest` nunca volta ao `prove` → «No subtests run». Isto é um gap de
**plumbing de stdio**, ORTOGONAL a capabilities/namespaces.

Prova directa, container `delonix oci-runtime` a root com os 5 conjuntos iguais
`{CHOWN, KILL, NET_RAW}` (máscara `0x2021`), ambient incluído, lendo
`/proc/self/status` do próprio init:

```
CapInh: 0000000000002021
CapPrm: 0000000000002021
CapEff: 0000000000002021
CapBnd: 0000000000002021
CapAmb: 0000000000002021   ← ambient LEVANTADO (antes 0 / recusado)
```

Os cinco conjuntos instalam-se exactamente como a bundle pede, e o ambient é
levantado via `PR_CAP_AMBIENT_RAISE` (precondição `ambient ⊆ permitted ∩
inheritable`, validada no `create`). É precisamente o que
`runtimetest.validateCapabilities` verificaria (5 × 41 asserções).

## Próxima alavanca rumo a 53 (sessão nova)

O bloqueador dominante dos ~30 casos `RuntimeInsideValidate`
(`default`/`process`/`hostname`/`linux_readonly_paths`/`mounts`/…) é o plumbing
de stdout do par `create`+`start`: o container tem de herdar os fds de stdio de
`create` (ficheiros `stdout-<uuid>`/`stderr-<uuid>` na bundle, lidos por
`ReadStandardStreams`). Além disso, `process_capabilities` falha a montante em
`config.json: serde failed` (a crate `oci-spec` não desserializa a lista
privilegiada que `SetupPrivileged(true)` gera) — gap separado da crate, não do
engine.
