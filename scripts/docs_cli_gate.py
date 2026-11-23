#!/usr/bin/env python3
"""Documentation gate: every `delonix ...` cited in the CURRENT documentation
must resolve against the binary's tree.

It exists because two back-to-back command removals (v2.0.0 and v3.0.0) slipped
through unnoticed: eight current documentation files kept teaching `delonix pod
ls`, `delonix vm status` and `delonix schema print`, all answering
`unrecognized subcommand` in the published binary. A reader who copies the line
off the page hits the error; the gate hits it first.

The authoritative list of paths comes from `scripts/cli-tree.sh`, which walks
the `--help` of the TREE's binary. It is not measured against the `delonix`
installed on the host: this finding was first measured that way and came from a
version behind.

    python3 scripts/docs_cli_gate.py            # checks (exit 1 if there are orphans)
    python3 scripts/docs_cli_gate.py --list     # shows each citation and where it is

What is NOT checked, and why:

  * `docs/releases/*.md`, `docs/RELEASES.md`, `docs/AUDITORIA-E2E.md` and
    `docs/discovery/*` are dated historical record. It is CORRECT for them to
    cite the spelling of the version they talk about — rewriting them would
    erase the record.
  * The five audit/measurement reports in `docs/` (`COMPARACAO-DOCKER-
    PODMAN.md`, `paridade-docker-podman.md`, `measured-comparison.md`,
    `RELATORIO-PRE-PRODUCAO.md`, `cri-conformance.md`) are the SAME class —
    each already carries a date and version in its own header, and none is
    rewritten on every release. They stay published (`comparacao.html`/
    `cri.html`/`AGENTS.md`/`cli-stability.md` link to them as the detail behind
    a summary), they just are not "current documentation" for this gate's
    purposes.
  * Citations are only extracted from CODE CONTEXT (`<code>`, `<pre>`, markdown
    fences, reST `::`, YAML comments). Prose like "delonix is an engine" is not
    a command citation and no attempt is made to resolve it.
"""

from __future__ import annotations

import argparse
import html
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLI_TREE = ROOT / "scripts" / "cli-tree.sh"
ALLOW = Path(__file__).resolve().parent / "docs_cli_allow.tsv"

# Dated historical record — see the module docstring.
EXCLUDE = (
    "docs/releases/",
    "docs/RELEASES.md",
    "docs/AUDITORIA-E2E.md",
    "docs/discovery/",
    # Dated audit/measurement reports — see the note in the module docstring.
    # Measured 2026-09-11: none produced an orphan citation today; the
    # exclusion is a preventive categorisation, not the fix for an active
    # failure — but they belong to the same class as the four above, and not to
    # the "current documentation" this gate keeps in sync with the binary.
    "docs/COMPARACAO-DOCKER-PODMAN.md",
    "docs/paridade-docker-podman.md",
    "docs/measured-comparison.md",
    "docs/RELATORIO-PRE-PRODUCAO.md",
    "docs/cri-conformance.md",
)

TARGETS = (
    "docs/*.html",
    "docs/comandos/*.html",
    "docs/*.md",
    "docs/roadmap/*.md",
    "docs/runtime/*.md",
    # The contributor handbook: a stale command there teaches every new
    # contributor the wrong CLI on day one.
    "docs/dev/*.md",
    # ...and its translations. Only the English root was scanned, so when #772
    # removed `vm build`, `docs/dev/delonixfile-and-vmfile.md` was fixed and its
    # pt-AO/fr-FR/zh-CN copies kept teaching the removed spelling.
    "docs/dev/*/*.md",
    # The image recipes and the appliance scripts: each README/`vm.yaml` header
    # is the command a reader copies to build that image. Measured 2026-11-23:
    # 30+ of them still said `delonix vm build`, which the binary answers with
    # `unrecognized subcommand 'build'` (exit 2), and none was in this list.
    "images/*.md",
    "images/**/*.md",
    "images/**/*.yaml",
    "scripts/**/*.md",
    "CONTRIBUTING.md",
    "examples/*.md",
    "examples/*.yaml",
    "examples/**/*.yaml",
    "examples/**/*.md",
    "README.rst",
)

# A token starting with `-`, `$`, `<`, `{` or an upper-case letter is no longer a
# subcommand: it is a flag, a shell substitution or a placeholder.
ARG = re.compile(r"^[a-z][a-z0-9-]*$")

# `[^\S\n]` and not `\s`: an invocation lives on ONE line. With `\s` the regex
# crossed the newline and glued the `delonix` ending one line to the first token
# of the next — that is how citations of `delonix echo` and `delonix delonix`,
# which nobody wrote, showed up.
INVOCATION = re.compile(
    r"\bdelonix[^\S\n]+((?:[A-Za-z0-9$<{./_-][^\s|;&>)\"']*)"
    r"(?:[^\S\n]+[^\s|;&>)\"']+)*)"
)


