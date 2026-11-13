# GAP-0009 — Anexo in-toto/OCI de attestations (Onda 1A, áreas CNCF 3+4)

> Pivô da campanha de aceitação CNCF. Entrega a área 4 (SBOM interoperável) quase
> inteira e o mecanismo de anexo da área 3 (SLSA, GAP-0008). Base: develop @
> `67ddef41`. Contrato de referência: `moby/buildkit`
> `exporter/containerimage/writer.go` + `util/attestation/types.go`.

## Objectivo
`delonix build --sbom` anexa à imagem um SBOM SPDX embrulhado em in-toto; `delonix
image sbom <ref>` exporta o SPDX. A attestation fica descoberta por um consumidor
standard (`cosign verify-attestation`, `docker buildx imagetools inspect`).

## Contrato a implementar (literais do BuildKit)
- **Statement in-toto cru** (NÃO DSSE): `_type: https://in-toto.io/Statement/v1`,
  `predicateType: https://spdx.dev/Document`, `subject: [{name, digest:{sha256}}]`,
  `predicate: <SPDX-2.3>`.
- **Layer** do attestation manifest: `mediaType application/vnd.in-toto+json`,
  annotation `in-toto.io/predicate-type = https://spdx.dev/Document`.
- **Attestation manifest**: `artifactType =
  application/vnd.docker.attestation.manifest.v1+json`, `config =` empty
  (`application/vnd.oci.empty.v1+json`), `subject =` descriptor do manifest da
  imagem alvo.
- **Descoberta**: OCI 1.1 **referrers** via `subject` (push por digest). Opcional
  (fase 2): referenciar também num index com `platform=unknown/unknown` +
  annotations `vnd.docker.reference.type=attestation-manifest` /
  `vnd.docker.reference.digest=<img>` (convenção buildx).

## Pontos de enxerto (reutilização)
- `delonix-scanner/lib.rs::sbom_spdx()` — SPDX-2.3 já pronto → só embrulhar.
- `delonix-oci/registry.rs::push_layer_annotated()` (:2548) — constrói manifest
  com `artifact_type` + layer anotada; **falta `.subject(...)`** → adicionar
  variante `push_attestation(subject_descriptor, …)`.
- `delonix-oci/sign.rs` — padrão de resolver o digest do manifest da imagem
  (reusar para montar o `subject`).
- `bins/delonix/src/cmd/build.rs:1410` — já chama `write_sbom` por build; ponto
  de ligação do `--sbom` para anexar além de gravar local.

## Passos
1. **Wrapper in-toto** (pure fn `intoto_statement(spdx, image_name, image_digest)`
   em `delonix-scanner` ou `delonix-oci`) + testes de forma.
2. **`push_attestation`** em `registry.rs`: resolve descriptor do alvo, monta
   manifest com `subject`+`artifactType`, push por digest.
3. **CLI**: `delonix image sbom <ref> [--attach]`; `build --sbom` liga o anexo.
4. **Testes**: round-trip contra registry local (reusar `serve_anon_registry`),
   verificar referrers/subject e o `predicate-type`.

## Done (efeito verificável) — FEITO 2026-11-12

Implementado:
- Wrapper in-toto (`delonix-scanner`: `intoto_statement`, `sbom_intoto_statement`).
- Anexo OCI (`delonix-oci/registry.rs`: `push_attestation` — `subject` +
  `artifactType` + layer `vnd.in-toto+json` + annotation `predicate-type`),
  **com fallback por referrers-tag-schema** (`sha256-<hex>` índice) para
  registries sem Referrers API.
- Leitor nativo (`list_referrers` + `Referrer`; CLI `delonix image attestations`),
  com fallback ao tag-schema quando a API dá 404.
- CLI `delonix image sbom <ref> --attach`.

Prova:
- **Unit** (mock registry, HTTP real in-process): `push_attestation_binds_to_the_
  image_as_a_referrer` — push imagem → attach → Referrers API lista a attestation
  (subject=digest, artifactType, predicate-type); blob = Statement exacto. +2 no
  scanner. Suites: 196 (oci) + 30 (scanner), verdes.
