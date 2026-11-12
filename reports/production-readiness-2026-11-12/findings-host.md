# Delonix — prontidão para produção crítica: Ronda 1 (host rootless)

> Objectivo: encontrar onde o Delonix **quebra** para produção crítica —
> desempenho, tempo de execução, recuperação de falha, densidade — comparando
> com Docker e Podman. Doutrina: só o **efeito verificado** conta; `exit 0` não
> basta.
>
> **Ambiente:** host partilhado i9-13900H (20 threads), 62 GiB, kernel 7.0.0-42,
> **rootless** (cgroup2 cpu/memory/pids delegados; io/cpuset não). **Sob carga**
> (load 6.22, containers do dono a correr) — dispersão alta; as medianas são
> direccionais, não números de bancada limpa. Binários: delonix 0.1.0 (release),
> docker 29.9.3, podman 4.9.3.

## Achados (ordenados por severidade para produção)

### P1 — Rede isolada é o custo dominante (gargalo de escala)
Arranque de container `run --rm alpine true` (mediana, n≈8-10):

| Motor | Config | Mediana |
|---|---|---|
| docker | bridge (netns+veth+NAT) | **3779 ms** |
| podman | slirp4netns | **378 ms** |
| **delonix** | **`--net host`** (default) | **106 ms** |
| **delonix** | **rede isolada `benchnet`** (SDN netns) | **679 ms** |

- O número «rápido» do Delonix (106ms) vem de **não isolar a rede** (`--net host`
  é o default). Com rede isolada real, o arranque é **~6× mais lento** (679ms) e
  **mais lento que o podman slirp** (378ms), embora muito à frente do docker.
- `pod create` real = **~1,6s** (1,0s a montar a netns SDN do pod + 0,6s o
  container). **A montagem da netns SDN é o custo dominante.**
- **Impacto produção — MEDIDO, e melhor do que temido:** workloads com rede
  isolada pagam ~300ms-1s/pod, MAS o caminho SDN **paraleliza e é robusto**:
  - N=20 concorrente isolado → **20/20 sucesso, 0 falhas**;
  - N=50 concorrente isolado → **50/50 sucesso, 0 falhas** (subnet /16, pool de
    IPs enorme);
  - sequencial escala linear (~300ms/container).
  Logo **não é uma quebra de correcção — é um custo de *throughput*** (latência
  de arranque por pod). É o alvo nº1 de **optimização** (reduzir o setup de netns
  SDN), não um bloqueador de produção.

### P2 — RETIRADO: era erro de medição, NÃO bug do Delonix
A hipótese inicial (leak de limpeza sob churn) **não se confirmou**. O que parecia
um leak/falha concorrente era **o verbo de limpeza errado nos meus testes**:
`delete container <nome>` é a superfície **declarativa** e **recusa correctamente**
containers imperativos (`container run`) com `DX-1000` («Container is the
imperative surface — use `container ps`; declarative containers live in
`kind: Pod`»). O correcto para imperativos é **`container rm -f`** (força se a
correr). Como os meus testes suprimiam o erro, os containers ficavam para trás →
colisões de nome → as «falhas concorrentes» e o «leak». **A separação
imperativo/declarativo do Delonix funciona como desenhado.** Lição da doutrina:
nunca suprimir erros de limpeza.

### P3 — Densidade host-net: escala linear, sem colapso ✅
| N containers | total | ms/container |
|---|---|---|
| 1 | 220 ms | 220 |
| 20 | 1418 ms | ~70 |
| 50 | 4016 ms | ~80 |
Steady-state ~80 ms/container até N=50, **sem degradação**. Saudável para host-net.
(Densidade com rede isolada por medir — ver P1.)

### P4 — Recuperação em runtime: FUNCIONA ✅
- **Detecção de crash:** SIGKILL do PID do container → estado **«Dead»** capturado.
- **Restart policy:** `--restart always` → após SIGKILL reiniciou (PID 928650→929001,
  restart-count 1), via **supervisor destacado por container (daemonless)**.
  `no|on-failure[:max]|always|unless-stopped` suportados.

### P5 — Recuperação pós-reboot: PROVADA numa VM ✅ (mas com um bug)
Prova numa VM libvirt (golden `delonix-vm-k8s:1.36`, delonix 5.0.1, root):
1. `container run -d --restart unless-stopped survivor` + `system boot enable` →
   gera a unidade de sistema `delonix-boot-survivor.service` (enabled, em
   `multi-user.target.wants`);
2. **reboot REAL** da VM (`systemctl reboot`);
3. após o boot: `container inspect survivor` → **`"status": "Running"`**, PID do
   `sleep 3600` vivo, mesmo id — a unidade reiniciou-o (`journalctl`: «Started
   delonix-boot-survivor.service»). **A recuperação pós-reboot FUNCIONA.**

