# GAP_BACKLOG.md — 0.1.0 (@ 1898466b)

Funcionalidade ausente ou por medir — não é defeito de algo que deveria já
funcionar. Lentes de comparação: Docker 29.9.2, Podman 4.9.3, CRI v1.36.0.

> GAP-0001..0007: paridade Docker/Podman + CRI (ronda de testes).
> **GAP-0008..0014: 8 áreas de valor CNCF** (ver `CNCF_GAP_DOCKER.md` — leitura
> do código oficial Docker/Moby). Objectivo: 100% nas 8 áreas.

## P1

### GAP-0001 — PodPID / `shareProcessNamespace` recusado em pods host-network/CNI
- **Current behaviour:** o caminho existe — `crates/contexts/delonix-compute/src/domain/pod.rs:149-150` (`share_process_namespace`), aplicado em `:406-435`; `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:100` (`wants_shared_pid`), `wire_shared_pid` `:1913-1945`. **Mas há um caminho de recusa para pods hostNetwork/CNI em `:1930-1945`**, e o pod do critest cai nele.
- **Expected platform behaviour:** Docker/Podman/CRI suportam `shareProcessNamespace` — os containers de um pod veem os PIDs uns dos outros.
- **Why it matters:** padrão de sidecar (partilha de processos, signal-handling). Conformância CRI (spec «should support PodPID»).
- **Acceptance criteria:** o spec PodPID passa em rootful-VM.
- **Dependencies:** acoplado a **BUG-0004** (a recusa é para hostNetwork/CNI). **Complexity:** M.

### GAP-0002 — Cobertura do `differential` limitada a 2 fluxos
- **Current behaviour:** `tests/cli-parity/run.sh differential` exercita só flow1 (container interactivo) e flow2 (pod). 76 comandos ficam em «reconhecido-por-testar» (parser) e 36 em «por-fazer» sem equivalência funcional medida vs docker/podman.
- **Expected platform behaviour:** uma matriz de paridade credível mede volumes, networks e images contra os dois motores de referência, não só containers/pods.
- **Why it matters:** o «PASS» do differential assenta em 2 fluxos; o próprio harness avisa que não declara 100%. Sem isto, a afirmação de paridade é parcial.
- **Acceptance criteria:** fluxos differential para volumes (create/rm/inspect round-trip), networks (create/connect/rm) e images (pull/tag/save/load) a correr nos 3 motores.
- **Dependencies:** nenhuma. **Complexity:** M. (= ex-M3)

### GAP-0003 — Streaming port-forward em host-network
- **Current behaviour:** `crates/interfaces/delonix-cri/src/streaming.rs:227-261` tem o ramo host-network (liga directo no netns do host, guardado por `sandbox_is_host_network`, `:239`); `spdy.rs:854` `handle_port_forward`.
- **Expected platform behaviour:** CRI suporta portforward para pods em host network.
- **Why it matters:** conformância CRI (spec «portforward in host network»); kubectl port-forward para pods host-network.
- **Acceptance criteria:** o spec passa em rootful-VM.
- **Dependencies:** acoplado a **BUG-0004** (sandbox_is_host_network). **Complexity:** S depois de B4.

## P2

### GAP-0004 — Docker Engine API apenas parcial (logs + exec)
- **Current behaviour:** só `logs` e `exec` implementados da Docker Engine API.
- **Expected platform behaviour:** clientes/ferramentas do ecossistema Docker esperam a superfície da API (containers, images, networks, volumes, events).
- **Why it matters:** portabilidade de ecossistema (Testcontainers, Compose externos, SDKs).
- **Acceptance criteria:** roadmap por família de endpoints; conformância medida contra um cliente real.
- **Dependencies:** nenhuma. **Complexity:** XL.

### GAP-0005 — Compose com lacunas (DEF-003)
- **Current behaviour:** `delonix compose` suportado com lacunas conhecidas (DEF-003).
- **Expected platform behaviour:** paridade com `docker compose` v2 nas directivas comuns.
- **Acceptance criteria:** fechar DEF-003; matriz de directivas compose medida.
- **Dependencies:** nenhuma. **Complexity:** M.

## P3

