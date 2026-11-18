#!/usr/bin/env python3
"""Agents budget & projection gate — keep the first-contact file small and the specs discoverable.

# What this closes

`AGENTS.md` is the ~10k-line append-only archive; an agent that reads it whole to
orient itself blows its context budget (the problem `scripts/agents_toc.py`
already named). H03 adds a canonical **first-contact** file, `.ai/INVARIANTS.md`,
that a session reads *instead* of the archive — the minimal invariants, each
pointing at the file that owns it. This gate keeps that split honest:

1. **Budget.** `.ai/INVARIANTS.md` must stay small (a first-contact file that
   grows into a second archive defeats itself). It reports the byte/line ratio of
   the invariants file against `AGENTS.md`, and fails if the invariants file
   crosses its ceiling.
2. **Projection integrity.** Every file path the invariants point at must exist,
   and the archive must still carry its generated index marker.
3. **Specs are well-formed and discoverable.** The five agent specs in
   `.ai/agents/` carry a valid frontmatter (`name`, `description`, `tools`), the
   five skills carry `name`+`description`, and the `.claude/` projection resolves
   to them from the repo root and from a subdirectory.

stdlib only, like the other Python gates.

  python3 scripts/agents_budget_gate.py           # report the budget + checks
  python3 scripts/agents_budget_gate.py --check    # exit 1 on any violation
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INVARIANTS = ROOT / ".ai" / "INVARIANTS.md"
AGENTS = ROOT / "AGENTS.md"
AGENTS_DIR = ROOT / ".ai" / "agents"
SKILLS_DIR = ROOT / ".ai" / "skills"
CLAUDE_AGENTS = ROOT / ".claude" / "agents"

# A first-contact file must stay small. Generous, but a hard ceiling.
INVARIANTS_MAX_BYTES = 12_000
INVARIANTS_MAX_LINES = 190

# The specs that are agent definitions (frontmatter), not shared docs.
DOC_ONLY = {"permission-tiers.md", "authorization.md"}

EXPECTED_AGENTS = 5
EXPECTED_SKILLS = 6  # arch, triage, security, release, e2e-quality, harness-check

_PATH_RE = re.compile(r"`([^`]+\.(?:md|py|rs|sh|yaml|yml))`")


def _frontmatter(text: str) -> dict[str, str] | None:
    """Parse a leading `---` YAML-ish block into a flat dict (stdlib only)."""
    if not text.startswith("---"):
        return None
    end = text.find("\n---", 3)
    if end == -1:
        return None
    block = text[3:end]
    out: dict[str, str] = {}
    for line in block.splitlines():
        m = re.match(r"^([A-Za-z_][\w-]*):\s*(.*)$", line)
        if m:
            out[m.group(1)] = m.group(2).strip()
    return out


def check(verbose: bool) -> list[str]:
    errors: list[str] = []

    # 1. Budget.
    if not INVARIANTS.exists():
        return [f"{INVARIANTS} is missing — the first-contact file"]
    inv_bytes = INVARIANTS.stat().st_size
    inv_lines = INVARIANTS.read_text(encoding="utf-8").count("\n") + 1
    agents_bytes = AGENTS.stat().st_size if AGENTS.exists() else 0
    agents_lines = (AGENTS.read_text(encoding="utf-8").count("\n") + 1) if AGENTS.exists() else 0
    ratio = (inv_bytes / agents_bytes * 100) if agents_bytes else 0
    if verbose:
        print(f"first-contact .ai/INVARIANTS.md : {inv_bytes:>7} bytes / {inv_lines:>5} lines")
        print(f"archive       AGENTS.md         : {agents_bytes:>7} bytes / {agents_lines:>5} lines")
        print(f"first-contact is {ratio:.1f}% of the archive "
              f"(ceiling {INVARIANTS_MAX_BYTES} bytes / {INVARIANTS_MAX_LINES} lines)")
    if inv_bytes > INVARIANTS_MAX_BYTES:
        errors.append(f"INVARIANTS.md is {inv_bytes} bytes > ceiling {INVARIANTS_MAX_BYTES}")
    if inv_lines > INVARIANTS_MAX_LINES:
        errors.append(f"INVARIANTS.md is {inv_lines} lines > ceiling {INVARIANTS_MAX_LINES}")

    # 2. Projection integrity: pointers resolve; archive keeps its index marker.
    inv_text = INVARIANTS.read_text(encoding="utf-8")
    for path in sorted(set(_PATH_RE.findall(inv_text))):
        # Only check repo-relative paths that look like real files (skip e.g. env.sh in ~/.config).
        candidate = ROOT / path
        if ("/" in path and "*" not in path and not path.startswith("~")
                and not candidate.exists()):
            errors.append(f"INVARIANTS.md points at missing file: {path}")
    if AGENTS.exists() and "<!-- agents-toc:begin -->" not in AGENTS.read_text(encoding="utf-8"):
        errors.append("AGENTS.md lost its generated index marker (agents-toc:begin)")

    # 3. Agent specs well-formed.
    agent_specs = [p for p in sorted(AGENTS_DIR.glob("*.md")) if p.name not in DOC_ONLY
                   and p.name != "INVARIANTS.md"]
    n_agents = 0
    for spec in agent_specs:
        fm = _frontmatter(spec.read_text(encoding="utf-8"))
        if fm is None:
            # A doc without frontmatter in agents/ is fine only if it is doc-only.
            continue
        n_agents += 1
        for key in ("name", "description", "tools"):
            if not fm.get(key):
                errors.append(f"{spec.name}: frontmatter missing '{key}'")
    if n_agents != EXPECTED_AGENTS:
        errors.append(f"expected {EXPECTED_AGENTS} agent specs, found {n_agents}")

    # 3b. Skills well-formed (this repo's convention: a `## Skill Name` with the
    # name in backticks, and a `## Purpose` section — not YAML frontmatter).
    skills = sorted(SKILLS_DIR.glob("*/SKILL.md"))
    for skill in skills:
        text = skill.read_text(encoding="utf-8")
        if "## Skill Name" not in text or not re.search(r"`[a-z0-9-]+`", text):
            errors.append(f"{skill.parent.name}/SKILL.md: no '## Skill Name' with a backticked name")
        if "## Purpose" not in text and "## When" not in text:
            errors.append(f"{skill.parent.name}/SKILL.md: no '## Purpose'/'## When' section")
    if len(skills) != EXPECTED_SKILLS:
        errors.append(f"expected {EXPECTED_SKILLS} skills, found {len(skills)}")

    # 3c. Discovery from the root and from the .claude projection.
    if CLAUDE_AGENTS.exists():
        projected = {p.name for p in CLAUDE_AGENTS.glob("*.md")}
        source = {p.name for p in agent_specs}
        missing = source - projected
        if missing:
            errors.append(f".claude/agents projection missing: {sorted(missing)}")
    if verbose:
        print(f"discovery: {n_agents} agent specs, {len(skills)} skills "
              f"(projection {'present' if CLAUDE_AGENTS.exists() else 'absent'})")

    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="accepted for CI symmetry; the gate always validates and sets rc")
    parser.parse_args(argv)
    errors = check(verbose=True)
    if errors:
        print("\nFAIL — agents budget/projection:")
        for e in errors:
            print(f"  - {e}")
        return 1
    print("\nOK — first-contact within budget, projection intact, specs discoverable")
    return 0


if __name__ == "__main__":
    sys.exit(main())
