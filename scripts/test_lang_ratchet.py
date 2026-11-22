#!/usr/bin/env python3
"""Unit tests for the two LANG-01 decisions taken on 2026-11-22 —
stdlib `unittest`, no new dependency (same reasoning as `test_arch_fitness.py`).

`scripts/lang_ratchet.py` had no test file at all, and the two changes it took
that day are exactly the kind that rot in silence:

* **`ao` left the lexicon.** It collided with the locale tag this repo writes
  everywhere (`pt-AO`, `pt_AO`, `docs/dev/pt-AO/`, `*.pt-AO.md`) and with
  Angola's TLD, so English lines about our own i18n counted as Portuguese debt:
  30 entries depended only on that word, 29 of them false. Without a test, the
  next person adding a word has nothing telling them it was measured out.
* **`GENERATED_MD` skips `docs/providers/capability-matrix.md`**, which a test
  keeps equal to `delonix provider matrix`. An exclusion naming a path that
  stops existing guards nothing and reads green, which is why the path's
  existence is asserted here.

Run: python3 scripts/test_lang_ratchet.py
"""
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lang_ratchet as g  # noqa: E402


class Lexicon(unittest.TestCase):
    def setUp(self):
        self.pt = g.load_lexicon()

    def test_ao_is_out(self):
        self.assertNotIn("ao", self.pt)

    def test_the_locale_tag_is_not_portuguese(self):
        """The line that made the measurement: English prose about our own
        catalogue, flagged only because `pt-AO` contains `ao`."""
        self.assertFalse(
            g.is_pt_text(
                "the pt-AO help strings are in the `pt.po` catalog", self.pt
            )
        )
        self.assertFalse(g.is_pt_text("every string has a pt-AO translation.", self.pt))

    def test_the_angolan_tld_is_not_portuguese(self):
        self.assertFalse(g.is_pt_text("the latter is the `loja.exemplo.ao` case", self.pt))

    def test_real_portuguese_is_still_caught(self):
        """Removing one word must not blind the detector."""
        self.assertTrue(g.is_pt_text("morre com EACCES a resolver o ficheiro", self.pt))
        self.assertTrue(g.is_pt_text("a correccao entra no mesmo commit", self.pt))

    def test_the_lexicon_declares_no_homograph_it_warns_about(self):
        """The file's own header forbids these; a regression would poison the
        count the same way `ao` did."""
        for word in ("remove", "media", "data", "base", "no", "so", "ate", "ver", "pos", "seg"):
            self.assertNotIn(word, self.pt, f"'{word}' is a PT/EN homograph")


class GeneratedMd(unittest.TestCase):
    def test_every_excluded_path_still_exists(self):
        for rel in g.GENERATED_MD:
            self.assertTrue((g.ROOT / rel).is_file(), f"{rel} is gone — drop the exclusion")

    def test_the_excluded_file_is_generated_by_a_command(self):
        """It is skipped BECAUSE it is generated. If nothing regenerates it, the
        reason has evaporated and the exclusion is just a hole."""
        matrix = "docs/providers/capability-matrix.md"
        self.assertIn(matrix, g.GENERATED_MD)
        agents = (g.ROOT / "AGENTS.md").read_text(encoding="utf-8")
        self.assertIn("delonix provider matrix > docs/providers/capability-matrix.md", agents)

    def test_the_exclusion_does_not_skip_other_markdown(self):
        kept = [p for p in g.walk(g.ROOT) if p.suffix == ".md"]
        self.assertTrue(any(p.name == "AGENTS.md" for p in kept))


if __name__ == "__main__":
    unittest.main()
