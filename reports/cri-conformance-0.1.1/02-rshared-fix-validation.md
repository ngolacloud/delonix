# `rshared` (ADR-0093) — implementado e validado (2026-11-09)

VM golden `delonix-vm-k8s:1.36` (libvirt, 4 vCPU/4G, ubuntu 24.04), motor release
`scp`'d como root, CNI bridge+host-local+portmap, `DELONIX_CRI_CAP_CEILING=all`.

## Spec em foco — PASSA

```
FOCUS="mount with 'rshared'"  → SUCCESS! -- 1 Passed | 0 Failed | 121 Skipped
```

`mount with 'rshared' should support propagation from host to container and vice
versa` passa: um `mount --bind /etc <ponto>` feito DENTRO do container aparece no
host (metade container→host, que faltava). Confirmado também à mão, comparando
`/proc/self/mountinfo` do container e do host:

```
# origem no host
91 … /tmp/rs/src /tmp/rs/src … shared:1
# dentro do container o volume é PEER (shared:1, sem master:)
… /tmp/rs/src /mnt/rs … shared:1
… /etc /mnt/rs/sub … shared:3        ← submount criado no container
# no host, apareceu:
… /etc /tmp/rs/src/sub … shared:3    ← propagou container→host ✅
```

## Suite completa

| Harness | Passed/103 | Notas |
|---|---|---|
| mínimo (só CNI) | 90/103 | 9 AppArmor (securityfs não montado) + seccomp-nil + OOM + streaming/rede |
| + securityfs + `DELONIX_CRI_SECCOMP_NIL=unconfined` | **99/103** | fecha AppArmor + seccomp-nil |

**`rshared` passa em ambas** (não está entre as falhas). Das 4 a 99/103, uma é
flaky e três são ambiente/streaming — nenhuma é o motor nem `rshared`. Re-corrida
focada das 4: `6 Passed | 3 Failed`:

- `should support network` — **passou na re-corrida** (a 1ª falha foi um erro de
  **unpack de layer**, `libgcrypt.so…tmp`, corrida de extração na imagem
  `hostnet-nginx`): FLAKY, não determinística;
- `OOMKilled` — reporte via cgroup2-memory (o baseline marcou-o flaky/por-confirmar);
- `portforward in host network` — rede/hostNetwork;
- `exec with tty=true and stdin=true` — streaming (exit-code `-1`).

São as mesmas classes que a campanha baseline resolveu com o harness completo para
chegar a 102/103 (com `rshared` como única falha de **motor** restante). Com
`rshared` agora fechado, o motor não tem gap de conformidade de propagação de mount;
as restantes dependem do ambiente/harness de rede+streaming+OOM, não deste fix.

## Implementação

`crates/adapters/delonix-linux/src/lib.rs` (`setup_rootfs`, `detach_old_root`,
`enclosing_mount_point`, `per_mount_propagation`, `root_propagation`,
`rshared_allowed`), gate em `spawn` (`Error::BidirectionalNeedsPrivileged`,
DX-1117). Ver ADR-0093 para o mecanismo medido e porquê de cada passo.
