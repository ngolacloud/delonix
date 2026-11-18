#!/usr/bin/env python3
"""Tests for `scripts/harness_control_gate.py`, run by the CI job `script-tests`."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("harness_control_gate", HERE / "harness_control_gate.py")
assert _spec and _spec.loader
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)

FIXTURES = HERE / "harness_control_fixtures"


class Direction(unittest.TestCase):
    def test_debt_count_lower_is_tighten(self):
        self.assertEqual(gate.classify_value("scripts/arch_baseline.json", "library_prints", 10, 7), gate.TIGHTEN)

    def test_debt_count_higher_is_loosen(self):
        self.assertEqual(gate.classify_value("scripts/arch_baseline.json", "library_prints", 7, 10), gate.LOOSEN)

    def test_coverage_higher_is_tighten(self):
        self.assertEqual(gate.classify_value("scripts/cli_exec_baseline.json", "executed", 100, 120), gate.TIGHTEN)

    def test_coverage_lower_is_loosen(self):
        self.assertEqual(gate.classify_value("scripts/capability_baseline.json", "proved", 158, 150), gate.LOOSEN)

    def test_unlisted_key_is_opaque(self):
        self.assertEqual(gate.classify_value("scripts/cli_exec_baseline.json", "leaves", 278, 270), gate.OPAQUE)

    def test_non_numeric_change_is_opaque(self):
        self.assertEqual(gate.classify_value("scripts/folhas_baseline.json", "surfaces", ["a", "b"], ["a"]), gate.OPAQUE)


class Record(unittest.TestCase):
    def test_complete_record_passes(self):
        text = "Diff: 9 -> 14. Motivo: X. Impacto: +5 sites. Revisão: aprovado."
        self.assertEqual(gate.validate_record(text), [])

    def test_missing_fields_reported(self):
        self.assertIn("impact", gate.validate_record("Diff: 9 -> 14. Motivo: X."))

    def test_empty_record_misses_everything(self):
        self.assertEqual(set(gate.validate_record(None)), set(gate.RECORD_FIELDS))


class Assess(unittest.TestCase):
    def test_only_tighten_needs_no_record(self):
        self.assertEqual(gate.assess([gate.TIGHTEN, gate.UNCHANGED], None)[0], gate.OK)

    def test_loosen_without_record_blocks(self):
        self.assertEqual(gate.assess([gate.LOOSEN], None)[0], gate.NEEDS_RECORD)

    def test_opaque_without_record_blocks(self):
        self.assertEqual(gate.assess([gate.OPAQUE], None)[0], gate.NEEDS_RECORD)

    def test_loosen_with_complete_record_ok(self):
        rec = "Diff 9->14, Motivo x, Impacto y, Revisão z"
        self.assertEqual(gate.assess([gate.LOOSEN], rec)[0], gate.OK)

    def test_loosen_with_incomplete_record_fails(self):
        self.assertEqual(gate.assess([gate.LOOSEN], "Diff 9->14, Motivo x")[0], gate.RECORD_INCOMPLETE)


class Fixtures(unittest.TestCase):
    def test_check_fixtures_passes(self):
        self.assertEqual(gate.main(["check-fixtures", str(FIXTURES)]), 0)


class PureNoGit(unittest.TestCase):
    def test_no_subprocess_or_git_surface(self):
        src = (HERE / "harness_control_gate.py").read_text(encoding="utf-8")
        for forbidden in ("import subprocess", "subprocess.", "os.system(", "os.popen("):
            self.assertNotIn(forbidden, src)


if __name__ == "__main__":
    unittest.main()
