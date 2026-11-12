# BUG_BACKLOG.md — 0.1.0 (@ 1898466b)

Agrupado por severidade. Evidência em `report.md`; baselines CRI em
`reports/cri-conformance-0.1.1/`. Todos reproduzidos em 2026-11-08.

## P1

### BUG-0001 — `make build`/`make install` não geram o binário `delonix-cri`
- **Component:** `Makefile:44` (`PACKAGES`), alvo `install`/`build`
- **Owner:** unassigned
- **Impact:** instalação limpa da árvore falha (`error: target/release/delonix-cri is missing` → `Error 1`). Qualquer utilizador que faça `make install` não consegue instalar o conjunto completo; o CRI fica de fora.
- **Reproduction:** `make install` numa árvore limpa de `develop`. `PACKAGES` lista o crate-biblioteca `delonix-cri` (sem `[[bin]]`) em vez de `delonix-cri-bin` (`bins/delonix-cri-bin/Cargo.toml`, que produz o bin `delonix-cri`). O ADR-0086 moveu o bin; `scripts/critest.sh` já foi corrigido, o `Makefile` não.
- **Acceptance criteria:** `make install` numa árvore limpa instala os 5 binários sem passos manuais; `make build` produz `target/release/delonix-cri`.
- **Related tests:** nenhum (ver M-TEST-03: adicionar gate).
- **Dependencies:** nenhuma. **Complexity:** XS.

### BUG-0003 — AppArmor não é imposto (4 specs CRI falham em rootful-VM)
- **Component:** `crates/adapters/delonix-linux/src/lib.rs:3370` (`apply_apparmor`, chamado em `:4583` run / `:9803` exec); `crates/adapters/delonix-linux/src/run_host.rs:58-131` (`ensure_apparmor`, carrega perfil via `apparmor_parser`); `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:1664-1705` (parse do security context: campo novo `apparmor`/`SecurityProfile` + fallback `apparmor_profile`), validação `:625`, reaplicação `:2049`.
- **Owner:** unassigned
- **Impact:** o runtime não impõe o perfil AppArmor pedido pelo pod — escritas que o perfil deveria bloquear passam. Falha de isolamento de segurança; bloqueia conformância CRI e uso como runtime de nó k8s onde se exijam perfis.
- **Reproduction:** critest rootful-VM, specs «AppArmor runtime should support apparmor field» (blocking writes; permissive deprecated) e «should prefer new apparmor field» (blocking writes; work with profile). Falham com root real (`apparmor_linux.go:252`) → não é «perfil não carregado por ser rootless».
- **Acceptance criteria:** os 4 specs AppArmor passam em rootful-VM; um perfil que bloqueia escritas é efectivamente imposto ao container (escrita negada com EACCES).
- **Related tests:** critest `-ginkgo.focus=AppArmor`.
- **Dependencies:** investigar se `ensure_apparmor` carrega o perfil que a suite instala e se `apply_apparmor` (aa_change_onexec) corre no caminho rootful. **Complexity:** M.

### BUG-0004 — HostNetwork e HostIpc não suportados (4 specs CRI)
- **Component:** `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:38-43` (campos `host_network`/`host_ipc`), `:86-92` (`ns_mode` → `NamespaceMode::Node`), `:1093-1098`; `crates/adapters/delonix-linux/src/launch_spec.rs:43` (`host_ipc` transportado).
- **Owner:** unassigned
- **Impact:** pods que pedem host netns ou host ipc não obtêm a partilha — quebra DaemonSets de rede/monitorização e qualquer carga host-network (o modo em que o k8s corre muitos add-ons).
- **Reproduction:** critest rootful-VM, «Security Context NamespaceOption» HostNetwork true/false e HostIpc true/false. O flag é **registado e propagado**, mas o Explore não encontrou mecanismo de *join* de IPC-namespace do host no `delonix-linux` (sem `setns` IPC) — provável raiz do gap.
- **Acceptance criteria:** os 4 specs passam em rootful-VM; container com `hostIpc`/`hostNetwork` partilha efectivamente o namespace do host.
- **Related tests:** critest `-ginkgo.focus=NamespaceOption`.
- **Dependencies:** habilita GAP-0001 (PodPID) e GAP-0003 (port-forward host net). **Complexity:** L.

## P2

### BUG-0005 — Mount Propagation `rshared` não funciona (host↔container)
- **Component:** `crates/adapters/delonix-linux/src/lib.rs:2504-2514` (decisão de propagação da raiz: `MS_PRIVATE` por omissão, muda quando algum mount `wants_propagation()`), `:1505-1508`/`:1715-1718` (`mount()` com `MS_SLAVE`/`MS_SHARED`); `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:3357-3361` (`PropagationBidirectional` → `rshared`).
- **Owner:** unassigned
- **Impact:** montagens criadas no host depois do arranque não propagam bidireccionalmente para o container (e vice-versa). Afecta CSI/mount-propagation em k8s.
- **Reproduction:** critest rootful-VM, «Container Mount Propagation … `rshared` should support propagation from host to container and vice versa». (`rprivate`/`rslave`/readonly já passam.)
- **Acceptance criteria:** o spec `rshared` passa; um mount feito no host após o arranque aparece no container e vice-versa.
- **Related tests:** critest `-ginkgo.focus=Mount Propagation`.
- **Dependencies:** nenhuma. **Complexity:** M.

### BUG-0006 — OOMKilled não reportado (exitCode 137 / reason)
- **Component:** `crates/adapters/delonix-linux/src/lib.rs:146` (`died_of_oom`, lê `memory.events.local`), `:111`/`:126` (`parse_oom_kill`/`oom_kill_count`), tag `:9000`/`:9132-9144`; `crates/foundation/delonix-model/src/records.rs:227-230` (`exit_code()` → 137); `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:1022` (`exit_reason` → `"OOMKilled"`).
- **Owner:** unassigned
- **Impact:** um container morto por OOM não é reportado como `OOMKilled`/137 — operadores e o k8s perdem o sinal de restart por memória.
- **Reproduction:** critest rootful-VM, «Container OOM … should terminate with exitCode 137 and reason OOMKilled». A infra existe end-to-end; falha mesmo com root real.
- **Acceptance criteria:** o spec passa em rootful-VM; container que estoura o limite de memória reporta exit 137 + reason OOMKilled.
- **Related tests:** critest `-ginkgo.focus=OOM`.
- **Dependencies:** confirmar se `memory.max` é aplicado e `memory.events.local` é lido no cgroup do container dentro da VM. **Complexity:** M.

## Fora deste ficheiro
- A falha `same_behaviour "stats all"` do tier `fast` **NÃO é bug do produto**
  (handler idêntico, verificado por `diff`). É defeito do harness → **M-TEST-02**
  no `BACKLOG.md`. A divergência deliberada seccomp `nil`=unconfined fica como
  flag conformance-only, não bug.
