#!/usr/bin/env python3
"""Unit tests for `.githooks/commit-msg` — stdlib `unittest`, no new dependency
(same reasoning as `test_arch_fitness.py`).

The hook exists because the rule alone did not hold: measured 2026-11-22 over the
whole history, **810** `Co-Authored-By: Claude …` trailers, still arriving (126 in
August, 83 in September, 443 in October, 136 in November), against a rule written
on 2026-09-24. A contributors page counts a co-author trailer as a person, so each
one invented a contributor.

Two properties matter more than the refusal itself, and both are pinned here:

* **A message that NAMES the trailer while explaining the rule must pass.** The
  commit that introduced the `.mailmap` does exactly that, and a `grep` over the
  body would have refused it. The hook asks `git interpret-trailers --parse`
  instead, which returns only what a parser would act on.
* **A human co-author called Claude must pass.** The hook matches vendor DOMAINS,
  never names: refusing someone's credit is the costliest false positive there is.

Run: python3 scripts/test_commit_msg_hook.py
"""
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HOOK = ROOT / ".githooks" / "commit-msg"


def run(message: str) -> subprocess.CompletedProcess:
    with tempfile.TemporaryDirectory() as d:
        f = Path(d) / "COMMIT_EDITMSG"
        f.write_text(message, encoding="utf-8")
        return subprocess.run([str(HOOK), str(f)], capture_output=True, text=True)


class Hook(unittest.TestCase):
    def test_the_hook_is_executable(self):
        self.assertTrue(HOOK.is_file(), f"{HOOK} is missing")
        self.assertTrue(HOOK.stat().st_mode & 0o111, "the hook is not executable")

    def test_a_clean_message_passes(self):
        self.assertEqual(run("fix: a thing\n\nBecause of a reason.\n").returncode, 0)

    def test_the_anthropic_trailer_is_refused(self):
        r = run("feat: x\n\nwhy.\n\nCo-Authored-By: Claude Opus 5 <noreply@anthropic.com>\n")
        self.assertEqual(r.returncode, 1)
        self.assertIn("AI attribution trailer", r.stderr)
        self.assertIn("noreply@anthropic.com", r.stderr, "the refusal must quote the line")

    def test_the_case_of_the_key_does_not_matter(self):
        for key in ("Co-authored-by", "CO-AUTHORED-BY", "co-authored-by"):
            with self.subTest(key=key):
                r = run(f"feat: x\n\nwhy.\n\n{key}: Claude <noreply@anthropic.com>\n")
                self.assertEqual(r.returncode, 1, key)

    def test_prose_that_names_the_trailer_passes(self):
        """The `.mailmap` commit's shape: it quotes the trailer to explain the
        rule. A grep over the body refuses it; git's trailer parser does not."""
        msg = (
            "chore(git): a .mailmap joins the owner's identities\n\n"
            "The `claude` entry comes from 810 `Co-Authored-By: Claude …` trailers\n"
            "in the BODY of the messages, which the page counts as people.\n"
        )
        r = run(msg)
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_a_human_co_author_named_claude_passes(self):
        """Claude is a common first name. Matching names instead of domains would
        refuse a real person's credit."""
        r = run("feat: z\n\nx.\n\nCo-authored-by: Claude Dupont <claude.dupont@example.fr>\n")
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_a_human_co_author_passes(self):
        r = run("feat: z\n\nx.\n\nCo-authored-by: Alexandre Antonio Juca <corex@example.com>\n")
        self.assertEqual(r.returncode, 0, r.stderr)

    def test_any_trailer_key_with_a_vendor_domain_is_refused(self):
        """The rule is about AI ATTRIBUTION, not about one key."""
        r = run("feat: x\n\nwhy.\n\nAssisted-by: Something <bot@openai.com>\n")
        self.assertEqual(r.returncode, 1)

    def test_it_refuses_without_a_message_file(self):
        r = subprocess.run([str(HOOK)], capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)


class WiredUp(unittest.TestCase):
    """A hook nothing installs is a hook nobody runs."""

    def test_the_bootstrap_points_core_hookspath_at_the_versioned_dir(self):
        s = (ROOT / "scripts" / "dev-bootstrap.sh").read_text(encoding="utf-8")
        self.assertIn("install_git_hooks", s)
        self.assertIn("core.hooksPath", s)
        self.assertIn("install_git_hooks\n", s, "declared but never called")

    def test_the_report_says_when_it_is_not_wired(self):
        s = (ROOT / "scripts" / "dev-bootstrap.sh").read_text(encoding="utf-8")
        self.assertIn('row opt "git hooks"', s)


if __name__ == "__main__":
    unittest.main()
