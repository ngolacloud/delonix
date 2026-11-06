#!/usr/bin/env python3
"""Generate the table of contents of AGENTS.md, in place, between markers.

AGENTS.md is the append-only engineering log — ~10k lines, 124 `##` sections,
consulted BY SECTION, never end to end. Until now the only way to find a
section was to grep `^##`; an agent that read the whole file to orient itself
blew its context budget. This writes a navigable index at the top so a human
clicks and an agent reads one short block instead of grepping.

It follows the repo's generated-block convention (`dev-docs:begin` …): the index
lives between `<!-- agents-toc:begin -->` and `<!-- agents-toc:end -->`, is
regenerated from the headings, and `--check` fails when it drifts — so a new
section appended to the log without regenerating the index is caught in CI, the
same way a stale handbook fact is.

  python3 scripts/agents_toc.py           # rewrite the block in place
  python3 scripts/agents_toc.py --check   # exit 1 if the block is stale

stdlib only, like the other Python gates.
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
AGENTS = ROOT / "AGENTS.md"

BEGIN = "<!-- agents-toc:begin -->"
END = "<!-- agents-toc:end -->"

# GitHub's heading-anchor algorithm, reproduced so the links resolve on the
# rendered page: take the heading text, drop inline markdown (links → their
# text, backticks and emphasis removed), lowercase, remove every character that
# is not a word char (unicode letters/digits/underscore), whitespace or hyphen,
# turn spaces into hyphens, and disambiguate repeats with -1, -2, …
_LINK = re.compile(r"\[([^\]]+)\]\([^)]*\)")
_DROP = re.compile(r"[^\w\s-]", re.UNICODE)


def github_slug(text: str, seen: dict[str, int]) -> str:
    t = _LINK.sub(r"\1", text.strip())
    t = t.replace("`", "").replace("**", "").replace("*", "").replace("~~", "")
    s = _DROP.sub("", t.lower())
    s = s.replace(" ", "-")
    n = seen.get(s, 0)
    seen[s] = n + 1
    return s if n == 0 else f"{s}-{n}"


def section_titles(text: str) -> list[str]:
    """Every level-2 heading OUTSIDE a fenced code block and OUTSIDE the toc
    block itself (so the index never lists its own heading)."""
    titles: list[str] = []
    in_fence = False
    in_toc = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith(BEGIN):
            in_toc = True
            continue
        if stripped.startswith(END):
            in_toc = False
            continue
        if in_toc:
            continue
        if stripped.startswith("```") or stripped.startswith("~~~"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        if line.startswith("## "):
            titles.append(line[3:].strip())
    return titles


def render_toc(titles: list[str]) -> str:
    seen: dict[str, int] = {}
    lines = [BEGIN, "", f"## Índice ({len(titles)} secções)", ""]
    for title in titles:
        anchor = github_slug(title, seen)
        lines.append(f"- [{title}](#{anchor})")
    lines.append("")
    lines.append(END)
    return "\n".join(lines)


def splice(text: str, block: str) -> str:
    """Replace an existing toc block, or insert one right before the first
    `## ` heading (after the title + intro preamble)."""
    if BEGIN in text and END in text:
        pre, rest = text.split(BEGIN, 1)
        _, post = rest.split(END, 1)
        return pre + block + post
    # No block yet: insert before the first level-2 heading.
    lines = text.splitlines(keepends=True)
    for i, line in enumerate(lines):
        if line.startswith("## "):
            head = "".join(lines[:i])
            tail = "".join(lines[i:])
            sep_before = "" if head.endswith("\n\n") else ("\n" if head.endswith("\n") else "\n\n")
            return head + sep_before + block + "\n\n" + tail
    # Degenerate: no headings at all — append.
    return text.rstrip("\n") + "\n\n" + block + "\n"


def build(text: str) -> str:
    return splice(text, render_toc(section_titles(text)))


def main() -> int:
    p = argparse.ArgumentParser(description="Generate AGENTS.md's table of contents in place.")
    p.add_argument("--check", action="store_true", help="exit 1 if the toc block is stale")
    args = p.parse_args()

    text = AGENTS.read_text(encoding="utf-8")
    want = build(text)

    if args.check:
        if want != text:
            print(
                "AGENTS.md table of contents is stale — run: python3 scripts/agents_toc.py",
                file=sys.stderr,
            )
            return 1
        n = len(section_titles(text))
        print(f"ok    AGENTS.md toc: {n} section(s) indexed, up to date")
        return 0

    if want != text:
        AGENTS.write_text(want, encoding="utf-8")
        print(f"wrote AGENTS.md table of contents ({len(section_titles(want))} sections)")
    else:
        print("AGENTS.md table of contents already up to date")
    return 0


if __name__ == "__main__":
    sys.exit(main())
