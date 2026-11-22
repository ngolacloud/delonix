#!/usr/bin/env python3
"""NEG and PAIR tiers of the Delonix CLI audit matrix.

The design rests on one MEASURED fact about this parser: clap reports a
conflicting pair BEFORE it reports a missing required positional. Measured on
0.1.2:

    volume create --parent p --driver nfs     -> rc=2 "cannot be used with"
    volume create                             -> rc=2 "required arguments were not provided"

So a leaf that has at least one REQUIRED positional can be probed with an option
pair and no operands: the parser decides, the handler never runs, nothing on the
machine is touched. That is the only way to cover pairs broadly on a CLI whose
leaves mutate the host.

It also rests on a second measured fact, and this one is a limit: `--help`
short-circuits conflict validation (`volume create --parent p --driver nfs
--help` -> rc=0, help printed). So a pair tier built on `--help` proves
tokenisation only, never acceptance or refusal. This generator does NOT use
`--help`.

  NEG-<n>   the 22 conflicts/dependencies DECLARED in the parser, each as three
            cases: option A alone, option B alone, both together.
  PAIR-<n>  pairwise over the options of each leaf that has a required
            positional, with the pair given and the operands omitted.
"""
from __future__ import annotations

import argparse
import itertools
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from synth import synth_value, module_matches  # noqa: E402

REQUIRED_POS = re.compile(r"<(?!COMMAND>)[A-Z][A-Z0-9_]*>")

# Leaves that DO work even with the operands omitted, measured in the first
# pass: `build` defaults the context to the CWD and starts building.
EXECUTES_BLIND = {"build", "image build"}

# A synthetic value for options that take one. It has to parse as a plain
# string; options with a value_parser are validated before the conflict, so a
# case whose value is refused first is reported as such, not hidden.
# A file that exists, for options whose value is a path the engine opens
# before it evaluates anything else.
EXISTING_FILE = os.environ.get("AUD_EXISTING_FILE") or "/dev/null"


def val_for(opt: dict) -> list[str]:
    if not opt.get("takes_value"):
        return []
    return [synth_value(opt.get("value_name"), opt.get("description", ""),
                        existing_file=EXISTING_FILE)]


def flag_of(field: str) -> str:
    return "--" + field.replace("_", "-")


def leaf_has_required_positional(node: dict) -> bool:
    return bool(REQUIRED_POS.search(node.get("usage", "")))


def opts_of(node: dict) -> dict[str, dict]:
    return {o["long"]: o for o in node["options"] if o.get("long")}


def gen_neg(inv: dict, rules: list[dict]) -> tuple[list[dict], list[dict]]:
    cases: list[dict] = []
    unresolved: list[dict] = []
    n = 0
    for r in rules:
        if not r.get("field"):
            unresolved.append({**r, "why": "campo nao resolvido no codigo"})
            continue
        a = flag_of(r["field"])
        for target in r["targets"]:
            b = flag_of(target)
            # Resolve the leaf by MEASUREMENT: the leaf whose help carries both.
            hits = [
                (p, nd) for p, nd in inv["nodes"].items()
                if nd["is_leaf"] and a in opts_of(nd) and b in opts_of(nd)
                and module_matches(r["file"], p)
            ]
            if not hits:
                unresolved.append({**r, "a": a, "b": b,
                                   "why": "nenhuma folha publica as duas flags"})
                continue
            for path_str, nd in hits:
                oa, ob = opts_of(nd)[a], opts_of(nd)[b]
                va = [a] + val_for(oa)
                vb = [b] + val_for(ob)
                kind = r["kind"]
                is_conflict = kind.startswith("conflicts")
                safe = leaf_has_required_positional(nd)

                for label, argv, oracle, exp in (
                    ("a", nd["path"] + va,
                     {"exit_code": 2,
                      "output_not_contains": ["cannot be used with", "panicked"]},
                     f"{a} sozinha nao e rejeitada por conflito"),
                    ("b", nd["path"] + vb,
                     {"exit_code": 2,
                      "output_not_contains": ["cannot be used with", "panicked"]},
                     f"{b} sozinha nao e rejeitada por conflito"),
                    ("ab", nd["path"] + va + vb,
                     ({"exit_code": 2, "output_contains": ["cannot be used with"],
                       "output_not_contains": ["panicked"]} if is_conflict else
                      {"exit_code": 2, "output_not_contains": ["panicked"]}),
                     ("o par e rejeitado nomeando as duas opcoes"
                      if is_conflict else
                      "a dependencia e satisfeita: nao e rejeitada por conflito")),
                ):
                    n += 1
                    cases.append({
                        "id": f"NEG-{n:04d}",
                        "group": "negativo",
                        "title": f"`{path_str}` {kind} {a}/{b} [{label}]",
                        "argv": argv,
                        "oracle": oracle,
                        "expectation": exp,
                        "source": f"{r['file']}:{r['line']} {kind} = {r['targets']}",
                        "safe_no_operands": safe,
                        "timeout": 30,
                    })
                    if not safe:
                        # Without a required positional the handler WOULD run.
                        cases[-1]["skip"] = (
                            f"`{path_str}` nao tem positional obrigatorio: sem operandos o "
                            f"handler corria. Caso precisa de contexto valido proprio."
                        )
    return cases, unresolved


