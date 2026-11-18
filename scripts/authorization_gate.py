#!/usr/bin/env python3
"""Authorization gate — the collaboration-authority contract, as runnable logic.

# What this closes

`.ai/agents/authorization.md` is the single source of truth for who may perform
the acts that leave a task worktree and reach the shared repository or the
outside world: `push`, `pr_create`, `pr_edit`, `merge`, `release`. Until now that
rule lived only as prose, scattered across `AGENTS.md`, the GitFlow process page
and each agent spec, and nothing could tell an **absent**, a **partial** and an
**explicit** authorization apart in a way a fixture or a test could check.

This module turns the contract into a **pure decision function**: given an
operation, a target and the grants in effect, it returns ALLOW or REFUSE. It
never shells out to `git`/`gh` and never touches the network — so evaluating
authorization can never, itself, publish anything. That is the property the H01
fixtures rely on: they exercise the decision, not a real push.

# The rule (mirrors authorization.md)

- `read` / `local_edit` / `commit` are self-authorized within an open scope
  contract → always ALLOW here (the scope contract, not this gate, bounds them).
- `push` / `pr_create` / `pr_edit` / `merge` / `release` are refused by default.
  They ALLOW **iff** the grants list the **exact** target under the **exact**
  operation. A grant does not generalize: a push grant is not a PR grant, a PR
  grant is not a merge grant, and none of them is a release grant; a grant for
  one target does not cover another.
- An **unknown** operation fails closed → REFUSE (never silently allowed).

# Usage

    # Decide a single request (exit 0 = ALLOW, 3 = REFUSE, 2 = bad input):
    python3 scripts/authorization_gate.py decide \
        --op push --target branch:chore/x --grant grant.json

    # Validate a directory of fixtures (exit 0 = all match, 1 = a mismatch):
    python3 scripts/authorization_gate.py check-fixtures scripts/authorization_fixtures
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

# Self-authorized within an open scope contract — bounded elsewhere, not here.
SELF_AUTHORIZED = ("read", "local_edit", "commit")

# Everything that leaves the worktree; refused unless an explicit grant matches.
COLLAB_OPS = ("push", "pr_create", "pr_edit", "merge", "release")

ALLOW = "ALLOW"
REFUSE = "REFUSE"


def decide(operation: str, target: str | None, grant: dict[str, Any]) -> tuple[str, str]:
    """Return (decision, reason) for one (operation, target, grant).

    `grant` is `{"grants": {"<operation>": ["<target>", ...], ...}}` — the set of
    targets explicitly authorized per operation. An empty or missing mapping is
    the ABSENT state. This function is total and side-effect-free.
    """
    if operation in SELF_AUTHORIZED:
        return ALLOW, f"{operation} is self-authorized within an open scope contract"

    if operation not in COLLAB_OPS:
        # Fail closed: an unknown operation is treated as the most restricted.
        return REFUSE, f"unknown operation {operation!r} — fail closed"

    grants = grant.get("grants", {}) if isinstance(grant, dict) else {}
    allowed_targets = grants.get(operation, [])
    if not isinstance(allowed_targets, list):
        return REFUSE, f"malformed grant for {operation!r} — fail closed"

    if target is not None and target in allowed_targets:
        return ALLOW, f"explicit grant for {operation} {target}"

    if allowed_targets:
        return (
            REFUSE,
            f"partial authorization: {operation} granted for {allowed_targets}, "
            f"not for target {target!r}",
        )
    return REFUSE, f"absent authorization for {operation} (deliver the command to the owner)"


def _load_json(path: str) -> dict[str, Any]:
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def _cmd_decide(args: argparse.Namespace) -> int:
    grant: dict[str, Any] = {}
    if args.grant:
        grant = _load_json(args.grant)
    decision, reason = decide(args.op, args.target, grant)
    print(f"{decision}: {reason}")
    if decision == ALLOW:
        return 0
    return 3


def _cmd_check_fixtures(args: argparse.Namespace) -> int:
    root = Path(args.dir)
    files = sorted(root.glob("*.json"))
    if not files:
        print(f"::error::no fixtures in {root} — this gate stopped guarding anything")
        return 1

    failures = 0
    total = 0
    for fixture in files:
        data = _load_json(str(fixture))
        for case in data.get("cases", []):
            total += 1
            decision, reason = decide(
                case["operation"], case.get("target"), {"grants": case.get("grant", {})}
            )
            expect = case["expect"]
            status = "ok" if decision == expect else "FAIL"
            if decision != expect:
                failures += 1
                print(
                    f"[{status}] {fixture.name}::{case['name']} "
                    f"expected {expect}, got {decision} — {reason}"
                )
    if failures:
        print(f"{failures}/{total} fixture case(s) disagree with authorization.md")
        return 1
    print(f"all {total} fixture case(s) match the authorization contract")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)

    d = sub.add_parser("decide", help="decide a single (operation, target, grant)")
    d.add_argument("--op", required=True, help="operation, e.g. push / pr_create / release")
    d.add_argument("--target", default=None, help="target, e.g. branch:chore/x / develop / tag:v0.1.1")
    d.add_argument("--grant", default=None, help="path to a grant JSON (omitted = ABSENT)")
    d.set_defaults(func=_cmd_decide)

    c = sub.add_parser("check-fixtures", help="validate a fixtures directory")
    c.add_argument("dir", help="directory of *.json fixtures")
    c.set_defaults(func=_cmd_check_fixtures)

    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (OSError, json.JSONDecodeError, KeyError) as exc:
        print(f"::error::bad input: {exc}")
        return 2


if __name__ == "__main__":
    sys.exit(main())
