# Avaliação a fundo — delonix como base da plataforma NgolaCloud (2026-11-18)

Base: `origin/develop` @ `118a4132` (+309 commits desde v0.1.0). Worktree
`fix/platform-blockers-w1`. Doutrina: só o efeito/contrato verificado conta.

## Método
Auditoria read-only em 4 lentes (rootless · daemonless/estado · OCI/CRI ·
segurança/rede), cruzada com docker/podman/containerd/runc/k8s. Cada achado com
evidência `ficheiro:linha`. Base de testes medida antes: `make check` limpo,
`make test` **3339 testes / 0 falhas**, `version_gate` ok, `arch_fitness` 1 falha
trivial (baseline `cli_context_lines` −6).

## Ângulo de plataforma
O control-plane NÃO liga ao runtime como biblioteca (`delonix-seiva` é o framework
Seiva, não o runtime). O delonix **é** a plataforma de execução: corre/orquestra
os workloads e comanda nós pela node-api gRPC + CRI. O risco concentra-se em
**multi-tenancy rootless**, **recuperação daemonless** e **conformidade CRI/OCI**.

## 27 achados novos (fora de BUG-0007..0009, GAP-0018..0027, KRN/NAT)

### 🔴 Bloqueadores (8) — estado de confirmação
| # | Achado | Evidência | Confirmação |
|---|---|---|---|
| R1 | subuid/subgid: base host fixa 100000, ignora `start` real de `/etc/subuid` | `delonix-linux/src/lib.rs:3160` (`"0 {euid} 1 1 {USERNS_UID_BASE} {range}"`), `:3238` (`subid_file_grants` descarta `_start`) | ✅ código. Host actual `walter:100000` = caso feliz; reprodução exige 2º tenant (tier-VM/sudo) |
| R2 | netns do host partilhada por omissão (sem `-p`) | `launch.rs:61` (`new_netns` só `net_none`/`slirp_ports`), `lib.rs:9003` (`CLONE_NEWNET` só se `new_netns`), teste `lib.rs:16172` | ✅✅ **AO VIVO**: container sem `-p` leu `127.0.0.1:45999` do host; `ip addr` vê wlp0s20f3/br-*/virbr0 do host |
| R3 | `default_root` usa `geteuid()` e não `in_initial_userns()` | `delonix-state/src/store.rs:319`, `delonix-oci/src/image.rs:166` (vs correcção já feita em `lib.rs:3174-3177`) | ✅ código |
| D1 | `JsonStore<T>` sem downgrade-guard (VMs/túneis/gateways/object-storage) | `delonix-state/src/store.rs:730` (`load` sem gate) vs `:349` (`Store::decode` tem gate) | ✅ código |
| D2 | boot-persistence por nome nu, sem namespace | `boot.rs:398` (`{UNIT_PREFIX}{c.name}`), `:415` (vm) | ✅ código (já separa container/VM por prefixo, não por tenant) |
| O1 | pull multi-arch cai no 1º manifesto; `architecture` grava a pedida | `registry.rs:2232` (`.or_else(|| index.manifests().first())`), `:2434` (`architecture: requested_arch`) | ✅ código |
| O2 | `repo_digests` vazio em pull por tag → `imageID` kubelet | `registry.rs:2414` (só `repo_tags`), `delonix-cri/src/lib.rs:426` (`partition_refs` exige `@`) | ✅ código |
| S1 | `workloadSecurity: baseline` não refuta `--network host` | `admission.rs:519-561` (cobre `host_pid`/`host_ipc`, falta `host_network`) | ✅ código |

### 🟡 Médios (10)
O3 variant ARM ignorado (`registry.rs:2229`) · O4 manifesto Docker-v2 com mediaTypes
OCI (`registry.rs` `layer_media_type`) · O5 `load` não lê `docker save` clássico
(`load.rs:49`) · O6 `load` não valida layers (`load.rs:96`) · O7 `load`/`save`
bufferizam em memória → OOM (`load.rs:44`, `save.rs:153`) · D3 PID-reuse `starttime`
sem época de boot (`delonix-node/src/app/host.rs:119-137`) · D4 registos legados
confiam em PID reutilizado (`host.rs:135`) · R4 `/tmp/delonix-net-<uid>` previsível,
`set_permissions` ignorado (`delonix-sdn/src/infra.rs:286-358`) · S2 `fwguard` só
IPv4 (metadata v6 SSRF) (`infra.rs:749`) · S3 transição SELinux silenciosa
(`lib.rs:3764`) · S4 anti-spoof v6 link-local permissivo (`infra.rs:4463`).

