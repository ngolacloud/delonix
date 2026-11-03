# Sprint 08 — Validação de providers (§8) e E2E de instalação (ambiente limpo)

- **Release alvo:** Delonix v0.1.1 (linha `0.x`)
- **Binário medido:** `/home/walter/dlx-wt/stabilize-0.1.1/target/debug/delonix` → `delonix 0.1.0, catalog 1.4.0` (149 capacidades no catálogo)
- **Data:** 2026-10-31
- **Papel:** SRE. Nada foi construído, editado, commitado nem empurrado. Nenhuma VM pesada levantada. Apenas probes read-only.
- **§18 (regra de ouro):** ausência de ambiente ≠ aprovação. Abaixo distingo **validado-live** / **disponível-não-validado** / **bloqueado-por-ambiente**.

---

## A) §8 — Providers medidos no host

Saída real de `delonix provider ls` (contagem por estado contra o catálogo):

| ID | Kind | Avail | Health | SUP | PART | UNSUP | EXT | NOT-IMPL |
|----|------|-------|--------|-----|------|-------|-----|----------|
| cloud-hypervisor | compute | yes | **Ok** | 37 | 4 | 41 | 2 | 6 |
| libvirt | compute | yes | **Ok** | 41 | 10 | 30 | 2 | 7 |
| proxmox | compute | **no** | **NotConfigured** | 40 | 12 | 26 | 3 | 9 |
| linux | compute | yes | **Ok** | 19 | 3 | 67 | 0 | 1 |
| linux | network | yes | **Ok** | 20 | 6 | 20 | 1 | 1 |
| proxmox | network | **no** | **NotConfigured** | 7 | 7 | 25 | 0 | 9 |
| linux | storage | yes | **NetworkSharesNeedRoot** | 6 | 0 | 0 | 2 | 0 |
| opnsense | gateway | **no** | **NotConfigured** | 10 | 0 | 24 | 2 | 12 |

### Estado por provider

#### libvirt (compute) — disponível, Ok
- `available=true`; `provider ls` sonda `qemu:///system` e passa. libvirt 10 / QEMU 8.2.2 vivos no host.
- **Validado-live aqui:** a disponibilidade e o health (`Ok`) são medidos neste host. As 41 células `supported` citam evidência (battery/scenario/test) conforme `describe`. É o `defaultProvider` recomendado pelo install.sh.
- **Nota:** a 1ª capacidade (`provider.availability`) está marcada `partial` com limite escrito — `available()` só vê os 2 binários; é o `provider ls` que sonda de facto o `qemu:///system`.

#### cloud-hypervisor (compute) — disponível, Ok
- `available=true`; `provider ls` sonda `/dev/kvm` + firmware e passa.
- **Validado-live aqui:** disponibilidade/health medidos. 37 `supported` com evidência declarada.

#### linux (compute / network / storage) — disponível
- compute/network **Ok**; storage **NetworkSharesNeedRoot** (nfs/cifs/webdav precisam de `CAP_SYS_ADMIN` para montar; volumes locais não afetados → `3 unavailable-on-host` são declarados, não falhas).
- **Validado-live aqui.** `delonix system doctor` passa em todos os pré-requisitos (ver secção B).

#### proxmox (compute + network) — **NotConfigured** → BLOQUEADO-POR-AMBIENTE
- Confirmado `available=false`, `NotConfigured`. Hint: `set DELONIX_PROXMOX_URL/_NODE and a credential to register a target`.
- O appliance **pve92 foi podado hoje** (disco já não existe). `virsh list --all` mostra `pve92 shut off` sem disco utilizável.
- Todas as **47 células `supported`** (40 compute + 7 network) têm evidência que aponta para `crates/providers/delonix-proxmox/tests/live.rs` (ex.: `provider.readback`, `vm.create`, `net.dns.records`, `net.ipam.*`). Esses testes são `#[ignore]` e exigem um PVE alcançável via `DELONIX_PROXMOX_URL/_NODE` + credencial.
- **Quantificação do que fica por demonstrar:** sem reprovisionar um alvo PVE (proibido nesta sessão), **nenhuma** das 47 células supported pode ser re-provada live para a v0.1.1. Ficam também por exercitar 12+7 `partial` e os caminhos de erro. A evidência histórica em `live.rs` existe mas **não é re-corrível neste host** → classificar como **disponível-não-validado-nesta-release** (código presente, sem re-prova live).
- **Recomendação:** marcar proxmox como "regression não corrida" na matriz da v0.1.1; a prova live precisa de 1 nó PVE dedicado (reprovisionamento de pve92 ou equivalente) — fora do âmbito e do orçamento de ambiente desta sessão.

