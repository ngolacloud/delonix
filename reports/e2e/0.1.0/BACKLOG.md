# BACKLOG.md (master) — Delonix 0.1.0 @ 1898466b — 2026-11-08

Intercala BUG/GAP/MELHORIA por severidade (P0→P3). Produzido pela ronda de
paridade+CRI+instalação (`report.md`). Para groom → fila, usar
`.ai/skills/delonix-bug-triage`.

> **Regra do lema:** rootless-first. Vários itens (BUG-0003/04/05/06, GAP-0001/03)
> só se manifestam/resolvem com root real → o alvo de correcção é a **matriz
> rootful-VM**, que materializa «rootful onde exigir». Não degradar o default
> rootless para os fechar.

## P0
_(nenhum — nada bloqueia totalmente o produto nesta ronda)_

## P1

| ID | Tipo | Título | Component | Compl. | Dep. |
|---|---|---|---|---|---|
| BUG-0001 | BUG | `make build/install` não gera `delonix-cri` | `Makefile:44` | XS | — |
| BUG-0003 | BUG | AppArmor não imposto (4 specs) | `delonix-linux` apply/ensure_apparmor + `delonix-cri` lifecycle | M | — |
| BUG-0004 | BUG | HostNetwork/HostIpc não suportados (4 specs) | `delonix-cri` lifecycle + `delonix-linux` launch_spec (IPC setns em falta) | L | habilita G1,G3 |
| GAP-0001 | GAP | PodPID recusado em host-net/CNI | `delonix-compute` pod.rs + `delonix-cri` lifecycle:1930 | M | BUG-0004 |
| GAP-0002 | GAP | `differential` só cobre 2 fluxos | `tests/cli-parity` | M | — |
| GAP-0003 | GAP | Port-forward host-network | `delonix-cri` streaming.rs:227 | S* | BUG-0004 |
| GAP-0008 | GAP | **CNCF 3:** Provenance SLSA no build | `delonix-oci` build.rs + sign.rs | L | GAP-0009 |
| GAP-0009 | GAP | **CNCF 4:** SBOM SPDX in-toto na imagem | `delonix-scanner` + `delonix-oci` | M | — |
| GAP-0012 | GAP | **CNCF 1+2:** conformância OCI/CRI comprovada + ADR runtime | cross | XL | BUGs CRI |
| GAP-0015 | GAP | **CNCF 2 (Podman):** interop k8s YAML `kube play`/`generate` | `delonix` cmd + `delonix-compute` | L | — |
| M-DOC-01 | MELHORIA | Narrativa de conformância: 84/19 rootless · 90/13 rootful-VM (não ~104/104) | docs/README/relatórios | XS | — |
| M-TEST-03 | MELHORIA | Gate que falha o build se faltar binário de `BINARIES` (apanharia BUG-0001) | `Makefile` + `scripts/` | S | — |

## P2

| ID | Tipo | Título | Component | Compl. | Dep. |
|---|---|---|---|---|---|
| BUG-0005 | BUG | Mount Propagation `rshared` | `delonix-linux` lib.rs:2504 | M | — |
| BUG-0006 | BUG | OOMKilled / exit 137 não reportado | `delonix-linux` died_of_oom + `delonix-cri` exit_reason | M | — |
| GAP-0004 | GAP | Docker Engine API parcial (logs+exec) | API docker | XL | — |
| GAP-0005 | GAP | Compose (DEF-003) | `delonix compose` | M | — |
| GAP-0011 | GAP | **CNCF 6:** convergir Compose Specification | `delonix compose` | M | GAP-0005 |
| GAP-0013 | GAP | **CNCF 8:** scanner policy + compare + CI gating | `delonix-scanner` + CLI `scan` | M | — |
| GAP-0010 | GAP | **CNCF 5:** build cache distribuído + multi-plataforma | `delonix-oci` build.rs + registry.rs | XL | — |
| GAP-0016 | GAP | **CNCF 5 (Podman):** Quadlet/systemd p/ workloads do utilizador | `delonix` cmd (reusa serve.rs) | M | — |
| GAP-0017 | GAP | **CNCF 4/8 (Podman):** trust de assinatura por policy | `delonix-security` policy.rs + `delonix-oci` sign.rs | M | GAP-0009 |
| M-TEST-02 | MELHORIA | `same_behaviour "stats all"` dá falso-positivo com container vivo (amostra volátil) | `tests/cli-parity/frequent-verbs.sh:112` | XS | — |