- **Live** (registry:3 real / distribution, imagem alpine:3.20 real):
  1. `delonix image attestations` lista `sha256:5099…
     application/vnd.docker.attestation.manifest.v1+json  https://spdx.dev/Document`;
  2. `curl` ao índice tag-schema devolve o referrer com a annotation;
  3. **`docker buildx imagetools inspect`** (terceiro) lê o mesmo índice.
  A Referrers API nativa deste registry deu 404 → o fallback tag-schema foi o que
  tornou a attestation descoberta (como buildx/cosign/oras fazem).

Notas:
- Statement **cru, não DSSE** (shape BuildKit). `image sign` (cosign `.sig`)
  intacto — mecanismos complementares.
- `cosign verify-attestation` NÃO valida isto: exige assinatura DSSE própria do
  cosign. Assinar a attestation (DSSE + cosign) é passo seguinte (GAP-0017/trust).

---

# GAP-0008 — SLSA provenance no build (Onda 1B, área CNCF 3) — FEITO 2026-11-12

Reutiliza inteiramente o mecanismo do GAP-0009 (in-toto wrapper + `push_attestation`
+ fallback tag-schema). Só acrescenta: gerar o **predicate SLSA v1** durante o build.

Implementado:
- `delonix-scanner`: `PREDICATE_TYPE_SLSA`, `ProvenanceMode{Min,Max}`, `Material`,
  `ProvenanceFacts`, `slsa_provenance_predicate()` (pura; shape BuildKit
  `buildDefinition{buildType,externalParameters,internalParameters,
  resolvedDependencies}` + `runDetails{builder,metadata}`; `byproducts` omitido
  como no BuildKit). Min retira valores de build-args; Max inclui-os.
- `build --provenance[=min|max]` (default max quando presente): `write_provenance`
  re-parseia o Dockerfile, resolve as bases `FROM` no store → `resolvedDependencies`
  (materials), e cacheia em `<root>/images/<id>.provenance.json`.
- `image provenance <ref> [--attach]`: imprime, ou embrulha em in-toto (predicate
  SLSA) e anexa via `push_attestation`.

Prova:
- **Unit**: `slsa_max_includes_build_arg_values_and_materials`,
  `slsa_min_withholds_build_arg_values_but_keeps_keys` (scanner 32 verdes).
- **Live** (registry:3 + build real `FROM alpine:3.20`):
  1. `build --provenance=max` gerou predicate SLSA v1 com material `alpine:3.20`
     @ `bf8527eb…` (digest real), builder id, finishedOn, invocationId;
  2. `image provenance --attach` publicou-o;
  3. `image attestations` lista **as duas** attestations na mesma imagem:
     `https://spdx.dev/Document` **e** `https://slsa.dev/provenance/v1`;
  4. curl independente ao índice tag-schema: 2 referrers, predicate-types distintos.

Resultado CNCF: **área 3 (SLSA) de ✗ → ◑ forte** (predicate emitido + anexado,
verificável); **área 4 (SBOM) → ✔ no mecanismo** (gerado + anexado + descoberto).
Clippy `-D warnings` limpo; folhas.tsv actualizado (image attestations/provenance).

Follow-up (não nesta onda): assinar as attestations (DSSE+cosign) p/
`cosign verify-attestation` — encaixa no GAP-0017 (trust por policy).

---

# GAP-0013 — Scanner policy + compare + CI gating (Onda 2, área CNCF 8) — FEITO 2026-11-12

Alavanca o OSV + `--fail-on` já existentes. Acrescenta a camada de maturidade
que o Docker Scout tem e o Delonix não tinha.

Implementado:
- `delonix-scanner` (puro): `Policy{fail_on,ignore,ignore_unfixed}` + `Policy::load`,
  `evaluate_policy() -> PolicyOutcome{kept,waived,worst,failed}`,
  `compare_findings() -> ScanDiff{added,removed}` (chave `(id,package)`).
- CLI `image scan`:
  - `--policy <ficheiro>`: gate declarativo (threshold + waivers com audit trail +
    `ignore_unfixed`); precede o `--fail-on`; sai 1 em falha.
  - `--compare-to <image>`: delta de CVEs entre duas imagens (como `scout compare`).
  - helper `findings_for` (reusa extract_sbom + load_advisories + scan sem imprimir).

