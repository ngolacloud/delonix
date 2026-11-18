# report.md — campanha de regressão do CLI `delonix 0.1.2`

Campanha de regressão da **superfície pública** do binário entregue, pelo
executável real com argumentos reais, host **rootless**, 2026-11-18. Liga a H05
(roteador da skill E2E) e H07 (esquema de evidência) do harness; a evidência
estruturada vive em `evidence.jsonl` e valida contra `docs/dev/evidence-schema.md`
(`scripts/evidence_gate.py`).

## 1. Baseline do candidato (secção 1 do prompt mestre)

| Campo | Valor |
|---|---|
| Versão | `delonix 0.1.2` |
| SHA base/candidato | `b72edeedf38f4f46075520a449a37665228956e3` (HEAD de `develop` no arranque) |
| Digest do binário | `cfc096220a0e064f60036993361188469f82585ec06844fd54a1957c5d650b16` |
| Build | `cargo build --release --locked -p delonix-bin …` (perfil de release: `lto=thin`, `codegen-units=1`) |
| Binários | **5 no mesmo target** — `delonix`, `delonix-cri`, `delonix-mgmt`, `delonix-mcp`, `delonix-node-api` (todos 0.1.2) |
| Toolchain | `rustc`/`cargo 1.96.0`; tag em HEAD `consolidacao-pre-20261118-2012` |
| Host | i9-13900H (20 threads), 62 GiB, kernel 7.0.0-43, rootless; `cloud-hypervisor`+`libvirt`+`/dev/kvm` presentes; **sem sudo** |
| Lentes | docker 29.10.0, podman 4.9.3, k8s/CRI, cloud-hypervisor |

> **Nota de concorrência.** Durante a campanha o dono avançou `develop` de
> `b72edeed` para `9ec12592`, mesclando `fix/cli-ls-parity-and-buildcache-prune`
> (`29b10702`), que fecha **GAP-0019** (`ls -q/-a`) e **GAP-0027** (prune do
> build-cache) — ambos observados nesta ronda como ainda abertos em `b72edeed`. O
> candidato testado é `b72edeed`; não se re-construiu para `9ec12592`.

## 2. Descoberta da árvore pública (secção 2)

`scripts/cli-tree.sh` percorre a árvore do **parser real** do binário entregue:

```
comandos: 413   folhas: 366   grupos: 78
classes:  250 ~ (grafia)   97 = (sem quebra)   17 → (movida por decisão)   2 ! (quebra documentada)
```

- `cli-tree.sh --gate` → **verde**: as 366 folhas batem com `scripts/cli_baseline.tsv`.
  **Zero drift de superfície** vs a última linha de base (0.1.1): as alterações do
  delta no parser (`build.rs`, `container.rs`, `stack.rs`, `system.rs`,
  `scaffold.rs`) foram **comportamentais**, não de contrato.
- `cli_exec_ratchet.py --from-results` → **272 de 366 folhas executadas sob
  asserção (74,3 %)**, igual ao baseline, `rc=0` — sem regressão de cobertura.

## 3. Execução da suite pública (secções 3-4) — bateria `e2e.sh`

Bateria canónica do repo (os 5 bins, host rootless, auto-isolada em
`DELONIX_ROOT`/`DELONIX_NET_RUNTIME_DIR` próprios). Artefacto completo:
`e2e-results.jsonl` (1792 registos).

```
PASS=1735   FAIL=22   SKIP=35   XFAIL=0   XPASS=0
```

### 3.1 Achado de robustez do arnês (mede-se a bancada antes do motor)

A **1ª corrida** deu `FAIL=25`. Seis dessas falhas eram de cloud-hypervisor
(`error[DX-1506] … socket path … is 143 bytes, a UNIX socket path is limited to
107`). **Não eram regressões nem falta de ambiente**: o `OUT`/`DELONIX_ROOT` ia
sob o caminho-scratch longo da sessão, e o socket do VMM (`<root>/vms/<nome>.sock`)
estourava o limite AF_UNIX de 107 bytes — que o binário **diagnostica
correctamente** ("use a shorter VM name or a shorter DELONIX_ROOT"). Com um root
curto, a mesma VM cloud-hypervisor **arranca** (`✓ VM started`). Os números acima
(`FAIL=22`) são já da re-corrida com root curto. → ver GAP/achado H-1.

## 4. Regressões da 0.1.1 — confirmadas CORRIGIDAS em 0.1.2

| Bug (0.1.1) | Verificação na 0.1.2 | Estado |
|---|---|---|
| **BUG-0007** ImageID docker-api = nome, não digest | `GET /containers/json` → `ImageID: sha256:3ea06e…`, `Image` mantém `alpine:3.24` | ✅ **PASS** |
| **BUG-0008** exec herda grupos `65534(nobody)` | `exec: no --group-add leaves no inherited host groups (65534)` | ✅ **PASS** |
| **BUG-0009** drift falso por casing `Always`≠`always` | `drift: logo a seguir ao apply não há deriva` + checks de `restartPolicy` | ✅ **PASS** |