def gen_pairs(inv: dict, cap: int) -> tuple[list[dict], dict]:
    cases: list[dict] = []
    stats = {"folhas_elegiveis": 0, "folhas_excluidas_sem_positional": 0,
             "pares_possiveis": 0, "pares_gerados": 0}
    n = 0
    for path_str, nd in sorted(inv["nodes"].items()):
        if not nd["is_leaf"]:
            continue
        if not leaf_has_required_positional(nd):
            stats["folhas_excluidas_sem_positional"] += 1
            continue
        if path_str in EXECUTES_BLIND:
            stats.setdefault("folhas_excluidas_por_executarem", 0)
            stats["folhas_excluidas_por_executarem"] += 1
            continue
        stats["folhas_elegiveis"] += 1
        opts = opts_of(nd)
        names = sorted(
            o for o in opts
            if o not in ("--help", "--l18n", "--version")
        )
        pairs = list(itertools.combinations(names, 2))
        stats["pares_possiveis"] += len(pairs)
        for a, b in pairs[:cap]:
            oa, ob = opts[a], opts[b]
            argv = list(nd["path"])
            argv += [a] + val_for(oa)
            argv += [b] + val_for(ob)
            n += 1
            cases.append({
                "id": f"PAIR-{n:04d}",
                "group": "pares",
                "title": f"`{path_str}` par {a} + {b} (sem operandos)",
                "argv": argv,
                "oracle": {
                    "exit_code_not": [0],
                    "output_not_contains": ["panicked", "RUST_BACKTRACE"],
                },
                "expectation": ("recusado antes de agir: conflito nomeado (rc=2), falta de "
                                "operando obrigatorio (rc=2) ou valor invalido do motor "
                                "(rc=1, DX-1000). Nunca rc=0, nunca panico."),
                "source": f"ajuda de `{path_str}`: pares do conjunto de opcoes",
                "timeout": 30,
            })
            stats["pares_gerados"] += 1
    return cases, stats


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--inventory", required=True)
    ap.add_argument("--rules", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--unresolved", required=True)
    ap.add_argument("--stats", required=True)
    ap.add_argument("--cap", type=int, default=8)
    args = ap.parse_args()

    inv = json.load(open(args.inventory))
    rules = json.load(open(args.rules))

    neg, unresolved = gen_neg(inv, rules)
    pairs, pstats = gen_pairs(inv, args.cap)

    with open(args.out, "w") as fh:
        for c in neg + pairs:
            fh.write(json.dumps(c, ensure_ascii=False) + "\n")
    json.dump(unresolved, open(args.unresolved, "w"), indent=1, ensure_ascii=False)

    stats = {
        "neg_casos": len(neg),
        "neg_ignorados_sem_contexto": sum(1 for c in neg if c.get("skip")),
        "neg_regras_declaradas": len(rules),
        "neg_regras_nao_resolvidas": len(unresolved),
        "pares": pstats,
        "cap_por_folha": args.cap,
    }
    json.dump(stats, open(args.stats, "w"), indent=1)
    print(json.dumps(stats, indent=1, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
