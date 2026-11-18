# HA multi-nó — `delonix cluster create --control-planes 3` (0.1.2)

Parte (c) da campanha: prova de **alta disponibilidade multi-nó** em rootless, sem
VMs e sem Docker — kind-mode (os nós são containers no próprio motor Delonix).

## Ambiente
- Motor `delonix 0.1.2` (release), host rootless, root isolado (`/tmp/dlxha`).
- Comando: `delonix cluster create --name ha012 --control-planes 3`.
- Imagens: `kindest/node:v1.37.0` (nós) + `haproxy:lts-alpine` (load balancer).

## O que subiu (reproduzível)
```
✓ Provisioning the HAProxy load balancer ⚖️ 4.8s
✓ Starting control-plane (kubeadm init) 🕹️ 1m30s
✓ Joining 2 control-plane(s) 🎮 2m49s
✓ Waiting for all 3 node(s) to be Ready ⏳ 56.5s
cluster "ha012" ready (HA)
```

## Verificação (de dentro do cluster)

**3 control-planes Ready:**
```
NAME                   STATUS   ROLES           VERSION
ha012-control-plane    Ready    control-plane   v1.37.0
ha012-control-plane2   Ready    control-plane   v1.37.0
ha012-control-plane3   Ready    control-plane   v1.37.0
```

**etcd empilhado com quorum de 3 membros** (`etcdctl member list`):
```
240d31b144fc1d03, started, ha012-control-plane3, ...:2380, ...:2379
3e868f92e46e4a94, started, ha012-control-plane2, ...:2380, ...:2379
9cf0ef4b1f69551a, started, ha012-control-plane,  ...:2380, ...:2379
```

**HAProxy VIP** (endpoint estável à frente dos 3 CPs):
```
haproxy:lts-alpine  Up  127.0.0.1:6443->6443/tcp  ha012-external-load-balancer
```

4 containers no total (3 nós + 1 load balancer).

## Teardown
`cluster destroy ha012 --force` → `cluster 'ha012' removed (4 node(s))`; 0 clusters,
0 containers, 0 resíduos.

## Veredicto
**HA multi-nó PROVADO em rootless.** `cluster create --control-planes 3` entrega um
plano de controlo Kubernetes de **3 réplicas Ready**, com **etcd empilhado em quorum
de 3** e um **HAProxy como VIP** — tudo em kind-mode (containers no motor), sem VMs
nem Docker, no perfil rootless deste host. Complementa o single-nó (Parte 2) e o
HA multi-nó já provado noutras sessões por via kubeadm/VMs.

## Fica para follow-up (exigem VM rootful)
`cluster kubeadm` (provisão de VMs + bootstrap), CRIU (`container checkpoint`/
`restore`/`clone`), e `bucket inspect`/`rm` (S3 vivo).

## Artefactos
- `ha-create.log` — a corrida do bring-up HA (marcos).
- `evidence.jsonl` — registo lab-ephemeral.
