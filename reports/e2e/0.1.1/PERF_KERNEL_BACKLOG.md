# PERF_KERNEL_BACKLOG.md — 0.1.1 (@596663c6)

**Eixo de prioridade de topo (decisão do dono, 2026-11-17):** tornar o delonix
cada vez mais uma ferramenta **nativa ao kernel Linux**, ser rígido quanto à
performance e tirar melhor proveito do Rust para os problemas que o Rust resolve
muito bem (segurança de memória, zero-cost abstractions, ABI/FFI controlado).

Inventário do que já é nativo (forte): namespaces completos (`delonix-linux`,
syscalls directas, "mini-runc como biblioteca"), **new mount API** `fsopen`/
`fsmount`/`move_mount` via `rustix` (`lib.rs:2146-2270`, ADR-0037), seccomp via
`seccompiler` (`seccomp_profile.rs`), cgroup v2 + freezer + PSI por fs directo
(`workload_view.rs:83`, `pressure.rs:296`), capabilities `capset` v3 cru
(`lib.rs:862`). Crates de base: `nix`/`libc`/`rustix`/`seccompiler`.

As 5 alavancas de maior retorno + higiene. Numeração própria `KRN-NNNN`.

## Estado (2026-11-17) — KRN-0001 e KRN-0004 com FATIA 1 FEITA (não committado)

| Item | Fatia 1 entregue | Prova | Teste | Gate |
|------|------------------|-------|-------|------|
| **KRN-0004** ✅ | Landlock FS sandbox best-effort no init (`delonix-linux/src/lib.rs` `apply_landlock`), opt-in via `--read-only`; cobre rootfs RO + tmpfs/`--tmpfs`/binds rw. dep `landlock=0.4` | `run --read-only --tmpfs /scratch`: tmpfs rw escreve, rootfs RO nega; EACCES distinto de EROFS prova enforcement | 250+ testes delonix-linux verdes; regressão em `scripts/e2e.sh` | `arch_fitness` verde (baseline `library_prints` 111→113, 2 eprintln de segurança) |
| **KRN-0005** ✅ (f1) | Sonda de KVM nativa via `kvm-ioctls=0.25` (rust-vmm): `kvm_usable()` em `delonix-provider-cloud-hypervisor` faz `Kvm::new()`+`get_api_version()==12` em vez de `Path::new("/dev/kvm").exists()` (que mentia com device presente mas sem permissão). NÃO toca no boot (VMM continua externo) | host com /dev/kvm: `provider ls`/`describe` Ok; strace `openat(/dev/kvm)+ioctl(KVM_GET_API_VERSION)=12` | 44 testes provider (sonda concorda com abertura real) | `arch_fitness` + `cargo check --workspace` verdes |
| **KRN-0003** ✅ | io_uring no I/O host-side: `copy_file_uring` (novo `delonix-oci/src/uring.rs`, crate `io-uring=0.7` síncrono) com fallback `std::fs::copy`; aplicado no `delonix cp`. HOST-side apenas (seccomp do container mantém io_uring_* negado) | `cp` roundtrip host→container→host de 1.5 MiB com sha256 idêntico; strace confirma 4 syscalls io_uring em ROOTLESS (caminho rápido exercitado, não o fallback) | 198 testes delonix-oci (byte-identical + mode + empty) | `arch_fitness` + `cargo check --workspace` verdes |
| **KRN-0006** ✅ | `cap_num` via crate `caps=0.5` (removida tabela ABI manual de ~45 entradas); `cap_name` amarrado ao crate por 2 testes (round-trip + concordância directa com `caps::all()`) | n/a (refactor); garantido por testes | 252 testes delonix-linux verdes | `arch_fitness` + `cargo check --workspace` verdes |
| **KRN-0002** ✅ | pidfd no envio de sinal: `send_signal` (`delonix-linux/src/lib.rs`) abre `pidfd_open` antes do check de identidade e usa `pidfd_send_signal` (fallback `kill` < 5.3) — fecha a janela TOCTOU de PID-reuse de `kill`/`stop`/docker-api | `kill`/`stop` matam via pidfd (kernel 7.0); teste de regressão: pidfd dá ESRCH após reap | 251 testes delonix-linux verdes | `arch_fitness` + `cargo check --workspace` verdes |
| **KRN-0001** ✅ | **F1** `link_exists`→`RTM_GETLINK` (novo `delonix-sdn/src/netlink.rs`, ponte sync→async tokio `block_on`; mata o bug de stdout-sniffing). **F2** `link_set_up`→`RTM_NEWLINK` (`LinkUnspec::new_with_index().up()`). **F3** `addr_add`→`RTM_NEWADDR` (`address().add()`). **Todos os call-sites HOST-SIDE migrados** (10 em infra.rs: link exists/up + addr add). deps `rtnetlink=0.24`+`tokio`+`futures-util` | F1 `lo`→true. F2/F3 rede e2e: bridge+veth `UP,LOWER_UP`, **gateway alcançável** (addr_add nativo), ingress porta HTTP 200, DNS, c2c | 349 testes delonix-sdn verdes | `arch_fitness` + `cargo check --workspace` verdes |

