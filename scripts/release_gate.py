#!/usr/bin/env python3
"""Release gate — a tag may not be published unless it was VALIDATED at its own
commit.

# What this closes (G2 of reports/go-nogo-alta-criticidade-20261101/)

The GO/NO-GO audit scored the release line NO-GO partly because the last legacy
tag (v5.1.1) was cut with a red CI and with the suites "run on develop weeks
ago", not on the tagged commit. `release.yml`'s `guard` checks the tag's SHAPE
and that it EXISTS — never that it is GREEN, nor that `cargo test` / `e2e.sh` /
`chaos.sh` / `critest` actually ran on THIS commit.

This gate does, and FAILS CLOSED. It binds every required suite's result to the
EXACT commit the tag points at: a suite recorded against an earlier commit is a
miss, not a pass — which is the precise failure mode ("validated, but not this
build") the audit named.

# The evidence manifest

`reports/release-validation/<tag>.json`, committed alongside the release:

    {
      "tag": "v0.1.1",
      "suites": {
        "cargo-test": {"commit": "<40-hex>", "passed": true, "when": "<iso>"},
        "e2e":        {"commit": "<40-hex>", "passed": true, "when": "<iso>"},
        "chaos":      {"commit": "<40-hex>", "passed": true, "when": "<iso>"},
        "critest":    {"commit": "<40-hex>", "passed": true, "when": "<iso>"}
      }
    }

Each suite writes its own line as it finishes (`record`), so the manifest grows
as the release candidate is validated; the gate (`check`) refuses the tag until
every required suite is present, passed, and pinned to the tag's commit.

# Usage
  # 1. Run every suite on the RELEASE-CONTENT commit C (the code being shipped),
  #    recording each against C as it passes:
  scripts/release_gate.py record --tag v0.1.1 --suite e2e --commit $(git rev-parse HEAD) --passed
  # 2. Commit the manifest. A manifest cannot record its OWN commit's SHA (that
  #    is a hash fixpoint — impossible), so the evidence is recorded on C and
  #    committed as a DOC-ONLY child D (it changes nothing but the manifest). Tag
  #    D. The gate accepts C as D's validated parent precisely because D added no
  #    code — the tagged code IS the validated code.
  git add reports/release-validation/v0.1.1.json && git commit -m "release: v0.1.1 validation evidence"
  git tag v0.1.1   # D

  # 3. The gate (release.yml guard runs this; exit 1 blocks the publish):
  scripts/release_gate.py check --tag v0.1.1 --commit <tag-commit> [--require-ci --repo o/r]

stdlib only, like `sbom.py`/`release_verify.py`/`verify_release_assets.py`.
"""
import argparse
import json
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VALIDATION_DIR = ROOT / "reports" / "release-validation"

# The suites G2 requires on the tagged commit. `cargo test` is the one CI itself
# runs; e2e/chaos/critest are the live batteries a human runs on a host — which
# is exactly why their commit has to be pinned, or "we ran them" drifts off the
# tag silently.
REQUIRED_SUITES = ("cargo-test", "e2e", "chaos", "critest")

SHA_RE = re.compile(r"^[0-9a-f]{7,40}$")


def manifest_path(tag: str) -> Path:
    # A tag is a filename component; keep it one (no slashes, no traversal).
    safe = tag.replace("/", "-")
    return VALIDATION_DIR / f"{safe}.json"


def load_manifest(tag: str) -> dict:
    p = manifest_path(tag)
    if not p.is_file():
        return {"tag": tag, "suites": {}}
    doc = json.loads(p.read_text())
    doc.setdefault("tag", tag)
    doc.setdefault("suites", {})
    return doc


def commits_match(a: str, b: str) -> bool:
    """Two git revisions name the same commit when one is a prefix of the other
    (a short SHA recorded by one suite, a full SHA from the tag, say)."""
    a, b = a.strip().lower(), b.strip().lower()
    if not a or not b:
        return False
    return a.startswith(b) or b.startswith(a)


# --- record -----------------------------------------------------------------


def cmd_record(args) -> int:
    if not SHA_RE.match(args.commit.strip().lower()):
        sys.exit(f"--commit {args.commit!r} is not a git sha")
    if args.suite not in REQUIRED_SUITES and not args.allow_extra:
        sys.exit(
            f"--suite {args.suite!r} is not one of {REQUIRED_SUITES} "
            "(use --allow-extra to record an additional suite)"
        )
    doc = load_manifest(args.tag)
    when = args.when or datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    doc["suites"][args.suite] = {
        "commit": args.commit.strip().lower(),
        "passed": not args.failed,
        "when": when,
    }
    VALIDATION_DIR.mkdir(parents=True, exist_ok=True)
    manifest_path(args.tag).write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")
    state = "FAILED" if args.failed else "passed"
    print(f"recorded {args.suite} {state} @ {args.commit[:12]} for {args.tag}")
    return 0


# --- check (the gate) -------------------------------------------------------


