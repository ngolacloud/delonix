# §13 — Instalação, actualização, remoção e preservação de dados

**Candidato:** `delonix 0.1.0` @ `8ab783dd` (linha `stabilize/0.1.1`).
**Data:** 2026-10-29. **Método:** inspecção de `install.sh`/`MIGRATION.md`/
`Makefile` + validação determinística da lógica; o E2E de ambiente limpo fica
marcado como pendente de VM/contentor descartável (não validado no host de dev).

## Resumo por item do §13

| Item | Estado | Evidência / nota |
|---|---|---|
| Requisitos | ✅ coberto | `install.sh` detecta distro por `/etc/os-release` (apt/dnf/zypper/pacman) e instala deps de runtime só se faltarem |
| Instalação | ✅ coberto | `install.sh` (binário+deps+config host); `--user` = binário rootless em `~/.local/bin` sem sudo |
| Configuração | ✅ coberto | `providers.yaml` via `--vm-provider` (ADR-0054); um ficheiro existente **nunca** é reescrito |
| Permissões | ✅ (root) | subuid/subgid, AppArmor, systemd — operações de root |
| Inicialização | ✅ coberto | rootless-first, daemonless; rede «on demand» |
| **Diagnóstico** | ✅ coberto | `delonix system doctor` + `delonix system info` (visto a reportar state root, modo, cgroup2) |
| **Actualização** | ✅ + fix | `install.sh` re-corre; `check_version` impõe os 5 binários-irmãos na MESMA versão; **FIX** `dab971fe`: aviso 5.x→0.1.x à frente |
| **Migração 5.x→0.1.x** | ✅ documentado + à frente | `MIGRATION.md` (latest-por-data=0.1.0; auto-update SemVer NÃO dispara→reinstalar; rollback p/ tags v5.* imutáveis); agora **avisado no upgrade** |
| **Preservação de dados** | ✅ por construção | `install.sh` NÃO tem nenhum `rm` de `~/.local/share/delonix` nem `/var/lib/delonix` — só apaga temporários (l.144/1256). Upgrade só substitui binários. P3 protege contra downgrade silencioso de estado |
| **Remoção (uninstall)** | ✅ RESOLVIDO | `install.sh --uninstall` (preserva dados por omissão; `--purge` remove estado/config; honra `--user`; desliga boot primeiro; nomeia a config de host partilhada). `bash -n` + lógica testada nos dois modos. Antes: só `make uninstall` (from-source) |
| E2E ambiente limpo | ⚠️ PENDENTE de ambiente | Instalação completa (deps via package manager + systemd + subuid) exige distro suportada + root → **validar em VM/contentor descartável**, não no host de dev (§18: ausência de ambiente ≠ aprovação) |

## Correcção aplicada (`dab971fe`)

`install.sh` só avisava na remoção de Kinds `0.64→0.65`. Adicionado um ramo
`1.*|2.*|3.*|4.*|5.*)` à verificação de versão ANTES de substituir o binário:
nomeia o re-base como deliberado (não downgrade), afirma a preservação de
estado/imagens/volumes/config/sockets/serviços, lembra o alinhamento dos 5
binários-irmãos, e aponta para `MIGRATION.md`. `bash -n` limpo; lógica do `case`
exercitada para `5.1.1` (avisa re-base), `0.64.0` (avisa Kinds), `0.1.0`/vazio
(sem aviso — instalação nova).

## Ligação ao F-1 (cura no upgrade)

O percurso de upgrade tem um efeito de estado já provado nesta campanha: camadas
de imagem desempacotadas por um binário pré-correcção são **curadas no 1.º uso**
após o upgrade (época de formato `LAYER_EPOCH`, prova A/B live com rabbitmq). Ou
seja, o upgrade do binário + primeiro uso reconcilia o estado em disco que o
próprio upgrade não toca.

## Gaps / acções recomendadas para o GO

1. **Uninstall do operador:** dar ao `install.sh` um `--uninstall` (remove os 5
   binários do `BIN_DIR`, preserva o state dir por omissão, com `--purge` opt-in
   para o apagar), OU documentar o procedimento manual explícito. P3/baixa.
2. **E2E de ambiente limpo:** correr `install.sh` numa VM/contentor de cada
   distro suportada (apt/dnf/zypper/pacman): instalar → `system doctor` verde →
   re-correr (upgrade, dados preservados) → remover. Bloqueia a afirmação
   "instalação suportada em produção" até ser feito.
3. **Nit de marca:** o cabeçalho do `--help` ainda diz «Delonix **Runtime**» —
   alinhar com a identidade `Delonix`.
