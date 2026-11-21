#!/usr/bin/env python3
"""Recursive discovery of the real `delonix` CLI surface.

Walks `--help` from the root down to every leaf and emits a machine-readable
inventory: one record per command path, with its positionals, options, aliases,
arity, defaults, allowed values, env vars and the source of each rule.

Why its own walker instead of `scripts/cli-tree.sh`: that script emits paths
only. The matrix needs the PARAMETERS of each leaf, and the parameters are what
the help text spells out per node.

Cycles: `help` is never recursed into (it re-prints its parent), and every
canonical path is visited once.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from collections import deque

BIN = os.environ.get("DELONIX_BIN") or sys.argv[1]
TIMEOUT = 30

# Sections clap emits. Order of appearance is not guaranteed, so we key by name.
# Section headers are at column 0 and end in ":". The CLI uses custom
# spellings ("Lifecycle Commands:", "completion Commands:" — the latter
# lowercase, which a [A-Z]-anchored regex silently dropped, and with it the
# six leaves under `completion`/`manifest`).
SECTION_RE = re.compile(r"^([A-Za-z][A-Za-z0-9 _-]{0,38}):\s*$")


def run(argv: list[str]) -> tuple[int, str, str]:
    p = subprocess.run(
        [BIN] + argv,
        capture_output=True,
        text=True,
        timeout=TIMEOUT,
        env={**os.environ, "COLUMNS": "200", "NO_COLOR": "1", "DELONIX_L18N": "en"},
    )
    return p.returncode, p.stdout, p.stderr


def split_sections(text: str) -> dict[str, list[str]]:
    out: dict[str, list[str]] = {"_preamble": []}
    cur = "_preamble"
    for line in text.splitlines():
        m = SECTION_RE.match(line)
        if m:
            cur = m.group(1).strip()
            out.setdefault(cur, [])
            continue
        out[cur].append(line)
    return out


def join_entries(lines: list[str]) -> list[str]:
    """Clap wraps long descriptions. An entry starts at indent <= 6 with content;
    continuation lines are more deeply indented."""
    entries: list[str] = []
    for line in lines:
        if not line.strip():
            continue
        indent = len(line) - len(line.lstrip())
        if indent <= 6 and (
            line.lstrip().startswith("-") or re.match(r"^\s{0,6}\S", line)
        ):
            # Heuristic: a new entry begins when the first token looks like a
            # flag, an UPPERCASE positional, or a lowercase command name.
            tok = line.split()[0]
            if entries and not (
                tok.startswith("-")
                or re.fullmatch(r"<?[A-Z][A-Z0-9_\-]*>?\.{0,3}", tok)
                or re.fullmatch(r"\[?[a-z][a-z0-9\-]*\]?", tok)
            ):
                entries[-1] += " " + line.strip()
                continue
            entries.append(line.strip())
        else:
            if entries:
                entries[-1] += " " + line.strip()
    return entries


BRACKET_RE = re.compile(
    r"\[(env|default|possible values|aliases|alias|short aliases|short alias)\s*:\s*([^\]]*)\]"
)


def parse_brackets(desc: str) -> tuple[str, dict]:
    meta: dict = {}
    for m in BRACKET_RE.finditer(desc):
        key, val = m.group(1), m.group(2).strip()
        if key == "env":
            meta["env"] = val.rstrip("=")
        elif key == "default":
            meta["default"] = val
        elif key == "possible values":
            meta["possible_values"] = [v.strip() for v in val.split(",")]
        elif key in ("aliases", "alias", "short aliases", "short alias"):
            meta.setdefault("aliases", []).extend(v.strip() for v in val.split(","))
    clean = BRACKET_RE.sub("", desc).strip()
    return clean, meta


FLAG_RE = re.compile(
    r"^(?P<shorts>-[^-,\s][^,\s]*(?:,\s*-[^-,\s][^,\s]*)*)?"
    r"(?:,\s*)?"
    r"(?P<long>--[A-Za-z0-9][A-Za-z0-9\-]*)?"
    r"(?P<value>(?:[ =]<[^>]+>|\s+<[^>]+>)(?:\.\.\.)?)?"
    r"(?P<rest>.*)$"
)


def parse_option(entry: str) -> dict | None:
    m = FLAG_RE.match(entry)
    if not m or (not m.group("shorts") and not m.group("long")):
        return None
    shorts = []
    if m.group("shorts"):
        shorts = [s.strip() for s in m.group("shorts").split(",") if s.strip()]
    long = m.group("long")
    value = (m.group("value") or "").strip()
    desc, meta = parse_brackets(m.group("rest") or "")
    takes_value = bool(value)
    multiple = value.endswith("...") if value else entry.rstrip().endswith("...")
    rec = {
        "kind": "option",
        "short": shorts,
        "long": long,
        "value_name": value.strip(" =").strip("<>").rstrip(".") if takes_value else None,
        "takes_value": takes_value,
        "repeatable": multiple,
        "description": desc.strip(),
    }
    rec.update(meta)
    return rec


POS_RE = re.compile(r"^(?P<name><?\[?[A-Za-z][A-Za-z0-9_\-]*\]?>?)(?P<dots>\.\.\.)?\s{2,}(?P<rest>.*)$")


def parse_positional(entry: str) -> dict | None:
    m = POS_RE.match(entry)
    if not m:
        parts = re.split(r"\s{2,}", entry, maxsplit=1)
        if len(parts) == 1:
            name, rest = parts[0], ""
        else:
            name, rest = parts
        if name.startswith("-"):
            return None
        m_dots = name.endswith("...")
        name = name.rstrip(".")
        desc, meta = parse_brackets(rest)
        rec = {
            "kind": "positional",
            "name": name.strip("<>[]"),
            "optional": name.startswith("[") or name.startswith("<") is False,
            "repeatable": m_dots,
            "description": desc.strip(),
        }
        rec.update(meta)
        return rec
    name = m.group("name")
    desc, meta = parse_brackets(m.group("rest"))
    rec = {
        "kind": "positional",
        "name": name.strip("<>[]"),
        "optional": name.startswith("["),
        "repeatable": bool(m.group("dots")),
        "description": desc.strip(),
    }
    rec.update(meta)
    return rec


def parse_command(entry: str) -> dict | None:
    parts = re.split(r"\s{2,}", entry, maxsplit=1)
    head = parts[0].strip()
    desc = parts[1].strip() if len(parts) > 1 else ""
    if head.startswith("-"):
        return None
    names = [n.strip() for n in head.split(",") if n.strip()]
    if not names or not re.fullmatch(r"[a-z][a-z0-9\-]*", names[0]):
        return None
    desc, meta = parse_brackets(desc)
    rec = {"name": names[0], "visible_aliases": names[1:], "description": desc}
    rec.update(meta)
    return rec


def main() -> None:
    rc, out, err = run(["--version"])
    version_text = out.strip() or err.strip()

    inventory: dict[str, dict] = {}
    q: deque[list[str]] = deque([[]])
    seen: set[tuple[str, ...]] = set()

    while q:
        path = q.popleft()
        key = tuple(path)
        if key in seen:
            continue
        seen.add(key)

        rc, out, err = run(path + ["--help"])
        text = out if out.strip() else err
        secs = split_sections(text)

        usage = ""
        for line in secs.get("Usage", []) + secs.get("_preamble", []):
            if line.strip().startswith("delonix") or "Usage:" in line:
                usage = line.strip().replace("Usage:", "").strip()
                break
        if not usage:
            for name in ("Usage", "USAGE"):
                if name in secs and secs[name]:
                    usage = " ".join(l.strip() for l in secs[name] if l.strip())
                    break

        commands: list[dict] = []
        for sec_name in list(secs):
            if sec_name.endswith("Commands") or sec_name in ("Subcommands", "Commands"):
                for e in join_entries(secs[sec_name]):
                    c = parse_command(e)
                    if c:
                        commands.append(c)

        options: list[dict] = []
        positionals: list[dict] = []
        for sec_name in list(secs):
            low = sec_name.lower()
            if "option" in low or low in ("flags", "global options"):
                for e in join_entries(secs[sec_name]):
                    o = parse_option(e)
                    if o:
                        o["section"] = sec_name
                        options.append(o)
            elif low in ("arguments", "args"):
                for e in join_entries(secs[sec_name]):
                    p = parse_positional(e)
                    if p:
                        positionals.append(p)

        node = {
            "path": path,
            "path_str": " ".join(path) if path else "(root)",
            "usage": usage,
            "help_rc": rc,
            "help_on_stderr": not out.strip() and bool(err.strip()),
            "commands": [c["name"] for c in commands],
            "command_detail": commands,
            "options": options,
            "positionals": positionals,
            "is_leaf": not commands,
            "help_bytes": len(text),
        }
        inventory[node["path_str"]] = node

        for c in commands:
            if c["name"] == "help":
                continue
            q.append(path + [c["name"]])

    result = {
        "binary": os.path.realpath(BIN),
        "version_text": version_text,
        "nodes": inventory,
        "totals": {
            "paths": len(inventory),
            "leaves": sum(1 for n in inventory.values() if n["is_leaf"]),
            "groups": sum(1 for n in inventory.values() if not n["is_leaf"]),
            "options": sum(len(n["options"]) for n in inventory.values()),
            "positionals": sum(len(n["positionals"]) for n in inventory.values()),
        },
    }
    json.dump(result, sys.stdout, indent=1, sort_keys=False)
    print()


if __name__ == "__main__":
    main()
