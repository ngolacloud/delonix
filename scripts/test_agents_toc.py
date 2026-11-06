#!/usr/bin/env python3
"""Tests for `scripts/agents_toc.py`, run by the CI job `script-tests`."""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    "agents_toc", Path(__file__).resolve().parent / "agents_toc.py"
)
agents_toc = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(agents_toc)


class SectionTitles(unittest.TestCase):
    def test_ignores_headings_inside_code_fences(self):
        text = (
            "# Title\n\nintro\n\n"
            "## Real one\n\n"
            "```\n## not a heading, it is shell\n```\n\n"
            "## Real two\n"
        )
        self.assertEqual(agents_toc.section_titles(text), ["Real one", "Real two"])

    def test_ignores_tilde_fences(self):
        text = "# T\n\n## A\n\n~~~\n## fenced\n~~~\n\n## B\n"
        self.assertEqual(agents_toc.section_titles(text), ["A", "B"])

    def test_excludes_the_toc_block_itself(self):
        # A regenerated file must not list the index's own "## Índice" heading.
        built = agents_toc.build("# T\n\nintro\n\n## A\n\n## B\n")
        self.assertEqual(agents_toc.section_titles(built), ["A", "B"])


class Slug(unittest.TestCase):
    def test_keeps_accents_lowercases_strips_punctuation(self):
        self.assertEqual(
            agents_toc.github_slug("Âmbito: a sessão (ADR-0040)", {}),
            "âmbito-a-sessão-adr-0040",
        )

    def test_drops_backticks_and_keeps_inner_text(self):
        self.assertEqual(
            agents_toc.github_slug("O portão (`contract_gate.py`)", {}),
            "o-portão-contract_gatepy",
        )

    def test_disambiguates_duplicates(self):
        seen: dict[str, int] = {}
        self.assertEqual(agents_toc.github_slug("Comandos", seen), "comandos")
        self.assertEqual(agents_toc.github_slug("Comandos", seen), "comandos-1")
        self.assertEqual(agents_toc.github_slug("Comandos", seen), "comandos-2")

    def test_link_markup_reduced_to_text(self):
        self.assertEqual(agents_toc.github_slug("See [README](README.md)", {}), "see-readme")


class Splice(unittest.TestCase):
    def test_inserts_before_first_level2_heading(self):
        out = agents_toc.build("# T\n\nintro\n\n## First\n\nbody\n")
        self.assertIn(agents_toc.BEGIN, out)
        self.assertIn(agents_toc.END, out)
        # The block sits before the first real section and after the intro.
        self.assertLess(out.index(agents_toc.BEGIN), out.index("## First"))
        self.assertLess(out.index("intro"), out.index(agents_toc.BEGIN))
        self.assertIn("- [First](#first)", out)

    def test_idempotent(self):
        once = agents_toc.build("# T\n\nintro\n\n## A\n\n## B\n")
        twice = agents_toc.build(once)
        self.assertEqual(once, twice)

    def test_check_detects_a_new_appended_section(self):
        built = agents_toc.build("# T\n\nintro\n\n## A\n")
        stale = built + "\n## Newly appended\n"
        # A section added after the index makes the regenerated form differ.
        self.assertNotEqual(agents_toc.build(stale), stale)
        self.assertIn("Newly appended", agents_toc.build(stale))


if __name__ == "__main__":
    unittest.main()