def evaluate(doc: dict, accept) -> list[str]:
    """Pure: the list of reasons the tag is NOT releasable (empty = releasable).
    Testable without git, gh or the network.

    `accept` is the commit the tag points at, or a list of acceptable commits
    (the tag's commit plus its validated parent — see `acceptable_commits`). A
    suite matches when its recorded commit prefix-matches ANY acceptable one."""
    accept_list = [accept] if isinstance(accept, str) else list(accept)
    problems: list[str] = []
    suites = doc.get("suites", {})
    for name in REQUIRED_SUITES:
        rec = suites.get(name)
        if rec is None:
            problems.append(f"{name}: no result recorded for this tag")
            continue
        if not rec.get("passed"):
            problems.append(f"{name}: recorded as FAILED (at {rec.get('commit', '?')[:12]})")
            continue
        rc = str(rec.get("commit", ""))
        if not any(commits_match(rc, c) for c in accept_list):
            problems.append(
                f"{name}: ran at {rc[:12] or '?'}, not the tag's commit "
                f"{accept_list[0][:12]} — validation is from a DIFFERENT build"
            )
    return problems


def acceptable_commits(tag_commit: str) -> list[str]:
    """The commits whose validation counts for this tag: the tag's own commit,
    plus its first parent IF the tag commit adds NOTHING but the validation
    manifest. Because a manifest cannot name its own commit (a SHA that contains
    itself — a hash fixpoint, cryptographically impossible), the suites are
    recorded on the release-content commit and the manifest is committed as a
    doc-only child that becomes the tag. Accepting that parent keeps the
    guarantee — the tagged CODE equals the validated code, since the child
    touched no code — while making the gate satisfiable at all. Git-dependent,
    so it lives outside the pure `evaluate`."""
    accept = [tag_commit]
    try:
        parent = subprocess.run(
            ["git", "rev-parse", "--verify", f"{tag_commit}^"],
            capture_output=True, text=True, cwd=ROOT,
        )
        if parent.returncode != 0:
            return accept  # root commit, or unknown rev
        parent_sha = parent.stdout.strip()
        diff = subprocess.run(
            ["git", "diff", "--name-only", parent_sha, tag_commit],
            capture_output=True, text=True, cwd=ROOT,
        )
        if diff.returncode != 0:
            return accept
        changed = [ln.strip() for ln in diff.stdout.splitlines() if ln.strip()]
        rel = "reports/release-validation/"
        if changed and all(p.startswith(rel) for p in changed):
            accept.append(parent_sha)
    except OSError:
        pass
    return accept


def ci_is_green(repo: str, commit: str) -> tuple[bool, str]:
    """Asks GitHub whether every check-run on `commit` concluded success. Needs
    an authenticated `gh`; a missing gh or a transport error is reported, never
    silently treated as green."""
    if not shutil.which("gh"):
        return False, "`gh` not installed — cannot confirm CI is green"
    proc = subprocess.run(
        ["gh", "api", f"repos/{repo}/commits/{commit}/check-runs", "--paginate"],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        return False, (proc.stderr or proc.stdout).strip().splitlines()[-1:][0] if (proc.stderr or proc.stdout).strip() else "gh refused"
    runs = []
    for chunk in proc.stdout.strip().splitlines():
        try:
            runs.extend(json.loads(chunk).get("check_runs", []))
        except json.JSONDecodeError:
            pass
    if not runs:
        return False, "no check-runs found for the commit (did CI run on the tag?)"
    bad = [r["name"] for r in runs if r.get("conclusion") not in ("success", "neutral", "skipped")]
    if bad:
        return False, f"{len(bad)} check(s) not green: {', '.join(sorted(set(bad))[:6])}"
    return True, f"{len(runs)} check-run(s) all green"


def cmd_check(args) -> int:
    doc = load_manifest(args.tag)
    print(f"Release gate for {args.tag} (commit {args.commit[:12]})\n")
    accept = acceptable_commits(args.commit)
    problems = evaluate(doc, accept)
    for name in REQUIRED_SUITES:
        rec = doc.get("suites", {}).get(name)
        if rec and rec.get("passed") and any(
            commits_match(str(rec.get("commit", "")), c) for c in accept
        ):
            print(f"  ✓ {name}: passed @ {str(rec['commit'])[:12]}")
    for p in problems:
        print(f"  ✗ {p}")

    if args.require_ci:
        green, detail = ci_is_green(args.repo, args.commit)
        print(f"  {'✓' if green else '✗'} CI: {detail}")
        if not green:
            problems.append(f"CI not confirmed green: {detail}")

    print()
    if problems:
        print(f"RESULT: FAIL — {args.tag} is NOT releasable ({len(problems)} problem(s) above)")
        return 1
    print(f"RESULT: PASS — {args.tag} validated on its own commit, releasable")
    return 0


def main() -> int:
    p = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = p.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("record", help="record one suite's result for a tag")
    r.add_argument("--tag", required=True)
    r.add_argument("--suite", required=True)
    r.add_argument("--commit", required=True)
    r.add_argument("--passed", action="store_true", help="the suite passed (default unless --failed)")
    r.add_argument("--failed", action="store_true", help="the suite FAILED")
    r.add_argument("--when", help="ISO timestamp (defaults to now)")
    r.add_argument("--allow-extra", action="store_true", help="record a suite outside the required set")
    r.set_defaults(func=cmd_record)

    c = sub.add_parser("check", help="the gate: refuse a tag not validated at its commit")
    c.add_argument("--tag", required=True)
    c.add_argument("--commit", required=True, help="the commit the tag points at")
    c.add_argument("--require-ci", action="store_true", help="also require CI green on the commit (needs gh)")
    c.add_argument("--repo", default="ngolacloud/delonix", help="owner/name for the CI check")
    c.set_defaults(func=cmd_check)

    args = p.parse_args()
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
