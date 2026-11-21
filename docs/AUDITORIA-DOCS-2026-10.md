# Delonix documentation audit — 2026-10-28

Verifiable foundation for consolidating the documentation site (`github.com/ngolacloud/delonix`).
Every line was confirmed against the code, the real `--help`, or the schemas — **not** assumed
from the documentation itself. Branch: `docs/auditoria-consolidacao`.

## Confirmed canonical facts

| Fact | Value | Evidence |
|---|---|---|
| Official remote | `github.com/ngolacloud/delonix` | `git remote -v` |
| Published Latest release | **`v0.1.0`** (2026-10-28T11:16:35Z) | `gh release list`; `releases/v0.1.0.md:19,49` |
| Historical tags | `v4.1.1`…`v5.1.1` (preserve) | `git tag --list` |
| Pages base URL | `ngolacloud.github.io/delonix` (no CNAME/own domain) | `releases/v0.1.0.md:19,49`; `RELEASES.md:3806,3836`; absence of a `CNAME` |
| Official image | `ghcr.io/ngolacloud/delonix` | `releases/v0.1.0.md` |
| Previous product name | `delonix-runtime` → `delonix` (the actual crate `bins/delonix-runtime-bin` keeps the name) | code on disk |

## Documentation architecture (two sites, two generators)

- **Main site** — `docs/gen.py` (6421 lines): **91 pages** (15 top-level + 76 under `commands/`).
  Its source of truth is the **binary** `target/release/delonix` (`gen.py:23`): embeds real `--help` via
  `help_of()` (`gen.py:128`); PT/EN i18n via CSS classes `lang-pt`/`lang-en` in the same file (`bi()`, `gen.py:166`);
  themes via `data-theme` + `prefers-color-scheme` (`gen.py:3228-3239`).
- **Handbook** — `scripts/dev_docs_site.py`: **105 pages** (`handbook/index.html` + 26 × 4 languages: `en`, `pt-AO`, `fr-FR`, `zh-CN`) built from `docs/dev/**/*.md`.
- **Publishing**: GitHub Pages serves the branch's `docs/` folder ("branch → /docs" mode) with `docs/.nojekyll`.
  **There is no deploy workflow**; the `.html` files are committed directly. `ci.yml` **validates** (runs `gen.py` + `dev_docs_site.py --check` and fails if `git diff` on `docs/` shows a difference).
- **Editing rule**: `gitops.html`, `estabilidade.html`, `labs.html`, `kinds.html`, etc. are **generated**.
  Fix the **source** (`gen.py` or the `.md` files) and regenerate — never edit the generated HTML.

---

## Matrix convention

Columns: **Page/component | Problem | Evidence | Change | Priority | Validation**.
Priority: 🔴 High · 🟠 Medium · 🟢 Low.

---

## A. Generator and cross-cutting components

