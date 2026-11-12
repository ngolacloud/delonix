#!/usr/bin/env python3
"""Unit tests for `scripts/release_gate.py` — stdlib `unittest`, zero new
dependency (same reasoning as `test_release_verify.py`).

The gate's core — binding each required suite to the tag's commit — is tested as
a pure function (`evaluate`) and through a `record`→`check` round-trip in a temp
manifest dir. The CI-green step is tested through its can't-run branch, which
needs neither `gh` nor the network.

Run: python3 scripts/test_release_gate.py
"""
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import release_gate as rg  # noqa: E402

SHA_A = "a" * 40
SHA_B = "b" * 40


def full(suites: dict) -> dict:
    return {"tag": "v9.9.9", "suites": suites}


def all_green(commit: str) -> dict:
    return full({s: {"commit": commit, "passed": True, "when": "t"} for s in rg.REQUIRED_SUITES})


class CommitsMatchTests(unittest.TestCase):
    def test_prefix_either_way_matches(self):
        self.assertTrue(rg.commits_match(SHA_A, SHA_A[:12]))
        self.assertTrue(rg.commits_match(SHA_A[:7], SHA_A))

    def test_different_commits_do_not_match(self):
        self.assertFalse(rg.commits_match(SHA_A, SHA_B))

    def test_empty_never_matches(self):
        self.assertFalse(rg.commits_match("", SHA_A))
        self.assertFalse(rg.commits_match(SHA_A, ""))


class EvaluateTests(unittest.TestCase):
    def test_all_required_passed_at_the_tag_commit_is_releasable(self):
        self.assertEqual(rg.evaluate(all_green(SHA_A), SHA_A), [])

    def test_a_short_commit_still_matches_the_full_tag_commit(self):
        doc = all_green(SHA_A[:12])
        self.assertEqual(rg.evaluate(doc, SHA_A), [])

    def test_a_missing_suite_blocks(self):
        doc = all_green(SHA_A)
        del doc["suites"]["chaos"]
        probs = rg.evaluate(doc, SHA_A)
        self.assertTrue(any("chaos" in p and "no result" in p for p in probs))

    def test_a_failed_suite_blocks(self):
        doc = all_green(SHA_A)
        doc["suites"]["critest"]["passed"] = False
        probs = rg.evaluate(doc, SHA_A)
        self.assertTrue(any("critest" in p and "FAILED" in p for p in probs))

    def test_a_suite_from_a_different_commit_blocks(self):
        # The exact failure G2 names: validated, but not on THIS build.
        doc = all_green(SHA_A)
        doc["suites"]["e2e"]["commit"] = SHA_B
        probs = rg.evaluate(doc, SHA_A)
        self.assertTrue(any("e2e" in p and "DIFFERENT build" in p for p in probs))

    def test_empty_manifest_blocks_on_every_required_suite(self):
        probs = rg.evaluate(full({}), SHA_A)
        self.assertEqual(len(probs), len(rg.REQUIRED_SUITES))


class EvaluateAcceptsListTests(unittest.TestCase):
    def test_a_list_matches_any_acceptable_commit(self):
        # Suites recorded on the parent (SHA_B) pass when the parent is in the
        # acceptable set — the doc-only-child-becomes-tag case.
        doc = all_green(SHA_B)
        self.assertEqual(rg.evaluate(doc, [SHA_A, SHA_B]), [])

    def test_a_commit_outside_the_set_still_blocks(self):
        doc = all_green("c" * 40)
        self.assertTrue(rg.evaluate(doc, [SHA_A, SHA_B]))


class AcceptableCommitsTests(unittest.TestCase):
    """`acceptable_commits` needs a real repo; build a throwaway one."""

    def _git(self, repo, *args):
        import subprocess
        subprocess.run(["git", *args], cwd=repo, check=True,
                       capture_output=True, text=True,
                       env={"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t",
                            "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t",
                            "PATH": __import__("os").environ.get("PATH", "")})

    def _head(self, repo):
        import subprocess
        return subprocess.run(["git", "rev-parse", "HEAD"], cwd=repo,
                              capture_output=True, text=True).stdout.strip()

    def test_doc_only_child_lets_the_parent_count(self):
        with tempfile.TemporaryDirectory() as d:
            repo = Path(d)
            self._git(repo, "init", "-q")
            (repo / "code.rs").write_text("fn main() {}\n")
            self._git(repo, "add", "-A")
            self._git(repo, "commit", "-q", "-m", "release content")
            parent = self._head(repo)
            # doc-only child: only the validation manifest changes.
            vd = repo / "reports" / "release-validation"
            vd.mkdir(parents=True)
            (vd / "v0.1.1.json").write_text("{}\n")
            self._git(repo, "add", "-A")
            self._git(repo, "commit", "-q", "-m", "validation evidence")
            tag = self._head(repo)
            with mock.patch.object(rg, "ROOT", repo):
                accept = rg.acceptable_commits(tag)
            self.assertIn(tag, accept)
            self.assertIn(parent, accept)  # the validated parent counts

    def test_a_child_that_touches_code_does_not_lend_the_parent(self):
        with tempfile.TemporaryDirectory() as d:
            repo = Path(d)
            self._git(repo, "init", "-q")
            (repo / "code.rs").write_text("fn main() {}\n")
            self._git(repo, "add", "-A")
            self._git(repo, "commit", "-q", "-m", "c1")
            parent = self._head(repo)
            (repo / "code.rs").write_text("fn main() { /* changed */ }\n")
            self._git(repo, "add", "-A")
            self._git(repo, "commit", "-q", "-m", "c2 touches code")
            tag = self._head(repo)
            with mock.patch.object(rg, "ROOT", repo):
                accept = rg.acceptable_commits(tag)
            self.assertEqual(accept, [tag])  # parent NOT accepted


class RecordRoundTripTests(unittest.TestCase):
    def test_record_then_check_passes_only_when_complete(self):
        with tempfile.TemporaryDirectory() as d:
            with mock.patch.object(rg, "VALIDATION_DIR", Path(d)):
                tag = "v0.1.1"
                args = lambda suite: mock.Mock(  # noqa: E731
                    tag=tag, suite=suite, commit=SHA_A, passed=True,
                    failed=False, when="t", allow_extra=False,
                )
                # One short of complete → the gate blocks.
                for s in rg.REQUIRED_SUITES[:-1]:
                    rg.cmd_record(args(s))
                doc = rg.load_manifest(tag)
                self.assertTrue(rg.evaluate(doc, SHA_A))
                # The last one → releasable.
                rg.cmd_record(args(rg.REQUIRED_SUITES[-1]))
                doc = rg.load_manifest(tag)
                self.assertEqual(rg.evaluate(doc, SHA_A), [])

    def test_record_rejects_a_non_sha_commit(self):
        with tempfile.TemporaryDirectory() as d:
            with mock.patch.object(rg, "VALIDATION_DIR", Path(d)):
                a = mock.Mock(tag="v1", suite="e2e", commit="not-a-sha",
                              passed=True, failed=False, when="t", allow_extra=False)
                with self.assertRaises(SystemExit):
                    rg.cmd_record(a)


class CiGreenTests(unittest.TestCase):
    def test_no_gh_is_reported_not_assumed_green(self):
        with mock.patch.object(rg.shutil, "which", return_value=None):
            green, detail = rg.ci_is_green("o/r", SHA_A)
            self.assertFalse(green)
            self.assertIn("gh", detail)


if __name__ == "__main__":
    unittest.main(verbosity=2)