## 5. Triagem das 22 falhas — **0 regressões de binário**

| # | Categoria | Falhas | Veredicto |
|---|---|---|---|
| A | **Limitadas por privilégio/rootless** (lab-ephemeral) | CRI default-caps, landlock+tmpfs, 2× mount `/sys` on-top/rshared, 2× `net netns firewall/publish` + 2 cascatas, slirp-flaky | **BLOCKED** — o host não mede rootless; mediria em VM rootful + CNI |
| B | **Staleness da bateria vs 0.1.2** (binário correcto) | 2× `--disk-size 0` (clap recusa a `rc=2`, teste esperava `1`), 2× kind `networkgateways` (não existe no catálogo; há `Gateway`+`Router.networkGateways`), `/metrics` (`# TYPE delonix_build info` OpenMetrics — teste procura a linha antiga `_info`), `cluster network profiles`/`system monitor` formato, `cluster kube generate` exit `1`≠`4` | **desvio-de-teste** — asserção stale, não defeito |
| C | **Setup de `vm --require`** | imagem inexistente dispara DX-4000 antes do probe de capacidade (ordem discutível) | setup/needs-image |
| D | **Achado genuíno (GAP-0028)** | VM com o mesmo nome **noutra namespace** não é conflito (`rc=0 ensured`), ao passo que #824/`3de5db5c` tornou-o conflito para **containers** | 🟡 GAP novo |

Detalhe e reprodução em `FINDINGS_BACKLOG.md`.

## 6. Labs privilegiados (perfil lab-ephemeral) — o que correu, o que fica

| Lab | Resultado | Base |
|---|---|---|
| **Compute / microVM (cloud-hypervisor)** | ✅ **PASS rootless** | a bateria cria/arranca/snapshota VMs CH com root curto; `/dev/kvm`+VMM+libvirt presentes |
| **KaaS — `cluster create` (kind-mode, sem Docker)** | ✅ **PASS rootless** | control-plane `kindest/node v1.37.0`, kubeadm, CNI kindnet; `kubectl get nodes` → `camp012-control-plane Ready control-plane v1.37.0 containerd://2.3.4`; `destroy --force` limpo (0 nós, 0 containers). Log: `cluster-lab.log` |
| **CRI conformance (critest) completa** | ⛔ **BLOCKED** | exige root + conflist CNI (`portmap`) + imagem golden `delonix-vm-k8s`; host rootless, sem sudo (política do repo: privilegiado só em VM) |
| **Rootful mounts / netns-nft / multi-nó HA** | ⛔ **BLOCKED** | mesmo pré-requisito (VM rootful descartável) |

## 7. Veredicto por perfil (secção 5 — gate)

| Perfil | Veredicto | Base |
|---|---|---|
| **pr-rootless** (gates de merge→develop) | ✅ **APROVA** | `cli-tree-gate`, `cli-exec-ratchet` e `regression-bug-0007-0008-0009` todos `PASS exit 0` (`evidence_gate.approves` = APROVA); árvore sem drift; 3 bugs confirmados corrigidos |
| **lab-ephemeral** (produz evidência) | ◑ **parcial** | compute-CH e KaaS **PASS**; a bateria `e2e` completa é `FAIL=22` (categorias A-D: 0 regressões de binário); critest/rootful **BLOCKED** |
| **release** (promoção→main/tag) | ⛔ **BLOCKED** | fora de âmbito (sem corte de tag); exige critest verde num lab rootful |

### Conclusão
O **artefacto 0.1.2 entregue não tem regressão de contrato nem de binário**
mensurável no perfil rootless: a árvore pública é estável (zero drift), a
cobertura de execução mantém-se (272/366), os 3 bugs da 0.1.1 estão corrigidos, e
as duas maiores promessas privilegiadas que **se podem** provar rootless
(microVM cloud-hypervisor e KaaS kind-mode) **passam**. As 22 falhas da bateria
repartem-se por lacunas de ambiente (BLOCKED, precisam de VM rootful) e
**staleness da própria bateria** (o binário comporta-se correctamente, as
asserções é que envelheceram) — mais **um** GAP genuíno novo (GAP-0028) e **um**
achado de robustez do arnês (H-1, sensibilidade ao comprimento de `DELONIX_ROOT`).
A promoção para release fica **bloqueada** apenas no que exige lab rootful
(critest), tal como o harness prevê.

## 8. Artefactos
- `e2e-results.jsonl` — bateria completa (1792 registos, PASS/FAIL/SKIP por folha).
- `e2e-run.log` — stdout da re-corrida (root curto).
- `cluster-lab.log` — bring-up+teardown do cluster KaaS rootless.
- `evidence.jsonl` — 6 registos de evidência (valida contra `evidence-schema.md`).
- `COVERAGE_MATRIX.md` — cobertura por eixo (canónicos/aliases/parâmetros/funcional/perfis).
- `FINDINGS_BACKLOG.md` — GAP-0028, achado H-1 e os itens de staleness da bateria.
