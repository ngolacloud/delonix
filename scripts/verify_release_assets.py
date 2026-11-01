#!/usr/bin/env python3
"""Verify a release's ARTEFACTS — the part that was "PASS by design".

# What this closes (G1e of reports/go-nogo-alta-criticidade-20261101/)

The release pipeline PRODUCES a signed `SHA256SUMS` (minisign), an SBOM folded
INTO that signed file, and SLSA provenance (`actions/attest-build-provenance`).
Nothing, until now, VERIFIED those three against the real published assets from
the outside: the GO/NO-GO audit scored signature/provenance/SBOM as "by design,
not verified against artefacts". `release_verify.py` is a different check
(evidence-rot of the maturity table); `install.sh` verifies a single asset a
user downloads. Neither answers "does THIS release, as published, hold up end to
end?".

This does, and FAILS CLOSED — a missing signature, a swapped byte, an SBOM that
is not inside the signed file, or provenance that does not verify is a non-zero
exit with the reason named, not a warning after the fact.

# The four facts it checks, against a directory of assets (or a `--tag` it pulls)

1. SIGNATURE — `SHA256SUMS.minisig` verifies against the minisign public key the
   installer trusts. The trust root is read FROM `scripts/install.sh`
   (`MINISIGN_PUBKEY=`), the same key a user's installer embeds — not a second
   copy that could drift. "Came from us", not merely "not corrupted".
2. INTEGRITY — every file listed in `SHA256SUMS` is present and hashes to the
   recorded digest. The signature above makes these digests trustworthy; this
   step makes them true of the bytes on disk.
3. SBOM — `delonix-sbom.spdx.json` is present, is valid SPDX, AND is itself
   listed in `SHA256SUMS` (so it rode inside the signature: an inventory
   published outside the signature is one anyone could swap, which the release
   workflow's own comment calls out).
4. PROVENANCE — each attested binary verifies with
   `gh attestation verify <file> --repo <repo>` (SLSA build provenance, signed
   by GitHub's OIDC identity). Needs network + an authenticated `gh`; when that
   is absent the check is SKIPPED, and a skip is a FAILURE unless `--no-provenance`
   explicitly waives it (and says so loudly). It never silently passes.

# Usage
  scripts/verify_release_assets.py --dir dist-release
  scripts/verify_release_assets.py --tag v0.1.0            # gh release download, then verify
  scripts/verify_release_assets.py --dir D --no-provenance # offline: waive step 4 loudly
  scripts/verify_release_assets.py --dir D --pubkey-file K # verify against a different trust root

Exit 0 only if every required check passed. stdlib only, like `sbom.py` and
`release_verify.py` — dev tooling, nothing this engine ships.
"""
import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

SUMS_NAME = "SHA256SUMS"
SIG_NAME = "SHA256SUMS.minisig"
SBOM_NAME = "delonix-sbom.spdx.json"
DEFAULT_REPO = "ngolacloud/delonix"

# Assets that ride in SHA256SUMS but are NOT build-provenance subjects: the
# signed manifest cannot attest itself, its own signature, the installer script,
# or the editor extension (built in a separate repo). Everything else named
# `delonix-*` is a subject the release workflow fed to attest-build-provenance.
NOT_PROVENANCE_SUBJECTS = {SUMS_NAME, SIG_NAME, "install.sh", "delonix-vscode"}


class Result:
    """Collects per-check outcomes so the whole report prints, then the exit
    code reflects whether anything required failed (one pass, not stop-on-first:
    a release manager wants every problem at once, not a whack-a-mole)."""

    def __init__(self) -> None:
        self.lines: list[str] = []
        self.failed = False

    def ok(self, msg: str) -> None:
        self.lines.append(f"  ✓ {msg}")

    def fail(self, msg: str) -> None:
        self.lines.append(f"  ✗ {msg}")
        self.failed = True

    def warn(self, msg: str) -> None:
        self.lines.append(f"  ! {msg}")

    def section(self, title: str) -> None:
        self.lines.append(title)

    def dump(self) -> None:
        print("\n".join(self.lines))


# --- pure helpers (no subprocess, no network) — the testable core -----------


