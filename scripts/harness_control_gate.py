#!/usr/bin/env python3
"""Harness control gate — loosening a gate's floor needs a reason and a reviewer.

# What this closes

The gates are the repo's memory of its invariants. A baseline that quietly
loosens, a tolerance that widens, a coverage denominator that shrinks — each
lowers the floor without a decision, and the next regression walks through the
gap. `docs/dev/harness-governance.md` is the rule; this gate makes it checkable.

A change to a control is classified by DIRECTION, not by file:

- **TIGHTEN** — floor rises / debt count falls / coverage rises / tolerance
  narrows → land it; the ratchet is meant to move this way.
- **LOOSEN** — the opposite → needs a governance record.
- **OPAQUE** — a structural change whose direction cannot be inferred → treated
  as LOOSEN, fail closed, until a record says otherwise.

Direction is NOT uniform: for a debt count lower is tighter; for a coverage count
higher is tighter; for a tolerance lower is tighter. `CONTROL_DIRECTION` below
holds the per-control direction; an unlisted key is OPAQUE. The gate is pure
logic over (old, new) values and the record text — it runs no git, so it is fully
driven by fixtures.

  python3 scripts/harness_control_gate.py check-fixtures scripts/harness_control_fixtures
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

TIGHTEN, LOOSEN, OPAQUE, UNCHANGED = "TIGHTEN", "LOOSEN", "OPAQUE", "UNCHANGED"

# Per control, per key: is a HIGHER value tighter or looser?
# "lower_is_tighter" → a debt count (fewer is better).
# "higher_is_tighter" → a coverage count (more is better).
# A key not listed for a control is OPAQUE (structural / a denominator).
CONTROL_DIRECTION: dict[str, dict[str, str]] = {
    "scripts/arch_baseline.json": {"*": "lower_is_tighter"},     # debt counts
    "scripts/lang_baseline.json": {"*": "lower_is_tighter"},     # untranslated counts
    "scripts/capability_baseline.json": {"proved": "higher_is_tighter"},  # applicable = denominator → OPAQUE
    "scripts/cli_exec_baseline.json": {"executed": "higher_is_tighter"},  # leaves = denominator → OPAQUE
}

# Outcomes of assess().
OK, NEEDS_RECORD, RECORD_INCOMPLETE = "OK", "NEEDS_RECORD", "RECORD_INCOMPLETE"

# A governance record must evidence these four, in either language.
RECORD_FIELDS = {
    "diff": (r"→", r"\bdiff\b", r"before.*after", r"\d+\s*->\s*\d+"),
    "reason": (r"\bmotivo\b", r"\breason\b", r"\brationale\b", r"\bporqu[eê]"),
    "impact": (r"\bimpacto\b", r"\bimpact\b"),
    "review": (r"\brevis[aã]o\b", r"\breview(er)?\b", r"\baprova"),
}


def _dir_for(path: str, key: str) -> str:
    table = CONTROL_DIRECTION.get(path, {})
    return table.get(key) or table.get("*") or "opaque"


def classify_value(path: str, key: str, old, new) -> str:
    """Classify a single numeric control value change by its real direction."""
    if old == new:
        return UNCHANGED
    if not isinstance(old, (int, float)) or not isinstance(new, (int, float)):
        return OPAQUE
    direction = _dir_for(path, key)
    if direction == "lower_is_tighter":
        return TIGHTEN if new < old else LOOSEN
    if direction == "higher_is_tighter":
        return TIGHTEN if new > old else LOOSEN
    return OPAQUE


def classify_baseline(path: str, old: dict, new: dict) -> list[tuple[str, str]]:
    """Classify every changed key between two baseline dicts."""
    out: list[tuple[str, str]] = []
    for key in sorted(set(old) | set(new)):
        o, n = old.get(key), new.get(key)
        if o == n:
            continue
        out.append((key, classify_value(path, key, o, n)))
    return out


def validate_record(text: str | None) -> list[str]:
    """Return the record fields that are missing (empty list = complete)."""
    if not text or not text.strip():
        return list(RECORD_FIELDS)
    missing = []
    for field, patterns in RECORD_FIELDS.items():
        if not any(re.search(p, text, re.IGNORECASE) for p in patterns):
            missing.append(field)
    return missing


def assess(classifications: list[str], record: str | None) -> tuple[str, str]:
    """Decide the outcome for a set of change classifications and a record."""
    needs = [c for c in classifications if c in (LOOSEN, OPAQUE)]
    if not needs:
        return OK, "only tightening/unchanged — no record required"
    missing = validate_record(record)
    if record is None or len(missing) == len(RECORD_FIELDS):
        return NEEDS_RECORD, f"{len(needs)} loosening/opaque change(s) without a governance record"
    if missing:
        return RECORD_INCOMPLETE, f"record missing field(s): {', '.join(missing)}"
    return OK, f"{len(needs)} loosening change(s) with a complete governance record"


def _assess_fixture_case(case: dict) -> str:
    path = case["path"]
    old, new = case.get("old", {}), case.get("new", {})
    if isinstance(old, dict) and isinstance(new, dict):
        classes = [c for _, c in classify_baseline(path, old, new)]
    else:
        classes = [classify_value(path, case.get("key", "*"), old, new)]
    outcome, _ = assess(classes, case.get("record"))
    return outcome


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
            got = _assess_fixture_case(case)
            if got != case["expect"]:
                failures += 1
                print(f"[FAIL] {fixture.name}::{case['name']} expected {case['expect']}, got {got}")
    if failures:
        print(f"{failures}/{total} fixture case(s) disagree with the governance rule")
        return 1
    print(f"all {total} fixture case(s) match the harness-governance rule")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    f = sub.add_parser("check-fixtures", help="validate a fixtures directory")
    f.add_argument("dir")
    f.set_defaults(func=_cmd_check_fixtures)
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (OSError, json.JSONDecodeError, KeyError) as exc:
        print(f"::error::bad input: {exc}")
        return 2


if __name__ == "__main__":
    sys.exit(main())
