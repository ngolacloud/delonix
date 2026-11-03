#!/usr/bin/env python3
"""Unit tests for `scripts/verify_release_assets.py` — stdlib `unittest`, zero
new dependency (same reasoning as `test_release_verify.py`: dev tooling, not
something this engine ships).

The pure core (checksum parsing, integrity, SBOM shape) is tested without any
external tool. The signature round-trip runs only where `minisign` exists
(`skipUnless`), with a THROWAWAY key — never the real release key. The
provenance step is tested through its waive/can't-run branches, which need
neither `gh` nor the network.

Run: python3 scripts/test_verify_release_assets.py
"""
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import verify_release_assets as vra  # noqa: E402


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_sbom(path: Path) -> None:
    path.write_text(
        json.dumps(
            {
                "spdxVersion": "SPDX-2.3",
                "packages": [{"name": "delonix", "versionInfo": "0.1.0"}],
            }
        )
    )


def make_release(directory: Path, *, list_sbom: bool = True) -> dict[str, str]:
    """A minimal, internally consistent release dir: two 'binaries' + an SBOM,
    with a SHA256SUMS that matches. Returns the {name: digest} map written."""
    files = {
        "delonix-x86_64-linux": b"fake binary one",
        "delonix-cri-x86_64-linux": b"fake binary two",
    }
    sums = {}
    for name, data in files.items():
        (directory / name).write_bytes(data)
        sums[name] = sha256(data)
    write_sbom(directory / vra.SBOM_NAME)
    if list_sbom:
        sums[vra.SBOM_NAME] = sha256((directory / vra.SBOM_NAME).read_bytes())
    lines = [f"{digest}  {name}" for name, digest in sums.items()]
    (directory / vra.SUMS_NAME).write_text("\n".join(lines) + "\n")
    return sums


class ParseTests(unittest.TestCase):
    def test_parses_two_space_form_and_skips_junk(self):
        text = f"{'a' * 64}  delonix-x86_64-linux\n\n# a comment\n{'b' * 64} *binmarker\n"
        got = vra.parse_sha256sums(text)
        self.assertEqual(
            got, {"delonix-x86_64-linux": "a" * 64, "binmarker": "b" * 64}
        )

    def test_empty_text_is_no_files(self):
        self.assertEqual(vra.parse_sha256sums(""), {})


class IntegrityTests(unittest.TestCase):
    def test_all_present_and_matching_passes(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory)
            res = vra.Result()
            vra.verify_checksums(directory, sums, allow_missing=False, res=res)
            self.assertFalse(res.failed)

    def test_a_swapped_byte_fails(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory)
            (directory / "delonix-x86_64-linux").write_bytes(b"tampered!")
            res = vra.Result()
            vra.verify_checksums(directory, sums, allow_missing=False, res=res)
            self.assertTrue(res.failed)

    def test_a_missing_listed_file_fails_closed(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory)
            (directory / "delonix-cri-x86_64-linux").unlink()
            res = vra.Result()
            vra.verify_checksums(directory, sums, allow_missing=False, res=res)
            self.assertTrue(res.failed)

    def test_missing_is_a_warning_not_a_failure_when_allowed(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory)
            (directory / "delonix-cri-x86_64-linux").unlink()
            res = vra.Result()
            vra.verify_checksums(directory, sums, allow_missing=True, res=res)
            self.assertFalse(res.failed)


