#!/usr/bin/env python3
"""Prompt lint gate — an active prompt has the required shape and no dangling references.

# What this closes

A campaign prompt tells a fresh session what to do and what «done» means. Without a
fixed shape prompts drift: no base SHA (so «it worked» is unverifiable), a dangling
`ADR-NNNN`, an authorized artefact that no longer exists, or a conclusion copied
from a months-old report that no longer holds. `.ai/prompt-template.md` is the
shape; this gate enforces it statically:

1. **Required fields present** — ID, Base/SHA, Objective, Authorized artefacts,
   Dependencies, Lab profile, Tests, Evidence validity, Expected output
   (Issue/ADR is optional — «when one exists»).
2. **References resolve** — a named `ADR-NNNN` maps to `docs/adr/NNNN-*.md`; every
   authorized artefact path exists; Base names a branch or a SHA. A dangling
   reference fails the lint.
3. **Staleness guard** — Evidence validity `historical` without an explicit
   «reconfirm against code and a fresh run» fails: an old report is a hypothesis
   until re-checked.

Pure over the prompt text plus the repo tree; no build, no git.

  python3 scripts/prompt_lint_gate.py lint <prompt-file>
  python3 scripts/prompt_lint_gate.py check-fixtures scripts/prompt_lint_fixtures
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

KEY_ALIASES = {
    "id": "id",
    "base": "base", "base/sha": "base",
    "objective": "objective", "objectivo": "objective",
    "issue/adr": "issue_adr", "issue": "issue_adr", "adr": "issue_adr",
    "authorized artefacts": "artefacts", "artefacts": "artefacts",
    "artefactos autorizados": "artefacts", "artefactos": "artefacts",
    "dependencies": "dependencies", "dependências": "dependencies", "dependencias": "dependencies",
    "lab profile": "lab", "lab": "lab", "perfil de laboratório": "lab", "perfil-lab": "lab",
    "tests": "tests", "testes": "tests",
    "evidence validity": "evidence", "evidence": "evidence",
    "validade da evidência": "evidence", "evidência": "evidence",
    "expected output": "output", "output": "output",
    "saída esperada": "output", "saida": "output",
}

REQUIRED = {"id", "base", "objective", "artefacts", "dependencies",
            "lab", "tests", "evidence", "output"}

_FIELD_RE = re.compile(r"^\s*[-*]\s*\*\*(.+?):\*\*\s*(.+?)\s*$")
_NONE = {"none", "n/a", "nenhuma", "nenhum", "-"}


def parse_fields(text: str) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in text.splitlines():
        m = _FIELD_RE.match(line)
        if not m:
            continue
        key = KEY_ALIASES.get(m.group(1).strip().lower())
        if key and key not in fields:
            fields[key] = m.group(2).strip()
    return fields


def _artefact_exists(token: str) -> bool:
    token = token.strip().strip("`").rstrip("/")
    if not token or token.lower() in _NONE:
        return True
    if "*" in token:
        return any(ROOT.glob(token))
    return (ROOT / token).exists()


def validate(fields: dict[str, str], root: Path = ROOT) -> list[str]:
    errors: list[str] = []

    for key in sorted(REQUIRED):
        if not fields.get(key):
            errors.append(f"missing required field: {key}")

    # Issue/ADR references resolve.
    for num in re.findall(r"ADR-(\d{3,4})", fields.get("issue_adr", "")):
        if not list(root.glob(f"docs/adr/{num}-*.md")):
            errors.append(f"dangling reference: ADR-{num} has no docs/adr/{num}-*.md")

    # Base names a branch or a SHA.
    base = fields.get("base", "")
    if base and not re.search(r"\b(develop|main)\b|[0-9a-f]{7,40}", base):
        errors.append(f"Base/SHA names neither a branch nor a commit: {base!r}")

    # Authorized artefacts exist.
    for token in fields.get("artefacts", "").split(","):
        if not _artefact_exists(token):
            errors.append(f"authorized artefact does not exist: {token.strip()}")

    # Staleness guard.
    evidence = fields.get("evidence", "").lower()
    if ("historical" in evidence or "histórico" in evidence or "historico" in evidence):
        if not re.search(r"reconfirm|confirmar|confirm against", evidence):
            errors.append("historical evidence without a reconfirmation instruction "
                          "(must say 'reconfirm against code and a fresh run')")

    return errors


def _cmd_lint(args: argparse.Namespace) -> int:
    text = Path(args.file).read_text(encoding="utf-8")
    errors = validate(parse_fields(text))
    if errors:
        print(f"FAIL — {args.file}:")
        for e in errors:
            print(f"  - {e}")
        return 1
    print(f"OK — {args.file} is a well-formed active prompt")
    return 0


def _cmd_check_fixtures(args: argparse.Namespace) -> int:
    root = Path(args.dir)
    files = sorted(root.glob("*.json"))
    if not files:
        print(f"::error::no fixtures in {root} — this gate stopped guarding anything")
        return 1
    failures = total = 0
    for fixture in files:
        data = json.loads(fixture.read_text(encoding="utf-8"))
        for case in data.get("cases", []):
            total += 1
            errors = validate(parse_fields(case["prompt"]))
            got = "FAIL" if errors else "OK"
            if got != case["expect"]:
                failures += 1
                print(f"[FAIL] {fixture.name}::{case['name']} expected {case['expect']}, "
                      f"got {got} ({'; '.join(errors) or 'no errors'})")
    if failures:
        print(f"{failures}/{total} fixture case(s) disagree with the prompt template")
        return 1
    print(f"all {total} fixture case(s) match the prompt template")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    lint = sub.add_parser("lint", help="lint one prompt file")
    lint.add_argument("file")
    lint.set_defaults(func=_cmd_lint)
    fx = sub.add_parser("check-fixtures", help="validate a fixtures directory")
    fx.add_argument("dir")
    fx.set_defaults(func=_cmd_check_fixtures)
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (OSError, json.JSONDecodeError, KeyError) as exc:
        print(f"::error::bad input: {exc}")
        return 2


if __name__ == "__main__":
    sys.exit(main())
