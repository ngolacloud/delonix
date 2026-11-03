# G4 — control-plane k8s HA sobre Delonix: PROVADO

> **Data:** 2026-11-03. **Âmbito:** a segunda metade do caminho "via prova" do G4
> (a primeira, o overlay inter-nó, está em `g4-overlay-forwarding.md`). Critério:
> um control-plane k8s **HA (≥3 CP, etcd stacked, VIP) a sobreviver à queda de um
> CP, tudo sobre o Delonix**. **Medido ao vivo**, não afirmado.

## Montagem (turnkey, um comando)

```
delonix cluster kubeadm --name g4ha --control-plane 3 --workers 0 --cni default
```

Provisionou, em VMs do golden `delonix-vm-k8s:1.36`, numa rede própria:

- **3 control-planes** (`g4ha-cp1/2/3`), etcd **stacked** (co-locado);
- um **HAProxy load balancer** (`g4ha-lb`) como endpoint do control-plane (o
  `--control-plane-endpoint` do kubeadm) — o VIP da HA;
- **Cilium** como CNI (o default HA cross-node), instalado e esperado;
- **`delonix-cri` como CRI de cada nó** (o kubelet fala com ele em vez de
  containerd/CRI-O): `delonix-cri: next to delonix` + `delonix-cri re-execs it to
  spawn every container`. Confirmado no `kubectl get nodes`: `CONTAINER-RUNTIME
  delonix://0.1.0`.

Resultado do bootstrap: `cilium agent Ready on 3/3 node(s); 3/3 node(s) Ready`.

## Estado base (medido)

| Facto | Evidência |
|---|---|
| kubectl fala via o **HAProxy VIP** | `server: https://10.243.122.126:6443` (os CP são `10.243.179.x`; o LB é `.122.126`) |
| 3 CP Ready, runtime delonix | `g4ha-cp1/2/3 Ready control-plane v1.36.5 … delonix://0.1.0` |
| etcd stacked + Cilium | `etcd-g4ha-cp{1,2,3}`, `cilium-*` todos `Running` |
| **CNI funciona (pod real)** | `g4test` **Running**, IP Cilium `10.244.0.70`, `eth0` + ping-self ok — o percurso que o bug CRI de entrypoint bloqueava, agora vivo |

## Failover — queda de um control-plane (o critério)

`delonix vm stop g4ha-cp1` (um CP com etcd), e logo a seguir, pela API via o VIP:

| Observação | Resultado |
|---|---|
| kubectl via HAProxy VIP | um `TLS handshake timeout` momentâneo, depois **responde** (HAProxy encaminha para cp2/cp3) |
| **escrita no etcd durante a falha** | `configmap/g4ha-failover created`; relido depois: `survived=yes` — **quórum 2/3 mantido** |
| **scheduling durante a falha** | `kubectl run g4test2` → **Running** em `g4ha-cp3`, IP Cilium `10.244.2.80` |
| deteção da falha | após o grace, `g4ha-cp1 NotReady`, `cp2/cp3 Ready` |

## Self-heal — o CP volta

`delonix vm start g4ha-cp1` → **3/3 Ready de novo em ~20 s** (o membro etcd
re-sincroniza, o kubelet re-reporta). A HA não só sobrevive à perda como recupera
quando o nó regressa.

## Bónus medido

O cluster inteiro foi **parado e rearrancado a frio** (as 4 VMs `shut off` →
`vm start`) e **recompôs-se em ~20 s** (etcd re-formou quórum, apiserver subiu):
o estado kubeadm + etcd persiste em disco.

## Veredicto

O critério G4-HA está **PROVADO sobre o Delonix**: ≥3 CP + etcd stacked + VIP
(HAProxy) + **sobrevive à queda de um CP** (API viva, escrita e scheduling a
funcionar, quórum 2/3) + self-heal, com `delonix-cri` como CRI e Cilium como CNI.
Com o overlay inter-nó (`g4-overlay-forwarding.md`), **as duas metades do G4
"via prova" estão provadas**.

## Nota de produto — candidato FECHADO (2026-11-03, ADR-0085)

À data desta prova, a HA só se levantava pelo caminho **kubeadm-em-VMs**
(`cluster kubeadm`); o turnkey `cluster create` (kind-mode, containers) recusava
`--control-planes > 1`. **Esse candidato foi fechado a seguir:** `cluster create`
passa a provisionar um **container HAProxy** como endpoint e a fazer o bootstrap
HA — `cluster create --control-planes 3` dá 3 CP + LB + etcd stacked **em
containers, sem VMs**, e sobrevive à queda de um CP (API via o LB, escrita com
quórum 2/3, scheduling). Ver ADR-0085.

## Reprodutível

`delonix cluster kubeadm --name g4ha --control-plane 3 --workers 0 --cni default`,
`delonix vm stop g4ha-cp1`, `kubectl --kubeconfig <…> …`, `delonix cluster destroy g4ha`.
