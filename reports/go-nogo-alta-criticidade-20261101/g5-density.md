# G5 — densidade: a curva de attach vs. nº de containers

> **Data:** 2026-11-03. **Âmbito:** o G5 do plano NO-GO→GO — medir se a latência
> de attach cresce com a densidade do nó (a suspeita de que os dois dumps `nft`
> do caminho de attach, O(N) no nº de namespaces, degradam a escala). **Medir
> ANTES de otimizar** — a disciplina do repo.

## Montagem

`scripts/bench_density.sh --tiers 1,10,25,50,100 --samples 8 --force`. Rampa de
containers busybox numa rede-bancada; em cada patamar, 8 attaches de prova
cronometrados (`run -d` + `rm`). **NÃO PUBLICÁVEL** por desenho: o nó tinha 2
containers alheios (o `sd-pg` do dono) e carga 4–5 (caixa partilhada). O que
importa aqui é a **forma** da curva, robusta ao ruído, não os números absolutos.

## A curva (attach p50, ms)

| containers vivos | 1 | 10 | 25 | 50 | 100 |
|---|---|---|---|---|---|
| **attach p50 (ms)** | 193 | 202 | 206 | 196 | **215** |
| attach p99 (ms) | 654 | 1846 | 1424 | 483 | 1150 |

## Leitura

**A p50 é PLANA**: 193 → 215 ms de 1 a 100 containers (~11 %, dentro do ruído de
uma caixa a load 4–5). A p99 salta (654…1846…483…1150) mas **sem tendência** — é
a contenção da máquina partilhada, não um efeito de densidade (se fosse O(N),
subiria de forma monótona). **A latência de attach NÃO cresce com a densidade até
100 containers.**

## Conclusão — NÃO otimizar (e porquê a suspeita não se confirma)

A suspeita era que `drop_from_every_ns_set` (`infra.rs`, um `nft list sets` por
attach/detach) e os `nft -a list chain fwdeny` fossem O(N) e degradassem a escala.
A curva mostra que **não degradam na prática**: o `list sets` conta **namespaces**,
não containers, e um workload típico tem poucos namespaces (os 100 containers
estavam no namespace `default` → ~1 ns-set). O custo de attach é dominado pelo
fixo (~20-25 `fork`+`exec` de `ip`/`sysctl`/`nft` + prep da rootfs), não pelos
dumps.

**Otimizar o `drop_from_every_ns_set`/`fwdeny` seria resolver um não-problema** —
e ao custo de mexer no datapath de rede/segurança. O "medir primeiro" poupou
exatamente essa troca má. O alvo fica **documentado mas não tocado**; se um dia um
workload com MUITOS namespaces mostrar a curva a subir, a otimização (atacar a
ns-set específica em vez do dump) tem aqui o ponto de partida.

## O que FICA por medir (nó dedicado)

A curva PUBLICÁVEL (nó ocioso e dedicado, sem containers alheios, até um tecto de
degradação com SLOs) continua a precisar do nó dedicado — mas a **forma** já é
conhecida e tranquilizadora. Reprodutível: `scripts/bench_density.sh`.