#### opnsense (gateway) — **NotConfigured**; VM viva mas INALCANÇÁVEL + SEM CREDENCIAL → BLOQUEADO-POR-AMBIENTE
Investigação feita (tudo read-only):
- A VM libvirt `opnsense` está **a correr** (`virsh list` → running; disco `/var/lib/libvirt/images/opnsense.qcow2`). Tem 5 NICs em bridges: `br-wan` (10.0.99.0/24), `br-lan` (192.168.10.0/24), `br-dmz` (192.168.20.0/24), `br-dev`, `br-backup`.
- **Sem guest-agent:** `virsh domifaddr opnsense` (lease e arp) devolve vazio — o host não sabe o IP de nenhuma interface da appliance.
- **GUI/API inalcançável a partir do host:**
  - `curl -sk https://<ip>/` nos IPs prováveis (192.168.10.1/.2/.254, 10.0.99.2, 192.168.20.1) → todos `000` (sem resposta).
  - Varrimento ping read-only em `192.168.10.0/24`: só responde `192.168.10.1` (que é o **próprio host** na br-lan); todas as entradas ARP para a appliance ficam `INCOMPLETE`. A appliance não responde no segmento LAN a partir do host (provável: firewall WAN a bloquear + a interface que o host vê não é a de gestão, ou a GUI escuta noutro segmento/porta não exposto ao host).
- **Requisitos do provider opnsense** (de `crates/providers/delonix-opnsense/src/lib.rs` e `capabilities.rs`):
  1. `DELONIX_OPNSENSE_URL` **https:// obrigatório** (`validate_target_url` recusa http — a key/secret iria em claro; recusa userinfo no URL);
  2. **credencial = API key + secret** (OPNsense usa par key/secret, não user/password), via `auth` em `providers.yaml` ou `DELONIX_OPNSENSE_KEY/_SECRET`;
  3. para o certificado self-signed: `tls.insecureSkipVerify` / `tls.caFile` ou `DELONIX_OPNSENSE_INSECURE_TLS=1`;
  4. `Client::connect` **prova a credencial à cabeça** com `GET core/firmware/status` antes de qualquer coisa.
- **O que falta exatamente para registar e sondar (§8 firewall/NAT/zonas):**
  - (a) **URL de gestão alcançável** — a GUI da appliance não está exposta ao host por nenhum IP conhecido; seria preciso acesso à consola serial (`/dev/pts/1`) para descobrir/configurar a interface de gestão, o que é mais do que um probe read-only e toca na VM do dono → **não feito**.
  - (b) **API key + secret** — têm de ser criadas DENTRO da GUI (System › Access › Users › API). Não foram fornecidas e não podem ser geradas sem login → **ausentes**.
  - Conclusão: mesmo que o URL fosse descoberto, sem credencial o `connect` falharia no `firmware/status`. **Dois bloqueios independentes.**
- **As 10 células `supported` do gateway** (`net.gateway.filter`, `net.gateway.alias`, `net.gateway.rule-order`, `net.nat.snat`, `net.nat.dnat`, `net.apply.staged`, `net.observe`, `net.ownership-marker`, `firewall.stateless`, `firewall.logging`) **citam todas evidência live** (`crates/providers/delonix-opnsense/tests/live.rs::*`, testes `#[ignore]` que exigem `DELONIX_OPNSENSE_TEST_URL/_KEY/_SECRET` + appliance alcançável).
- **Veredicto opnsense:** **bloqueado-por-ambiente** (inalcançável + sem credencial). Zero células §8 re-provadas live nesta sessão. A VM estar "viva" NÃO conta como aprovação (§18).

### Resumo §8 — o que dá para validar aqui vs. bloqueado

| Provider | Validável nesta sessão | Classificação v0.1.1 |
|----------|------------------------|----------------------|
| libvirt (compute) | SIM — avail/health Ok medidos | **validado-live** (disponibilidade/health) |
| cloud-hypervisor (compute) | SIM — avail/health Ok medidos | **validado-live** (disponibilidade/health) |
| linux (compute/network/storage) | SIM — doctor passa; storage shares precisam root | **validado-live** |
| proxmox (compute+network) | NÃO — NotConfigured, pve92 podado | **bloqueado-por-ambiente** (47 supported por re-provar; precisa PVE dedicado) |
| opnsense (gateway) | NÃO — VM viva mas inalcançável + sem API key/secret | **bloqueado-por-ambiente** (10 supported §8 por re-provar) |

