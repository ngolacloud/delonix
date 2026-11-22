#!/usr/bin/env python3
"""Generate docs/compatibility.md from the tests/interop/ evidence records.

The campaign (mission H) proves Delonix's interoperability with the CNCF ecosystem
one measured claim at a time. Each claim is a JSON record under
`tests/interop/results/*.jsonl` (schema: `tests/interop/schema.json`). This script
reads every record, enforces the doctrine the schema encodes, and renders the
`docs/compatibility.md` matrix (tool / version / mode / result / proof) followed by
the contract's limits (`tests/interop/LIMITS.md`).

Doctrine enforced here, fail-closed:
  * a PASS record (a "compatible" claim) must name the tool version, the Delonix
    version and an `artifact_ref` that proves it;
  * PARTIAL and FAIL must carry an `artifact_ref`;
  * every `artifact_ref` must point at a path that exists in the tree;
  * without a proof the state is NOT_TESTED — never "compatible".

Output is a pure function of the input files: there is no generation timestamp, so
the drift gate (`--check`) compares a deterministic render against the committed doc.

Usage:
    python3 scripts/gen_compatibility.py            # write docs/compatibility.md
    python3 scripts/gen_compatibility.py --check     # validate + fail on drift (CI)
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RESULTS_DIR = ROOT / "tests" / "interop" / "results"
LIMITS_SRC = ROOT / "tests" / "interop" / "LIMITS.md"
OUT = ROOT / "docs" / "compatibility.md"

# Mirror of tests/interop/schema.json (that file stays the canonical spec; this keeps
# the generator dependency-free, matching the other pure-Python gates).
PHASES = ["H0", "H1", "H2", "H3", "H4", "H5", "H6", "H7"]
STATES = ["PASS", "PARTIAL", "FAIL", "NOT_TESTED"]
PRIVILEGES = ["rootless", "root"]
CAPS = [
    "privileged", "hostNetwork", "hostPID", "hostPath",
    "mountPropagation", "eBPF", "capabilities", "deviceAccess",
]
KNOWN_KEYS = {
    "tool", "tool_version", "delonix_version", "mode", "phase", "state",
    "privilege", "capabilities", "environment", "command", "artifact_ref",
    "sha", "notes",
}

PHASE_TITLE = {
    "H0": "H0 — SPIFFE identity",
    "H1": "H1 — Kubernetes (certifiable, Mode 1)",
    "H2": "H2 — Demanding DaemonSets (Mode 1)",
    "H3": "H3 — Delonix under containerd / CRI-O",
    "H4": "H4 — Observability (Modes 1 and 2)",
    "H5": "H5 — Policy and runtime security",
    "H6": "H6 — Registry and supply chain",
    "H7": "H7 — Interoperability without Kubernetes (Mode 2)",
}

STATE_CELL = {
    "PASS": "✅ pass",
    "PARTIAL": "◑ partial",
    "FAIL": "✗ fail",
    "NOT_TESTED": "☐ not tested",
}


def _is_kebab(s: str) -> bool:
    return bool(s) and all(c.isalnum() or c == "-" for c in s) and s[0].isalnum()


def validate(rec: dict, where: str, errors: list[str]) -> None:
    """Append one message per rule the record breaks (schema.json, in code)."""
    def err(msg: str) -> None:
        errors.append(f"{where}: {msg}")

    extra = set(rec) - KNOWN_KEYS
    if extra:
        err(f"unknown field(s) {sorted(extra)}")

    for field in ("tool", "mode", "phase", "state"):
        if field not in rec:
            err(f"missing required field {field!r}")

    tool = rec.get("tool")
    if isinstance(tool, str) and not _is_kebab(tool):
        err(f"tool {tool!r} is not lower-kebab")

    if rec.get("mode") not in (1, 2):
        err(f"mode must be 1 or 2, got {rec.get('mode')!r}")
    if rec.get("phase") not in PHASES:
        err(f"phase must be one of {PHASES}, got {rec.get('phase')!r}")
    state = rec.get("state")
    if state not in STATES:
        err(f"state must be one of {STATES}, got {state!r}")
    if "privilege" in rec and rec["privilege"] not in PRIVILEGES:
        err(f"privilege must be one of {PRIVILEGES}, got {rec['privilege']!r}")

    caps = rec.get("capabilities")
    if caps is not None:
        if not isinstance(caps, list):
            err("capabilities must be a list")
        else:
            bad = [c for c in caps if c not in CAPS]
            if bad:
                err(f"unknown capabilities {bad}")

    # The doctrine: what each state must carry.
    if state == "PASS":
        for field in ("tool_version", "delonix_version", "artifact_ref"):
            if not rec.get(field):
                err(f"PASS requires {field!r} (a 'compatible' claim needs the version and a proof)")
    elif state in ("PARTIAL", "FAIL"):
        if not rec.get("artifact_ref"):
            err(f"{state} requires an 'artifact_ref' (there is a measurement)")
    elif state == "NOT_TESTED":
        if rec.get("artifact_ref"):
            err("NOT_TESTED must not carry an 'artifact_ref' — mark it PARTIAL/FAIL/PASS")

    ref = rec.get("artifact_ref")
    if ref:
        if (ROOT / ref).exists():
            pass
        else:
            err(f"artifact_ref {ref!r} does not exist in the tree")


def load_records(errors: list[str]) -> list[dict]:
    records: list[dict] = []
    if not RESULTS_DIR.is_dir():
        errors.append(f"{RESULTS_DIR} is missing")
        return records
    for path in sorted(RESULTS_DIR.glob("*.jsonl")):
        rel = path.relative_to(ROOT)
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except json.JSONDecodeError as exc:
                errors.append(f"{rel}:{n}: invalid JSON — {exc}")
                continue
            validate(rec, f"{rel}:{n}", errors)
            records.append(rec)
    return records


def _proof_cell(rec: dict) -> str:
    ref = rec.get("artifact_ref")
    if not ref:
        return "—"
    # docs/compatibility.md -> a repo-root path is one level up.
    return f"[{ref}](../{ref})"


def _row(rec: dict) -> str:
    cells = [
        f"`{rec['tool']}`",
        rec.get("tool_version", "—"),
        rec.get("delonix_version", "—"),
        f"{rec['mode']} ({'K8s' if rec['mode'] == 1 else 'no-K8s'})",
        rec.get("privilege", "—"),
        ", ".join(rec.get("capabilities", [])) or "—",
        STATE_CELL[rec["state"]],
        _proof_cell(rec),
    ]
    return "| " + " | ".join(cells) + " |"


def render(records: list[dict]) -> str:
    lines: list[str] = []
    lines.append("<!--")
    lines.append("  GENERATED FILE — DO NOT EDIT BY HAND.")
    lines.append("  Source: tests/interop/results/*.jsonl + tests/interop/LIMITS.md.")
    lines.append("  Regenerate: python3 scripts/gen_compatibility.py")
    lines.append("-->")
    lines.append("")
    lines.append("# Delonix — CNCF ecosystem compatibility")
    lines.append("")
    lines.append(
        "This matrix is **generated** from the measured interoperability records in "
        "`tests/interop/`. Every cell is a claim with a tool version, a mode and a "
        "proof — or it says **not tested**. A claim is never written as \"compatible\" "
        "without a test that proves it (engine doctrine; see `AGENTS.md`, "
        "\"Engine identity and boundary\")."
    )
    lines.append("")
    lines.append("**Modes.** Mode 1 = Delonix *under* a real Kubernetes, via the CRI. "
                 "Mode 2 = Delonix *without* Kubernetes, open interfaces only.")
    lines.append("")
    lines.append("**Result.** ✅ pass (proven) · ◑ partial (measured, incomplete) · "
                 "✗ fail (measured not to work) · ☐ not tested (no proof yet).")
    lines.append("")

    counts = {s: sum(1 for r in records if r["state"] == s) for s in STATES}
    lines.append(
        "**Totals:** "
        + " · ".join(f"{STATE_CELL[s]}: {counts[s]}" for s in STATES)
        + f" · records: {len(records)}"
    )
    lines.append("")

    header = (
        "| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |"
    )
    sep = "|---|---|---|---|---|---|---|---|"

    for phase in PHASES:
        rows = [r for r in records if r["phase"] == phase]
        if not rows:
            continue
        rows.sort(key=lambda r: (r["tool"], r["mode"]))
        lines.append(f"## {PHASE_TITLE[phase]}")
        lines.append("")
        lines.append(header)
        lines.append(sep)
        for rec in rows:
            lines.append(_row(rec))
        lines.append("")
        notes = [(r["tool"], r["notes"]) for r in rows if r.get("notes")]
        if notes:
            lines.append("<details><summary>Notes</summary>")
            lines.append("")
            for tool, note in notes:
                lines.append(f"- **`{tool}`** — {note}")
            lines.append("")
            lines.append("</details>")
            lines.append("")

    lines.append("## Limits (part of the contract)")
    lines.append("")
    limits = LIMITS_SRC.read_text(encoding="utf-8")
    # Strip the leading HTML comment (the editing note); keep the prose.
    if limits.lstrip().startswith("<!--"):
        limits = limits.split("-->", 1)[1]
    lines.append(limits.strip())
    lines.append("")

    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    check = "--check" in argv[1:]

    errors: list[str] = []
    records = load_records(errors)
    if errors:
        print("tests/interop records are invalid:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    rendered = render(records)

    if check:
        current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if current != rendered:
            print(
                "::error::docs/compatibility.md is out of date — run "
                "'python3 scripts/gen_compatibility.py' and commit.",
                file=sys.stderr,
            )
            return 1
        print("docs/compatibility.md is up to date "
              f"({len(records)} interop records).")
        return 0

    OUT.write_text(rendered, encoding="utf-8")
    print(f"wrote {OUT.relative_to(ROOT)} from {len(records)} interop records")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
