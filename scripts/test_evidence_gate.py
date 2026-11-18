#!/usr/bin/env python3
"""Tests for `scripts/evidence_gate.py`, run by the CI job `script-tests`."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("evidence_gate", HERE / "evidence_gate.py")
assert _spec and _spec.loader
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)

FIXTURES = HERE / "evidence_fixtures"
SHA = "e923d617080de011580b7ee6b7a4fe6df134590e"


def rec(**over):
    base = {"sha": SHA, "suite": "e2e", "version": "develop", "environment": "ci",
            "profile": "release", "command": "scripts/e2e.sh", "exit_code": 0,
            "denominator": 103, "exclusions": [], "state": "PASS",
            "artifact_ref": "reports/x/r.jsonl"}
    base.update(over)
    return base


class Validate(unittest.TestCase):
    def test_valid_pass(self):
        self.assertEqual(gate.validate_record(rec()), [])

    def test_pass_needs_exit_code(self):
        r = rec(); del r["exit_code"]
        self.assertTrue(any("exit_code" in e for e in gate.validate_record(r)))

    def test_skip_needs_reason(self):
        r = rec(state="SKIP"); del r["exit_code"]
        self.assertTrue(any("reason" in e for e in gate.validate_record(r)))

    def test_blocked_needs_reason(self):
        r = rec(state="BLOCKED"); del r["exit_code"]
        self.assertTrue(any("reason" in e for e in gate.validate_record(r)))

    def test_invalid_state(self):
        self.assertTrue(gate.validate_record(rec(state="GREENISH")))

    def test_bad_sha(self):
        self.assertTrue(any("sha" in e for e in gate.validate_record(rec(sha="abc"))))

    def test_invalid_profile(self):
        self.assertTrue(any("profile" in e for e in gate.validate_record(rec(profile="prod"))))


class Promotion(unittest.TestCase):
    REQUIRED = [{"suite": "e2e", "profile": "release"}, {"suite": "critest", "profile": "release"}]

    def test_approved_when_all_required_pass_executed(self):
        recs = [rec(suite="e2e"), rec(suite="critest")]
        ok, reasons = gate.approves(recs, self.REQUIRED)
        self.assertTrue(ok, reasons)

    def test_skip_does_not_approve_mandatory_profile(self):
        recs = [rec(suite="e2e"), rec(suite="critest", state="SKIP", reason="no VM")]
        ok, reasons = gate.approves(recs, self.REQUIRED)
        self.assertFalse(ok)
        self.assertTrue(any("critest@release" in r for r in reasons))

    def test_blocked_does_not_approve(self):
        recs = [rec(suite="e2e"), rec(suite="critest", state="BLOCKED", reason="no client")]
        self.assertFalse(gate.approves(recs, self.REQUIRED)[0])

    def test_fail_does_not_approve(self):
        recs = [rec(suite="e2e"), rec(suite="critest", state="FAIL")]
        self.assertFalse(gate.approves(recs, self.REQUIRED)[0])

    def test_wrong_profile_is_not_applicable(self):
        recs = [rec(suite="e2e"), rec(suite="critest", profile="lab-ephemeral")]
        self.assertFalse(gate.approves(recs, self.REQUIRED)[0])

    def test_pass_with_nonzero_exit_does_not_approve(self):
        # Defensive: a PASS mislabelled with a nonzero exit is not an executed pass.
        self.assertFalse(gate.is_executed_pass(rec(exit_code=1)))


class Render(unittest.TestCase):
    def test_render_has_a_row_per_record(self):
        out = gate.render([rec(suite="e2e"), rec(suite="critest", state="SKIP")])
        self.assertIn("| e2e |", out)
        self.assertIn("| critest |", out)
        self.assertIn("SKIP", out)


class Fixtures(unittest.TestCase):
    def test_check_fixtures_passes(self):
        self.assertEqual(gate.main(["check-fixtures", str(FIXTURES)]), 0)


if __name__ == "__main__":
    unittest.main()