def parse_sha256sums(text: str) -> dict[str, str]:
    """`<hex>  <name>` per line (sha256sum's two-space form; a `*` binary
    marker is tolerated). Returns {name: lowercase-hex}. Malformed lines are
    skipped — a stray blank line is not a verification failure."""
    out: dict[str, str] = {}
    for line in text.splitlines():
        m = re.match(r"^([0-9a-fA-F]{64}) [ *](.+)$", line)
        if m:
            out[m.group(2).strip()] = m.group(1).lower()
    return out


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def verify_checksums(
    directory: Path, sums: dict[str, str], allow_missing: bool, res: Result
) -> None:
    res.section("INTEGRITY (every listed file present and matching)")
    if not sums:
        res.fail(f"{SUMS_NAME} lists no files — nothing to verify (empty or malformed)")
        return
    present = matched = 0
    for name, want in sorted(sums.items()):
        path = directory / name
        if not path.is_file():
            if allow_missing:
                res.warn(f"{name}: not present (--allow-missing)")
            else:
                res.fail(f"{name}: listed in {SUMS_NAME} but not present")
            continue
        present += 1
        got = sha256_file(path)
        if got == want:
            matched += 1
        else:
            res.fail(f"{name}: sha256 mismatch (want {want[:12]}…, got {got[:12]}…)")
    if matched and matched == present and (allow_missing or present == len(sums)):
        res.ok(f"{matched}/{len(sums)} listed asset(s) present and matching")
    elif matched == present and present:
        res.ok(f"{matched} present asset(s) match (of {len(sums)} listed)")


def validate_sbom(directory: Path, sums: dict[str, str], res: Result) -> None:
    res.section("SBOM (present, valid SPDX, and inside the signed manifest)")
    sbom = directory / SBOM_NAME
    if not sbom.is_file():
        res.fail(f"{SBOM_NAME}: not present — the release ships no SBOM to verify")
        return
    if SBOM_NAME not in sums:
        res.fail(
            f"{SBOM_NAME}: present but NOT listed in {SUMS_NAME} — it rode OUTSIDE "
            "the signature, so it is an inventory anyone could swap"
        )
    else:
        res.ok(f"{SBOM_NAME} is listed in {SUMS_NAME} (signed inventory)")
    try:
        doc = json.loads(sbom.read_text())
    except (OSError, json.JSONDecodeError) as e:
        res.fail(f"{SBOM_NAME}: not valid JSON ({e})")
        return
    version = str(doc.get("spdxVersion", ""))
    packages = doc.get("packages")
    if not version.startswith("SPDX-"):
        res.fail(f"{SBOM_NAME}: spdxVersion {version!r} is not an SPDX document")
    elif not isinstance(packages, list) or not packages:
        res.fail(f"{SBOM_NAME}: SPDX {version} but names no packages")
    else:
        res.ok(f"{SBOM_NAME} is valid {version} with {len(packages)} package(s)")


# --- steps that shell out (guarded in tests) --------------------------------


def read_pubkey_from_install() -> str | None:
    """The trust root: the base64 line of `MINISIGN_PUBKEY=` in
    scripts/install.sh — the exact key a user's installer embeds."""
    try:
        text = (ROOT / "scripts/install.sh").read_text()
    except OSError:
        return None
    m = re.search(r'MINISIGN_PUBKEY="([A-Za-z0-9+/=]+)"', text)
    key = m.group(1) if m else None
    return key if key and key != "__POR_PREENCHER__" else None


def verify_signature(directory: Path, pubkey: str, res: Result) -> None:
    res.section("SIGNATURE (SHA256SUMS signed by the key the installer trusts)")
    sums, sig = directory / SUMS_NAME, directory / SIG_NAME
    if not sums.is_file():
        res.fail(f"{SUMS_NAME}: not present")
        return
    if not sig.is_file():
        res.fail(f"{SIG_NAME}: not present — the manifest is unsigned")
        return
    if not shutil.which("minisign"):
        res.fail("minisign not installed — cannot verify the signature (install minisign)")
        return
    with tempfile.NamedTemporaryFile("w", suffix=".pub", delete=False) as f:
        f.write(f"untrusted comment: delonix-runtime\n{pubkey}\n")
        pubfile = f.name
    try:
        proc = subprocess.run(
            ["minisign", "-V", "-p", pubfile, "-m", str(sums)],
            capture_output=True,
            text=True,
        )
    finally:
        Path(pubfile).unlink(missing_ok=True)
    if proc.returncode == 0:
        res.ok(f"{SUMS_NAME} verifies against the installer's embedded public key")
    else:
        detail = (proc.stderr or proc.stdout).strip().splitlines()
        res.fail(
            f"{SUMS_NAME} does NOT verify against the trusted key — wrong key or "
            f"tampered manifest ({detail[-1] if detail else 'minisign refused'})"
        )