**Fatia 4 (COMPLETA, excepto sync dinâmico)**: `with_netns` (setns CLONE_NEWNET numa throwaway thread, padrão de `cni::set_netns_sysctls`) corre netlink DENTRO do netns do container. Novas ops nativas: `route_add_default` (RTM_NEWROUTE v4+v6) e `addr_add_flags` (IFA_F_NODAD). `do_attach` e `do_attach_extra` totalmente migrados (v4+v6: addr/link/route). **Provado em rootless**: container com v4 (`UP,LOWER_UP`) + v6 ULA nodad + rotas default v4 e v6, conectividade + ingress HTTP 200. **Zero `ip netns exec` no setup de interface.** Só resta na CLI o `sync_workload_routes` (query+diff+mutate de rotas com parsing — fatia própria da família route).

Restam: KRN-0001 `sync_workload_routes` (família route: show/add/del tipados). KRN-0004 fatia 2 (opt-in próprio). KRN-0002/0003/0005/0006 por começar.

---

## P1 (performance / caminho quente)

### KRN-0001 — Netlink nativo (`rtnetlink`) em vez de `ip`/`tc`/`bridge`/`nft` por CLI
- **Current behaviour:** todo o datapath SDN faz `Command::new("ip"/"tc"/"nft")`: `crates/adapters/delonix-sdn/src/infra.rs:4821,4921,5291,5583`, `cni.rs:703-773`, `wg.rs:75`, slirp em `infra.rs:2522`. Dezenas de fork+exec+parse-de-texto por arranque de container/VM.
- **Opportunity:** substituir por `rtnetlink`/`netlink-packet` (e avaliar `nftables`/`rustables` para o firewall). Ganhos: menos latência no caminho quente (sem fork+exec+parse), **erros estruturados** em vez de parsing de texto, e remoção da dependência de iproute2/nftables instalados no host — mais perto de "nativo ao kernel".
- **Acceptance criteria:** criação de netns+veth+bridge+addr de um container sem invocar `ip` (medir redução de syscalls/latência no arranque vs baseline); fallback documentado. Benchmark antes/depois em `tests/`.
- **Dependencies:** nenhuma. **Complexity:** XL. **Tier:** B.

### KRN-0003 — io_uring no I/O massivo (logs CRI, cópias CAS, copy-up de overlay)
- **Current behaviour:** AUSENTE por precaução (`crates/adapters/delonix-linux/src/lib.rs:719` — "io_uring is LEFT OUT, as in Docker"); async = `tokio` epoll (`delonix-cri/src/lib.rs:500`). Mas o runtime move muitos bytes: streaming de logs (`delonix-cri/src/streaming.rs`), extracção/cópia de layers OCI, copy-up de overlay.
- **Opportunity:** backend `io-uring`/`tokio-uring` **no lado-host do runtime** (nunca dentro do seccomp do container — manter `io_uring_*` negado aí). Maior ganho de performance puro em I/O, menos syscalls e context switches.
- **Acceptance criteria:** caminho de I/O de logs e de cópia de layers com backend io_uring opt-in, com benchmark de throughput/latência vs epoll; io_uring continua negado no perfil seccomp do container.
- **Dependencies:** nenhuma. **Complexity:** XL. **Tier:** B.

## P1 (robustez / segurança)