### GAP-0006 — Contextos / remote-mgmt (`system connection`) fora-de-âmbito
- **Current behaviour:** 22 comandos de `system`/contextos marcados fora-de-âmbito por ADR-0010.
- **Expected platform behaviour:** docker `context` / podman `system connection`.
- **Why it matters:** gestão multi-host. É decisão arquitectural deliberada — registar como WONT_FIX até ADR-0010 reabrir, não como dívida silenciosa.
- **Dependencies:** ADR-0010. **Complexity:** L.

### GAP-0007 — 36 comandos «por-fazer» na matriz CLI
- **Current behaviour:** inclui `network prune` e famílias de `image`/`volume` por completar (ver `tests/cli-parity/matriz-cli-parity.csv`).
- **Acceptance criteria:** cada comando sobe a «reconhecido» → «passou» ou é classificado fora-de-âmbito com motivo.
- **Dependencies:** GAP-0002 (medição). **Complexity:** varia (S–M por comando).

---

# Áreas de valor CNCF (GAP-0008..0014)

Fundamentação completa em `CNCF_GAP_DOCKER.md`. Já alinhado: OSV (scanner),
cosign/sigstore (`sign.rs`), rootless-first.

## P1

### GAP-0008 — Provenance SLSA no `delonix build` (área CNCF 3)
- **Current behaviour:** `delonix-oci/src/build.rs` constrói a imagem mas não emite proveniência.
- **Expected platform behaviour:** BuildKit emite SLSA provenance (v0.2/v1.1, mode min/max, builder-id, reproducible) como in-toto attestation anexada à imagem.
- **Why it matters:** supply-chain verificável — o sinal que a CNCF hoje espera de um builder (SLSA). Aditivo: `sign.rs` (cosign) já dá o mecanismo de attestation.
- **Acceptance criteria:** `delonix build --provenance[=mode=min|max]` anexa predicate SLSA v1.1 (builder.id, materials, invocation) em in-toto ao índice OCI; verificável por um cliente terceiro.
- **Dependencies:** GAP-0009 (partilham o anexo de attestations). **Complexity:** L.

### GAP-0009 — SBOM SPDX in-toto anexado à imagem (área CNCF 4)
- **Current behaviour:** `delonix-scanner` extrai SBOM (apk/dpkg) para match CVE; não o publica como attestation da imagem.
- **Expected platform behaviour:** BuildKit anexa SBOM **SPDX** em **in-toto**, scanner plugável (syft).
- **Why it matters:** interoperabilidade de supply-chain (consumível por Scout, cosign, cadeias SLSA).
- **Acceptance criteria:** `delonix build --sbom` emite SPDX JSON (do scanner existente) em in-toto anexado ao índice OCI; `delonix image sbom <ref>` exporta SPDX.
- **Dependencies:** —. **Complexity:** M.

### GAP-0012 — Conformância OCI/distribution/CRI comprovada + posição de runtime (áreas CNCF 1+2)
- **Current behaviour:** runtime próprio runc-compat; CRI parcial (84/19 · 90/13); sem passagem publicada das suites OCI.
- **Expected platform behaviour:** containerd (CNCF graduated) passa CRI 100%; Moby é referência OCI image/distribution.
- **Why it matters:** a CNCF valoriza conformância demonstrável e reutilização de projectos graduados.
- **Acceptance criteria:** CRI ≥95% rootful-VM (fecha com BUG-0003/04/05/06 + GAP-0001/0003); correr+publicar **oci image-spec/distribution-spec/runtime-spec conformance**; ADR a decidir «runtime próprio vs adoptar containerd».
- **Dependencies:** BUGs CRI. **Complexity:** XL.

## P2

### GAP-0011 — Convergência para a Compose Specification (área CNCF 6)
- **Current behaviour:** compose próprio com lacunas (DEF-003, ver GAP-0005).
- **Expected platform behaviour:** aderir à Compose Specification (`compose-spec/compose-spec`, comunitária).
- **Why it matters:** interoperabilidade de manifests; a spec é governada em comunidade.
- **Acceptance criteria:** suite de conformância contra a spec; directivas comuns verdes; divergências documentadas.
- **Dependencies:** GAP-0005. **Complexity:** M.

### GAP-0013 — Scanner maduro em CI: policy + compare + gating (área CNCF 8)
- **Current behaviour:** `delonix-scanner` faz SBOM+CVE (OSV); sem policy/compare/CI.
- **Expected platform behaviour:** Docker Scout: policy engine, `compare` entre imagens, `--exit-code --only-severity`.
- **Why it matters:** gating de supply-chain em pipelines — uso cloud-native real.
- **Acceptance criteria:** `delonix scan <ref> --exit-code --only-severity critical,high`; `delonix scan compare A B`; política declarável (kind).
- **Dependencies:** —. **Complexity:** M.

