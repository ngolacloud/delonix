# Delonix v0.1.1 — decisão de release

**Data:** 2026-10-31 · **Linha:** `stabilize/0.1.1` (worktree, 20 commits, SEM push)
· **Base:** `origin/develop` `8ab783dd` (= `v0.1.0` + 39) · **v0.1.0 já publicada
e imutável** → estas correcções são para **`v0.1.1`**.

## Veredicto: **GO** no âmbito explicitamente validado (o único residual foi root-caused e CORRIGIDO)

A `v0.1.1` é uma melhoria inequívoca sobre a `v0.1.0` já no terreno: fecha 1 P1 de
integridade, advisories de memory-safety alcançáveis, 1 TOCTOU de estado, e 3
achados P2/P3 — todos com teste **e** prova live. O único residual (o hang do
attach de rede custom) foi root-caused e CORRIGIDO: era um self-deadlock que o
próprio P2-1 introduziu (name-claim segura através do re-exec) + um padrão
sistémico de `flock(LOCK_EX)` sem timeout; ambos tratados. Custom-net attach
completa em ~1s, e o DEF-003 original (compose não reconcilia) foi finalmente
testável e NÃO reproduz.

## 1. Correcções (todas: fix + teste + prova live)

| Item | Severidade | Prova |
|---|---|---|
| F-1 cache de camadas envenenada no upgrade | P1 integridade | unit + A/B live (rabbitmq curado no 1º uso) |
| SEC-01 bump zstd 0.14.3 | Médio segurança | `cargo deny advisories`=ok; 6 RUSTSEC alcançáveis por imagem eliminadas |
| P2-1 `run --name` concorrente (TOCTOU) | P2 estado | 4× race live → 1 criado, 3 `DX-5000` |
| P3 carimbo de versão + gate de downgrade | P3 recuperação | live: record formato 99 recusa `start`/`rm` |
| S15-1 lock-files ilimitados | P3 escala | pool 256 shards + GC no `system prune` (live: 20 recuperados) |
| restart_policy não-persistido | P2 (achado novo) | live: `--restart always` volta a Running; `no` fica Stopped |

Mais: `install.sh --uninstall`/`--purge`; aviso P2-2 (restart não sobrevive reboot sem `boot enable`).

## 2. Âmbito SUPORTADO para produção (validado)

- **Containers/pods OCI rootless** (§7) — lifecycle/OOM/isolamento provados; **OOM imposto ao vivo** (32M mata, 128M sobrevive).
- **VMs** CH/libvirt — matriz gated **180/270 N2** (tecto honesto 247).
- **§8 rede/gateway** — **proxmox (47 células) e opnsense validados ao vivo com os appliances** (pve92/opnsense); evidência no repo, gate-enforced (`every_supported_capability_cites_evidence_that_exists`).
- **Recuperação §12** — sem P1; reconcile de crash, escrita atómica, idempotência, backup/restore provados.
- **Instalação/upgrade/remoção/preservação de dados §13** — **ciclo completo provado ao vivo numa Ubuntu 24.04 limpa** (família apt).
- **Endpoint CRI** — `serve cri` + `crictl` handshake `runtime.v1`, **RuntimeReady=true**.
- **Integridade/segurança de artefactos §16** — assinatura minisign, SHA256SUMS, SBOM 402 pkgs, `release_verify` 6/6; SEC-01 corrigido.

## 3. Experimentais / não-validados (não operar sem aceitação de risco)

42 células EXPERIMENTAL + 6 IMPLEMENTADA-NÃO-VALIDADA (ver `contrato-suporte-producao.md`).

## 4. Residual — ROOT-CAUSED e CORRIGIDO (eram DOIS defeitos)

O "attach de rede custom pendura sem timeout" era **dois** defeitos, ambos agora
corrigidos e provados ao vivo:

1. **Self-deadlock introduzido pelo P2-1.** O `cmd_run` segurava a name-claim
   (namespace,name) através de TODA a função — incluindo o caminho de rede
   custom, que **re-exec** um SEGUNDO `cmd_run` e ESPERA por ele. O filho
   re-exec'd re-adquiria a MESMA name-lock e bloqueava no próprio pai. Fix: a
   claim + o check de unicidade só no 1º passo (`!reexec`) — o re-exec é o mesmo
   create lógico. **Attach custom completa agora em ~1 s** (antes: pendura). A
   garantia do P2-1 mantém-se (um create de utilizador é sempre 1º passo → 4×
   `create --name` concorrentes = 1 criado + 3 recusados, re-provado).

2. **Padrão sistémico de `flock(LOCK_EX)` sem timeout** (~7 sítios). Novo helper
   partilhado `delonix_node::flock_exclusive` (NB + tecto 30s, EINTR-safe)
   roteado em TODOS: `delonix-state` Store+secret, `delonix-sdn` infra +
   ExclusiveLock + IpamLock, e os locks do bin ippool/ingress_proxy/
   container-start/hosts_file (este ignorava o resultado). Cada um já tinha via
   de recusa (Err/None) → passam a **falhar-limpo ao fim do tecto, nunca
   pendurar** — foi também assim que #1 se diagnosticou (o timeout nomeou o lock).

**Prova live:** attach custom completa 1 s; P2-1 race holds; e o **DEF-003
original** (compose não reconcilia serviço parado), finalmente testável, **NÃO
reproduz** — `compose up -d` traz o `proj-cache` parado de volta a Running.
Testes: delonix-node flock 20/20, state 22/22, sdn 325/325, node-api 14/14.

## 5. Breadth por fazer (não-bloqueadores)

- E2E instalação dnf/zypper/pacman (apt provado; mesmo guião, +1 VM cada).
- critest completo de CRI (endpoint provado; falta root + CNI + host limpo).

## 6. Condições para cortar a `v0.1.1`

1. Integrar os 24 commits de `stabilize/0.1.1` em `develop` (merge do dono).
2. Cortar/etiquetar `v0.1.1` via o pipeline de release (`release.yml` + `release_verify.py`); NÃO tocar na `v0.1.0`.

(O residual §4 — o hang de attach — foi corrigido, por isso já não é uma
condição; recomenda-se ainda uma passagem num nó de produção real com churn de
rede para re-confirmar o attach custom sob carga concorrente.)

## 7. Rastreabilidade

Commits `stabilize/0.1.1`: F-1(`80e37e7c`,`2bebab71`) · §3(`a61f3ded`) · §12/§16(`081421d5`)
· SEC-01(`7ecd2f11`) · P2-1(`9eaa0f76`,`be81ee84`) · P3(`4fbd6beb`) · fmt(`455b3490`)
· §13 install(`dab971fe`,`7408805e`) · S15-1(lock-files) · P2-2(aviso) · restart_policy
· uninstall · §8/install-E2E/env relatórios. Gates nas mudanças: verdes
(folhas/contract FAIL = baseline/ambiente). `version_gate` ok.