### KRN-0002 — pidfd como supervisão primária de containers/VMMs (hoje PARCIAL)
- **Current behaviour:** `pidfd` só no `exec` (`crates/interfaces/delonix-cri/src/child_handle.rs:62,94`, ADR-0027). O core ainda assinala por PID e reconhece a lacuna (`delonix-linux/src/lib.rs:3401` "to fix with a pidfd"). O campo `pidfd` do `clone3` já existe na struct mas não é consumido (`lib.rs:11900`).
- **Opportunity:** `clone3(CLONE_PIDFD)` + `pidfd_open`/`pidfd_send_signal` em todo o ciclo de vida (supervisor `launch.rs`, watchers). Fecha a janela de reutilização de PID na supervisão/sinalização — correcção e robustez.
- **Acceptance criteria:** supervisor e watchers assinalam por pidfd; teste que provoca reciclagem de PID e confirma que o sinal vai ao processo certo.
- **Dependencies:** liga-se a GAP-0026 (ensure-running). **Complexity:** L. **Tier:** B.

### KRN-0004 — Landlock como sandbox de FS unprivileged (AUSENTE)
- **Current behaviour:** confinamento de FS só via namespaces+seccomp+caps+AppArmor; `landlock_create_ruleset` nunca é chamado (só listado em `seccomp_profile.rs:148-150`).
- **Opportunity:** Landlock é *unprivileged* por design — encaixa no modelo **rootless-first**. Um ruleset no `init` do container (e nos helpers/daemons do próprio runtime) dá defesa-em-profundidade de FS sem exigir capabilities. Ganho de segurança de baixo custo.
- **Acceptance criteria:** ruleset Landlock aplicado ao init do container (opt-in por política); teste que prova que um acesso fora do ruleset é negado mesmo sem AppArmor. Avaliar o crate `landlock`.
- **Dependencies:** nenhuma. **Complexity:** M. **Tier:** B.

## P2 (investimento de plataforma)

### KRN-0005 — KVM/virtio nativo em Rust (rust-vmm) no caminho microVM
- **Current behaviour:** `/dev/kvm` só sondado (`crates/providers/delonix-provider-cloud-hypervisor/src/lib.rs:256`); todo o VMM é externo (cloud-hypervisor por api-socket; libvirt por `virsh`). Sem `kvm-ioctls`/`vhost`/`virtio-bindings`.
- **Opportunity:** integrar o ecossistema rust-vmm (que o próprio Cloud Hypervisor usa) para arranque de microVMs sem depender do binário externo nem de parsing de api-socket — mais controlo, menos superfície de processo, melhor proveito do Rust no subsistema de virtualização.
- **Acceptance criteria:** caminho mínimo de boot de microVM via `kvm-ioctls` (PoC medido vs cloud-hypervisor externo). Decisão por ADR dado o tamanho.
- **Dependencies:** nenhuma. **Complexity:** XL. **Tier:** D (precisa `/dev/kvm`/VM).

#### Prompt de execução (sessão futura, Tier D — num host com `/dev/kvm`)

```
/escopo

KRN-0005 fatia 1 — adoptar o ecossistema rust-vmm (kvm-ioctls) no delonix,
começando pela peça MAIS ISOLADA e de menor risco, sem tocar no caminho de boot
do cloud-hypervisor. É Tier D: precisa de `/dev/kvm` (host bare-metal com KVM ou
uma VM com nested virt). Dep nova `kvm-ioctls=0.25` (ADR-0040: versão no
workspace; membro `{ workspace = true }`).

Contexto: hoje o VMM é 100% externo (cloud-hypervisor por api-socket, atrás do
port VmBackend de delonix-compute) e o KVM só é SONDADO por existência de ficheiro
(`delonix-provider-cloud-hypervisor/src/lib.rs:256` -> `Path::new("/dev/kvm").exists()`).

FATIA 1 recomendada (a mais segura): substituir a sonda de existência por uma
sonda NATIVA via kvm-ioctls — `Kvm::new()` + `get_api_version()` (== 12) +
`check_extension` de umas poucas capabilities-chave (USER_MEMORY, etc.). Isto:
(a) adopta o rust-vmm de forma mínima e provável; (b) melhora o diagnóstico
(hoje `/dev/kvm` pode existir mas o processo não ter permissão / nested-virt estar
off — a sonda de existência mente); (c) NÃO toca no caminho de boot (zero risco de
regressão de VMs reais). Fallback: se `Kvm::new()` falhar (sem permissão, sem
device), reportar NoKvm com a razão do errno, como hoje mas mais preciso.

FATIA 2+ (fora deste escopo, ADR obrigatório): PoC de boot de microVM mínima
(Kvm + VmFd + VcpuFd + vm-memory + linux-loader a correr um kernel trivial),
medido vs cloud-hypervisor; depois virtio-blk/net via vhost. É XL — só avançar
com ADR que nomeie o consumidor concreto.

Done when:
- A sonda de capacidade do provider cloud-hypervisor usa kvm-ioctls (Kvm::new +
  api_version + check_extension), com fallback de erro preciso; compila e
  `cargo test -p delonix-provider-cloud-hypervisor` passa.
- Prova ao vivo num host com /dev/kvm: `delonix provider ls`/`describe
  cloud-hypervisor` reporta KVM OK via o caminho nativo (e NoKvm+razão quando
  /dev/kvm é removido/sem permissão). Teste de regressão da sonda.
- arch_fitness + cargo check --workspace verdes.

Fora de escopo: substituir o boot do cloud-hypervisor; virtio/vhost; libvirt.
```

