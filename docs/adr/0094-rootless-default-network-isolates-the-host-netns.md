# ADR-0094 — Em rootless, a rede por omissão isola a netns do host (alinhar com Docker/Podman)

**Estado:** Aceite (2026-11-18) — decisão do dono na campanha de endurecimento de
plataforma (avaliação a fundo, `reports/e2e/0.1.1/PLATFORM_EVAL_2026-11-18.md`,
achado **R2**). **Gap medido ao vivo** nesta sessão; **design validado**;
**implementação é uma fatia e2e-gated própria** (toca o contrato de rede em 32+
call-sites — ver «Implementação»). **Relaciona:** ADR-0082 (IPv6 dataplane),
ADR-0084 (overlay/wireguard/hostfwd), ADR-0078 (perfis de rede de pod).

> **A decisão, numa linha:** o default de `--net` em **rootless** passa de `host`
> (partilha a netns do host — sem isolamento) para uma **rede NAT isolada** (netns
> própria + `slirp4netns` com SNAT de egress), como fazem `docker`/`podman`
> rootless. `--net host` continua disponível, **explícito**.

## Contexto

### O gap, medido ao vivo (host rootless, 2026-11-18)

Hoje `--net` tem `default_value = "host"`
(`bins/delonix/src/cmd/container.rs:1166`). Em rootless, `host` faz o container
**partilhar a network namespace do processo engine**, que é a do host:
`Launch::new_netns()` (`delonix-compute/src/domain/launch.rs:61`) só cria netns
própria com `--net none` ou com `-p`, e `clone_flags` (`delonix-linux/src/lib.rs`)
só junta `CLONE_NEWNET` quando `new_netns` — o teste
`the_clone_flags_isolate_by_default_and_waive_what_is_asked` fixa precisamente
`!CLONE_NEWNET` para o default.

Prova ao vivo (binário `0.1.1` deste worktree, utilizador rootless):

```
$ python3 -m http.server 45999 --bind 127.0.0.1 &      # serviço SÓ no loopback do host
$ delonix run --rm alpine:3.24 -- wget -qO- http://127.0.0.1:45999/
<!DOCTYPE HTML> …                                         # ← o container ALCANÇOU-O
$ delonix run --rm alpine:3.24 -- ip -o addr
… wlp0s20f3  inet 192.168.1.108/24 …                      # WiFi do host
… br-lan  inet 192.168.10.1/24 …  br-wan … virbr0 …       # TODAS as bridges/VMs do host
```

Um container de tenant vê e alcança **toda** a rede do host: serviços em
`127.0.0.1` (bases de dados locais, o próprio *CRI streaming server*), todas as
interfaces, as bridges das outras redes e das VMs. Numa plataforma multi-tenant
(NgolaCloud) isto é uma regressão de isolamento face a Docker/Podman rootless, que
dão a cada container uma netns própria com egress por `slirp4netns`/`pasta`.

### Porque é que o modelo actual suporta a correcção

O datapath rootless já tem o mecanismo todo: um **holder netns** com um
**`slirp4netns` único** (`delonix-sdn/src/infra.rs` `start_slirp`), que corre com
`--configure` (SNAT de egress) e um `--api-socket` para os `add_hostfwd` da Fase
4. **O egress não depende de `-p`:** os `hostfwd` são só para *ingress*; o SNAT de
saída existe no momento em que o container entra na holder netns. Logo um
container **sem** `-p` pode ter isolamento **e** egress juntando-se ao holder — é
exactamente o modelo Podman.

## Decisão

1. **Default rootless de `--net` = rede NAT isolada** (netns própria + holder/slirp
   com egress), não `host`.
2. **`--net host` fica explícito** — quem quer partilhar a netns do host pede-o.
   O `delonix run --net host` é a válvula de escape (tal como `docker run
   --network host`).
3. **Em modo root (não-rootless)** o default não muda nesta ADR (o root já tem
   caminhos de bridge/CNI próprios; o gap medido é rootless).
4. **Admissão:** com `workloadSecurity: baseline`, `--net host` passa a ser
   refutado (ADR deste mesmo lote, achado S1) — coerente: o host-net é opt-in **e**
   sujeito à política PSS.

## Consequências

- **Isolamento por omissão**: um container de tenant deixa de ver/alcançar a rede
  do host; `127.0.0.1` passa a ser o do container.
- **Egress preservado**: saída para a internet via SNAT do slirp (como hoje com
  `-p`).
- **Custo**: cada container rootless passa a ter netns + (partilha do) slirp —
  custo já pago pelos containers com `-p`; para os sem `-p` é novo. Medir o
  overhead de arranque vs. o default actual.
- **Migração / quebra**: scripts que hoje dependem de o container ver `127.0.0.1`
  ou as interfaces do host **partem** — resolvem-se com `--net host` explícito.
  Documentar em `MIGRATION.md` e nas notas de release; emitir um aviso único na
  primeira vez que um `--net host` implícito teria sido o comportamento antigo (um
  release de transição), antes de o default mudar de facto.

## Implementação (fatia e2e-gated própria — NÃO neste commit)

Mudar o default de rede toca o **contrato de rede em todo o lado** — medidos 32+
call-sites que assumem `net == "host"` / `net.is_empty()` como «partilha host»
(`container.rs:3428`, `compute/app/run.rs:178`, `gitops.rs:1396`, pods, compose,
stack, CRI). Por isso a troca é uma fatia governada, com a suite e2e de rede
completa como gate, não uma edição cirúrgica.

Passos:

1. Introduzir o modo isolado como valor de `--net` (ex. `nat`/`private`): netns
   própria + holder/slirp egress, `-p` acrescenta `hostfwd` por cima (reutiliza o
   caminho de `-p`, que já faz isto).
2. `Launch::new_netns()` passa a ser `true` para o modo isolado; `slirp_ports`
   vazio ⇒ slirp egress-only (sem `hostfwd`).
3. Trocar `default_value` de `host` para o modo isolado; auditar os 32+ call-sites
   e os defaults de pod/compose/stack/gitops/CRI para o novo default.
4. **Gate e2e (o critério «nada falha»)**, em rootless: (a) isolamento — o teste
   do loopback do host acima passa a **falhar** a ligação; (b) egress — o container
   alcança a internet; (c) `-p` continua a publicar ingress; (d) DNS interno e
   container-a-container; (e) pods (netns partilhada do pod) intactos; (f)
   conformância CRI intacta. Benchmark de arranque vs. baseline.
5. Actualizar os testes unitários de `launch`/`clone_flags` (incl.
   `the_clone_flags_isolate_by_default_and_waive_what_is_asked`) para o novo
   default, e `MIGRATION.md`.

Até a fatia aterrar, o gap fica **documentado e medido**; `--net host` é o
comportamento actual e o modo isolado é o alvo do default.