### GAP-0010 — Build cloud-native: cache distribuído + multi-plataforma (área CNCF 5)
- **Current behaviour:** build local; sem cache exportável nem multi-arch nativo.
- **Expected platform behaviour:** BuildKit: `--cache-to/--cache-from` (registry), `--platform` multi-arch, frontends LLB.
- **Why it matters:** builds de CI reprodutíveis e partilháveis; imagens multi-arch.
- **Acceptance criteria:** `delonix build --platform linux/amd64,linux/arm64` → índice multi-arch; `--cache-to/--cache-from type=registry` round-trip. (Frontends LLB fora de âmbito — decisão arquitectural.)
- **Dependencies:** —. **Complexity:** XL.

## P3

### GAP-0014 — Governança aberta do projecto (área CNCF 7)
- **Current behaviour:** repo privado, org própria.
- **Expected platform behaviour:** governança aberta — Moby (doou runc/containerd); Podman em LFX/Linux Foundation com **releases PGP-signed**, security mailing list, **LTS**, roadmap público, LFX Health Score.
- **Why it matters:** a CNCF pesa abertura e governança, não só código.
- **Acceptance criteria:** decisão do dono — público com governança documentada, SECURITY.md, CODEOWNERS, política de release, **releases assinadas (PGP/sigstore)**, canal de segurança, roadmap público. **Não é só código.**
- **Dependencies:** decisão estratégica. **Complexity:** M.

---

# Diferenciais do Podman (GAP-0015..0017)

Fundamentação em `CNCF_GAP_PODMAN.md`. O Delonix já está alinhado em daemonless,
rootless, pods, `policy.json` de runtime, cosign verify e unit systemd do node-api.

## P1

### GAP-0015 — Interop Kubernetes YAML: `kube play` / `kube generate` (área CNCF 2)
- **Current behaviour:** manifests próprios `compute.delonix.io/v1beta1`; `lower_application` baixa a Application a pods; não lê nem emite YAML Kubernetes standard de workloads.
- **Expected platform behaviour:** `podman kube play <k8s.yaml>` corre um Pod/Deployment k8s; `podman kube generate` emite YAML k8s de um pod/container existente.
- **Why it matters:** o Kubernetes é o centro do ecossistema CNCF; correr/gerar YAML k8s é o diferencial de interop mais forte do Podman.
- **Acceptance criteria:** `delonix kube play <manifest.yaml>` corre workloads em `apiVersion: v1`/`apps/v1` (Pod no mínimo); `delonix kube generate <ref>` emite YAML k8s válido (round-trip verificado).
- **Dependencies:** —. **Complexity:** L.

## P2

### GAP-0016 — Quadlet/systemd para workloads do utilizador (área CNCF 5)
- **Current behaviour:** `delonix serve` gera unit só para o `node-api` (`cmd/serve.rs`), não para containers/pods do utilizador.
- **Expected platform behaviour:** Quadlet / `podman generate systemd` — correr containers/pods como unidades systemd (boot, supervisão).
- **Why it matters:** integração nativa com o init do host — operação cloud-native em nós single-host.
- **Acceptance criteria:** `delonix generate systemd <container|pod>` emite unit funcional; opcionalmente formato Quadlet declarativo.
- **Dependencies:** —. **Complexity:** M.

### GAP-0017 — Trust de assinatura por policy declarativa (áreas CNCF 4/8)
- **Current behaviour:** `sign.rs` verifica cosign por chave explícita; `policy.json` existe mas para admissão/runtime, não trust de assinatura por registo.
- **Expected platform behaviour:** `containers-policy.json` do Podman — confiança declarativa por registo/repo (sigstore + GPG), aplicada no pull/run.
- **Why it matters:** cadeia de confiança accionável em produção (recusar imagem não-assinada por política), não só verificação ad-hoc.
- **Acceptance criteria:** estender `policy.json` com trust de assinatura por registo/repo (sigstore + chave); `delonix container run`/`image pull` recusa imagem fora da policy.
- **Dependencies:** GAP-0009 (formato de attestation). **Complexity:** M.