def verify_provenance(
    directory: Path, sums: dict[str, str], repo: str, waived: bool, res: Result
) -> None:
    res.section("PROVENANCE (SLSA build provenance per attested binary)")
    subjects = [
        n
        for n in sorted(sums)
        if n not in NOT_PROVENANCE_SUBJECTS and (directory / n).is_file()
    ]
    if waived:
        res.warn(
            f"provenance WAIVED (--no-provenance) for {len(subjects)} subject(s) — "
            "this release was NOT checked for SLSA provenance"
        )
        return
    if not shutil.which("gh"):
        res.fail("`gh` not installed — cannot verify provenance (run with --no-provenance to waive)")
        return
    if not subjects:
        res.fail("no provenance subjects found among the assets")
        return
    verified = 0
    for name in subjects:
        proc = subprocess.run(
            ["gh", "attestation", "verify", str(directory / name), "--repo", repo],
            capture_output=True,
            text=True,
        )
        if proc.returncode == 0:
            verified += 1
        else:
            detail = (proc.stderr or proc.stdout).strip().splitlines()
            res.fail(
                f"{name}: provenance did NOT verify "
                f"({detail[-1] if detail else 'gh refused'})"
            )
    if verified == len(subjects):
        res.ok(f"{verified}/{len(subjects)} binary provenance attestation(s) verified against {repo}")


# --- driver -----------------------------------------------------------------


def download_release(tag: str, repo: str, into: Path) -> None:
    if not shutil.which("gh"):
        sys.exit("`gh` not installed — cannot download a release by --tag")
    subprocess.run(
        ["gh", "release", "download", tag, "--repo", repo, "--dir", str(into), "--clobber"],
        check=True,
    )


def main() -> int:
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    src = p.add_mutually_exclusive_group(required=True)
    src.add_argument("--dir", type=Path, help="directory holding the release assets")
    src.add_argument("--tag", help="download this release with gh, then verify it")
    p.add_argument("--repo", default=DEFAULT_REPO, help=f"owner/name for provenance (default {DEFAULT_REPO})")
    p.add_argument("--pubkey", help="minisign public key (base64 line) to trust instead of install.sh's")
    p.add_argument("--pubkey-file", type=Path, help="file with the minisign public key to trust")
    p.add_argument("--no-provenance", action="store_true", help="waive the SLSA provenance check (loudly)")
    p.add_argument("--allow-missing", action="store_true", help="verify only the assets present (partial dir)")
    args = p.parse_args()

    if args.tag:
        directory = Path(tempfile.mkdtemp(prefix="delonix-release-"))
        download_release(args.tag, args.repo, directory)
    else:
        directory = args.dir
        if not directory.is_dir():
            sys.exit(f"not a directory: {directory}")

    if args.pubkey_file:
        pubkey = args.pubkey_file.read_text().strip().splitlines()[-1].strip()
    elif args.pubkey:
        pubkey = args.pubkey.strip()
    else:
        pubkey = read_pubkey_from_install()
        if not pubkey:
            sys.exit("no trust root: scripts/install.sh has no MINISIGN_PUBKEY (pass --pubkey/--pubkey-file)")

    sums_text = ""
    sums_path = directory / SUMS_NAME
    if sums_path.is_file():
        sums_text = sums_path.read_text()
    sums = parse_sha256sums(sums_text)

    res = Result()
    print(f"Verifying release assets in {directory}\n")
    verify_signature(directory, pubkey, res)
    verify_checksums(directory, sums, args.allow_missing, res)
    validate_sbom(directory, sums, res)
    verify_provenance(directory, sums, args.repo, args.no_provenance, res)
    res.dump()

    print()
    if res.failed:
        print("RESULT: FAIL — this release did not verify end to end (see ✗ above)")
        return 1
    if args.no_provenance:
        print(
            "RESULT: PASS — signature, integrity and SBOM verified; "
            "provenance WAIVED (not checked, --no-provenance)"
        )
        return 0
    print("RESULT: PASS — signature, integrity, SBOM and provenance all verified")
    return 0


if __name__ == "__main__":
    sys.exit(main())
