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

### KRN-0006 — Higiene de crates: `caps` e `aya`/`libbpf-rs` (menos `unsafe`/ABI frágil)
- **Current behaviour:** `capset` v3 escrito à mão (`delonix-linux/src/lib.rs:862-899`); device-cgroup eBPF escreve `bpf_attr` a 128 bytes com offsets manuais (`lib.rs:5372-5408`); datapath tc/clsact por objecto C+clang (`delonix-sdn/src/bpf.rs`, `build.rs:51`). Sem `aya`/`libbpf-rs`.
- **Opportunity:** adoptar `caps` (substituir o capset cru) e `aya`/`libbpf-rs` (eliminar o cálculo manual de offsets de `bpf_attr`). Não dá perf nova, mas remove `unsafe`/ABI frágil — alinhado com "tirar melhor proveito do Rust".
- **Acceptance criteria:** capset e device-cgroup/eBPF passam a usar crates do ecossistema, sem offsets manuais; testes existentes continuam verdes.
- **Dependencies:** nenhuma. **Complexity:** M. **Tier:** B (eBPF device-cgroup precisa validação rootful — parcial D).

---

## Sequência sugerida dentro do eixo
KRN-0001 (netlink) e KRN-0003 (io_uring) primeiro — maior ganho de performance no
caminho quente. KRN-0002 (pidfd) e KRN-0004 (landlock) a seguir — robustez e
segurança de baixo custo. KRN-0005 (KVM nativo) é o maior investimento (ADR).
KRN-0006 pode correr em paralelo como higiene contínua.