def paths_from_binary() -> tuple[set[str], set[str]]:
    """The tree's paths, split into nodes and leaves.

    The distinction is not cosmetic: a NODE (`net tunnel`) only accepts
    subcommands, and a LEAF (`get`) accepts free arguments. Without it, `delonix
    net tunnel ls` resolved via the `net tunnel` prefix and the `ls` — removed —
    slipped through unnoticed.
    """
    out = subprocess.run(
        ["bash", str(CLI_TREE)], capture_output=True, text=True, cwd=ROOT
    )
    if out.returncode != 0:
        sys.stderr.write(out.stderr)
        sys.exit("cli-tree.sh failed — build the tree's binary first")
    nodes: set[str] = set()
    leaves: set[str] = set()
    for line in out.stdout.splitlines():
        if "|" not in line:
            continue
        kind, path = line.split("|", 1)
        (leaves if kind.strip() == "LEAF" else nodes).add(path.strip())
    return nodes, leaves


def code_regions(path: Path) -> list[str]:
    """Returns only the parts of the file that are code context."""
    text = path.read_text(encoding="utf-8", errors="replace")
    suffix = path.suffix.lower()
    regions: list[str] = []

    if suffix in {".html", ".htm"}:
        for m in re.finditer(r"<(code|pre)\b[^>]*>(.*?)</\1>", text, re.S | re.I):
            regions.append(html.unescape(re.sub(r"<[^>]+>", "", m.group(2))))
    elif suffix in {".md", ".markdown"}:
        for m in re.finditer(r"^```.*?^```", text, re.S | re.M):
            regions.append(m.group(0))
        # Indented code blocks (4 spaces), not just fences: `docs/
        # guia-nas-vm-sdn-ch.md` writes every command that way and slipped past the
        # first version of this gate entirely.
        for m in re.finditer(r"(?:^(?: {4}|\t).*\n)+", text, re.M):
            regions.append(m.group(0))
        for m in re.finditer(r"`([^`\n]+)`", text):
            regions.append(m.group(1))
    elif suffix == ".rst":
        for m in re.finditer(r"``([^`]+)``", text):
            regions.append(m.group(1))
        # reST literal blocks: `::` followed by indented lines
        for m in re.finditer(r"::\n\n((?:[ \t]+\S.*\n|\n)+)", text):
            regions.append(m.group(1))
    elif suffix in {".yaml", ".yml"}:
        for line in text.splitlines():
            if line.lstrip().startswith("#"):
                regions.append(line)
    else:
        regions.append(text)
    return regions


def citations(path: Path):
    """(normalized_command, file_line) for each citation."""
    raw = path.read_text(encoding="utf-8", errors="replace")
    for region in code_regions(path):
        for m in INVOCATION.finditer(region):
            tokens = []
            for tok in m.group(1).split():
                if not ARG.match(tok):
                    break
                tokens.append(tok)
            if not tokens:
                continue
            snippet = "delonix " + " ".join(tokens)
            line = 0
            idx = raw.find(snippet)
            if idx >= 0:
                line = raw.count("\n", 0, idx) + 1
            yield tokens, line


def binary_accepts(path: str, cache: dict[str, bool]) -> bool:
    """Asks the binary itself whether this path exists.

    The `cli-tree.sh` tree reads the `Commands:` section of `--help`, which
    lists the CANONICAL names and not the ALIASES — `container ls` is `container
    ps` and does not appear there. Without this fallback, every documented alias
    came out as an orphan.
    """
    if path not in cache:
        r = subprocess.run(
            [str(binary()), *path.split(), "--help"], capture_output=True, text=True
        )
        cache[path] = r.returncode == 0
    return cache[path]


def binary() -> Path:
    for c in (ROOT / "target/release/delonix", ROOT / "target/debug/delonix"):
        if c.is_file():
            return c
    sys.exit("no binary: run `cargo build --release -p delonix-bin`")


def resolve(
    tokens: list[str],
    nodes: set[str],
    leaves: set[str],
    cache: dict[str, bool] | None = None,
) -> str | None:
    """Descends the tree token by token. Returns the path reached, or None.

    Three outcomes:
      * lands on a LEAF — the rest are arguments (`delonix get pods` → `get`);
      * stays on a NODE and the citation ends there (`delonix vm --help`) — valid;
      * stays on a NODE with one more token that is NOT a subcommand of it —
        does not resolve, and this is the case that catches `net tunnel ls` and
        `vm status`.
    """
    path: list[str] = []
    for tok in tokens:
        candidate = " ".join(path + [tok])
        if candidate in leaves:
            return candidate
        if candidate in nodes:
            path.append(tok)
            continue
        # The token is not a child of the node we are standing on. A node takes
        # subcommands and nothing else, so there is no reading of this that
        # resolves — including the removed-subcommand case this gate exists for.
        # One last authority before failing: the tree lists canonical names, so
        # a documented ALIAS (`container ls` for `container ps`) is missing from
        # it and only the binary can confirm it.
        if cache is not None and binary_accepts(candidate, cache):
            return candidate
        return None
    return " ".join(path) if path else None