> Nota sobre "supported": a matriz marca `supported` com base em evidência (test/scenario/battery) gravada no catálogo. Para proxmox e opnsense essa evidência vem de testes **live** `#[ignore]` que **não são re-corríveis neste host na v0.1.1**. Para o gate de release, tratar como "código presente + evidência histórica, re-prova live pendente de ambiente", não como "aprovado agora".

---

## B) E2E de instalação em ambiente limpo (`scripts/install.sh`)

### O que o install.sh faz (1907 linhas, lido)
- Instala uma **release publicada** (binário assinado + irmãos + deps de runtime + config de host). **Não compila** (isso é o Makefile). Uso documentado: `curl … | bash` — todo o corpo está dentro de `{ … }` como guarda contra download truncado.
- **Detecção de distro** por `ID`/`ID_LIKE` de `/etc/os-release`, com fallback pelo gestor presente:
  - `apt` (Debian/Ubuntu/Mint/Zorin/Pop) · `dnf` (Fedora/RHEL/CentOS/Rocky/Alma) · `zypper` (openSUSE/SLES) · `pacman` (Arch/Manjaro/Endeavour). Distro sem nenhum → `die`.
- **Deps de runtime** (lição de instalações reais): `slirp4netns`, `uidmap` (newuidmap/newgidmap setuid), `nftables`, `iproute2`, `conntrack`; VMs: `libvirt+qemu`, `qemu-img`, `cloud-localds`; opcional `cloud-hypervisor`, `nvidia-container-toolkit`, `minisign` (verificação de assinatura), `delonix-cri` (`--with-cri`).
- **Config de host rootless:** `/etc/subuid` + `/etc/subgid` (`ensure_subid`), perfil AppArmor, drop-in de **delegação de cgroup** (`Delegate=cpu cpuset io memory pids` em `/etc/systemd/system/user@.service.d/`), sysctls/módulos (`--no-tune` desliga), tuning `--production`/`--performance`.
- **providers.yaml** (ADR-0054): escrito com `defaultProvider` = `--vm-provider` (default libvirt) **só se não existir**; nunca reescrito. Local: `/etc/delonix/providers.yaml` (sistema) ou `~/.config/delonix/providers.yaml` / `$XDG_CONFIG_HOME` (com `--user`).
- **Verificação final (doctor embutido):** confere binário, `slirp4netns`, `newuidmap` (+ privilégio setuid/cap), `nft`, ranges subuid/subgid, `/dev/net/tun`, user-namespaces (via perfil AppArmor onde `kernel.apparmor_restrict_unprivileged_userns=1`), backend VM, default provider lido PELO binário (`delonix vm default-backend`), e **delegação de cgroup de verdade** (cria cgroup filho + tenta `+memory`, e verifica cpu/cpuset/io/memory/pids). É equivalente — mas não idêntico — a `delonix system doctor`.

### Peças do E2E e o que cada passo exige

| Passo | Exige | Observações |
|-------|-------|-------------|
| **Instalar** (binário + deps + config) | **root/sudo** (apt/dnf/zypper/pacman, escrever `/etc`, `/usr/local/bin`, AppArmor, drop-ins) + **gestor de pacotes da distro** + rede | 1ª chamada a sudo é a instalação do binário (`|| die` com mensagem acionável). `minisign` para verificar assinatura (ou `--insecure-skip-signature`). |
| **`system doctor`** | nada destrutivo (read-only) | Precisa de **cgroup v2 + systemd user** para a delegação ler `Ok`; `--strict` para gate CI. |
| **Re-correr = upgrade** | igual ao instalar | Preserva estado/imagens/volumes/config/sockets/serviços. Avisos de migração **só** se `PREV_VER` for `0.64.*` (3 Kinds removidos) ou `1.*–5.*` (re-base da linha `delonix-runtime` → `0.x`, deliberado, não downgrade). providers.yaml existente **nunca** reescrito → prova a preservação. |
| **Remover** | **manual** | ⚠️ **Não há subcomando de uninstall** no install.sh nem `--uninstall`. Reverter é manual: `rm` dos binários, `rm /etc/delonix`, remover linhas de `/etc/subuid`/`/etc/subgid`, `rm` dos drop-ins (`user@.service.d`, `delonix-performance`, perfil AppArmor) + `systemctl daemon-reload`. **Lacuna da release para um "remover" limpo e script-ável.** |

### Contentor vs VM como ambiente limpo — avaliação

