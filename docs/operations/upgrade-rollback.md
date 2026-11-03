# Runbook — upgrade e rollback do motor Delonix

> **Âmbito:** substituir o binário do motor (`delonix`) num nó com carga VIVA, e
> voltar atrás, sem perder containers nem estado. Fecha o ponto G3 do plano
> NO-GO→GO. O comportamento aqui descrito é **medido**, não afirmado — a prova
> executável é `scripts/e2e_upgrade_rollback.sh` (17 asserções, verde em
> 2026-11-02). O que ainda falta prova está dito em [Limites](#limites).

## O modelo mental: daemonless

O Delonix não tem um daemon central. Um `delonix run -d` deixa um processo
**supervisor** vivo, lançado a partir do binário **daquele instante**, e cada
invocação seguinte da CLI fala com ele pelo estado em disco e sockets de
controlo. Consequências diretas para um upgrade:

- Um supervisor em execução continua a correr o código com que **nasceu**,
  mesmo depois de o binário em disco mudar. O ficheiro novo é um inode novo; o
  processo antigo fica agarrado ao inode antigo, ainda mapeado em memória.
- Não há "reiniciar o daemon" para atualizar. O que muda é (1) o binário em
  disco e (2) a CLI que a partir daí gere os containers antigos.

## Procedimento de upgrade (com carga viva)

**Regra de ouro: substituir o binário por RENAME atómico, nunca por `cp` em
cima.** Medido: um `cp` por cima de um ELF mapeado por um supervisor vivo falha
com **`ETXTBSY` (Text file busy)** — o kernel recusa reescrever um executável em
uso. Um rename troca o nome por um inode novo e não toca no inode antigo, por
isso nunca bate em `ETXTBSY` e os supervisores vivos sobrevivem.

Os gestores de pacotes fazem isto por si: `dpkg`, `rpm` e o `install(1)` instalam
por rename. A armadilha é só o upgrade **manual** com `cp`/`cat >`.

```sh
# 1. Põe o binário novo AO LADO do atual, no MESMO filesystem (rename só é
#    atómico dentro de um filesystem).
install -m 0755 delonix-novo /usr/local/bin/.delonix.novo     # ou cp para um .tmp ao lado

# 2. Troca atomicamente. Os containers vivos NÃO são tocados.
mv -f /usr/local/bin/.delonix.novo /usr/local/bin/delonix

# 3. A partir daqui, toda a CLI é a versão nova. Confirma e reconcilia.
delonix --version
delonix ps                 # os containers nascidos sob a versão antiga continuam Up
```

O que sobrevive à troca, medido:

| Facto | Resultado |
|---|---|
| Container `-d` vivo | continua `Up`, **mesmo id** (não é recriado) |
| `exec`/`logs` pela CLI nova sobre container antigo | funcionam |
| `stop` → `start` pela CLI nova | reconcilia; a camada gravável (estado) persiste |
| Redes criadas antes | sobrevivem |

## Rollback

Simétrico: rename do binário antigo de volta. Os supervisores continuam a ser os
mesmos processos (nunca morreram), por isso o rollback do binário é instantâneo e
a CLI antiga retoma a gestão com o estado intacto.

```sh
mv -f /usr/local/bin/delonix.v-anterior /usr/local/bin/delonix
delonix --version
delonix ps                 # tudo continua Up, estado preservado
```

Guarda sempre a versão anterior ao lado antes de um upgrade:

```sh
cp -a /usr/local/bin/delonix /usr/local/bin/delonix.v-anterior
```

## Validar num nó antes de confiar

```sh
# Mecanismo (uma só versão): sobrevivência + gestão + reconciliação + rollback.
scripts/e2e_upgrade_rollback.sh

# Compatibilidade CRUZADA de versões (quando houver uma tag anterior):
scripts/e2e_upgrade_rollback.sh --old /caminho/para/delonix-vN
```

O formato de estado EM DISCO entre versões é coberto, à parte, por
`scripts/e2e_state_upgrade.sh`.

## Limites

O que está **provado** (mecanismo): substituir o binário por rename com carga
viva, a sobrevivência dos containers, a gestão pelo binário novo, a
reconciliação por stop/start, e o rollback.

O que **falta** provar, e não se deve afirmar até lá:

- **Compatibilidade de código entre versões diferentes** — o binário novo a LER
  estado/sockets escritos por uma versão antiga. O harness já aceita `--old
  <tag-anterior>` e passa a medir isto sem mais nada; falta uma tag anterior
  compilada para apontar.
- **Hot-patch de um supervisor vivo** — NÃO existe. Um supervisor corre o código
  com que nasceu até ser recriado. Uma correção de segurança no caminho de
  supervisão só chega a um container existente quando ele for recriado (parar +
  arrancar recria o processo a partir do binário corrente). Planear o upgrade de
  um nó crítico como "rename do binário (containers sobrevivem) + recriação
  faseada dos que precisam do código novo no supervisor".
- **Rollback a meio de um `apply`** — um `kind:` apply que falha a meio não tem
  hoje rollback transacional (`docs/discovery/47_IAC_REVISAO.md:400`). Até existir,
  tratar um `apply` falhado como estado a reconciliar manualmente, não a desfazer.
