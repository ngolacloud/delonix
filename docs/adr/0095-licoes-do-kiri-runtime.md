# ADR-0095 — Lições do runtime Kiri: o que adoptamos, o que não, e o único ganho real

**Estado:** Proposed (2026-11-18) — análise do runtime **Kiri**
(`github.com/libracoder/kiri`, container runtime em Rust, Docker-compatível) a
pedido do dono, para ver o que render com os ~2 ms de arranque dele. Conclusão:
o delonix **já implementa, e bem, todas as técnicas de arranque do Kiri que
respeitam a sua filosofia**; o único ganho real que falta é o **plano morno/pool**
— trabalho de fatia própria, não um drop-in. **Nada a implementar neste ADR.**
GO do dono é que o torna Aceite. **Relaciona:** ADR-0040 (binário multicall,
daemonless), ADR-0094 (rede isolada por omissão), ADR-0056 (overlay `volatile`).

> **A decisão, numa linha:** adoptamos **zero mudanças de arranque** do Kiri (o
> delonix já faz o equivalente seguro), **recusamos** o modelo warm-VM/agente
> persistente como omissão (parte o daemonless) e o `clone3`+`CLONE_INTO_CGROUP`
> (reescrita de risco por ganho sub-ms numa corrida já resolvida), e **perseguimos**
> apenas o **plano morno/pool** (holder netns + IPAM + slirp) nos caminhos
> persistentes (`serve cri`/`serve api`) e/ou opt-in — que já é o item nº 1 da
> lista de optimização do README.

## Contexto

O Kiri anuncia ~2 ms de arranque de container e o dono pediu para extrair lições
para o delonix. A análise (código real de ambos) desfaz o número e separa o que
transfere do que não.

### O que os ~2 ms do Kiri REALMENTE medem

Do `bench/api.sh` do Kiri (`hyperfine --warmup 3 --runs 30`, `alpine true` via
**Docker Engine API**):

| | Kiri | Docker Desktop | OrbStack |
|---|---|---|---|
| Docker Engine API | **2,55 ms** | 213 ms | 380 ms |
| docker CLI | 14,2 ms | 243 ms | 442 ms |

Lido com honestidade:

- **A VM e o agente já estão mornos.** O boot da microVM (~100 ms) e o pull/unpack
  de imagem estão **fora** do cronómetro. Os 2,55 ms são `create+start+wait+delete`
  com a VM viva, o agente (PID 1, ~900 KB) vivo e as camadas já desempacotadas.
  A VM só dorme após 60 s de inactividade.
- **É o caminho da API, não o CLI.** O CLI do Kiri custa 14,2 ms — o overhead é o
  *próprio binário* `docker`, não o runtime.
- Portanto **não é comparável de forma directa** aos ~91 ms de arranque a frio do
  delonix (`docs/comparacao-medida.md`): o delonix é **daemonless** (ADR-0040) —
  cada `container run` é um processo efémero que re-paga arranque de processo +
  montagem de overlay + namespaces + fork do supervisor + exec. O Kiri reutiliza
  tudo num processo persistente.

As **técnicas de arranque**, essas, transferem-se — e o delonix já as tem.

### O que o Kiri faz bem e o delonix JÁ implementa

| Técnica do Kiri (`crates/kiri-init/src/linux/`) | Estado no delonix |
|---|---|
| `clone` para namespaces, sem runc/shim/dockerd | ✅ `clone_init`/`clone_flags` (`crates/adapters/delonix-linux/src/lib.rs`) — clone directo, zero runc/shim |
| Trabalho pesado ANTES do clone (CStrings, mounts ordenados, `/etc/*` pré-gerados) | ✅ `inputs`/`IoSpec` montados no pai antes do `clone_init` |
| Pipes de prontidão em vez de polling (`root_ready`, `net_ready`) | ✅ `ready_w`, `sync`, `cg_sync` (`StartPipes`) |
| Corrida de colocação no cgroup (pai escreve `cgroup.procs` vs exec do filho) | ✅ **já resolvida** pela barreira `cg_sync` — o filho espera a colocação antes do exec |
| Vista de cgroup privada (`CLONE_NEWCGROUP`) | ✅ `unshare(CLONE_NEWCGROUP)` após a colocação (Docker/Podman `--cgroupns=private`) |
| Overlay afinado | ✅ `volatile` (ADR-0056); `mount_overlay_from_lowers` é mínimo, sem sondagem de features por-mount |
| Agente init minúsculo que só reap/forward de sinais | ✅ `supervise.rs` (supervisor efémero por container, modelo `conmon`) |

## Decisão

### Não adoptar

1. **Warm-VM / agente persistente como modelo por omissão.** É a origem real dos
   2 ms do Kiri, mas **parte o guardrail «daemonless by design»** (ADR-0040): o
   delonix não tem (nem quer) um daemon sempre-presente no caminho do CLI. O
   modelo persistente do Kiri é uma escolha de produto diferente; copiá-lo como
   omissão trocaria a filosofia do delonix por latência que o CLI a frio não
   precisa de ter.
2. **`clone3` + `CLONE_INTO_CGROUP`.** Colocaria o container no cgroup de forma
   atómica, dispensando a barreira `cg_sync`. Mas:
   - a corrida que resolveria **já está resolvida** pela `cg_sync` (correcção, não
     micro-optimização);
   - o delonix usa `nix::sched::clone`, não `clone3` — adoptá-la é **reescrever o
     caminho mais perigoso do runtime** (userns/console/pidns/`cg_sync`) por um
     ganho **sub-ms** irrelevante face aos ~91 ms;
   - é risco por quase nada. **Fica fora.**

### Perseguir (trabalho futuro, ADR + fatia e2e-gated própria)

3. **Plano morno/pool** — holder netns + IPAM + `slirp4netns` pré-construídos e
   reutilizados. É onde está o salto real (~91 ms host → ~216–344 ms rede isolada
   a frio, porque o rootless reconstrói veth/IPAM/slirp do zero) e **é já o item
   nº 1 da lista de optimização do README** do delonix. Respeita o daemonless se
   for aplicado **só aos caminhos que já têm processo vivo** (`serve cri` para o
   kubelet, `serve api`) e/ou como **pool opt-in** — o CLI a frio fica igual.
   **Medir primeiro** (onde se vão os ~91 ms) antes de desenhar o pool.

## Consequências

- **Zero alterações de código neste ADR.** O delonix não ganha nada em copiar
  técnicas de arranque que já tem, e perde (risco) em reescrever o `clone`.
- O caminho de optimização fica **nomeado e justificado** para uma sessão futura
  (plano morno), com a fronteira de filosofia explícita: nunca no CLI a frio.
- Serve a narrativa competitiva (o buraco nº 1 de adopção): o delonix sabe
  exactamente onde está face a um concorrente rápido, e porquê, com evidência.

## Alinhamento com os guardrails

- **Daemonless by design:** reafirmado — recusamos o warm-VM-por-omissão por isto.
- **No silent failure / GO-NO-GO antes de nova fronteira de privilégio:** o plano
  morno não abre fronteira nova (reusa infra que o rootless já usa), mas entra por
  ADR + fatia e2e-gated própria, não por aqui.