### BUG (confirmado em develop) — unidade systemd com `Restart=` inválido
`boot.rs:264` escreve `Restart={policy}` com a policy do Delonix **verbatim**:
```
[Service]
Type=forking
Restart=unless-stopped   ← systemd: "Failed to parse service restart specifier, ignoring"
```
`unless-stopped` e `on-failure:N` **não são especificadores válidos do systemd**
(só `no|always|on-failure|on-success|on-abnormal|on-abort|on-watchdog`) → o
systemd ignora-os. **Impacto:** a recuperação-no-boot ainda funciona (o `ExecStart`
corre `delonix container start`, cujo supervisor aplica a policy real), **mas o
systemd não reinicia o supervisor se ESTE morrer a meio** para containers
`unless-stopped`. **Fix:** mapear policy→systemd (`unless-stopped`/`always`→
`Restart=always`, `on-failure[:max]`→`Restart=on-failure`).

### NÃO-bug (apanhado ao verificar) — `ps` ausente na VM
`delonix ps` deu «unrecognized subcommand» na VM — mas é a **versão antiga** da
golden image (5.0.1, anterior ao alias de raiz `ps` de #930), **não** um bug de
listagem. O `container inspect` confirma o container Running.

### Notas de qualidade (armadilhas de medição apanhadas pela doutrina)
- `--net bridge` **não existe** (erro `DX-4303`) — produziu tempos de 25-81ms que
  eram **erro-rápido**, não arranques. Verificar sempre o resultado funcional.
- `pod create` a «21ms» era **conflito de nome** (erro-rápido); o real é ~1,6s.
- **Polish bug:** a mensagem `DX-4303` tem dupla-negação — «no such ingress
  network 'bridge' does not exist».

### P6 — Paridade CNI (root + CNI): CONFORME ✅
Via root+CNI (bridge real + veth + IPAM, IP de pod roteável), através do
`delonix-cri` a correr root num container `--privileged` (sem VM):

| Specs CRI/CNI (critest v1.36.0) | Resultado |
|---|---|
| `PortForward` + `port mapping` | **2/2 PASS** |
| `networking` + `PortForward` + `port mapping` + `DNS` + `HostNetwork` | **6/6 PASS, 0 falhas** |

Combina com o **103/103** do critest completo (campanha de conformância anterior):
o caminho CNI — bridge/veth/IPAM, port-forward, port-mapping, DNS, host-network —
está conforme. Plugins em `/opt/cni/bin` (bridge, dhcp, firewall, bandwidth…).
**Paridade CNI com Docker/Podman: sim** no plano de conformância k8s.

### BUG — harness CNI partido pelo ADR-0086 (mesma classe que BUG-0001)
`scripts/critest-root-cni.sh` faz `cargo build -p delonix-cri` mas o binário
moveu-se para `delonix-cri-bin` (ADR-0086) → **o script não arranca** sem patch.
Mesmo defeito do `Makefile` (BUG-0001 do e2e 0.1.0). A corrigir no repo.

## Por fazer (precisa de VM root — reboot real)
- **R2-reboot:** provar `system boot enable` após um reboot REAL (o único teste
  que exige VM; o host partilhado não se reinicia). Mecanismo presente (P5).
- **R1b:** densidade com **rede isolada** (o gargalo P1 a escala — ~1s×N).
- **R2b:** teste de churn dedicado para quantificar o leak P2.

## Veredicto da Ronda 1 (host + VM) — corrigido após testes rigorosos
- **Onde é forte (medido):** arranque host-net rápido (106ms), densidade linear
  (até 50, sem colapso), **rede isolada robusta e paralela** (50/50 concorrente,
  0 falhas), restart policies, **recuperação de crash E de reboot real**,
  **conformância CNI 6/6 + 103/103**, build ≈ docker. Separação imperativo
  (`container rm`) / declarativo (`delete …`/`kind: Pod`) correcta.
- **Onde quebra / a resolver (só DOIS, ambos pequenos):**
  1. **BUG `Restart=` systemd (média):** a unidade de `system boot enable` usa a
     policy do Delonix como especificador systemd inválido (`unless-stopped`,
     `on-failure:N`). Recuperação-no-boot funciona; falta o mapeamento.
  2. **BUG harness (baixa):** `critest-root-cni.sh` referencia `delonix-cri`
     (ADR-0086 → `delonix-cri-bin`).
- **Custo (não quebra):** rede isolada ~3-6× o host-net por pod — alvo de
  **optimização de throughput**, não bloqueador.
- **Paridade:** à frente do docker em latência; filosoficamente par com o podman
  (rootless/daemonless/pods) e com recuperação pós-reboot comprovada; conforme em
  CNI/k8s.
- **Hipóteses RETIRADAS por serem erro de medição (doutrina «verificar, não
  suprimir»):** o «leak de limpeza» (P2) e as «falhas concorrentes» eram o verbo
  de delete errado nos testes, não defeitos do motor.

## Conclusão
Para produção crítica, o Delonix mostrou-se **surpreendentemente sólido**:
recupera de crash e de reboot real, a rede isolada escala sem falhas, e a CNI é
conforme. Os dois defeitos reais são de *polish* (systemd `Restart=`, nome de
pacote no harness). O único eixo de **optimização** com peso é a latência de
arranque com rede isolada (throughput), não uma quebra de correcção.
