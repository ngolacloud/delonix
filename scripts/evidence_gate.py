#!/usr/bin/env python3
"""Evidence gate — a result is evidence only if it says where it ran, and only PASS approves.

# What this closes

A green console line is not evidence; a structured record is. `release_gate.py`
binds each suite to the tag's commit and `e2e.sh` writes PASS/FAIL/SKIP per leaf,
but neither carries the full shape a reader needs — profile, denominator,
exclusions — nor the `BLOCKED` state, nor the rule that a mandatory profile is
approved only by applicable, executed evidence. `docs/dev/evidence-schema.md` is
the schema; this gate validates a record, decides promotion, and renders a view.

- **validate**: every field present; `state` in PASS/FAIL/SKIP/BLOCKED; a PASS/FAIL
  must carry an `exit_code` (it was executed); a SKIP/BLOCKED must carry a `reason`.
- **promote**: for each required (suite, profile), there must be an applicable
  record that is executed and passing (`state == PASS and exit_code == 0`). A suite
  covered only by SKIP/BLOCKED/FAIL does not approve — fail closed, name the gap.
- **render**: a human-readable table of the records.

Pure over records; no build, no git.

  python3 scripts/evidence_gate.py check-fixtures scripts/evidence_fixtures
  python3 scripts/evidence_gate.py render reports/<x>/evidence.jsonl
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

STATES = {"PASS", "FAIL", "SKIP", "BLOCKED"}
PROFILES = {"pr-rootless", "lab-ephemeral", "release"}
REQUIRED_FIELDS = ["sha", "suite", "version", "environment", "profile",
                   "command", "denominator", "exclusions", "state", "artifact_ref"]
_SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def validate_record(rec: dict) -> list[str]:
    errors: list[str] = []
    for f in REQUIRED_FIELDS:
        if f not in rec or rec[f] in (None, ""):
            errors.append(f"missing field: {f}")
    state = rec.get("state")
    if state not in STATES:
        errors.append(f"invalid state: {state!r} (want one of {sorted(STATES)})")
    if rec.get("profile") not in PROFILES:
        errors.append(f"invalid profile: {rec.get('profile')!r}")
    if "sha" in rec and not _SHA_RE.match(str(rec.get("sha", ""))):
        errors.append("sha is not a 40-hex commit")
    if state in ("PASS", "FAIL") and not isinstance(rec.get("exit_code"), int):
        errors.append(f"state {state} requires an integer exit_code (it was executed)")
    if state in ("SKIP", "BLOCKED") and not rec.get("reason"):
        errors.append(f"state {state} requires a reason")
    if "denominator" in rec and not isinstance(rec.get("denominator"), int):
        errors.append("denominator must be an integer")
    if "exclusions" in rec and not isinstance(rec.get("exclusions"), list):
        errors.append("exclusions must be a list")
    return errors


def is_executed_pass(rec: dict) -> bool:
    return rec.get("state") == "PASS" and rec.get("exit_code") == 0


def approves(records: list[dict], required: list[dict]) -> tuple[bool, list[str]]:
    """A mandatory (suite, profile) needs an applicable, executed-passing record."""
    reasons: list[str] = []
    for req in required:
        suite, profile = req["suite"], req["profile"]
        applicable = [r for r in records if r.get("suite") == suite and r.get("profile") == profile]
        if not any(is_executed_pass(r) for r in applicable):
            seen = sorted({r.get("state") for r in applicable}) or ["<none>"]
            reasons.append(f"{suite}@{profile}: no executed PASS (saw {seen})")
    return (not reasons), reasons


def render(records: list[dict]) -> str:
    head = "| suite | profile | state | exit | denom | excl | sha | artifact |\n"
    head += "|---|---|---|---|---|---|---|---|\n"
    rows = []
    for r in records:
        rows.append("| {suite} | {profile} | {state} | {exit} | {denom} | {excl} | {sha} | {art} |".format(
            suite=r.get("suite", "?"), profile=r.get("profile", "?"), state=r.get("state", "?"),
            exit=r.get("exit_code", "-"), denom=r.get("denominator", "-"),
            excl=",".join(r.get("exclusions", [])) or "-",
            sha=str(r.get("sha", "?"))[:8], art=r.get("artifact_ref", "-")))
    return head + "\n".join(rows)


def _load_jsonl(path: str) -> list[dict]:
    out = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line:
            out.append(json.loads(line))
    return out


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
            errors = validate_record(case["record"])
            got = "FAIL" if errors else "OK"
            if got != case["expect"]:
                failures += 1
                print(f"[FAIL] {fixture.name}::{case['name']} expected {case['expect']}, "
                      f"got {got} ({'; '.join(errors) or 'no errors'})")
    if failures:
        print(f"{failures}/{total} fixture case(s) disagree with the evidence schema")
        return 1
    print(f"all {total} fixture case(s) match the evidence schema")
    return 0


def _cmd_render(args: argparse.Namespace) -> int:
    print(render(_load_jsonl(args.file)))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    fx = sub.add_parser("check-fixtures")
    fx.add_argument("dir")
    fx.set_defaults(func=_cmd_check_fixtures)
    rn = sub.add_parser("render")
    rn.add_argument("file")
    rn.set_defaults(func=_cmd_render)
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (OSError, json.JSONDecodeError, KeyError) as exc:
        print(f"::error::bad input: {exc}")
        return 2


if __name__ == "__main__":
    sys.exit(main())
