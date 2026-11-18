#!/usr/bin/env python3
"""Tests for `scripts/prompt_lint_gate.py`, run by the CI job `script-tests`."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("prompt_lint_gate", HERE / "prompt_lint_gate.py")
assert _spec and _spec.loader
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)

FIXTURES = HERE / "prompt_lint_fixtures"

VALID = (
    "- **ID:** DX-1\n- **Base/SHA:** develop @ e923d617\n- **Objective:** x.\n"
    "- **Issue/ADR:** ADR-0040\n- **Authorized artefacts:** scripts/prompt_lint_gate.py\n"
    "- **Dependencies:** none\n- **Lab profile:** rootless (Tier B)\n"
    "- **Tests:** scripts/test_prompt_lint_gate.py\n- **Evidence validity:** fresh\n"
    "- **Expected output:** diff\n"
)


class Parse(unittest.TestCase):
    def test_parses_fields_and_aliases(self):
        f = gate.parse_fields(VALID)
        self.assertEqual(f["id"], "DX-1")
        self.assertEqual(f["base"], "develop @ e923d617")
        self.assertIn("evidence", f)

    def test_portuguese_aliases(self):
        f = gate.parse_fields("- **Objectivo:** o\n- **Testes:** t\n- **Saída esperada:** s\n")
        self.assertEqual(f["objective"], "o")
        self.assertEqual(f["tests"], "t")
        self.assertEqual(f["output"], "s")


class Validate(unittest.TestCase):
    def test_valid_prompt_has_no_errors(self):
        self.assertEqual(gate.validate(gate.parse_fields(VALID)), [])

    def test_dangling_adr_is_flagged(self):
        fields = gate.parse_fields(VALID.replace("ADR-0040", "ADR-9999"))
        self.assertTrue(any("ADR-9999" in e for e in gate.validate(fields)))

    def test_missing_artefact_is_flagged(self):
        fields = gate.parse_fields(VALID.replace("scripts/prompt_lint_gate.py", "scripts/nope.py"))
        self.assertTrue(any("does not exist" in e for e in gate.validate(fields)))

    def test_missing_field_is_flagged(self):
        fields = gate.parse_fields(VALID)
        del fields["tests"]
        self.assertTrue(any("tests" in e for e in gate.validate(fields)))

    def test_historical_without_reconfirm_is_flagged(self):
        fields = gate.parse_fields(VALID.replace("fresh", "historical (2026-10-24)"))
        self.assertTrue(any("reconfirm" in e.lower() for e in gate.validate(fields)))

    def test_historical_with_reconfirm_passes(self):
        fields = gate.parse_fields(VALID.replace(
            "fresh", "historical (2026-10-24); reconfirm against code and a fresh run"))
        self.assertEqual(gate.validate(fields), [])

    def test_base_must_name_branch_or_sha(self):
        fields = gate.parse_fields(VALID.replace("develop @ e923d617", "yesterday"))
        self.assertTrue(any("Base" in e for e in gate.validate(fields)))


class Fixtures(unittest.TestCase):
    def test_check_fixtures_passes(self):
        self.assertEqual(gate.main(["check-fixtures", str(FIXTURES)]), 0)


class Template(unittest.TestCase):
    def test_template_fenced_example_passes_its_own_lint(self):
        import re
        tmpl = (gate.ROOT / ".ai" / "prompt-template.md").read_text(encoding="utf-8")
        # Validate the fenced example (not the field descriptions above it).
        blocks = re.findall(r"```\n(.*?)```", tmpl, re.DOTALL)
        self.assertTrue(blocks, "the template must carry a fenced example prompt")
        self.assertEqual(gate.validate(gate.parse_fields(blocks[-1])), [])


if __name__ == "__main__":
    unittest.main()