- O install.sh é **systemd-cêntrico e root-cêntrico**: `systemctl enable --now libvirtd.socket`, drop-ins de `user@.service`, delegação de cgroup que **só toma efeito no próximo login**, AppArmor, módulos de kernel. Nada disto é exercitável de forma fiel sem **PID1 = systemd** e **root**.
- **Contentor via o próprio delonix (rootless):** serve para um *smoke* da **detecção de distro** e do caminho de pacotes por distro (apt/dnf/zypper/pacman) **se** a imagem trouxer systemd como PID1 e se correr privilegiado — o que contraria o modelo rootless do delonix. Um contentor rootless comum **não** tem systemd PID1, **não** consegue carregar módulos de kernel, **não** aplica AppArmor do host, e a delegação de cgroup/`user@.service` não existe lá dentro. Logo um contentor **não** prova os passos que mais falham em campo (delegação, libvirtd, AppArmor, subuid a nível de host).
- **Veredicto:** o E2E fiel por distro **exige uma VM** (uma por família de pacote: Debian/Ubuntu→apt, Fedora/Rocky→dnf, openSUSE→zypper, Arch→pacman), com **systemd + root + /dev/kvm** para também cobrir o backend VM. Um contentor serve, no máximo, como **pré-smoke barato** da ramificação de distro, não como prova de aceitação. **Nenhuma destas VMs foi levantada nesta sessão** (proibido levantar VMs pesadas) → o E2E multi-distro fica **bloqueado-por-ambiente / pendente**.

### Caminho `--user` rootless — é testável SEM tocar no HOME real nem sudo?

- `--user` instala o binário em `~/.local/bin` e escreve providers.yaml em `~/.config/delonix/` (ou `$XDG_CONFIG_HOME`). **Sem `--user`, o 1º sudo é a instalação do binário.**
- Com `--user` E `--no-binary` (ou um binário já presente), e **redirecionando o HOME** (`HOME=<tmp> XDG_CONFIG_HOME=<tmp>/.config`), o troço `--user` escreve só em dir temporário. **MAS**: o install.sh real continua a chamar `require_dep`/`pkg_install` (que usam sudo) para as deps de runtime, e a secção de subuid/AppArmor/delegação precisa de root. Um `--user` "puro-binário" não cobre nada do que interessa validar na §B (deps + config de host).
- **Decisão (honesta):** **não corri** uma instalação `--user` nesta máquina de dev. Mesmo com HOME redirecionado, o valor acrescentado seria quase nulo (só provaria que o binário aterra em `~/.local/bin`), e o host de dev já tem um `delonix` e um ambiente que não quero poluir/mascarar (o install.sh avisa explicitamente sobre binários que se fazem sombra no PATH). Fica **documentado como pendente**: testável em VM limpa, não aqui.

### Veredicto E2E de instalação
- **Instalar / upgrade / doctor:** desenho sólido e completo (incl. doctor embutido rigoroso e avisos de migração 0.64→0.65 e 5.x→0.x). **Precisa de VM com systemd+root+pkg-manager por família de distro** para prova de aceitação fiel. **Pendente — bloqueado-por-ambiente** (VMs não levantadas nesta sessão).
- **Remover:** **lacuna real** — não existe caminho de desinstalação script-ável; a remoção é manual e multi-ficheiro. Recomenda-se, para a v0.1.1, ou um `install.sh --uninstall`/script `uninstall.sh`, ou pelo menos documentar o procedimento de reversão num único sítio.
- **Contentor:** insuficiente como ambiente de aceitação (sem systemd PID1 / root / cgroup-deleg / AppArmor / módulos); vale só como pré-smoke da ramificação de distro.

---

## Veredicto global (§18-consciente)

- **Validado-live neste host:** libvirt, cloud-hypervisor e linux (disponibilidade + health `Ok`; `system doctor` passa em todos os pré-requisitos; storage-shares precisam root, por desenho).
- **Bloqueado-por-ambiente (NÃO aprovado):** proxmox (47 células supported, pve92 podado — precisa PVE dedicado) e opnsense (10 células §8, VM viva mas inalcançável + sem API key/secret — dois bloqueios independentes).
- **E2E de instalação:** desenho completo para instalar/upgrade/doctor; **precisa de VM** (systemd+root+pkg-manager) por família de distro; **contentor não chega**; **remoção não tem caminho script-ável** (lacuna). Nada do E2E multi-distro foi corrido (proibido levantar VMs) → **pendente**.
- A ausência de alvos proxmox/opnsense e de VMs de instalação **não** é aprovação — é dívida de verificação a agendar numa sessão com ambiente dedicado.