class SbomTests(unittest.TestCase):
    def test_valid_and_listed_passes(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory, list_sbom=True)
            res = vra.Result()
            vra.validate_sbom(directory, sums, res)
            self.assertFalse(res.failed)

    def test_present_but_not_listed_fails(self):
        # Rode OUTSIDE the signature — the swap the workflow comment warns about.
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory, list_sbom=False)
            res = vra.Result()
            vra.validate_sbom(directory, sums, res)
            self.assertTrue(res.failed)

    def test_absent_sbom_fails(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory)
            (directory / vra.SBOM_NAME).unlink()
            res = vra.Result()
            vra.validate_sbom(directory, sums, res)
            self.assertTrue(res.failed)

    def test_spdx_with_no_packages_fails(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            (directory / vra.SBOM_NAME).write_text(
                json.dumps({"spdxVersion": "SPDX-2.3", "packages": []})
            )
            sums = {vra.SBOM_NAME: sha256((directory / vra.SBOM_NAME).read_bytes())}
            res = vra.Result()
            vra.validate_sbom(directory, sums, res)
            self.assertTrue(res.failed)

    def test_non_json_sbom_fails(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            (directory / vra.SBOM_NAME).write_text("not json {")
            sums = {vra.SBOM_NAME: sha256((directory / vra.SBOM_NAME).read_bytes())}
            res = vra.Result()
            vra.validate_sbom(directory, sums, res)
            self.assertTrue(res.failed)


class ProvenanceTests(unittest.TestCase):
    def test_waived_is_a_warning_not_a_failure(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory)
            res = vra.Result()
            vra.verify_provenance(directory, sums, "o/r", waived=True, res=res)
            self.assertFalse(res.failed)

    def test_cannot_run_and_not_waived_fails_closed(self):
        with tempfile.TemporaryDirectory() as d:
            directory = Path(d)
            sums = make_release(directory)
            res = vra.Result()
            with mock.patch.object(vra.shutil, "which", return_value=None):
                vra.verify_provenance(directory, sums, "o/r", waived=False, res=res)
            self.assertTrue(res.failed)


class TrustRootTests(unittest.TestCase):
    def test_reads_a_real_key_from_install_sh(self):
        # The repo's install.sh must carry a real embedded key, not the
        # placeholder — the whole signature check hangs off this being present.
        key = vra.read_pubkey_from_install()
        self.assertIsNotNone(key)
        self.assertNotEqual(key, "__POR_PREENCHER__")


@unittest.skipUnless(shutil.which("minisign"), "minisign not installed")
class SignatureTests(unittest.TestCase):
    """End-to-end signature round-trip with a THROWAWAY key (never the real
    release key) — proves the check both accepts a good signature and refuses a
    tampered manifest / a missing signature."""

    def _keypair(self, d: Path) -> str:
        sec, pub = d / "sec.key", d / "pub.key"
        subprocess.run(
            ["minisign", "-G", "-p", str(pub), "-s", str(sec), "-W"],
            check=True,
            capture_output=True,
        )
        self.sec = sec
        return pub.read_text().strip().splitlines()[-1].strip()

    def _sign(self, sums: Path) -> None:
        subprocess.run(
            ["minisign", "-S", "-s", str(self.sec), "-m", str(sums)],
            check=True,
            capture_output=True,
            input="\n",
            text=True,
        )

    def test_good_signature_passes(self):
        with tempfile.TemporaryDirectory() as dd:
            directory = Path(dd)
            make_release(directory)
            pubkey = self._keypair(directory)
            self._sign(directory / vra.SUMS_NAME)
            res = vra.Result()
            vra.verify_signature(directory, pubkey, res)
            self.assertFalse(res.failed, "\n".join(res.lines))

    def test_tampered_manifest_fails(self):
        with tempfile.TemporaryDirectory() as dd:
            directory = Path(dd)
            make_release(directory)
            pubkey = self._keypair(directory)
            self._sign(directory / vra.SUMS_NAME)
            # Change the manifest AFTER signing it.
            with (directory / vra.SUMS_NAME).open("a") as f:
                f.write(f"{'c' * 64}  smuggled-asset\n")
            res = vra.Result()
            vra.verify_signature(directory, pubkey, res)
            self.assertTrue(res.failed)

    def test_missing_signature_fails(self):
        with tempfile.TemporaryDirectory() as dd:
            directory = Path(dd)
            make_release(directory)
            pubkey = self._keypair(directory)  # no _sign() → no .minisig
            res = vra.Result()
            vra.verify_signature(directory, pubkey, res)
            self.assertTrue(res.failed)


if __name__ == "__main__":
    unittest.main(verbosity=2)
