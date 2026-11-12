# CNCF gap analysis — Delonix × Podman (diferenciais específicos)

> Leitura do código oficial do Podman (via `gh`). **Nota de org:** o projecto
> renomeou-se de `containers/*` para **`podman-container-tools/*`**
> (`podman`, `buildah`, `skopeo`); `crun`/`image`/`storage` continuam em
> `containers/*`. Cruzado com o código do Delonix.

## Posicionamento
O Delonix é **filosoficamente mais próximo do Podman** do que do Docker:
daemonless, rootless-first, pods nativos. Por isso esta análise foca o que o
Podman tem de **distinto** e que o Delonix ainda não — não repete o que já está
em `CNCF_GAP_DOCKER.md`.

## Já alinhado com o Podman (não refazer)
- **Daemonless + rootless-first** — mesma arquitectura. ✔
- **Pods nativos** — Delonix tem (declarativo, `kind: Pod`). ✔
- **`policy.json` de runtime** — `delonix-security/src/domain/policy.rs` (`<root>/policy.json`, `RuntimePolicy`, admission). ✔
- **Unit systemd** — `delonix serve` gera `delonix-node-api.service` (mas só para o próprio serviço — ver GAP-0016). ◑
- **cosign verify** — `delonix-oci/sign.rs`. ✔

## Diferenciais do Podman → GAPs novos

| Diferencial Podman (evidência) | Delonix hoje | GAP | Área CNCF |
|---|---|---|---|
| **`podman kube play` / `kube generate` / `kube apply`** — corre e gera **YAML Kubernetes standard** (docs `podman-kube-*.md`) | kinds **próprios** (`compute.delonix.io/v1beta1`), `lower_application`; **não** consome/gera `apiVersion: v1`/`apps/v1` | **GAP-0015** | 2 (interop k8s) |
| **Quadlet / `generate systemd`** — corre containers/pods do utilizador como **unidades systemd** (docs `podman-quadlet-*`, `podman-generate-systemd`) | gera unit só para o `node-api`, não para workloads do utilizador | **GAP-0016** | 5 (ops cloud-native) |
| **Verificação por policy de confiança** — `containers/image` `containers-policy.json` + sigstore + GPG (trust declarativo por registo/repo) | tem `policy.json` de **admissão/runtime** e `sign.rs` (cosign verify por chave), mas **não** trust de assinatura declarativo por registo | **GAP-0017** | 4/8 (supply-chain) |
| **Governança LFX madura** — LFX Health Score, releases **PGP-signed**, security mailing list, LTS, roadmap público | repo privado | reforça **GAP-0014** | 7 (governança) |
| **Skopeo** — `copy`/`inspect`/`sign` entre registos **sem pull local nem privilégio** | sem copy registo→registo sem pull | nota (menor) | 5 |

## Impacto nas 8 áreas CNCF
Os diferenciais Podman **não abrem uma 9ª área** — refinam as existentes:
- **Área 2 (conformância/interop k8s):** GAP-0015 adiciona a dimensão «correr/gerar
  YAML Kubernetes de *workloads*», distinta da conformância CRI. É o diferencial
  CNCF mais forte do Podman (o k8s é o centro do ecossistema).
- **Área 5 (ops cloud-native):** GAP-0016 (systemd/Quadlet) junta-se a GAP-0010.
- **Áreas 4/8 (supply-chain):** GAP-0017 (trust por policy) junta-se a GAP-0008/09/13.
- **Área 7 (governança):** GAP-0014 ganha critérios concretos (PGP releases, LFX,
  security list, LTS, roadmap público).

## Fichas dos GAPs novos

### GAP-0015 — Interop Kubernetes YAML: `kube play` / `kube generate` (área 2)
- **Current:** manifests próprios `compute.delonix.io/v1beta1`; `lower_application` baixa a Application a pods; não lê nem emite YAML Kubernetes standard de workloads.
- **Expected (Podman):** `podman kube play <k8s.yaml>` corre um Pod/Deployment k8s; `podman kube generate` emite YAML k8s de um pod/container existente.
- **Target 100%:** `delonix kube play <manifest.yaml>` corre workloads descritos em `apiVersion: v1`/`apps/v1` (Pod, no mínimo); `delonix kube generate <ref>` emite YAML k8s válido.
- **Component:** `bins/delonix/src/cmd/` + `delonix-compute` (conversor k8s↔envelope). **Compl.:** L. **Dep.:** —.

### GAP-0016 — Quadlet/systemd para workloads do utilizador (área 5)
- **Current:** `delonix serve` gera unit só para o `node-api` (`cmd/serve.rs`).
- **Expected (Podman):** Quadlet / `podman generate systemd` — correr containers/pods do utilizador como unidades systemd (arranque no boot, supervisão).
- **Target 100%:** `delonix generate systemd <container|pod>` emite uma unit funcional; opcionalmente um formato Quadlet declarativo.
- **Component:** `bins/delonix/src/cmd/` (reutiliza a mecânica de `serve.rs`). **Compl.:** M. **Dep.:** —.

### GAP-0017 — Trust de assinatura por policy declarativa (áreas 4/8)
- **Current:** `sign.rs` verifica cosign por chave explícita; `policy.json` existe mas para admissão/runtime, não para trust de assinatura por registo.
- **Expected (Podman):** `containers-policy.json` — confiança declarativa por registo/repo (sigstore + GPG), aplicada no pull/run.
- **Target 100%:** estender o `policy.json` com regras de trust de assinatura por registo/repo (sigstore + chave); `delonix container run`/`image pull` recusa imagem não-assinada conforme a policy.
- **Component:** `delonix-security/domain/policy.rs` + `delonix-oci/sign.rs` + `registry.rs`. **Compl.:** M. **Dep.:** GAP-0009 (formato de attestation).
