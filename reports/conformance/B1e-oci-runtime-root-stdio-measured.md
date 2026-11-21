# B1e — `delonix oci-runtime`: stdio herdado do `create` (modelo runc de fd)

Medido 2026-11-21 na `h3-root-vm` (qemu/KVM, Ubuntu 26.04, cgroup v2 unified,
uid 0), com um `delonix` **release** construído do working tree (modelo de
capabilities 5-conjuntos + validação ns de B1d + trabalho não-committado de
seccomp-args do dono + **esta alavanca de stdio**). Suite:
`runtime-tools@v0.9.1-...bd3b0bd`, via `scripts/oci-runtime-rt4-wrapper.sh`.

## Resultado

| binário | ok | not ok | ficheiros .t totalmente verdes |
|---|---|---|---|
| delonix 0.1.2 (B1d, ANTES — sem stdio) | **21** | 58 | 7 |
| delonix 0.1.2 (ESTE run — stdio herdado) | **32** | 58 | **17** |
| runc 1.5.3 (referência, B1c) | 53 | 53 | 24 |

`linux_ns_path_type` mantém **14/14** (validado isolado primeiro, antes da suite
completa). `cargo test -p delonix-linux` 263 ok; `-p delonix-bin` âmbito
`oci_runtime` 29 ok (28 + o novo `oci_stdio_target_*`).

## O que mudou e porquê (+11, sem regressão)

A suite lê o stdout do container dos **fds herdados do `create`** (modelo runc):
`validation/util/container.go` abre `<bundle>/stdout-<uuid>` e `stderr-<uuid>`,
aponta-lhes `cmd.Stdout`/`cmd.Stderr` do processo `create`, e depois de o
container parar lê esses ficheiros (`ReadStandardStreams`) — o `runtimetest`
escreve o seu TAP no stdout. Antes, o `start` do `delonix oci-runtime` corria
`detach:true` SEM `log_path`, logo `detach_stdio` mandava o stdout para
`/dev/null`: o TAP perdia-se e o `prove` via «No subtests run / No plan found in
TAP output» em todo o caso `RuntimeInsideValidate` (`default.t`, `process.t`,
`process_capabilities.t`, `misc_props.t`, …).

Agora o `create` resolve `/proc/self/fd/{1,2}` para os PATHs desses ficheiros
(só quando são ficheiros regulares — tty/pipe/socket/`/dev/null` caem no caminho
histórico) e persiste-os em `Container.oci_stdout`/`oci_stderr`; o `start`
reabre-os (`O_WRONLY|O_APPEND`) e o init faz `dup2` deles directamente sobre
1/2 em `detach_stdio` — **sem shim de log, sem rotação, sem framing CRI**: o TAP
chega byte-a-byte ao ficheiro que a suite lê.

Casos que passaram de «No subtests run» a verdes (medido vs baseline B1d):
`default.t`, `process.t`, `process_capabilities.t`, `misc_props.t`,
`mounts.t`, `poststart.t`, `poststop.t`, `prestart*.t`,
`linux_cgroups_relative_*`, `delete_*`, entre outros — 7 → 17 ficheiros `.t`
totalmente verdes.

## O que falta para os 53 do runc

- `process_capabilities.t` cria o container (TAP verde no 1º subteste), mas um
  dos seus bundles ainda cai em `error[DX-1000] oci-runtime: config.json: serde
  failed` a montante (gap da crate `oci-spec` em `process.capabilities` nalgumas
  formas) — não perseguido nesta alavanca.
- `misc_props.t`/`mounts.t`: `start` recusa `rshared` (`DX-1117`) por o container
  não ser privilegiado — recusa correcta, mas conta como `not ok`.
- `killsig.t`: timeout à espera do estado do container.
- `linux_cgroups_*` (não-relativos): a suite upstream não suporta cgroup v2 (falha
  igual para o runc).

O número não é monótono: o total de `not ok` mantém-se em 58 porque os casos que
agora CRIAM o container emitem asserções de lifecycle que antes nem corriam.