### 🟢 Baixos / higiene (9)
O8 `Cas::verify` lê tudo em memória (`cas.rs:141`) · O9 CRI `inodes_used=0` + dupla
contagem (`cri/lib.rs:377`) · O10 `remove_image` engole erros (`cri/lib.rs:351`) ·
O11 resolve por prefixo ambíguo (`image.rs:296`) · D5 `--rm` watcher órfão
(`container.rs:4391`) · D6 reconcile duplo (`container.rs:4334`) · R5 slirp sem
`--enable-sandbox/seccomp` (`infra.rs:2507`) · S5 `CAP_NET_BROADCAST` no default
(`capabilities.rs:24`) · arch_fitness baseline.

## Decisões do dono (2026-11-18)
- **Campanha completa** até tudo verde (gate: `make ci` + rootless e2e + daemonless
  crash/reboot).
- **Alinhar defaults divergentes com Docker/Podman** (R2 netns isolada por omissão;
  R5 slirp sandbox; S5 CAP_NET_BROADCAST; GAP-0023 restartPolicy) com ADR + nota de
  migração por cada mudança de default.

## Ondas
F0 (esta) → W1 bloqueadores → W2 médios → W3 baixos → W4 nativização kernel
(KRN/NAT) + auditoria de áreas não-cobertas (VM/storage).

## Auditoria W4 — microVM + storage (5 achados novos, V1–V5)

Áreas que as 4 lentes iniciais não cobriram a fundo. Numeração `V`.

| # | Achado | Evidência | Estado |
|---|---|---|---|
| V1 | Volume quota-hard (loopback): remount pós-reboot falhado serve `_data` VAZIO em silêncio (`Ok`) → BD reinicializa datadir (perda/fork do dataset do tenant) | `delonix-volume/src/lib.rs:619` (`let _ = run("mount"...)`) | ✅ **CORRIGIDO** — remount falhado é erro duro; rootless não afectado (`img.exists()` falso) |
| V2 | `policy.json` só aplicada no CLI; node-API gRPC e CRI/kubelet não a consultam → bypass de `deny_latest`/`allowed_registries`/`deny_device_passthrough` | `delonix-node-api/src/vms.rs:166`, `delonix-cri/.../lifecycle.rs:1658` (comentário admite a lacuna) | ☐ ALTO — arquitectural (mover `policy::enforce` do `-bin` para o motor); próxima fatia |
| V3 | VFIO passthrough sem allowlist por-dispositivo; controlo é tudo-ou-nada global | `cloud-hypervisor/src/lib.rs:2128`, `libvirt/src/lib.rs:2125`, `admission.rs:474` | ✅ **CORRIGIDO** — `allowedPassthroughDevices` (excepção ao `denyDevicePassthrough`); `evaluate` recusa só os BDF fora da lista, match via `normalize_pci_bdf` (BDF nu ou caminho sysfs, case-insensitive); fail-closed p/ não-BDF |
| V4 | libvirt descarta `spec.devices` mal-formado (não-BDF) em silêncio (sem ramo `else`) | `libvirt/src/lib.rs:2125-2142` | ✅ **CORRIGIDO** — `validate_passthrough_devices` no topo do `boot` recusa qualquer não-BDF antes de qualquer efeito; mesmo parser do `<hostdev>`. (CH passa verbatim → falha alto no VMM, sem o defeito silencioso) |
| V5 | `libvirt_xml_overlay`: XML cru injectado verbatim, sem gate de admissão | `libvirt/src/lib.rs:2160` | ✅ **CORRIGIDO** — `denyLibvirtXmlOverlay` + `Violation::LibvirtXmlOverlay`; flag `Request::with_libvirt_xml_overlay` ligada no apply de manifesto (única superfície com XML cru). Prova viva do EFEITO fora de âmbito (precisa root+PCI sobrante) |

Áreas sólidas confirmadas: allowlist/ownership de pools de storage, validação de
nomes de volume, `dir_usage` (quota rootless), CH `stop`/cgroup-leaf/`move_namespace`,
clone/flatten crash-consistent, isolamento de namespace libvirt.

## Nativização ao kernel (W4) — fatias feitas
`ss` → `/proc/net/*` (vm.rs) · `losetup` → sysfs find + `LOOP_SET_CAPACITY` ioctl
(delonix-volume, fallback kernel antigo) · KRN-0001 F5 `sync_workload_routes` →
netlink tipado (SHOW/DEL/REPLACE). Backlog restante: KRN-0007 (nft netlink, XL/ADR),
KRN-0005 fatia 2 (boot microVM KVM, Tier-D), NAT-0001..0005.
