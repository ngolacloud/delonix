#!/usr/bin/env python3
"""Gate de língua: o código deste repositório escreve-se em inglês.

Conta quatro dívidas — identificadores, comentários, mensagens ao utilizador e
documentação `.md` ainda em português — e compara com `scripts/lang_baseline.json`.

O gate é um RATCHET, não um tecto: falha se o número SUBIR (entrou português
novo) e falha se DESCER sem a linha de base ter sido baixada no mesmo commit.
Um `<=` deixaria a dívida a ler-se como verde para sempre.

    python3 scripts/lang_ratchet.py             # verifica (exit 1 se desalinhado)
    python3 scripts/lang_ratchet.py --list      # mostra o que falta traduzir
    python3 scripts/lang_ratchet.py --update    # rebaixa a linha de base
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEXICON = Path(__file__).resolve().parent / "lang_pt_lexicon.txt"
BASELINE = Path(__file__).resolve().parent / "lang_baseline.json"

SKIP_DIRS = {".git", "target", "node_modules", ".claude", "vendor", "dist", "build",
             ".ansible", ".venv", "site-packages", "third_party"}

# O gate não se conta a si próprio: o léxico É português por definição, e a
# documentação do script está em PT como o resto das notas da casa.
SKIP_FILES = {"scripts/lang_ratchet.py", "scripts/lang_pt_lexicon.txt"}

# The contributor handbook site is GENERATED (scripts/dev_docs_site.py) and has a
# pt-AO translation on purpose: its search index is Portuguese because the page is.
# LANG-01 is about the code; counting a translation as debt would make the gate
# fail for doing exactly what the handbook is for.
SKIP_PREFIXES = ("docs/handbook/",)

# Same argument, other generated artefact: `docs/providers/capability-matrix.md`
# is produced by `delonix provider matrix` and a test keeps the file equal to that
# output, so it cannot be edited by hand — the language it carries is the language
# of the SOURCE strings (the `Evidence` of each `Capability`, which cite the
# titles of `scripts/e2e.sh` checks). Counting the artefact asks for the one edit
# the repo forbids. Measured 2026-11-22: 17 of its lines counted, 14 of them
# already inside the baseline, so the gate was reporting the battery's Portuguese
# in the wrong place for as long as the matrix has existed. Translating it belongs
# to the English-source campaign, at the source, together with the battery.
#
# `docs/RELEASES.md` is the same case: `scripts/gen-releases.sh` concatenates
# `docs/releases/v*.md` under a fixed header and the release workflow recommits it
# on every tag ("Do not edit by hand"). Every line it carries is ALREADY counted at
# its source note, so counting the appendix counted each one twice. Measured
# 2026-11-23: its 8 remaining counted lines were all duplicates of
# `docs/releases/v*.md` lines; the regeneration in 5d0d5947 ([skip ci]) added the
# two `erro[DX-4201]` rows of v5.1.1 (literal pt-locale output, already counted in
# v5.1.1.md) and turned the gate red on a commit CI never ran.
GENERATED_MD = ("docs/providers/capability-matrix.md", "docs/RELEASES.md")

# Documentação `.md`: a FONTE escreve-se em inglês, mas há locales traduzidos de
# propósito ao lado da fonte (docs/dev/<página>.md ↔ docs/dev/pt-AO/<página>.md,
# ADRs `*.pt-AO.md`). Contá-los como dívida faria o gate exigir que se
# "traduzisse" o que já é, por desenho, a tradução. Estes ficam de fora; o resto
# do `.md` (incluindo reports/ e docs/ sem sufixo de locale) conta.
# `docs/releases/archive/` is a FROZEN historical changelog (v0.md): the release
# notes of versions long shipped. It is a record of what was written at the time,
# not living source we maintain, so it is excluded like the handbook — translating
# frozen history would churn an immutable archive for no reader's benefit.
MD_SKIP_PREFIXES = ("docs/dev/pt-AO/", "docs/dev/zh-CN/", "docs/dev/fr-FR/",
                    "docs/releases/archive/")
MD_SKIP_SUFFIX = ".pt-AO.md"

# Declarações que introduzem um nome, por linguagem.
DECL = {
    ".rs": re.compile(
        r"\b(?:fn|struct|enum|trait|const|static|type|mod|union)\s+([A-Za-z_][A-Za-z0-9_]*)"
        r"|\blet\s+(?:mut\s+)?([a-z_][a-z0-9_]*)"
    ),
    ".ts": re.compile(
        r"\b(?:function|class|interface|enum|type|const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)"
    ),
    ".go": re.compile(r"\b(?:func|type|var|const)\s+([A-Za-z_][A-Za-z0-9_]*)"),
    ".py": re.compile(r"\b(?:def|class)\s+([A-Za-z_][A-Za-z0-9_]*)"),
}
DECL[".tsx"] = DECL[".ts"]
DECL[".js"] = DECL[".ts"]

COMMENT = {
    ".rs": re.compile(r"^\s*(?://|///|//!)\s*(.+)$"),
    ".ts": re.compile(r"^\s*(?://|\*|/\*)\s*(.+)$"),
    ".go": re.compile(r"^\s*//\s*(.+)$"),
    ".py": re.compile(r"^\s*#\s*(.+)$"),
    ".yml": re.compile(r"^\s*#\s*(.+)$"),
}
COMMENT[".tsx"] = COMMENT[".ts"]
COMMENT[".js"] = COMMENT[".ts"]
COMMENT[".yaml"] = COMMENT[".yml"]

# Texto que o operador lê. Em Rust vem por macro; em YAML pelo `name:` do Ansible.
USER_TEXT = {
    ".rs": re.compile(
        r"(?:anyhow!|bail!|panic!|eprintln!|println!|format!|expect|unimplemented!)"
        r"\s*\(\s*\"([^\"]{8,})\""
    ),
    ".yml": re.compile(r"^\s*-?\s*name:\s*[\"']?([^\"'\n]{8,})"),
}
USER_TEXT[".yaml"] = USER_TEXT[".yml"]

WORD = re.compile(r"[A-Za-z]+")


def strip_accents(s: str) -> str:
    return "".join(c for c in unicodedata.normalize("NFD", s) if not unicodedata.combining(c))


def load_lexicon() -> set[str]:
    words = set()
    for line in LEXICON.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line and not line.startswith("#"):
            words.add(strip_accents(line).lower())
    return words


def segments(identifier: str) -> list[str]:
    """snake_case e camelCase para a mesma lista de segmentos minúsculos."""
    spaced = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", identifier)
    return [s for s in spaced.lower().split("_") if s]


def is_pt_identifier(name: str, pt: set[str]) -> bool:
    return any(s in pt for s in segments(name))


def is_pt_text(text: str, pt: set[str]) -> bool:
    return any(strip_accents(w).lower() in pt for w in WORD.findall(text))


def walk(root: Path):
    # NB: filtrar por partes RELATIVAS — o caminho absoluto pode conter um
    # `.claude/worktrees/...` e engolia a árvore inteira em silêncio.
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(root)
        if any(part in SKIP_DIRS for part in rel.parts):
            continue
        if rel.as_posix() in SKIP_FILES or rel.as_posix().startswith(SKIP_PREFIXES):
            continue
        if path.suffix == ".md":
            relp = rel.as_posix()
            if relp.startswith(MD_SKIP_PREFIXES) or relp.endswith(MD_SKIP_SUFFIX):
                continue
            yield path
        elif path.suffix in DECL or path.suffix in COMMENT:
            yield path


def scan(pt: set[str]):
    hits = {"identifiers": [], "comments": [], "user_text": [], "docs": []}
    for path in walk(ROOT):
        ext = path.suffix
        try:
            lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            continue
        rel = path.relative_to(ROOT).as_posix()
        if ext == ".md" and rel in GENERATED_MD:
            continue
        if ext == ".md":
            # Prosa: uma linha conta se tiver pelo menos uma palavra do léxico PT.
            # Saltar blocos de código (``` … ```): comandos e output não são prosa
            # e dariam falsos positivos com nomes de flags/saídas.
            in_fence = False
            for n, line in enumerate(lines, 1):
                s = line.strip()
                if s.startswith("```"):
                    in_fence = not in_fence
                    continue
                if in_fence or not s:
                    continue
                if is_pt_text(s, pt):
                    hits["docs"].append((rel, n, s[:70]))
            continue
        for n, line in enumerate(lines, 1):
            if ext in DECL:
                for m in DECL[ext].finditer(line):
                    name = next((g for g in m.groups() if g), None)
                    if name and is_pt_identifier(name, pt):
                        hits["identifiers"].append((rel, n, name))
            if ext in COMMENT:
                m = COMMENT[ext].match(line)
                if m and is_pt_text(m.group(1), pt):
                    hits["comments"].append((rel, n, m.group(1)[:70]))
            if ext in USER_TEXT:
                for m in USER_TEXT[ext].finditer(line):
                    if is_pt_text(m.group(1), pt):
                        hits["user_text"].append((rel, n, m.group(1)[:70]))
    return hits


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--list", action="store_true", help="mostra cada ocorrência")
    ap.add_argument("--update", action="store_true", help="reescreve a linha de base")
    ap.add_argument("--only", choices=sorted(("identifiers", "comments", "user_text", "docs")))
    args = ap.parse_args()

    pt = load_lexicon()
    hits = scan(pt)
    counts = {k: len(v) for k, v in hits.items()}

    if args.list:
        for kind, rows in hits.items():
            if args.only and kind != args.only:
                continue
            print(f"\n=== {kind} ({len(rows)}) ===")
            for rel, n, what in rows:
                print(f"{rel}:{n}: {what}")
        return 0

    if args.update:
        BASELINE.write_text(json.dumps(counts, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"linha de base reescrita: {counts}")
        return 0

    if not BASELINE.exists():
        print("sem linha de base — corre `python3 scripts/lang_ratchet.py --update`", file=sys.stderr)
        return 1

    base = json.loads(BASELINE.read_text(encoding="utf-8"))
    failed = False
    for kind in sorted(counts):
        actual, expected = counts[kind], base.get(kind)
        if expected is None:
            print(f"FALHA {kind}: categoria nova sem linha de base ({actual})", file=sys.stderr)
            failed = True
        elif actual > expected:
            print(
                f"FALHA {kind}: {actual} > {expected} — entrou português novo.\n"
                f"       `python3 scripts/lang_ratchet.py --list --only {kind}` mostra onde.",
                file=sys.stderr,
            )
            failed = True
        elif actual < expected:
            print(
                f"FALHA {kind}: {actual} < {expected} — traduziste, mas não baixaste a\n"
                f"       linha de base. Corre `python3 scripts/lang_ratchet.py --update`\n"
                f"       e comita `scripts/lang_baseline.json` no MESMO commit.",
                file=sys.stderr,
            )
            failed = True
        else:
            print(f"ok    {kind}: {actual}")

    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