def allowed() -> dict[tuple[str, str], str]:
    """The deliberate exceptions, each with its reason.

    It is a list of EXCEPTIONS and not a heuristic filter on purpose: prose like
    "delonix picks a free one" and a migration note like "the original `delonix
    restore` now…" are indistinguishable from a removed command without someone
    reading the sentence. Whoever adds a line has to write down why, and a new
    citation that teaches running a dead command still fails.
    """
    if not ALLOW.exists():
        return {}
    out: dict[tuple[str, str], str] = {}
    for raw in ALLOW.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        if len(parts) < 3:
            sys.exit(f"docs_cli_allow.tsv: 3 TAB-separated columns — {raw!r}")
        out[(parts[0], parts[1])] = parts[2]
    return out


def collect() -> list[Path]:
    seen: list[Path] = []
    for pattern in TARGETS:
        for p in sorted(ROOT.glob(pattern)):
            rel = p.relative_to(ROOT).as_posix()
            if any(rel.startswith(x) for x in EXCLUDE):
                continue
            if p.is_file() and p not in seen:
                seen.append(p)
    return seen


def readme_group_table() -> list[str]:
    """The `Group` column of README.rst's «Command groups» list-table.

    Each cell is a code-span (``x``) or a couple joined by `·`; what comes back
    is the flat set of group names the table claims to document.
    """
    readme = ROOT / "README.rst"
    if not readme.exists():
        return []
    text = readme.read_text(encoding="utf-8", errors="replace")
    try:
        table = text.split("Command groups", 1)[1].split("\nLanguages", 1)[0]
    except IndexError:
        return []
    names: list[str] = []
    for line in table.splitlines():
        stripped = line.strip()
        # Only the FIRST column of a row (`   * - ``x``` ), never the prose in
        # the second: that one legitimately names subcommands and flags.
        if not stripped.startswith("* - "):
            continue
        names.extend(re.findall(r"``([a-z][a-z0-9-]*)``", stripped))
    return names


def check_readme_groups(nodes: set[str], leaves: set[str]) -> list[str]:
    """**Every top-level group is in the README table, and the table invents none.**

    The `delonix ...` citation check above never saw this: a table cell holding
    ```volumes``` is not a command line, so nothing tried to resolve it. Measured
    2026-09-10, with the gate green: the table still documented five groups that
    the v2/v3 restructuring had removed (`volumes`, `storage`, `sharevolume`,
    `schema`, `dash`) and promised six subcommands that were cut with them
    (`pod ls/describe/rm`, `vm status`, `image --vm`, `net boot`) — while
    fourteen real groups, the generic verbs among them, were missing entirely.

    A reader picking a group name off that table typed a command the binary
    answers `unrecognized subcommand` to, which is the exact failure the rest of
    this gate exists to prevent.
    """
    tops = {p.split()[0] for p in list(nodes) + list(leaves) if p}
    claimed = set(readme_group_table())
    problems = []
    for name in sorted(claimed - tops):
        problems.append(f"README.rst cites the group `{name}`, which the binary does not have")
    for name in sorted(tops - claimed):
        problems.append(f"README.rst does not document the group `{name}`")
    return problems


def site_group_keys() -> set[str]:
    """The real top-level command each `docs/gen.py` page documents.

    Imports `docs/gen.py` as a module (its only module-level work is defining
    dicts/functions and reading `sys.argv[1]` into a path — nothing runs the
    binary until a function is actually called) so this reads the REAL
    `GROUPS`/`GROUP_PATH`, never a second, hand-parsed copy of them that could
    itself drift from the file it is meant to check.

    A `GROUPS` key whose `GROUP_PATH` has more than one element is a NESTED
    CHILD of another group (`ingress` → `net ingress`, `boot` → `system
    boot`) — it is not supposed to have a top-level page of its own, so it is
    excluded here rather than compared against the top-level command list.
    The one single-element alias in `GROUP_PATH` (`dash` → `dashboard`) is
    resolved to the real name it documents.
    """
    import importlib.util

    spec = importlib.util.spec_from_file_location("delonix_docs_gen", ROOT / "docs" / "gen.py")
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    tops = set()
    for key in mod.GROUPS:
        path = mod.GROUP_PATH.get(key, (key,))
        if len(path) > 1:
            continue
        tops.add(path[0])
    return tops