Prova:
- **Unit** (scanner 35 verdes): `policy_waives_by_id_and_judges_the_rest_against_fail_on`,
  `policy_ignore_unfixed_drops_findings_with_no_fix`,
  `compare_reports_added_and_removed_by_id_and_package`.
- **Live** (alpine:3.20 busybox 1.36.1-r31; advisory forjado fixed r40, High):
  - (A) `scan --fail-on high` → **exit 1** (gate CI falha no finding);
  - (B) `scan --policy` (waiver do CVE) → «policy waived 1 finding(s)» → **exit 0**;
  - (C) `scan alpine:3.20 --compare-to debian:12-slim` → `- removed CVE … busybox High`
    (o advisor apk não casa em dpkg — delta correcto).
- Clippy `-D warnings` limpo; sem folha CLI nova (só flags em `image scan`).

Resultado CNCF: **área 8 (scanning) de ◑ → ◑ forte / quase ✔** — policy declarativa,
compare e gating CI, tudo sobre o OSV existente. Falta (GAP-0017) o trust de
assinatura por policy, que reutiliza o anexo de attestations desta campanha.

---

# GAP-0017 — Trust de assinatura por policy declarativa (Onda 2, áreas CNCF 4/8) — FEITO 2026-11-12

Fecha o eixo supply-chain. Reutiliza `sign.rs::verify_signature` (cosign por
chave) + o padrão de admissão-no-pull (como `admission_scan_on_pull`).

Implementado (CLI, `scan.rs`):
- `repo_matches(pattern, reference)` — pura: casa o repo (sem :tag/@digest) por
  exacto ou wildcard prefixo (`host/ns/*`).
- `admission_trust_on_pull(images, reference)` — lê `$DELONIX_TRUST_POLICY` ou
  `<root>/trust.json`; se uma regra casa o repo, verifica a assinatura cosign
  contra a chave de confiança; `mode:"enforce"` (default) RECUSA não-assinada,
  `mode:"warn"` só avisa. Sem policy / sem regra a casar → sem exigência
  (explícito, nunca allow silencioso para repo sob policy).
- Hook em `image pull`: trust ANTES do scan-on-pull ("quem é" antes de "o que traz").

Formato `trust.json`:
`{ "mode":"enforce", "rules":[ {"repo":"ghcr.io/ngolacloud/*", "key":"/path/key.pub"} ] }`

Prova:
- **Unit**: `repo_matches_exact_wildcard_and_drops_the_tag` (bin verde).
- **Live** (registry:3 + delonix `image sign`):
  - (A) pull da imagem **assinada** → «→ trust: valid signature …» → **exit 0**;
  - (B) pull da **não-assinada** (enforce) → «refused: not signed by a key trusted
    for this repo per the trust policy …» → **exit 1**;
  - (C) pull da não-assinada (mode=warn) → warning + imagem baixada → **exit 0**.
- Clippy `-D warnings` limpo; sem folha CLI nova (env+ficheiro no caminho de pull).

Resultado CNCF: **áreas 4/8 reforçadas** — trust de assinatura declarativo por
repo, fail-closed, aplicado no pull. Eixo supply-chain (3+4+8) agora coerente:
SBOM+provenance anexados (0009/0008), scanning com policy/compare/gate (0013),
e trust por policy (0017).

Follow-up (interop externa, bloqueado no host): assinar as attestations em DSSE +
`.att` cosign para `cosign verify-attestation` as validar (cosign não instalável
aqui). O trust-by-policy já é verificável nativamente.

---

# GAP-0015 — Interop Kubernetes YAML: kube play/generate (Onda 3, área CNCF 2) — FEITO 2026-11-12

Maior diferencial CNCF do Podman (k8s é o centro do ecossistema). Reutilização
máxima: o `PodSpec`/`PodContainer` do Delonix JÁ usa nomes de campo k8s (serde
rename) — a única diferença para um Pod k8s v1 é o `apiVersion`.