### KRN-0006 — Higiene de crates: `caps` e `aya`/`libbpf-rs` (menos `unsafe`/ABI frágil)
- **Current behaviour:** `capset` v3 escrito à mão (`delonix-linux/src/lib.rs:862-899`); device-cgroup eBPF escreve `bpf_attr` a 128 bytes com offsets manuais (`lib.rs:5372-5408`); datapath tc/clsact por objecto C+clang (`delonix-sdn/src/bpf.rs`, `build.rs:51`). Sem `aya`/`libbpf-rs`.
- **Opportunity:** adoptar `caps` (substituir o capset cru) e `aya`/`libbpf-rs` (eliminar o cálculo manual de offsets de `bpf_attr`). Não dá perf nova, mas remove `unsafe`/ABI frágil — alinhado com "tirar melhor proveito do Rust".
- **Acceptance criteria:** capset e device-cgroup/eBPF passam a usar crates do ecossistema, sem offsets manuais; testes existentes continuam verdes.
- **Dependencies:** nenhuma. **Complexity:** M. **Tier:** B (eBPF device-cgroup precisa validação rootful — parcial D).

---

## Oportunidades de nativização adicionais (levantamento 2026-11-17, KRN-0007..0014)

Varrimento de `Command::new`/`run`/`capture` por ferramenta externa em todo o
motor. Lista o que AINDA depende de binário externo + parsing de texto e podia
ser syscall/netlink/crate nativo. Fora de âmbito (externos por natureza):
`git`, `systemctl`/`busctl` (systemd/dbus), `ssh`/`scp`, `kubectl`/`helm`,
`qemu-img`/`virt-*` (libguestfs/qcow2), `sudo`, `nvidia-smi`.

### KRN-0007 — Firewall/NAT por netlink NFTABLES (substitui `nft` CLI)
- **Current behaviour:** ~64 invocações de `nft` (das quais 57 em `crates/adapters/delonix-sdn/src/infra.rs`, + `policy_nft.rs:4`) constroem regras por texto e lêem estado por parsing. É o maior bloco de CLI+texto que resta e está no caminho quente da rede.
- **Opportunity:** netlink NFTABLES nativo (crate `rustables`/`nftables`) — regras tipadas, erros estruturados, sem fork+parse, sem dependência do binário `nft`.
- **Acceptance criteria:** uma família de regras (ex. a cadeia de publish/DNAT de um container) construída por netlink; publicação de porta + isolamento provados ao vivo; fallback/migração incremental. **ADR** dado o tamanho.
- **Dependencies:** nenhuma. **Complexity:** XL. **Tier:** B.

### KRN-0008 — Resto do `ip` por rtnetlink (netns mgmt, rule, route show)
- **Current behaviour:** ~31 `ip` restantes após a migração de link/addr/route (`delonix-sdn/src/infra.rs:19`, `lan.rs:4`, `cni.rs:4`, `service_lb.rs:2`): criar/apagar netns, `ip rule`, `ip route show` (o `sync_workload_routes` com parsing — infra.rs:~5652), `bridge fdb`.
- **Opportunity:** completar o `rtnetlink` já integrado (KRN-0001) para route show/add/del tipado + rule + netns, retirando o último parsing de texto de rede.
- **Acceptance criteria:** `sync_workload_routes` e a gestão de netns sem `ip netns exec`/`ip route show`; testes de rede e2e intactos.
- **Dependencies:** sobre KRN-0001. **Complexity:** L. **Tier:** B.

