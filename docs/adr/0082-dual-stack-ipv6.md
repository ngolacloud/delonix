# ADR-0082 — Dual-stack IPv6 (plano faseado)

**Estado:** Proposto (Fase 1 — design). **Data:** 2026-11-01.
**Relaciona:** a recusa deliberada actual em `delonix-sdn/src/infra.rs`
(`ipv6_sdn_enabled`, `disable_ipv6_argv`, `ingress_v6_refusal_ruleset`).

## Contexto

Hoje o IPv6 está **desligado de propósito**, em duas camadas: (1) `disable_ipv6`
(sysctl) em cada netns de container; (2) `table ip6 <ingress> forward drop` no
holder. `DELONIX_ENABLE_IPV6=1` é um escape **inseguro e ruidoso**.

A razão é uma **invariante de segurança**, não falta de trabalho: **todo o
firewall é `table ip` (só-IPv4)** — `ingress`, `egress`, `policy deny`,
isolamento por namespace, `kind: Dependency` e o guard L4 são todos IPv4. Um
endereço v6 (ULA derivável do v4) contornaria **todas** essas políticas.

### Descoberta que fixa o faseamento (2026-11-01)

Não há atalho "ligar só uma fatia":
- **Nem o host-mode via slirp** é seguro ligar isoladamente: a política de
  **egress** também é `table ip` (v4), por isso v6 egress (mesmo via slirp NAT)
  **contornaria `egress: deny`** — exfiltração sobre v6.
- O **firewall v6 precisa de endereços v6** para escrever as regras (`ip6 saddr/
  daddr`), e esses endereços vêm do IPAM (hoje só v4).

**INVARIANTE (não negociável):** nenhum caminho de v6 (ingress OU egress, host-
mode OU bridge) é ligado antes de o firewall dual-family cobrir v6. O default
mantém-se v6-OFF em cada fase até a Fase 4.

## Decisão — alvo e fases

**Alvo** (já nomeado pelo próprio código): migrar o firewall de `table ip` para
**`table inet`** (dual-family v4+v6), com **IPAM v6 (ULA `fd00::/8`)** e **sets
v6** a par dos `ipv4_addr`. Executar por fases, cada uma commit+teste próprios, e
**v6 só fica realmente ligado na Fase 4**, com prova live.

| Fase | Âmbito | Ficheiros | Prova | Liga v6? |
|---|---|---|---|---|
| **1 — Design** (esta) | Invariante + plano + alvo `table inet` | `docs/adr/0082` | revisão | não |
| **2 — IPAM v6 (ULA)** | Derivação determinística de um ULA por `(rede,id)`, a par do `derive_ip_in` v4; registo dual-stack por rede | `delonix-sdn/src/{lib.rs,ipam.rs}` | unit (ULA estável, sem colisão, dentro do /64) — **inerte** (nada o consome ainda) | não |
| **3 — Firewall dual-family** | `table ip`→`table inet`; cada regra ganha o par `ip6 saddr/daddr`; sets `ipv6_addr` a par dos `ipv4_addr`; egress+ingress+policy+namespace+Dependency cobrem v6; remover `ingress_v6_refusal` (substituído por filtragem real) | `delonix-sdn/src/{policy_nft.rs,infra.rs}` | unit (ruleset dual rende verdicts golden p/ v4 **e** v6); v4 byte-idêntico ao actual (sem regressão) | não (sets v6 vazios até a 4) |
| **4 — Datapath + habilitar** | Parar `disable_ipv6` quando a rede é dual-stack; atribuir o ULA na netns; gateway/RA v6 na bridge; slirp `--enable-ipv6` (host-mode); opt-in por rede (`network create --ipv6`) | `delonix-sdn/src/{infra.rs,lib.rs}`, `bins/.../cmd/{network.rs,container.rs}` | **live**: container numa rede dual-stack recebe v4+v6, `egress: deny` bloqueia v6, isolamento de namespace vale em v6 | **SIM** |
| **5 — Contratos** | `--ip` v6, publicação de portas v6, DNS interno AAAA, `provider matrix` `net.ipv6`→supported (c/ evidência) | `delonix-sdn`, `delonix-node-api`, docs | live + gate de capacidade | sim |

## Consequências

- Até à Fase 4 o comportamento observável **não muda** (v6 OFF); as Fases 2–3
  são fundação inerte + testada → zero risco de regressão de segurança.
- A capacidade `net.ipv6` **só** vira `supported` na Fase 4/5, com prova live de
  que egress/ingress/isolamento valem em v6 — nunca antes (senão era o escape
  inseguro actual com outro nome).
- `DELONIX_ENABLE_IPV6=1` (escape inseguro) é **removido** quando a Fase 4
  entregar v6 filtrado.
- Custo: a Fase 3 (`table inet`) toca o caminho quente do firewall — exige que
  o output v4 fique byte-idêntico (os testes golden de `policy_nft` são o guarda).

## Porque NÃO foi feito mais nesta sessão

A Fase 1 é design por integridade: ligar v6 em qualquer fatia antes da Fase 3/4
reintroduziria exactamente o buraco que a recusa actual fecha (v6 sem política de
egress). O primeiro código seguro é a Fase 2 (IPAM v6, inerte). Não se finge
"IPv6 suportado" com uma mudança parcial não-filtrada.
