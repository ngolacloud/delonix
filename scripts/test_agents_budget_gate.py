#!/usr/bin/env python3
"""Tests for `scripts/agents_budget_gate.py`, run by the CI job `script-tests`."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("agents_budget_gate", HERE / "agents_budget_gate.py")
assert _spec and _spec.loader
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)


class RepoState(unittest.TestCase):
    def test_repo_passes_the_gate(self):
        self.assertEqual(gate.main([]), 0)

    def test_first_contact_file_exists_and_is_under_budget(self):
        self.assertTrue(gate.INVARIANTS.exists(), "the first-contact file must exist")
        size = gate.INVARIANTS.stat().st_size
        lines = gate.INVARIANTS.read_text(encoding="utf-8").count("\n") + 1
        self.assertLessEqual(size, gate.INVARIANTS_MAX_BYTES)
        self.assertLessEqual(lines, gate.INVARIANTS_MAX_LINES)

    def test_first_contact_is_a_small_fraction_of_the_archive(self):
        # The whole point of H03: first contact must be a fraction of the archive.
        if gate.AGENTS.exists():
            self.assertLess(gate.INVARIANTS.stat().st_size, gate.AGENTS.stat().st_size * 0.1)

    def test_no_errors_reported(self):
        self.assertEqual(gate.check(verbose=False), [])


class Frontmatter(unittest.TestCase):
    def test_parses_leading_block(self):
        fm = gate._frontmatter("---\nname: x\ntools: Read, Grep\n---\nbody\n")
        self.assertEqual(fm, {"name": "x", "tools": "Read, Grep"})

    def test_none_without_block(self):
        self.assertIsNone(gate._frontmatter("# just a doc\n\ntext\n"))

    def test_every_agent_spec_has_name_and_tools(self):
        for spec in gate.AGENTS_DIR.glob("*.md"):
            if spec.name in gate.DOC_ONLY or spec.name == "INVARIANTS.md":
                continue
            fm = gate._frontmatter(spec.read_text(encoding="utf-8"))
            if fm is None:
                continue
            self.assertTrue(fm.get("name"), spec.name)
            self.assertTrue(fm.get("tools"), spec.name)


class Pointers(unittest.TestCase):
    def test_canonical_pointers_resolve(self):
        text = gate.INVARIANTS.read_text(encoding="utf-8")
        for must in ("authorization.md", "permission-tiers.md", "engineering-process.md"):
            self.assertIn(must, text, f"invariants must route to {must}")

    def test_globs_are_not_treated_as_files(self):
        # scripts/test_*.py is a glob mentioned in prose, not a missing file.
        self.assertNotIn("test_*.py", str(gate.check(verbose=False)))


if __name__ == "__main__":
    unittest.main()