### KRN-0009 — `setns`/`unshare` por syscall (substitui `nsenter`/`unshare` CLI)
- **Current behaviour:** `nsenter` ×9 + `unshare` ×10 como BINÁRIOS (18 em `delonix-sdn/src/infra.rs`), para entrar/criar namespaces.
- **Opportunity:** `setns(2)`/`unshare(2)` directos — o padrão da throwaway-thread `with_netns` (`delonix-sdn/src/netlink.rs`, provado nesta ronda) generaliza para estes call-sites.
- **Acceptance criteria:** os call-sites host-side de `nsenter`/`unshare` passam a syscall; arranque de container/VM e rede e2e intactos.
- **Dependencies:** nenhuma (padrão provado). **Complexity:** M. **Tier:** B.

### KRN-0010 — eBPF datapath por `aya` (substitui `tc` + objecto C/clang)
- **Current behaviour:** `delonix-sdn/src/bpf.rs` + `build.rs:51` compilam um objecto eBPF em **C com clang**, carregado via `tc`.
- **Opportunity:** reescrever o datapath em `aya` (eBPF em Rust puro, sem toolchain C), carregado por netlink/bpf syscall nativo.
- **Acceptance criteria:** o programa eBPF de observabilidade/datapath passa a `aya`; métricas por-container iguais; sem clang no build.
- **Dependencies:** nenhuma. **Complexity:** L. **Tier:** B (parcial D).

### KRN-0011 — device-cgroup sem `bpf_attr` manual (via `aya`/`libbpf-rs`)
- **Current behaviour:** `delonix-linux/src/lib.rs:5460` escreve um `bpf_attr` (PROG_LOAD) a 128 bytes com offsets calculados à mão (`bpf_insn` em `:5427`) — frágil a mudanças de ABI.
- **Opportunity:** `aya`/`libbpf-rs` eliminam o cálculo manual de offsets do device-cgroup eBPF.
- **Acceptance criteria:** device-cgroup carregado via crate do ecossistema; política de devices igual; testes verdes.
- **Dependencies:** acopla a KRN-0010 (mesma escolha de crate). **Complexity:** M. **Tier:** D (device-cgroup precisa validação rootful).

### KRN-0012 — Escrita de capabilities via crate `caps` (capset cru)
- **Current behaviour:** `capset` v3 escrito por syscall cru em `delonix-linux/src/lib.rs:930` (`switch_user_keeping_caps`) e `:1024` (`drop_capabilities`). KRN-0006 migrou só a LEITURA (`cap_num`).
- **Opportunity:** `caps::set`/`drop` em vez do `SYS_capset` + struct manual.
- **Acceptance criteria:** o confinamento mantém-se EXACTO (KEEPCAPS, bounding drop, ordem setgid/setuid), provado por conformância CRI de capabilities. **CUIDADO: caminho de segurança crítico.**
- **Dependencies:** nenhuma. **Complexity:** M, risco alto. **Tier:** D (precisa rootful-VM para provar confinamento).

### KRN-0013 — WireGuard por netlink (substitui `wg` CLI)
- **Current behaviour:** `wg` ×2 (`delonix-sdn/src/wg.rs`) configura interfaces WireGuard por CLI.
- **Opportunity:** genetlink nativo (crate `wireguard-control`/`defguard_wireguard_rs`).
- **Acceptance criteria:** criação/config de um peer WireGuard por netlink; túnel provado.
- **Dependencies:** nenhuma. **Complexity:** M. **Tier:** B.

### KRN-0014 — Menores nativizáveis (agrupados)
- `bridge` fdb/vlan → rtnetlink; `ss` → `sock_diag` netlink; `losetup` → ioctl `LOOP_CONFIGURE`; `getent` → `getpwnam`/`getgrnam` (libc). Cada um S, baixo valor isolado, mas remove dependências de binários externos.
- **Complexity:** S cada. **Tier:** B.

> Nota: o boot nativo de microVM (KVM) é a **fatia 2 de KRN-0005** (já com prompt acima), não um item novo.

---

## Nativização de PROTOCOLO em Rust (NAT-0001..0005) — não kernel-nativo

Categoria distinta dos KRN: aqui não há syscall que substitua o binário — o
ganho é falar o PROTOCOLO num crate Rust (tipado, sem fork, sem parsing de
texto) em vez de `fork+exec+parse` de um CLI. Só vale onde há parsing frágil ou
dependência de binário a remover. Prefixo `NAT-` para não confundir com os KRN.

