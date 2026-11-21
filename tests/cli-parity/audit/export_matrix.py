#!/usr/bin/env python3
"""Export the audit matrix in a machine-readable form, and reconcile the totals.

Two artefacts:
  matriz.csv    one row per case: id, group, path, argv, expectation, source,
                status, rc, reasons, evidence.
  inventario.csv one row per DISCOVERED path: whether it has a traceable
                destination in the matrix, and which cases reach it.

The second is the audit's completion criterion: every discovered entry must
have a traceable destination. A path with no case is reported as such, never
omitted — an inventory that silently drops what nobody tested reads as covered.
"""
from __future__ import annotations

import argparse
import csv
import json
import shlex
import sys
from collections import defaultdict
from pathlib import Path


def load_jsonl(p: Path) -> list[dict]:
    if not p.exists():
        return []
    return [json.loads(l) for l in p.read_text().splitlines() if l.strip()]


def leaf_of(argv: list[str], paths: set[str]) -> str:
    """The longest discovered path that is a prefix of this argv.

    Leading flags are dropped first: the root's own case is `["--help"]`, and
    taking argv[0] verbatim attributed it to a path called `--help`, which left
    `(root)` reading as an entry nobody tested.
    """
    words = list(argv)
    while words and words[0].startswith("-"):
        words.pop(0)
    for i in range(len(words), 0, -1):
        cand = " ".join(words[:i])
        if cand in paths:
            return cand
    return "(root)" if not words else words[0]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--inventory", required=True)
    ap.add_argument("--matrix", action="append", required=True)
    ap.add_argument("--results", action="append", default=[])
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    out = Path(args.out); out.mkdir(parents=True, exist_ok=True)
    inv = json.load(open(args.inventory))
    paths = set(inv["nodes"])

    cases: dict[str, dict] = {}
    for m in args.matrix:
        for c in load_jsonl(Path(m)):
            cases[c["id"]] = c

    results: dict[str, dict] = {}
    for r in args.results:
        for row in load_jsonl(Path(r)):
            results[row["id"]] = row

    by_path: dict[str, list[str]] = defaultdict(list)
    with open(out / "matriz.csv", "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["id", "grupo", "caminho", "argv", "expectativa", "fonte",
                    "estado", "rc", "motivos", "evidencia"])
        for cid, c in sorted(cases.items()):
            argv = c.get("argv", [])
            path = leaf_of(argv, paths)
            by_path[path].append(cid)
            r = results.get(cid, {})
            status = r.get("status", "NAO_EXECUTADO")
            w.writerow([
                cid, c.get("group", ""), path,
                " ".join(shlex.quote(a) for a in argv),
                c.get("expectation", ""), c.get("source", ""),
                status, r.get("rc", ""),
                " | ".join(r.get("reasons", []) or ([c["skip"]] if c.get("skip") else [])),
                r.get("evidence", ""),
            ])

    # Inventory reconciliation.
    sem_destino = []
    with open(out / "inventario.csv", "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["caminho", "folha", "n_opcoes", "n_positionais",
                    "n_casos", "ids_dos_casos"])
        for p, node in sorted(inv["nodes"].items()):
            ids = by_path.get(p, [])
            if not ids:
                sem_destino.append(p)
            w.writerow([p, "sim" if node["is_leaf"] else "nao",
                        len(node["options"]), len(node["positionals"]),
                        len(ids), " ".join(ids[:40])])

    tally = defaultdict(int)
    for r in results.values():
        tally[r["status"]] += 1
    for cid, c in cases.items():
        if cid not in results:
            tally["NAO_EXECUTADO"] += 1

    summary = {
        "inventario": inv["totals"],
        "casos_na_matriz": len(cases),
        "casos_com_resultado": len(results),
        "estados": dict(tally),
        "caminhos_sem_destino_na_matriz": len(sem_destino),
        "caminhos_sem_destino": sem_destino,
        "reconciliacao": {
            "soma_dos_estados": sum(tally.values()),
            "igual_a_casos_na_matriz": sum(tally.values()) == len(cases),
        },
    }
    json.dump(summary, open(out / "resumo.json", "w"), indent=1, ensure_ascii=False)
    print(json.dumps({k: v for k, v in summary.items()
                      if k != "caminhos_sem_destino"}, indent=1, ensure_ascii=False))
    if sem_destino:
        print(f"\ncaminhos sem destino ({len(sem_destino)}), primeiros 20:")
        for p in sem_destino[:20]:
            print(f"  {p}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
