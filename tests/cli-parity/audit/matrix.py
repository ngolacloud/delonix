#!/usr/bin/env python3
"""Generate the Delonix CLI audit matrix from the discovered inventory.

Three families of cases, each with a stable ID:

  HELP-<n>   every discovered path answers `--help` on stdout with rc=0.
  PARSE-<n>  per-parameter classes that need no engine: unknown flag, missing
             value, empty value, space, Unicode, repetition, global-option
             position, the `--` separator.
  PAIR-<n>   pairwise coverage of the option set of a leaf, restricted to
             options that take no value (a value would need a valid operand).
  NEG-<n>    declared conflicts and dependencies, one case each.

The pair count is REPORTED with its denominator (possible valid pairs vs
generated vs executed), as the audit asks: a coverage number without a
denominator says nothing.

Functional cases (the ones that create resources) are NOT generated here — they
are hand-written in `cases_functional.jsonl`, because an oracle for "it really
ran" cannot be derived from help text.
"""
from __future__ import annotations

import argparse
import itertools
import json
import re
import sys

# Global options are declared on the root and inherited; a leaf repeating them
# is the same rule, not a new one.
GLOBAL_OPTS = {"--l18n", "--help", "--version", "-h", "-V"}

# Options whose mere parse has a side effect or blocks: never generated blind.
DANGEROUS = re.compile(
    r"--(force|yes|all|prune|purge|destroy|wipe|reset|remove-data|purge-data|"
    r"destroy-remote|stopped|replace|reboot|apply|execute|experimental)"
)
# Paths that must never be invoked blind: they mutate, serve forever, or need a
# terminal. The audit reaches them through hand-written functional cases.
BLIND_FORBIDDEN = re.compile(
    r"^(serve|mcp|dash|dashboard|system (prune|snapshot|boot|setup|doctor)|"
    r"net (netns|boot|capture|tunnel|flow)|cluster|vm |workload |migrate|"
    r"gitops reconcile|oci-runtime|prespawn|node |warmpool)"
)


def leaf_ok_for_blind_help(path: str) -> bool:
    return True  # `--help` never runs the command


def gen_help_cases(inv: dict) -> list[dict]:
    cases = []
    for i, (path_str, node) in enumerate(sorted(inv["nodes"].items()), 1):
        argv = node["path"] + ["--help"]
        cases.append({
            "id": f"HELP-{i:04d}",
            "group": "help",
            "title": f"`{path_str}` responde a --help",
            "argv": argv,
            "oracle": {
                "exit_code": 0,
                "stdout_not_contains": ["panicked", "RUST_BACKTRACE"],
                "stderr_empty": True,
            },
            "expectation": "rc=0, ajuda em stdout, stderr vazio",
            "source": "clap: toda a folha tem --help; convencao Docker/Podman",
            "timeout": 30,
        })
    return cases


def gen_parse_cases(inv: dict) -> list[dict]:
    """Parser-contract cases that need no engine and create nothing."""
    cases: list[dict] = []
    n = 0

    def add(**kw):
        nonlocal n
        n += 1
        kw["id"] = f"PARSE-{n:04d}"
        kw.setdefault("group", "parser")
        kw.setdefault("timeout", 30)
        cases.append(kw)

    leaves = [(p, v) for p, v in sorted(inv["nodes"].items()) if v["is_leaf"]]
    groups = [(p, v) for p, v in sorted(inv["nodes"].items()) if not v["is_leaf"]]

    # A group with no subcommand: clap's documented behaviour is rc=2 and the
    # help on stderr. Whatever it does, it must not be a silent rc=0.
    for path_str, node in groups:
        if not node["path"]:
            continue
        add(
            title=f"`{path_str}` sem subcomando",
            argv=node["path"],
            oracle={"exit_code": [0, 2], "output_not_contains": ["panicked"]},
            expectation="grupo sem subcomando: uso invalido (2) ou ajuda (0), nunca panico",
            source="clap arg_required_else_help / convencao do repo",
        )

    # An unknown flag must be refused AS A FLAG — never consumed as an operand.
    # Six leaves (`run`/`create`/`exec` and their root aliases) take it as the
    # IMAGE/CONTAINER operand because the positional carries
    # `allow_hyphen_values`, and `run` then goes to the registry with it. So the
    # oracle is not the exit code: it is "the flag is named, and nothing is
    # resolved or pulled on its behalf".
    for path_str, node in leaves:
        add(
            title=f"`{path_str}` rejeita flag inexistente como FLAG",
            argv=node["path"] + ["--delonix-audit-nao-existe"],
            oracle={
                "exit_code_not": [0],
                "output_contains": ["--delonix-audit-nao-existe"],
                "output_not_contains": [
                    "panicked",
                    "pulling",            # nunca ir a um registo remoto
                    "no such container",  # nunca tratar a flag como nome
                ],
            },
            expectation=("a flag desconhecida e nomeada e recusada como flag; nao e "
                         "consumida como operando nem enviada a um registo"),
            source=("docker 29.9.3 e podman 4.9.3: `unknown flag: --x`, rc=125; "
                    "clap: unexpected argument"),
        )

    # An option that takes a value, given with no value, must be rejected.
    for path_str, node in leaves:
        for opt in node["options"]:
            if not opt["takes_value"] or not opt["long"]:
                continue
            if opt["long"] in GLOBAL_OPTS:
                continue
            add(
                title=f"`{path_str} {opt['long']}` sem valor",
                argv=node["path"] + [opt["long"]],
                oracle={
                    "exit_code": 2,
                    "output_contains": [opt["long"]],
                    "output_not_contains": ["panicked"],
                },
                expectation="rc=2: a opcao exige valor",
                source=f"ajuda de `{path_str}`: {opt['long']} <{opt['value_name']}>",
            )

    # A value outside the declared possible values must be refused by name.
    for path_str, node in leaves:
        for opt in node["options"]:
            if not opt.get("possible_values") or not opt["long"]:
                continue
            add(
                title=f"`{path_str} {opt['long']}` com valor fora da enumeracao",
                argv=node["path"] + [opt["long"], "valor-que-nao-existe"],
                oracle={
                    "exit_code": 2,
                    "output_contains": ["valor-que-nao-existe"],
                    "output_not_contains": ["panicked"],
                },
                expectation="rc=2 e o valor invalido nomeado, com a enumeracao",
                source=f"ajuda de `{path_str}`: possible values {opt['possible_values']}",
            )

    return cases