### NAT-0001 — systemd/D-Bus via `zbus` (substitui `busctl`/`systemctl`) ✅ melhor candidato
- **Current behaviour:** `busctl` (`delonix-linux/src/lib.rs:11760`) e `systemctl` (`bins/delonix/src/cmd/vm.rs:2521`, boot.rs units) — texto parseado, frágil; fork por operação.
- **Opportunity:** crate `zbus` (D-Bus puro Rust) contra `org.freedesktop.systemd1` (StartUnit/EnableUnitFiles/etc.). O systemd continua externo (não o controlamos), mas a conversa passa a tipada.
- **Ganho:** ALTO (o `busctl` devolve texto; D-Bus é tipado). **Acceptance:** uma operação de unit (enable/start de boot-persistence) via zbus, provada ao vivo; sem parse de `busctl`.
- **Complexity:** M. **Tier:** B.

### NAT-0002 — cliente k8s via `kube` (kube-rs) (substitui `kubectl`)
- **Current behaviour:** `kubectl` (`bins/delonix/src/cmd/k8s_recipes.rs:104`, aplicar/gerar manifestos) — depende do binário e da sua versão; parse de output.
- **Opportunity:** `kube` (kube-rs) — apply/get/watch tipado contra a API-server.
- **Ganho:** ALTO se a interacção k8s for programática (tipos, erros estruturados, sem binário). **Acceptance:** um apply/get via kube-rs equivalente ao `kubectl` actual, provado contra um cluster.
- **Complexity:** L. **Tier:** B (precisa de um cluster para e2e).

### NAT-0003 — GPUs via `nvml-wrapper` (substitui `nvidia-smi`)
- **Current behaviour:** `nvidia-smi` (`delonix-linux/src/resource_advice.rs:571`) parseado para contagem/memória/utilização de GPU.
- **Opportunity:** crate `nvml-wrapper` (NVIDIA Management Library) — query tipada. Atrás de feature-flag (arrasta `libnvidia-ml`, proprietária, só hosts NVIDIA).
- **Ganho:** MODERADO-ALTO (remove parse frágil). **Acceptance:** contagem/uso de GPU via NVML == o que o `nvidia-smi` reportava; build sem NVIDIA continua a compilar (feature off).
- **Complexity:** M. **Tier:** B (precisa host NVIDIA para e2e).

### NAT-0004 — git via `gix`/`git2` (substitui o binário `git` no gitops)
- **Current behaviour:** `git` CLI no gitops (pull de sources, ADR-0021) e em build-time (`delonix-node-api/build.rs:58` — este é só `git describe`, NÃO migrar).
- **Opportunity:** `gix` (Rust puro, sem C) ou `git2` (libgit2) para clone/fetch/checkout do gitops, sem depender do binário `git`.
- **Ganho:** MODERADO — sobretudo para IMAGENS DISTROLESS do control-plane (sem `git`). **Acceptance:** um reconcile GitOps (clone+checkout de um ref) sem o binário `git`; o `git describe` de build fica como está.
- **Complexity:** L. **Tier:** B.

### NAT-0005 — SSH via `russh` (substitui `ssh`/`scp`) — só com controlo fino
- **Current behaviour:** `ssh` (`delonix-provider-proxmox/src/lib.rs:9322`) e `scp` (`bins/delonix/src/cmd/remote.rs:182`) — bootstrap de cluster/remote por CLI.
- **Opportunity:** `russh` (SSH2 puro Rust) — sessão programática (exec, SFTP), streaming de progresso, multiplexing.
- **Ganho:** MODERADO (o `ssh` CLI é muito robusto; SSH é superfície grande — auth, known_hosts). Só compensa com necessidade de controlo fino do canal. **Acceptance:** um exec remoto + transferência via russh equivalentes; known_hosts/auth preservados.
- **Complexity:** L, risco médio. **Tier:** B.

> MANTER EXTERNO (sem retorno / risco alto): `qemu-img`/`virt-*` (qcow2/libguestfs — formato complexo, qemu é a referência), `helm` (templating Go), `sudo` (mecanismo de escalada — o caminho "nativo" seria helpers setuid próprios como `delonix-storage-helper`, re-arquitectura de privilégio, não um crate).

---

## Sequência sugerida dentro do eixo
KRN-0001 (netlink) e KRN-0003 (io_uring) primeiro — maior ganho de performance no
caminho quente. KRN-0002 (pidfd) e KRN-0004 (landlock) a seguir — robustez e
segurança de baixo custo. KRN-0005 (KVM nativo) é o maior investimento (ADR).
KRN-0006 pode correr em paralelo como higiene contínua.
