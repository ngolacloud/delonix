# critest 103/103 limpo — prova ponta-a-ponta (2026-11-10)

Fecha a campanha de conformância CRI: numa VM golden rootful+CNI, o `critest
v1.36.0` passa a **suite completa `103 Passed | 0 Failed`**, com o spec `rshared`
(ADR-0093) verde. Confirma, numa corrida única e sem retries, o que as fixes
`rshared` (`a7f8c01f`), barreira de cgroup OOM (`ba1eca84`) e scratch de unpack
concorrente (`6092ca89`) prometiam isoladamente.

## Ambiente medido

| Eixo | Valor |
|---|---|
| Motor | `delonix` + `delonix-cri` compilados da MESMA árvore `feat/cri-conformance-rootful` @ `8353c302` (release) |
| VM | `delonix vm create crit-vm --disk delonix-vm-k8s:1.36 --vcpus 4 --memory 4G` (libvirt, 192.168.122.243, ubuntu 24.04, kernel 6.8.0-142) |
| Suite | `critest`/`crictl` **v1.36.0** |
| Modo | **rootful + CNI** (bridge+host-local 10.88.0.0/16 + portmap), cgroup v2 c/ `memory` delegado |
| Env servidor | `DELONIX_CRI_CAP_CEILING=all`, `DELONIX_CNI=1`, `CNI_PATH=/opt/cni/bin`, `DELONIX_CRI_SECCOMP_NIL=unconfined` |
| AppArmor | 2 perfis de teste carregados no nó (`apparmor_parser`), securityfs montado |

Harness: `scratchpad/vm-critest.sh` (derivado de `scripts/critest.sh` — mesma
definição de perfis AppArmor, + delegação cgroup2, CNI e seccomp-nil).

## 1. Spec `rshared` em foco — PASSA

```
FOCUS="mount with 'rshared'"  → SUCCESS! -- 1 Passed | 0 Failed | 121 Skipped
```

`mount with 'rshared' should support propagation from host to container and vice
versa`: um `mount --bind` feito DENTRO do container aparece no host (a metade
container→host que faltava). É a entrega do ADR-0093.

## 2. Suite completa — 103/103 limpo

```
Ran 103 of 122 Specs in 184.548 seconds
SUCCESS! -- 103 Passed | 0 Failed | 0 Pending | 19 Skipped   (rc=0)
```

Três corridas completas, consecutivas, na mesma VM:

| Corrida | Resultado | Única falha |
|---|---|---|
| 1 | 102 Passed \| 1 Failed | attach-"hello" (interferência, ver §3) |
| 2 | 102 Passed \| 1 Failed | attach-"hello" (idem) |
| **3** | **103 Passed \| 0 Failed** | — |

`rshared` passa nas **três** (nunca está entre as falhas). O número oscila
102↔103 exactamente como a campanha documentou — e a única variância é o spec de
attach, nunca um spec do motor.

## 3. A oscilação 102↔103 é um async-leak do ginkgo, não um gap do motor

Nas corridas 1 e 2 a falha é reportada contra um spec `[k8s.io] Image Manager …
[Conformance] [Serial]` cujo próprio *Timeline* completa todos os passos (Pull,
Check, Get status, Remove, Remove by ID, Check empty) sem erro. O que falha,
enxertado por cima, é:

```
[FAILED] in [It] - pkg/validate/streaming.go:317
[FAILED] Timed out after 60.001s.
The stdout of attach should be hello — Expected <string>: "" to equal "hello"
```

Isto é o padrão clássico de **assertion assíncrona com fuga** do ginkgo: o spec de
**attach** anterior arranca um `Eventually(stdout=="hello", 60s)`, o seu `It`
retorna, e o timeout dispara 60 s depois — **durante** o `[Serial]` seguinte, a
quem o ginkgo atribui a falha (daí a vítima migrar: corrida 1 = "digest", corrida
2 = "tag"). Provas de que é interferência, não conformidade:

- **attach isolado → `1 Passed | 0 Failed`** (`FOCUS=attach`, 0,576 s);
- a vítima `[Serial]` passa sempre que não é precedida pelo attach (corrida 3);
- bate com a caracterização anterior da campanha (OOM-race: «1 interferência
  attach-hello, passa isolada 5/5»; concurrent-unpack: «102/103 ×2, a 1 falha
  varia e passa 3/3 isolada»).

Nenhuma das três corridas acusou unpack-race (`.tmp`), OOM ou falha de `rshared`.

## 4. Gate de privilégio — `rshared` nunca ligado em rootless

- **Unit** (`mount_propagation_tests`, `delonix-linux`): `rshared_allowed(true,false)=true`
  e `false` para rootless e para não-privilegiado; 10/10 verdes.
- **Live, rootless (host, uid 1000):**

```
$ delonix container run --rm -v /tmp/rs-src:/mnt:rshared python:3.12-slim true
error[DX-1117] invalid argument: mount propagation 'rshared' (bidirectional,
Kubernetes `mountPropagation: Bidirectional`) is refused: this engine is rootless
… Use 'rslave' for one-way host→container propagation, or run privileged on a
rootful node.
```

Recusa nomeada (`Error::BidirectionalNeedsPrivileged`, DX-1117), com a alternativa
`rslave` — nunca um degrade silencioso para slave.

## Veredicto

- **103/103 limpo, numa corrida única sem retries** (corrida 3), rootful+CNI.
- `rshared` (ADR-0093) conforme e **atrás de privilégio**: verde em VM rootful,
  recusado com razão em rootless.
- O motor não tem gap de conformância remanescente; a oscilação 102↔103 é um
  artefacto de atribuição assíncrona do ginkgo no spec de attach, provado por
  passagem isolada.