## P3

| ID | Tipo | Título | Component | Compl. | Dep. |
|---|---|---|---|---|---|
| GAP-0006 | GAP | Contextos/remote-mgmt (ADR-0010) — candidato a WONT_FIX | `system` verbs | L | ADR-0010 |
| GAP-0007 | GAP | 36 comandos «por-fazer» na matriz | vários | S–M | GAP-0002 |
| GAP-0014 | GAP | **CNCF 7:** governança aberta do projecto (decisão do dono) | repo/projecto | M | estratégica |
| M-DOC-04 | MELHORIA | Documentar rootful-VM como caminho oficial para host-ns/AppArmor/OOM | docs | S | — |

## Progresso das 8 áreas de valor CNCF (alvo: tudo ✔)

Fonte: `CNCF_GAP_DOCKER.md`. ✗ ausente · ◑ parcial · ✔ feito.

| # | Área CNCF | Estado | Fecha com | → 100% |
|---|---|---|---|---|
| 1 | Linhagem de runtime | ✗ | GAP-0012 | ADR runtime + conformância |
| 2 | Conformância/interop k8s | ◑ | BUG-0003/04/05/06 · GAP-0001/0003 · GAP-0012 · **GAP-0015** | CRI ≥95% + suites OCI + `kube play/generate` |
| 3 | Provenance SLSA no build | ✗ | GAP-0008 | in-toto SLSA v1.1 anexada |
| 4 | SBOM interoperável | ◑ | GAP-0009 · **GAP-0017** | SPDX in-toto + trust por policy |
| 5 | Ops cloud-native (build+host) | ✗ | GAP-0010 · **GAP-0016** | cache/multi-arch + systemd/Quadlet |
| 6 | Compose Specification | ◑ | GAP-0005 + GAP-0011 | suite de conformância verde |
| 7 | Governança aberta | ✗ | GAP-0014 | público + governança + releases assinadas |
| 8 | Scanning maduro em CI | ◑ | GAP-0013 · **GAP-0017** | policy + compare + gating + trust |

**Já alinhado (não refazer):** OSV (scanner), cosign/sigstore (`sign.rs`),
rootless-first, **daemonless, pods nativos, `policy.json` de runtime, unit systemd
do node-api** (paridade filosófica com o Podman). Fontes: `CNCF_GAP_DOCKER.md`,
`CNCF_GAP_PODMAN.md`.

## Sequência sugerida (dependências)
1. **BUG-0001** + **M-TEST-03** (desbloqueia instalação limpa; XS/S, independentes).
2. **BUG-0004** (host-ns) → desbloqueia **GAP-0001** (PodPID) e **GAP-0003** (port-forward) no mesmo lote.
3. **BUG-0003** (AppArmor), **BUG-0005** (rshared), **BUG-0006** (OOM) — independentes, paralelizáveis.
4. **M-DOC-01** assim que (2)+(3) fecharem, para republicar os números reais.
5. **GAP-0002** (ampliar differential) sustenta a medição de **GAP-0007**.

## Sequência CNCF (supply-chain primeiro — maior peso na avaliação)
6. **GAP-0009** (SBOM SPDX in-toto) → **GAP-0008** (SLSA provenance) → **GAP-0017** (trust por policy): reutilizam `sign.rs`+scanner+`policy.json`.
7. **GAP-0013** (scanner policy/compare/CI): alavanca o OSV já existente.
8. **GAP-0015** (interop k8s `kube play/generate`): maior diferencial CNCF do Podman.
9. **GAP-0012** (conformância comprovada): consome os BUGs CRI fechados + corre suites OCI.
10. **GAP-0005 → GAP-0011** (Compose Spec).
11. **GAP-0010** (build multi-arch + cache), **GAP-0016** (systemd/Quadlet) e **GAP-0014** (governança — decisão do dono).
