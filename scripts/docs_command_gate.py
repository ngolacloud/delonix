#!/usr/bin/env python3
"""Cheap gate for the `docs/gen.py` site: the binary and the documentation agree
on what exists, WITHOUT generating a single page.

Runs `docs/gen.py`'s own `contract_gate()` assertion (imported as a module — it
only defines dicts/functions and reads `sys.argv[1]`, generating nothing until
someone calls `main()`), so it reads the REAL `GROUPS`/`GROUP_PATH`/`KINDS_DOC`,
never a second copy that could itself drift from the file it is meant to check.

Catches three silent drifts (see `contract_gate()`'s docstring):

  * a NEW command in the binary's COMMAND MAP that `GROUPS` does not yet document
    (silently dropped from the index/cheatsheet by `if g in GROUPS`);
  * a top-level page in `GROUPS` that is no longer a binary command (stale
    binary, like the one that lacked `bucket`, or a removed command);
  * a missing embedded `examples/<group>/<kind>.yaml` (today it silently
    publishes a `# (missing example: …)` block).

Relation to `scripts/docs_cli_gate.py`: that one resolves EVERY `delonix …`
cited in the documentation against the binary's entire tree (`cli-tree.sh`) and
needs the binary in `target/`. This is the cheap slice — a single `delonix
--help` — and accepts an explicit binary path, useful when generating against a
tree whose `target/` lives elsewhere (e.g. a worktree with no build of its own).

    python3 scripts/docs_command_gate.py                 # uses ./target/release|debug/delonix
    python3 scripts/docs_command_gate.py /path/delonix     # explicit binary

Exits 0 if they match; 1 (with the divergence) otherwise.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def binary() -> str:
    if len(sys.argv) > 1:
        return sys.argv[1]
    for c in (ROOT / "target/release/delonix", ROOT / "target/debug/delonix"):
        if c.is_file():
            return str(c)
    sys.exit(
        "no binary: pass the path as an argument or run "
        "`cargo build --release -p delonix-bin`"
    )


def load_gen(bin_path: str):
    # `docs/gen.py` reads `sys.argv[1]` for the binary path at import time.
    sys.argv = ["gen.py", bin_path]
    spec = importlib.util.spec_from_file_location(
        "delonix_docs_gen", ROOT / "docs" / "gen.py"
    )
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def main() -> int:
    bin_path = binary()
    gen = load_gen(bin_path)
    try:
        gen.contract_gate()  # SystemExit with the divergence, if any
    except SystemExit as e:
        print(e, file=sys.stderr)
        return 1
    members = gen.command_map_members()
    examples = sum(1 for _ in gen.KINDS_DOC)
    print(
        f"ok: COMMAND MAP ({len(members)} membros) e as páginas de `GROUPS` batem "
        f"certo, e os {examples} exemplos de Kind embebidos existem ({bin_path})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
