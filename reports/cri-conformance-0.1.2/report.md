# Conformidade CRI — `delonix-cri 0.1.2` (critest v1.36.0, VM rootful)

Parte 2 da campanha: o lab privilegiado que estava **BLOCKED** foi levantado e o
número foi **medido**. `critest` (cri-tools upstream, v1.36.0) corre contra um
`delonix-cri` **0.1.2 rootful + CNI** dentro de uma VM descartável.

## Ambiente (reproduzível)

| | |
|---|---|
| Motor | `delonix`/`delonix-cri` **0.1.2** (build release do host, scp para `/usr/local/bin`) |
| Suite | `critest`/`crictl` **v1.36.0** (upstream) |
| VM | golden `delonix-vm-k8s:1.36` (ubuntu 24.04, kernel 6.8.0-142, 4 vCPU/4G), libvirt, ip 192.168.122.243 |
| Root | sim (utilizador `delonix` no grupo `sudo` → `sudo` dentro da VM) |
| CNI | bridge + host-local + portmap (`/opt/cni/bin`, conflist gerada pelo harness) |
| Servidor | `DELONIX_CNI=1`, `DELONIX_CRI_CAP_CEILING=all`, `DELONIX_CRI_SECCOMP_NIL=unconfined` |
| Harness | `reports/cri-conformance-0.1.1/vm-critest.sh` (reutilizado) |

Teardown: `vm destroy crit-vm --force` → 827 MiB libertados, 0 domains, 0 resíduos.

## Resultado medido

```
Ran 103 of 122 Specs in 196.9 s
100 Passed | 3 Failed | 0 Pending | 19 Skipped
```

**100/103 (97%) dos specs corridos passam.** Os 19 skipped são specs que a suite
não aplica neste runtime/host (p. ex. features opcionais). Dos 3 Failed, um
re-run focado separou **ambiente** de **defeito real**:

| Spec | 1ª corrida | Re-run focado | Veredicto |
|---|---|---|---|
| `Container runtime … execSync [BeforeEach]` | FAIL | **PASS** | **ambiente** — race de pull (`PullImage … DeadlineExceeded` de gcr.io); setup falhou, passou na repetição |
| `Image Consistency … simultaneous RemoveImage calls [Serial]` | FAIL | FAIL | **defeito real → CRI-F1** |
| `Security Context … dropping ALL capabilities` | FAIL | FAIL | **defeito real → CRI-F2** |

**Conformidade efectiva: ~101/103** (descontando o ambiental); **2 lacunas reais**.

## Achados reais (backlog)

### CRI-F1 — `RemoveImage` não é idempotente sob chamadas simultâneas
- **Spec:** `[k8s.io] Image Consistency … should not fail on simultaneous RemoveImage calls [Conformance] [Serial]` (`pkg/validate/image_consistency.go:171`).
- **Observado:** duas `RemoveImage` concorrentes da mesma imagem → a 2ª devolve
  `rpc error: code = Internal desc = I/O error: No such file or directory (os error 2)`.
  O CRI exige que remoções simultâneas **não** devolvam erro (a imagem já ter sido
  removida é sucesso, não `Internal`).
- **Esperado:** `RemoveImage` tolera a ausência (ENOENT) e devolve OK/NotFound
  tratado, não um erro de I/O cru. **Tier:** B. **Área:** `delonix-cri` image service.

### CRI-F2 — largar TODAS as capabilities quebra o exec (`setgroups([]) failed`)
- **Spec:** `[k8s.io] Security Context … runtime should support dropping ALL capabilities` (`pkg/validate/security_context_linux.go:593`).
- **Observado:** com todas as caps largadas, o container falha com
  `delonix: exec setgroups([]) failed`. Sem `CAP_SETGID`, a chamada `setgroups([])`
  do caminho de exec é recusada (EPERM) — e **liga-se ao BUG-0008** (setgroups
  incondicional): o fix tornou `setgroups` sempre chamado, mas quando `CAP_SETGID`
  é largado essa chamada passa a não ser permitida.
- **Esperado:** quando `CAP_SETGID` não está no conjunto efectivo (ou sem
  `group_add`), **saltar** `setgroups` ou tolerar EPERM, em vez de abortar o exec.
  **Tier:** B. **Área:** `delonix-linux` exec/init (mesma zona do BUG-0008).

## Veredicto
O `delonix-cri 0.1.2` passa **97%** da conformidade CRI upstream num lab rootful
real — número **medido**, já não BLOCKED. Ficam **2 lacunas reais** (CRI-F1
concorrência de RemoveImage, CRI-F2 drop-all-caps/setgroups) para o backlog, e
uma falha ambiental (pull de imagem) que não é do motor. O caminho rootful
(CRI+CNI+cgroup2+AppArmor) está **provado neste host via VM descartável**.

## Artefactos
- `critest-full.console` — corrida completa (103 specs).
- `critest-focus.console` — re-run dos 3 (2 Passed | 2 Failed).
- `evidence.jsonl` — registo lab-ephemeral do critest.
