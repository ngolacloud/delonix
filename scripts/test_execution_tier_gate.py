#!/usr/bin/env python3
"""Tests for `scripts/execution_tier_gate.py`, run by the CI job `script-tests`.

Proves what H02 requires of the read/execution profiles:
1. Read-only inspection is allowed at Tier A; write/remote/credential/elevation/
   indirect-subprocess effects are refused.
2. Rootless build/test is allowed at Tier B; remote/privileged/destructive ops
   are refused.
3. Anything above SafeWrite never auto-allows, even at tier C/D (requires_confirm).
4. The classifier is pure: no subprocess/network surface (asserted structurally).
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
GATE_PATH = HERE / "execution_tier_gate.py"
FIXTURES = HERE / "execution_tier_fixtures"

_spec = importlib.util.spec_from_file_location("execution_tier_gate", GATE_PATH)
assert _spec and _spec.loader
gate = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gate)


class TierARead(unittest.TestCase):
    def test_read_only_allowed(self):
        for cmd in ("ls -la", "git log --oneline", "cargo tree", "grep -rn x ."):
            self.assertEqual(gate.decide(cmd, "A")[0], gate.ALLOW, cmd)

    def test_write_remote_cred_elevation_refused(self):
        for cmd in ("cargo build", "git push origin x", "cat ~/.aws/credentials",
                    "sudo ls", "tee /etc/hosts"):
            self.assertEqual(gate.decide(cmd, "A")[0], gate.REFUSE, cmd)

    def test_indirect_subprocess_is_judged_by_inner_command(self):
        self.assertEqual(gate.decide("bash -c 'git push --force'", "A")[0], gate.REFUSE)
        self.assertEqual(gate.decide("find . -delete", "A")[0], gate.REFUSE)
        self.assertEqual(gate.decide("xargs rm -rf", "A")[0], gate.REFUSE)

    def test_unknown_verb_fails_closed(self):
        level, reason = gate.classify("frobnicate")
        self.assertEqual(level, gate.PRIVILEGED)
        self.assertIn("fail closed", reason)


class TierBControlled(unittest.TestCase):
    def test_rootless_build_test_allowed(self):
        for cmd in ("cargo build --workspace", "cargo test -p x", "make check", "git add f"):
            self.assertEqual(gate.decide(cmd, "B")[0], gate.ALLOW, cmd)

    def test_remote_and_privileged_refused(self):
        for cmd in ("git push -u origin x", "gh pr create", "sudo mount x /mnt",
                    "nft add rule", "rm -rf /var/lib/delonix",
                    "delonix-storage-helper create-pool --wipe-devices"):
            self.assertEqual(gate.decide(cmd, "B")[0], gate.REFUSE, cmd)


class TierCDNeverAuto(unittest.TestCase):
    def test_above_safewrite_never_auto(self):
        for tier, cmd in (("C", "delonix stack destroy"), ("D", "mount -t tmpfs none /mnt"),
                          ("D", "nft add table inet filter")):
            self.assertEqual(gate.decide(cmd, tier)[0], gate.REFUSE, (tier, cmd))

    def test_read_and_safewrite_still_auto_at_cd(self):
        self.assertEqual(gate.decide("ls", "D")[0], gate.ALLOW)
        self.assertEqual(gate.decide("cargo build", "C")[0], gate.ALLOW)

    def test_unknown_tier_fails_closed(self):
        self.assertEqual(gate.decide("ls", "Z")[0], gate.REFUSE)


class Fixtures(unittest.TestCase):
    def test_check_fixtures_passes(self):
        self.assertEqual(gate.main(["check-fixtures", str(FIXTURES)]), 0)

    def test_fixtures_cover_every_refusal_mode(self):
        import json
        names = []
        for p in FIXTURES.glob("*.json"):
            for c in json.loads(p.read_text(encoding="utf-8"))["cases"]:
                names.append(c["name"])
        joined = " ".join(names)
        for mode in ("write", "remote", "credential", "elevation", "indirect"):
            self.assertIn(mode, joined, f"no fixture exercises {mode}")


class PureNoExecution(unittest.TestCase):
    def test_no_side_effecting_surface(self):
        # Match invocation/import patterns, not the bare word (the docstring
        # legitimately explains that it *decomposes* subprocess wrappers).
        src = GATE_PATH.read_text(encoding="utf-8")
        for forbidden in ("import subprocess", "subprocess.", "import socket",
                          "socket.", "os.system(", "os.popen(", "import urllib",
                          "import requests", "pty.spawn"):
            self.assertNotIn(forbidden, src, f"gate must not use {forbidden}")


if __name__ == "__main__":
    unittest.main()
