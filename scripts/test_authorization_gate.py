#!/usr/bin/env python3
"""Tests for `scripts/authorization_gate.py`, run by the CI job `script-tests`.

These tests exercise the pure authorization decision and the benign fixtures.
They prove three things H01 requires:

1. The three states — ABSENT, PARTIAL, EXPLICIT — are told apart.
2. Only the authorized operation+target is accepted; nothing generalizes.
3. Evaluating authorization publishes NOTHING: the gate is static logic with no
   `subprocess`/`socket`/`git`/`gh` surface, asserted structurally below.
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
GATE_PATH = HERE / "authorization_gate.py"
FIXTURES = HERE / "authorization_fixtures"

_spec = importlib.util.spec_from_file_location("authorization_gate", GATE_PATH)
assert _spec and _spec.loader
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)


class DecideAbsent(unittest.TestCase):
    def test_read_edit_commit_self_authorized(self):
        for op in ("read", "local_edit", "commit"):
            decision, _ = gate.decide(op, "anything", {})
            self.assertEqual(decision, gate.ALLOW, op)

    def test_publishing_acts_refused_when_absent(self):
        for op in ("push", "pr_create", "pr_edit", "merge", "release"):
            decision, _ = gate.decide(op, "target", {})
            self.assertEqual(decision, gate.REFUSE, op)

    def test_unknown_operation_fails_closed(self):
        decision, reason = gate.decide("force_push", "branch:main", {})
        self.assertEqual(decision, gate.REFUSE)
        self.assertIn("fail closed", reason)


class DecidePartial(unittest.TestCase):
    GRANT = {"grants": {"push": ["branch:chore/x"]}}

    def test_exact_target_allowed(self):
        decision, _ = gate.decide("push", "branch:chore/x", self.GRANT)
        self.assertEqual(decision, gate.ALLOW)

    def test_other_target_refused(self):
        decision, reason = gate.decide("push", "branch:chore/y", self.GRANT)
        self.assertEqual(decision, gate.REFUSE)
        self.assertIn("partial", reason)

    def test_grant_does_not_generalize_across_operations(self):
        for op in ("pr_create", "merge", "release"):
            decision, _ = gate.decide(op, "branch:chore/x", self.GRANT)
            self.assertEqual(decision, gate.REFUSE, op)


class DecideExplicit(unittest.TestCase):
    def test_named_operation_and_target_allowed(self):
        grant = {"grants": {"merge": ["pr:742"]}}
        self.assertEqual(gate.decide("merge", "pr:742", grant)[0], gate.ALLOW)

    def test_develop_integration_does_not_authorize_release(self):
        grant = {"grants": {"pr_create": ["develop"], "merge": ["pr:742"]}}
        self.assertEqual(gate.decide("release", "tag:v0.1.2", grant)[0], gate.REFUSE)

    def test_release_needs_its_own_explicit_grant(self):
        grant = {"grants": {"release": ["tag:v0.1.2"]}}
        self.assertEqual(gate.decide("release", "tag:v0.1.2", grant)[0], gate.ALLOW)


class Fixtures(unittest.TestCase):
    def test_check_fixtures_passes(self):
        rc = gate.main(["check-fixtures", str(FIXTURES)])
        self.assertEqual(rc, 0)

    def test_fixtures_cover_the_three_distinct_states(self):
        import json

        states = set()
        for path in FIXTURES.glob("*.json"):
            states.add(json.loads(path.read_text(encoding="utf-8"))["state"])
        self.assertEqual(states, {"ABSENT", "PARTIAL", "EXPLICIT"})


class PublishesNothing(unittest.TestCase):
    """The gate must be pure: no process/network/git/gh surface at all."""

    def test_no_side_effecting_imports_or_calls(self):
        src = GATE_PATH.read_text(encoding="utf-8")
        for forbidden in ("subprocess", "os.system", "socket", "urllib", "requests"):
            self.assertNotIn(forbidden, src, f"gate must not reference {forbidden}")

    def test_no_git_or_gh_invocation(self):
        src = GATE_PATH.read_text(encoding="utf-8")
        # Only the docstring may mention the command names; no call sites. The
        # simplest structural proof: the words "git " / "gh " never appear as a
        # shelled command (they only appear inside the documented refusal list).
        for line in src.splitlines():
            stripped = line.strip()
            if stripped.startswith("#") or stripped.startswith('"') or stripped.startswith("'"):
                continue
            self.assertNotIn('"git', line)
            self.assertNotIn('"gh ', line)


if __name__ == "__main__":
    unittest.main()