def check_site_groups(nodes: set[str], leaves: set[str]) -> list[str]:
    """**Every top-level group has a site page, and `docs/gen.py`'s `GROUPS` invents none.**

    Same shape as `check_readme_groups`, aimed at `docs/gen.py`'s own
    hand-authored taxonomy instead of README.rst's. Measured 2026-09-11, with
    `check_readme_groups` GREEN the whole time (it only reads README.rst):
    14 of the 33 real groups — the entire generic-verb half of the CLI, plus
    `net`/`serve` overview pages and `net l4guard` — had no page anywhere on
    the site. The citation-resolution gate above never saw this either: a
    missing PAGE produces no orphan citation, because there is no citation to
    check in a page that was never written.
    """
    tops = {p.split()[0] for p in list(nodes) + list(leaves) if p}
    documented = site_group_keys()
    problems = []
    for name in sorted(documented - tops):
        problems.append(f"docs/gen.py's GROUPS documents `{name}`, which the binary does not have")
    for name in sorted(tops - documented):
        problems.append(f"docs/gen.py's GROUPS has no page for `{name}`")
    return problems


TRANSLATION = re.compile(r"^docs/dev/[a-z]{2}-[A-Z]{2}/([^/]+)$")


def translation_source(rel: str) -> str:
    """`docs/dev/pt-AO/x.md` → `docs/dev/x.md`; anything else → itself.

    A translation of a DELIBERATE citation (a migration note whose subject is
    that the spelling no longer exists) is deliberate too, so it inherits the
    English file's exception instead of each language repeating the line. What
    it does NOT inherit is a citation the English file does not have — which is
    exactly how a translation that kept teaching a removed verb gets caught.
    """
    m = TRANSLATION.match(rel)
    return f"docs/dev/{m.group(1)}" if m else rel


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--list", action="store_true", help="show each resolved citation")
    args = ap.parse_args()

    nodes, leaves = paths_from_binary()
    allow = allowed()
    cache: dict[str, bool] = {}
    orphans: list[tuple[str, int, str]] = []
    used: set[tuple[str, str]] = set()
    total = 0

    for path in collect():
        rel = path.relative_to(ROOT).as_posix()
        for tokens, line in citations(path):
            total += 1
            cmd = "delonix " + " ".join(tokens)
            hit = resolve(tokens, nodes, leaves, cache)
            if hit is not None:
                if args.list:
                    print(f"ok    {rel}:{line}\t{cmd}\t→ {hit}")
                continue
            key = next(
                (k for k in ((rel, cmd), (translation_source(rel), cmd)) if k in allow),
                None,
            )
            if key is not None:
                used.add(key)
                if args.list:
                    print(f"excp  {rel}:{line}\t{cmd}\t— {allow[key]}")
                continue
            orphans.append((rel, line, cmd))

    rc = 0
    if orphans:
        print(
            f"FAILURE: {len(orphans)} citation(s) in the current documentation do "
            f"not resolve against the binary's tree ({total} checked):",
            file=sys.stderr,
        )
        for rel, line, cmd in orphans:
            print(f"  {rel}:{line}\t{cmd}", file=sys.stderr)
        print(
            "  (if the citation is deliberate — prose, or a sentence whose very "
            "subject IS the command not existing —\n   add it to "
            "scripts/docs_cli_allow.tsv WITH the reason)",
            file=sys.stderr,
        )
        rc = 1

    # An exception that no longer matches anything is debt posing as coverage:
    # either the sentence was fixed, or the file was renamed. Either way the line
    # has to go, or the list grows until nobody can read it.
    stale = sorted(set(allow) - used)
    if stale:
        print(
            f"FAILURE: {len(stale)} exception(s) in docs_cli_allow.tsv that no "
            "longer match any citation:",
            file=sys.stderr,
        )
        for rel, cmd in stale:
            print(f"  {rel}\t{cmd}", file=sys.stderr)
        rc = 1

    table = check_readme_groups(nodes, leaves)
    if table:
        print(
            f"FAILURE: {len(table)} divergence(s) between the README's «Command "
            "groups» table and the binary's groups:",
            file=sys.stderr,
        )
        for line in table:
            print(f"  {line}", file=sys.stderr)
        rc = 1

    site = check_site_groups(nodes, leaves)
    if site:
        print(
            f"FAILURE: {len(site)} divergence(s) between the site pages "
            "(docs/gen.py's GROUPS) and the binary's groups:",
            file=sys.stderr,
        )
        for line in site:
            print(f"  {line}", file=sys.stderr)
        rc = 1

    if rc == 0:
        print(
            f"ok: the {total} citations in the current documentation resolve against "
            f"the tree ({len(allow)} declared exceptions), and the README's group "
            "table and the site pages match the binary"
        )
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