| Page/component | Problem | Evidence | Change | Prio | Validation |
|---|---|---|---|---|---|
| `gen.py` / binary | `target/release/delonix` binary is **stale** (5.1.1, no `bucket`); regenerating now DISCARDS `bucket.html` and new commands | `gen.py:23`; the binary rejects `delonix bucket` despite `main.rs:295`; committed docs already have `bucket.html` (v0.1.0) | Rebuild mandatory before `gen.py`; document in the docs README; ideally a gate cross-checking the binary's `COMMAND MAP` vs `GROUPS` | 🔴 | `make build` + `python3 docs/gen.py` without aborting; `git diff docs/` empty |
| Ctrl+K search | **Absent** across the whole site (0 keyboard handlers, 0 index, 0 modal) | `gen.py:3461-3597` (SITE_JS only has highlighter/ToC/toggles); grep `ctrlKey`/`metaKey`=0 | Generate a static JSON index (titles+h2/h3+taglines) + client-side vendored Ctrl/Cmd+K modal | 🔴 | Ctrl+K opens the modal; Esc closes it; keyboard navigation; results show title+section+excerpt |
| Copy button | **Absent** on every `<pre><code>` | `gen.py:3669-3671,3693,3707`; grep `clipboard`=0 | Button on command `pre` blocks (not on `.out`/`.help`), bilingual `aria-label`, `execCommand` fallback | 🟠 | Copies in dark/light; signals success/failure; does not copy output or `--help` |
| Markdown pages (EN) | `md_page()` is not bilingual: `gitops.html`, `estrutura.html`, `estabilidade.html`, `guia-vm.html` show **body in PT only** even in EN mode | `gen.py:4598`, `6404-6410` | Translate the body or explicitly mark as PT-only; bilingual `md_page` long-term | 🔴 | Switch to EN and confirm the body is translated |
| `--help` blocks (i18n) | clap's `--help` is EN-only; in PT mode the `<div class='help'>` blocks stay in English | `gen.py:3693,3707` | Generate with `--l18n=pt` or label "real binary output (EN)" | 🟠 | PT mode: block labeled or translated |
| Missing EN taglines | `backup`, `config`, `namespace`, `oci-runtime` are in `GROUPS` but not in `GROUPS_EN` → tagline/intro PT-only | `gen.py:1740`, fallback `3680-3685` | Add entries to `GROUPS_EN` | 🟠 | Groups show an EN intro |
| Fixed `<html lang>` | Fixed at `"pt"`; the toggle only changes `data-lang` → screen readers announce EN content as PT | `gen.py:3612` | Update `lang` on toggle | 🟠 | Inspect `lang` after switching |
| **Light** theme — code tokens | `--code-bg:#0f172a` (dark) with dark `--tok-*` tokens (GitHub-light): blue/green/brown on near-black, low contrast | `gen.py:3229-3231` | Use the bright token palette in light theme (the code background is always dark) | 🔴 | Ratio ≥4.5:1 in code blocks in light theme |
| `muted` output | `.out pre` body in `--muted`, at the edge of readability | `gen.py:3304-3305` | Raise output text contrast | 🟠 | Ratio ≥4.5:1 |
| `tag`/`callout` outside tokens | `.tag.ok/mid/no`, `.callout.warn` with fixed hex + manual dark overrides | `gen.py:3324-3332` | Migrate to theme tokens | 🟢 | Switch theme and inspect |
| pt.po vs site dictionaries | the site's i18n is hard-coded (`GROUPS_EN`/`CATEGORY_PT`), independent of the binary's `pt.po` → drift risk | `gen.py:56-59,1740` | Document the separation; optionally derive from the binary | 🟢 | — |
| Mobile navigation | No menu button; below 840px the entire sidebar (incl. CLI Reference) stacks above the content | `gen.py:3340-3342` | Collapsible/hamburger nav on mobile | 🟢 | Test below 840px |

## B. Repository and release references