Implementado:
- `delonix kube play <k8s.yaml>` (`kube.rs::cmd_play`): parseia YAML k8s standard
  (multi-doc), deserializa o `spec` directamente em `PodSpec` e chama
  `pod::create_pod` (tornado `pub(crate)`). `kind: Pod` (apiVersion v1); Deployment/
  ReplicaSet/StatefulSet/DaemonSet corridos como 1 pod do `spec.template.spec`;
  kinds desconhecidos avisados e saltados.
- `delonix kube generate <ref>`: já existia (sob `cluster kube`); **corrigido** o
  bug de detecção de pod (filtrava por `.pod`, que guarda o nome da NETNS
  `pod-<name>`; agora filtra pelo label `delonix.io/pod`).
- Comando de topo `delonix kube {play,generate}` (main.rs) — paridade com
  `podman kube play/generate`. O `cluster kube generate` mantém-se.

Prova:
- **Unit**: kube (12 verdes, inclui pod_manifest).
- **Live** (host rootless, net-runtime-dir curto p/ SUN_LEN):
  1. `kube play` de um Pod k8s **standard** (`apiVersion: v1`) → `played pod
     demo-kube`, pod Running (ip 10.200.0.2), container a correr `sleep 3600`;
  2. `kube generate demo-kube` → YAML k8s válido (apiVersion v1, kind Pod,
     metadata, containers name/image/command/args/resources);
  3. validação estrutural independente (python yaml) → Pod v1 correcto;
  4. **round-trip**: `kube play` do YAML GERADO → `played pod demo-rt`.
  (kubectl --dry-run=client falha só por não haver cluster/openapi — não é
  defeito do YAML.)
- Clippy `-D warnings` limpo; folhas.tsv +2 (kube play/generate).

Resultado CNCF: **área 2 (interop k8s) de ✗ → ◑ forte** — corre e gera YAML
Kubernetes standard de workloads (a dimensão de interop, distinta da conformância
CRI que já estava em 103/103). Fica a nota: campos k8s avançados (probes,
lifecycle) são recusados por `POD_CONTAINER_FIELDS` — expansão incremental futura.

---

# Área CNCF 6 — Interpolação Compose `${VAR}` + `.env` (Onda 4) — FEITO 2026-11-12

O maior gap NORMATIVO do Compose (`12-interpolation.md`), que o Delonix não tinha
(lia o YAML directo por serde_yaml, sem substituição nem `.env`).

Implementado (`compose.rs`, puro + ligado):
- `interpolate(text, lookup)` — passe shell-style ANTES do parse YAML:
  `$VAR`, `${VAR}`, `${VAR:-def}`, `${VAR-def}`, `${VAR:?err}`, `${VAR?err}`,
  `${VAR:+rep}`, `${VAR+rep}`, nesting no argumento, e `$$` → `$` literal.
  Trabalha em bytes (UTF-8 multibyte passa intacto). Unset bare → vazio; `:?`/`?`
  unset → erro com mensagem.
- `parse_dotenv` — `.env` KEY=VALUE (comentários/aspas/`export` tratados).
- `load_compose`: lê o `.env` do dir do projecto, env do processo OVERRIDE o
  `.env` (precedência do docker compose), interpola cada ficheiro antes do parse.

Prova:
- **Unit** (compose 92 verdes): `interpolation_forms_and_escape` (todas as formas +
  `$$`), `interpolation_required_unset_is_an_error`, `dotenv_parsing_strips_quotes_and_comments`.
- **Live** (`compose config`):
  - `.env` com `TAG=3.20` → `image: alpine:3.20`;
  - `${PORT:-9000}` (PORT ausente) → `ports: ["9000:80"]`;
  - `TAG=9.9` no shell → `image: alpine:9.9` (env vence o `.env`).
- 89→92 testes compose sem regressão; clippy `-D warnings` limpo.

Nota (correcção de alvo da área 6): NÃO existe suite de conformância Compose; o
alvo é validar contra `compose-spec.json` + paridade `compose-go`. Interpolação
era a peça normativa em falta — agora presente. Restam GAP-0005 (up restart, já
corrigido antes) e a cauda longa de atributos (GAP-0011).

Resultado CNCF: **área 6 (Compose) de ◑ → ◑ forte** — superfície comum +
interpolação normativa + allowlist fail-closed.
