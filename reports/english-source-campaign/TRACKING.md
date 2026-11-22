# English-source campaign — tracking

**Goal.** Make all distributed code and documentation of delonix-runtime readable
by the international community, with a gate that stops Portuguese from creeping
back into the source. The repo's language policy already states that the source
is English (see the `pt.po` header); this campaign pays down the Portuguese debt
that leaked in, and extends the existing ratchet to also guard `.md`.

**What is NOT translated (deliberate localization — leave as is):**
- `bins/delonix/data/pt.po` — gettext Portuguese (pt_AO) catalogue; the UI/help
  localization layer. English is the `msgid` (source); PT is the translation.
- `docs/dev/pt-AO/`, `docs/dev/zh-CN/`, `docs/dev/fr-FR/` — localized docs; the
  English source lives next to them in `docs/dev/`.
- `docs/adr/*.pt-AO.md` — localized ADRs.
- `docs/handbook/` — generated site with an on-purpose pt-AO translation.
- `.claude/**` — gitignored session/harness files, never distributed.

## Gate

`scripts/lang_ratchet.py` is a **ratchet** against `scripts/lang_baseline.json`:
it fails if any category's Portuguese count goes **up** (new PT entered) or
**down** without the baseline being lowered in the same commit. This campaign
extended it from 3 code categories to 4, adding `docs` for `.md` prose (fenced
code excluded; locales above excluded).

Baseline on `develop` when the campaign started (@c289bf4f):

| Category | Count | Meaning |
|---|---|---|
| `identifiers` | 1011 | PT segments in code identifiers (`.rs/.ts/.go/.py`) |
| `comments` | 2979 | PT code comments |
| `user_text` | 113 | PT operator-facing strings (`anyhow!`, Ansible `name:` …) |
| `docs` | 34331 | PT prose lines in `.md` (new category) |

Commands:
```
python3 scripts/lang_ratchet.py                  # check (exit 1 if misaligned)
python3 scripts/lang_ratchet.py --list --only docs   # show what remains
python3 scripts/lang_ratchet.py --update         # lower the baseline (same commit)
```

## Phases

| Phase | Target | Volume | Status |
|---|---|---|---|
| 0 — Foundation | extend ratchet to `.md` + baseline + this tracking | infra | **done** |
| 1 — Root | `ARCHITECTURE/CHANGELOG/CNCF-SANDBOX-READINESS/MIGRATION/VERSIONING` | 562 doc lines | **in progress** |
| 2 — `AGENTS.md` | the 10 473-line agent handbook | 8374 doc lines | todo |
| 3 — `docs/**` | guides, ADRs, discovery, releases | ~11k doc lines | todo |
| 4 — `reports/**` | historical campaign reports | ~4.3k doc lines | todo |
| 5 — code | `user_text` → `comments` → `identifiers` | ~4100 occ. | todo (needs multi-agent orchestration) |

Each phase is one or more PRs off `develop`. Lower the `docs` (or code) baseline
in the same commit that translates, so the ratchet stays green.

## Notes

- Worktree `.wt/english-source`, branch `chore/english-source-ratchet` off
  `develop`.
- Phase 5 (code, ~4100 occurrences) is the bulk of the work and should be driven
  with multi-agent orchestration, crate by crate, with build + tests green and
  the baseline lowered per batch.