| Page/component | Problem | Evidence | Change | Prio | Validation |
|---|---|---|---|---|---|
| GitOps `$schema` | Pages URL with old org+name: `angolardevops.github.io/delonix-runtime/schema/v1/delonix.json` (does not resolve) | `gitops.md:225` → `gitops.html:376` | `ngolacloud.github.io/delonix/schema/v1/delonix.json` | 🔴 | `curl` to the URL returns 200 |
| Stability `$schema` | Same for `delonix.json` and `providers.json` | `cli-stability.md:368,448` → `estabilidade.html:548,616` | `ngolacloud.github.io/delonix/schema/v1/...` | 🔴 | `curl` 200 |
| Installer in the roadmap | `curl … angolardevops.github.io/delonix-runtime/install.sh` (the site already uses `releases/latest/download/install.sh`) | `ROTEIRO-VIDEOS.md:48` | Align with the canonical one-liner (`gen.py:171` already correct) | 🔴 | Compare with `gen.py:171` |
| Golden VM images | ✅ **VERIFIED (Section 2)** — golden images really do live on `angolardevops`, by design. ghcr: `angolardevops/delonix-vm-{base,k8s,appliances}` → **200 (public)**; `ngolacloud/delonix` (control) → 403 (private). Distinct from the product image on `ngolacloud` | ghcr (anonymous token); `dev/microvm-setup.md` (`OFFICIAL_REPOS` in `cmd/vmimage.rs`) | Clarifying note in `dev/microvm-setup.md` (don't realign) | 🟠 | ghcr `angolardevops/*` 200 ✅ |
| Lab provenance | 24 blocks "Validated on delonix **5.1.1** … 2026-10-24" | `gen.py:4937,4973,5016,5050,5087,5132,5261,5297,5339,5373,5410,5457`; `labs.html` 187…605; `labs/lab-*/README.md` | Re-run the labs and relabel as **0.1.0**, or mark as a historical baseline | 🟠 | Labs pass on 0.1.0 |
| Measured conformance | ✅ **DONE** — CRI now cites a real recent measurement **critest v1.36.0, 84/20 (2026-10-24)** (`reports/cri-conformance-2026-10-24/`), beating the historical 79/103; OCI is already dated (v5.1.1 @ ee615f3b), left intact; networking marked as historical 5.1.1 baseline (2026-10-15, to be re-measured on 0.1.0) | `cri-conformance.md`; `gen.py` (cri); `dev/plano-conformidade-oci-cri.md`; `testing/folhas-inventario.md` | Real measurement cited / per-axis baseline | 🟠 | critest 84/20 dated ✅; networking/CNI needs a 0.1.0 benchmark |
| Anomalous version | ✅ **DONE** — `Delonix 0.53.0` already fixed in an earlier Section 2 commit (does not reappear in `measured-comparison.md`) | `measured-comparison.md` | — | 🟠 | grep `0.53.0` = 0 ✅ |
| Contract boundary | ✅ **DONE (Section 2)** — 4 notes reframed to "on the 0.1.0 line" (5.1.x named as historical baseline in the first one) | `cli-stability.md:75,131,140,150` → `estabilidade.html` | Reframed to 0.1.0 | 🟠 | — |
| Brand residue | ✅ **DONE** — 345× "Delonix Runtime"→"Delonix" (generator `dev_docs_site.py` + 24 in `docs/dev/**/*.md`, 4 languages); the `delonix-runtime(-bin)` crate/binary name preserved; handbook regenerated | `dev_docs_site.py`; `docs/dev/**/*.md` | Replaced with "Delonix" | 🟢 | grep "Delonix Runtime" in the handbook = **0** ✅ |
| VS Code extension | ✅ **VERIFIED (Section 2)** — lives at `angolardevops/delonix-vscode` (**public**); `ngolacloud/delonix-vscode` → **404**. Reference correct, no change | GitHub API | — (confirmed) | 🟢 | `angolardevops` public ✅ |
| Duplicate title | `<title>Delonix Engine · Delonix Engine</title>` | `index.html:6` | Fix the template in `gen.py` | 🟢 | Check `<title>` |
| Missing metadata | No `<base>`, `canonical`, `og:`/`twitter:` across the whole site | `index.html` only has `viewport`+`title` | Add meta (base `ngolacloud.github.io/delonix`) | 🟢 | Check `<head>` |
| ghcr placeholders | `ghcr.io/angolardevops/{app,...}` as an example user | `gen.py:523,525`; `kinds.html:2125`; `commands/{push,pull,image}.html` | Clarify as a placeholder | 🟢 | — |
| History (don't touch) | `RELEASES.md`, `releases/v5*/v4*`, `releases/archive/v0.md`, `discovery/*` | — | Preserve as historical record | — | — |

## C. Structural integrity

| Page/component | Problem | Evidence | Change | Prio | Validation |
|---|---|---|---|---|---|
| Orphan pages | **None** — 196/196 reachable from `index.html`; 0 nav entries for a nonexistent file | BFS in `scratchpad/audit.py` | — (no action) | — | Re-run the BFS |
| `guia-vm.html` — index | 10 index anchors keep accents but the headings' `id`s are ASCII (slugify mismatch) | `#1-antes-de-começar` vs `id="1-antes-de-comecar"` (and +9 more) | Single shared slugify in `gen.py` | 🟠 | Click all 10 entries and confirm scrolling |
| Handbook — `environment-variables` | 12 cross-anchors in EN but translated `id`s (FR/PT-AO/ZH-CN) | `handbook/{fr-FR,pt-AO,zh-CN}/{index,start-here,environment-variables}.html` | Rewrite anchors when translating headings | 🟠 | Click the cross-links |
| `commands/image.html` | ✅ **ALREADY RESOLVED** — `vm.html` emits an `id` per subcommand (`gen.py:3700`, `<h2 id='{sub}'>`); `vm.html#image` exists and resolves | `commands/vm.html:530` (`id='image'`) | — (none; it was drift from the stale binary) | 🟢 | `vm.html#image` jumps to `vm image` ✅ |
| Handbook zh-CN | ✅ **DONE (Section 3)** — zh-CN added to the redirect and to the landing page's link list `handbook/index.html` | `dev_docs_site.py:817,820` | zh-CN on the landing page (`/zh/` redirect + 简体中文 link) | 🟢 | Landing lists zh-CN ✅ |
| Unlinked labs | ✅ **DONE (Section 3)** — `labs_page()` links each lab (lab-1..6) to its `README.md` + `lab.sh`; lab-7/8 stay unlinked (no harness on disk yet, guarded by `os.path.isdir`) | `gen.py` `labs_page()`; `labs.html` | "Materials" block per lab with real materials | 🟢 | Links from `labs.html` ✅ |
| Stale translations | ✅ **MOSTLY DONE** — `--status` dropped **46→5**: zh-CN complete; pt-AO/fr-FR all but `environment-variables` (large/partial, reverted to EN fallback) + `engineering-process.md` created in all 3. Honestly still stale: 2× `environment-variables`, 3× `plano-conformidade` (one conformance edit behind) | `dev_docs_site.py --status`; translations in `docs/dev/{pt-AO,fr-FR,zh-CN}/` | Faithful translations (code verbatim, hash re-synced) | 🟠 | 5 `--status` remaining (was 46) |

## D. Documentation ↔ code correspondence

| Page/component | Problem | Evidence | Change | Prio | Validation |
|---|---|---|---|---|---|
| `kinds.html` — Storage/ShareVolume | ✅ **DONE (Section 10)** — text said they were downgraded/rewritten into `Volume`; fixed to say **REMOVED**. Re-measured on the 0.1.0 binary: `Storage`/`ShareVolume`/`Egress` give **DX-1000 "was removed"** (not the old `removed_kind_hint`). The capability lives in `Volume` blocks (`nfs:`/`cifs:`/`webdav:`/`share:`) and `NetworkPolicy` (`direction: egress`) | tagline + `Volume` intro in `gen.py`; `stack validate -f kind:Storage` → DX-1000 | Text corrected (removed, not rewritten) | 🔴 | `delonix stack validate` on `kind: Storage` errors ✅; doc consistent ✅ |
| `kinds.html` — EN tagline | ✅ **DONE (Section 10)** — EN used `Vm`/`FirewallPolicy`/`Tunnel`; aligned to PT (`VirtualMachine`/`NetworkPolicy`/`Gateway`) + link to `estrutura.html`. Order is a deliberate representative subset (24 `in_stack` kinds; the full list is delegated to `estrutura.html`) | `gen.py` `kinds_page()`; `api-resources` | Canonical names in the EN tagline | 🟠 | `api-resources`/`stack_kinds()` ✅ |
| `kinds.html` — FirewallPolicy | ✅ **DONE (Section 10)** — section retitled `NetworkPolicy`; `FirewallPolicy` referenced as an alias in the intro. Confirmed: the binary normalizes `kind: FirewallPolicy`→`NetworkPolicy` | `gen.py` label; `kinds.rs:82`; `api-resources` (`np NetworkPolicy`) | Canonical title, alias noted | 🟠 | `api-resources` shows `NetworkPolicy` ✅ |
| `kinds.html` — NetworkZone | ✅ **DONE (Section 10)** — a *sunset* note added to the `Network` intro (still accepted on load, downgraded to `Network`). Confirmed: `api-resources` → `sunset → Network` | `gen.py` `Network` intro; `api-resources` | Note (it is sunset) | 🟢 | — |
| `kinds.html` — Application/VolumeClaim | Cited in the request but **don't exist** in either the docs or the code (a clean absence) | no `<h2>`; no `FACTS` entry | None (confirm they are not promised) | — | — |
| CLI — coverage | No documented command missing from the **source code**; `ingress-proxy` is hidden (correct) | enum `Cmd` `main.rs`; `main.rs:648` (`hide`) | — | — | — |
| CLI — drift | Risk materialized by the stale binary (see line A) | `gen.py:24` | (covered in A) | 🔴 | — |
| Environment variables — reference | **Clean and CI-enforced**: ~150 `DELONIX_*` from the code are all documented; none undocumented | `docs/dev/environment-variables.md`; `scripts/dev_docs.py::env_var_problems` | — (no action) | — | `dev_docs.py --check` green |
| Env vars in ADR/discovery | `DELONIX_OPENSTACK_AUTH_URL/SECRET` and `DELONIX_NET_PROVIDER` describe **unimplemented** features | `adr/0039:77-78`; `discovery/62_...:70` | Mark as design/not implemented | 🟠 | grep in the code = 0 |

---

## Priority summary

**🔴 High (promises what doesn't exist, or breaks a core function):**
1. ✅ `kinds.html` says Storage/ShareVolume still load — the code rejects them. *(D)* **(DONE — Section 10)**
2. ✅ Stale binary → regeneration discards `bucket` and new commands. *(A)* **(resolved: 0.1.0 binary; +`contract_gate` gate in Section 15)**
3. 3× `$schema` + installer point to `angolardevops.github.io/delonix-runtime` (doesn't resolve). *(B)* **(Section 2)**
4. ✅ Markdown pages show PT-only body in EN mode (4 guides). *(A)* **(DONE — Section 9: labeled "Portuguese only")**
5. ✅ Code token contrast in light theme. *(A)* **(DONE — Section 6; ratios ≥4.5:1)**
6. ✅ Missing Ctrl+K search (92 pages with no search). *(A)* **(DONE — Section 7)**

**🟠 Medium:** ✅ `angolardevops` golden images (verified public, by design — Sec 2); 5.1.1→0.1.0 provenance (labs ✅ relabeled Sec 11; conformance to be re-measured); ✅ `Delonix 0.53.0` (fixed); ✅ contract boundary reframed to 0.1.0 (Sec 2); ✅ copy button (Sec 8); ✅ EN-only `--help` labeled (Sec 9); ✅ EN taglines (Sec 9); ✅ `<html lang>` (Sec 9); ✅ guia-vm anchors (Sec 9; translated handbook deferred); ✅ handbook translations (46→5, Sec 9); ✅ FirewallPolicy/Tunnel tagline (Sec 10); ✅ design-only env vars (Sec 12).

**🟢 Low:** ✅ 345× "Delonix Runtime"→"Delonix" in the handbook (done); ✅ duplicate title (Sec 16); ✅ metadata (Sec 16); ✅ zh-CN on landing (Sec 3); ✅ linked labs (Sec 3); ✅ mobile nav (Sec 5); ✅ colors outside tokens (Sec 6).

## Mapping to the backlog (next fronts, one session each)

| Request section | Matrix entries feeding it |
|---|---|
| 2 — Repo/release references | ✅ **DONE** — 🔴 (schemas/installer→`ngolacloud`) + 🟠 (`angolardevops` golden verified public; `0.53.0`; boundary→0.1.0; VS Code confirmed). Only re-measuring conformance 5.1.1→0.1.0 remains |
| 3 — Navigation/redirects | ✅ **DONE** — **C** (zh-CN landing, unlinked labs); `image→vm#image` already resolved |
| 4 — Introduction/onboarding | ✅ **DONE** — `index` rewritten (4 paths, commands verified on the binary); links to the glossary |
| 5 — Layout/width | ✅ **DONE** — collapsible hamburger nav below 840px (**A**) |
| 6 — Themes/contrast | ✅ **DONE** — bright code tokens in light theme, `--out-ink`, tags/callouts via variables; ratios ≥4.5:1 (**A**) |
| 7 — Ctrl+K search | ✅ **DONE** — vendored Ctrl/Cmd+K modal + `search-index.json` (773 entries); anchors reconciled with the generator's ids (**A**) |
| 8 — Copy button | ✅ **DONE** — button only on command `<pre>`s, bilingual, `execCommand` fallback (**A**) |
| 9 — PT/EN translations | ✅ **DONE (almost)** — EN taglines, `<html lang>`, PT-only md labeled, EN `--help` labeled, guia-vm slugify (10→0); **+ contributor handbook translated: `--status` 46→5** (zh-CN complete, pt-AO/fr-FR nearly). 5 honestly stale items remain (2 `environment-variables` + 3 `plano-conformidade`) |
| 10 — Kinds | ✅ **DONE** — Storage/ShareVolume, EN tagline, FirewallPolicy→NetworkPolicy, NetworkZone sunset (**D**) |
| 11 — Labs | ✅ **DONE** — labs linked (Sec 3) + provenance relabeled as historical 5.1.x baseline (not re-measured on 0.1.0) |
| 12 — Variables | ✅ **DONE** — env vars from `adr/0039`+`discovery/62` marked as design (0 readers in the code, confirmed) |
| 13 — Glossary | ✅ **DONE** — new page `glossario.html` (22 terms, 5 groups), in the nav under "Concepts" |
| 14 — Architecture | ✅ **DONE** — `arquitectura.html` rebuilt with the ~10 missing crates; `c4.html` was already correct (both generated) |
| 15 — Testable examples | ✅ **DONE** — `contract_gate()` (binary↔GROUPS, bidirectional) + `scripts/docs_command_gate.py`; 26 examples confirmed |
| 16 — Visual/publishing validation | ✅ **DONE** — duplicate title fixed, `canonical`/`og:`/`twitter:` + `SITE_BASE`; Pages base URL validated (no `<base>`, by decision) |

## Counts

- **Pages**: 196 HTML (91 `gen.py` + 105 handbook). **Orphans: 0.**
- **Broken file links: 0.** **Broken anchors: 23** (10 `guia-vm`, 12 translated handbook, 1 `image→vm`).
- **Actionable repo/release references**: ~5 Pages/installer URLs (🔴), ~10 ghcr images (🟠), 24 lab provenance blocks (🟠), ~12 conformance ones (🟠), 1 anomalous version (🟠).
- **Kinds**: 26 documented; 2 (`Storage`/`ShareVolume`) promised but rejected; 1 (`NetworkZone`) accepted with no section.
- **Variables**: ~150 `DELONIX_*` — complete, CI-enforced reference; 3 only in design docs.