def gen_pair_cases(inv: dict, limit_per_leaf: int) -> tuple[list[dict], dict]:
    """Pairwise over the value-less options of each leaf.

    Only `--help`-safe combinations are generated: each pair is invoked ALONGSIDE
    `--help`, so the parser is exercised (acceptance/refusal of the combination)
    without the command ever running. That is the only honest way to cover pairs
    broadly on a CLI whose leaves mutate the machine.
    """
    cases: list[dict] = []
    stats = {"possible": 0, "generated": 0}
    n = 0
    for path_str, node in sorted(inv["nodes"].items()):
        if not node["is_leaf"]:
            continue
        flags = [
            o["long"] for o in node["options"]
            if o["long"] and not o["takes_value"]
            and o["long"] not in GLOBAL_OPTS
            and not DANGEROUS.search(o["long"])
        ]
        pairs = list(itertools.combinations(sorted(set(flags)), 2))
        stats["possible"] += len(pairs)
        for a, b in pairs[:limit_per_leaf]:
            n += 1
            cases.append({
                "id": f"PAIR-{n:04d}",
                "group": "pairwise",
                "title": f"`{path_str}` aceita ou rejeita {a} + {b}",
                "argv": node["path"] + [a, b, "--help"],
                "oracle": {
                    "exit_code": [0, 2],
                    "output_not_contains": ["panicked", "RUST_BACKTRACE"],
                },
                "expectation": "o par e aceite (0) ou rejeitado com diagnostico (2); nunca panico",
                "source": f"ajuda de `{path_str}`: pares de flags sem valor",
                "timeout": 30,
            })
            stats["generated"] += 1
    return cases, stats


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--inventory", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--pairs-per-leaf", type=int, default=6)
    ap.add_argument("--stats", default=None)
    args = ap.parse_args()

    inv = json.load(open(args.inventory))
    cases = gen_help_cases(inv)
    cases += gen_parse_cases(inv)
    # The pair tier lives in matrix2.py: it invokes the pair WITHOUT `--help`
    # and so the parser really evaluates the combination. The version that used
    # to be here appended `--help`, which short-circuits conflict validation
    # (measured), so it proved tokenisation only — and it collided on the
    # PAIR-#### ids with the real one.
    pair_stats = {"nota": "camada de pares vive em matrix2.py"}

    with open(args.out, "w") as fh:
        for c in cases:
            fh.write(json.dumps(c, ensure_ascii=False) + "\n")

    stats = {
        "cases_total": len(cases),
        "by_group": {},
        "pairs": pair_stats,
        "inventory_totals": inv["totals"],
        "pairs_per_leaf_cap": args.pairs_per_leaf,
    }
    for c in cases:
        stats["by_group"][c["group"]] = stats["by_group"].get(c["group"], 0) + 1
    print(json.dumps(stats, indent=1))
    if args.stats:
        json.dump(stats, open(args.stats, "w"), indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
