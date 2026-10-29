# F-1 — Envenenamento de cache de camadas no upgrade (whiteouts OCI)

- **Severidade:** P1 (integridade de imagem / caminho de upgrade). Bounded: só
  camadas desempacotadas por um binário *pré-correcção*; pulls novos já estão
  correctos; workaround = podar a cache de camadas.
- **Domínio:** §6 (integridade de dados), §7 (imagens, extracção segura),
  §12 (rollback de binário ≠ rollback de estado).
- **Tipo:** defeito do produto (reuso de cache sem validação de formato).
- **Candidato:** `v0.1.1`. Ambiente: `delonix 0.1.0` @ `8ab783dd`, rootless.
- **Estado:** CORRIGIDO + teste unitário; falta re-prova end-to-end ao vivo.

## O que se observou (forense, no host de teste)

O bug histórico de whiteouts OCI (`.wh.<name>` desempacotado como ficheiro
regular em vez de char device de overlay — a remoção nunca chega à camada
inferior) **já está corrigido em código**: o armazém de camadas tem o caso
exacto Apache MPM como char device `0:0`:

```
layers/c4d2115b…/etc/apache2/mods-enabled/mpm_event.conf   # char dev 0:0  (OK)
layers/c4d2115b…/etc/apache2/mods-enabled/mpm_event.load   # char dev 0:0  (OK)
```

**Mas** persistem camadas com `.wh.*` vazados como ficheiros regulares:

```
layers/ba1ebfcb…/etc/rabbitmq/conf.d/.wh.20-management_agent.disable_metrics_collector.conf
layers/25619be2…/tmp/.wh.pgvector
layers/42109c5c…/var/lib/apt/lists/.wh.auxfiles
layers/fa0f0019…/home/.wh.ubuntu
```

Cronologia das mtimes das camadas confirma a causa: as vazadas foram extraídas
em **2026-10-21 19:52** e **2026-10-22 17:44**; as corrigidas (char device) em
**2026-10-22 18:04**. A correcção de whiteouts aterrou entre as 17:44 e as
18:04 de 2026-10-22.

## Causa-raiz

`ImageStore::ensure_layers` / `extract_one` (`crates/adapters/delonix-oci/src/
overlay.rs`) reutilizam uma camada em cache **apenas pela presença do marcador
`.extracted`** (conteúdo `b"ok"`), sem validar o formato de disco. Como as
camadas são endereçadas por digest e o blob/digest nunca mudam, uma extracção
feita por um binário buggy **sobrevive para sempre** — o upgrade do binário
corrige o *código* mas não a *cache*. Qualquer imagem cujas camadas foram
puxadas antes da correcção continua com o whiteout partido em runtime
(ex.: `rabbitmq` com o plugin de métricas que devia estar desactivado a voltar,
`wordpress`/Apache com dois MPM).

## Correcção

Introduzida uma **época de formato** no marcador `.extracted` (`LAYER_EPOCH`,
hoje `b"2"`). Uma camada só é reutilizada se o marcador registar a época
corrente; marcador ausente (nunca terminou) ou token antigo (`ok`, pré-correcção
de whiteouts) → **não é confiável** → a camada é re-extraída do blob imutável no
próximo uso. O upgrade do binário passa a **curar** a cache de forma lazy e
per-layer (custo único de I/O na primeira utilização de cada imagem; documentar
em notas de migração).

- `overlay.rs`: `const LAYER_EPOCH` + `fn layer_is_current(dir)`; ligado nos três
  pontos de reuso (`ensure_layers`, guarda de entrada e verificação de corrida de
  `extract_one`). `publish_layer_dir` já substitui um `dir` não-vazio, por isso a
  re-extracção sobre uma camada envenenada funciona sem alteração adicional.

## Teste

- `overlay::tests::a_layer_from_an_older_epoch_is_reextracted_not_reused` —
  uma camada com marcador `b"ok"` + um `.wh.poison` vazado + conteúdo stale é
  re-extraída: o ficheiro do blob aparece, o lixo plantado desaparece, o marcador
  passa a `LAYER_EPOCH`. Verde.
- Suite completa `delonix-oci --lib`: **192/192** verdes (nenhuma regressão nos
  testes existentes que afirmam o marcador).

## Pendente (para fechar a P1)

Re-prova end-to-end ao vivo: com o binário corrigido, `delonix container run`
sobre uma das imagens de camada envenenada (ex.: `rabbitmq`) e confirmar que o
whiteout passa a ser honrado após a re-extracção automática. Requer build do
binário completo + execução rootless; não feito nesta sessão.
