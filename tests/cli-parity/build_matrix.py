#!/usr/bin/env python3
"""Regenerate the CLI-parity matrix from the ZIP's inventory + this review.

Reads the original `Matriz_CLI.csv` (the 198-path inventory measured at
99ecc796), re-measures RECOGNITION against the delonix binary built from THIS
checkout, assigns a domain, and annotates the paths this review verified against
the real engines with a per-engine state, the test that proves it, and the
evidence file. Paths not yet exercised keep an honest "por testar"/"fora do
âmbito" with a reason — a recognized parser path is not a passed contract.

    python3 tests/cli-parity/build_matrix.py <delonix-bin> <original.csv> <out.csv>
"""
from __future__ import annotations

import csv
import subprocess
import sys


def recognized(binp: str, path: str) -> int:
    p = subprocess.run([binp, *path.split(), "--help"],
                       capture_output=True)
    return p.returncode


def domain_of(path: str) -> str:
    head = path.split()[0]
    if head in ("container", "run", "create", "ps", "start", "stop", "restart",
                "rm", "exec", "logs", "kill", "attach", "stats", "cp", "port",
                "pause", "unpause", "rename", "top", "diff", "inspect", "wait",
                "commit", "export", "init", "cleanup", "mount", "umount",
                "unmount", "exists", "list", "restore", "runlabel", "checkpoint",
                "clone"):
        return "1-containers"
    if head in ("image", "images", "pull", "push", "tag", "save", "load", "rmi",
                "login", "logout", "search", "history", "import", "manifest",
                "untag", "sign", "trust", "scp", "tree", "build", "builder",
                "buildx", "bake"):
        return "3-images"
    if head == "pod" or path == "pod":
        return "2-pods"
    if head in ("volume",):
        return "4-volumes"
    if head in ("network",):
        return "4-networks"
    if head in ("system", "info", "events", "version", "context"):
        return "5-system-contexts"
    if head in ("swarm", "node", "service", "stack", "config", "secret",
                "plugin", "machine", "farm", "generate", "kube", "healthcheck",
                "auto-update", "compose", "unshare", "help", "secret"):
        return "fora-do-ambito"
    return "outros"


# Paths this review VERIFIED against the real engines, with the per-engine
# verdict, the proving test and the evidence. Keyed by the exact inventory path.
# The verdict words written to the CSV stay in the reviewer's language.
VERIFIED = {
    # Flow 1 — interactive container (differential, all three engines green).
    "run": ("passou", "docker=passou podman=passou delonix=passou",
            "flow1_interactive.py", "evidence-flow1.jsonl",
            "run --rm -it sh: PTY, test -t 0, exit 37, --rm removal; -i no-tty; "
            "stdout/stderr split; single-sample stats — identical across engines "
            "after the -i/-t parser fix"),
    "create": ("passou", "delonix=passou (parser)", "run_interactive_flag_parity_tests",
               "unit", "create shares RunArgs; -it now parses"),
    "ps": ("passou", "docker=passou podman=passou delonix=passou",
           "frequent-verbs.sh", "unit+script", "list containers; root==container ps"),
    "stats": ("passou", "delonix=passou", "flow1_interactive.py", "evidence-flow1.jsonl",
              "single sample, exits (no stream)"),
    # Flow 2 — pod two members.
    "pod create": ("divergência-deliberada",
                   "podman=imperativo(passou) delonix=declarativo(passou)",
                   "flow2_pod.py", "evidence-flow2.jsonl",
                   "no `pod create --name`: declarative `kind: Pod` + `pod create -f` "
                   "reaches the same outcome (two members, localhost comms, list, "
                   "inspect, teardown); `run --pod <absent>` now errs actionably"),
    "pod": ("passou", "delonix=passou", "flow2_pod.py", "evidence-flow2.jsonl",
            "get pods / describe pod / delete pod lifecycle proven"),
    "pod stop": ("passou", "podman=passou delonix=passou", "flow2_pod.py",
                 "evidence-flow2.jsonl", "`pod stop` added — stops every member"),
    "pod start": ("passou", "podman=passou delonix=passou", "flow2_pod.py",
                  "evidence-flow2.jsonl", "`pod start` added — starts every member"),
    "pod restart": ("passou", "podman=passou delonix=passou", "flow2_pod.py",
                    "evidence-flow2.jsonl", "`pod restart` added — stop then start"),
    "pod rm": ("passou", "podman=passou delonix=passou", "flow2_pod.py",
               "evidence-flow2.jsonl", "`pod rm` added — same as `delete pod`"),
    "pod logs": ("passou", "delonix=passou", "flow2_pod.py", "evidence-flow2.jsonl",
                 "member logs reachable"),
    # Flow 3 — containers/images root verbs.
    "cp": ("passou", "delonix=passou (root alias→container cp)", "frequent-verbs.sh",
           "unit+script", "root `cp` added; same handler as `container cp`"),
    "port": ("passou", "delonix=passou (root alias→container port)", "frequent-verbs.sh",
             "unit+script", "root `port` added; same handler as `container port`"),
    "login": ("passou", "delonix=passou (root alias→image login)", "frequent-verbs.sh",
              "unit+script", "root `login` added"),
    "logout": ("passou", "delonix=passou (root alias→image logout)", "frequent-verbs.sh",
               "unit+script", "root `logout` added"),
    "inspect": ("divergência-deliberada", "delonix=type-scoped",
                "docs/cli-stability.md", "doc",
                "no generic root `inspect`: `container inspect` / `image describe` "
                "resolve by Kind (documented policy)"),
    "diff": ("divergência-deliberada", "delonix=declarativo",
             "docs/cli-stability.md", "doc",
             "root `diff` is the MANIFEST three-way diff; `container diff` is the "
             "docker filesystem-changes form (documented collision resolution)"),
    "logs": ("passou", "docker=passou podman=passou delonix=passou", "frequent-verbs.sh",
             "unit+script", "root==container logs"),
    "wait": ("passou", "delonix=passou", "frequent-verbs.sh", "unit+script",
             "polymorphic: container operands vs manifest -f; mixing refused"),
    # Flow 4 — volumes/networks.
    "volume create": ("passou", "docker=passou delonix=passou", "manual",
                      "live", "data round-trip (write in one container, read in "
                      "another): SENTINEL matched; removal-in-use refused like docker"),
    "volume rm": ("passou", "docker=passou delonix=passou", "manual", "live",
                  "refused while a container holds the volume; succeeds once freed"),
    # Flow 5 — system/contexts.
    "info": ("passou", "delonix=passou (root alias→system info)", "frequent-verbs.sh",
             "unit+script", "root `info` added; == system info"),
    "events": ("passou", "delonix=passou (root alias→system events)", "frequent-verbs.sh",
               "unit+script", "root `events` added; == system events"),
    "context": ("fora-do-âmbito", "delonix=ausente (ADR-0010)",
                "docs/cli-stability.md", "doc",
                "remote management API recused (ADR-0010); no `context`/`system "
                "connection` — deliberate divergence, migration note required"),
    "system connection": ("fora-do-âmbito", "delonix=ausente (ADR-0010)",
                          "docs/cli-stability.md", "doc", "see `context`"),
}


def main():
    binp, src, out = sys.argv[1], sys.argv[2], sys.argv[3]
    rows = []
    with open(src) as fh:
        for r in csv.DictReader(fh):
            path = r["comando"]
            rc = recognized(binp, path)
            dom = domain_of(path)
            if path in VERIFIED:
                estado, per_engine, teste, evid, nota = VERIFIED[path]
            elif rc == 0:
                estado, per_engine, teste, evid, nota = (
                    "reconhecido-por-testar", "delonix=reconhecido", "", "",
                    "parser aceita; equivalência funcional por medir")
            else:
                estado, per_engine, teste, evid, nota = (
                    ("fora-do-âmbito" if dom == "fora-do-ambito" else "por-fazer"),
                    "delonix=ausente", "", "",
                    "recusado pelo parser; ver relatório para o próximo passo")
            rows.append({
                "dominio": dom,
                "comando": path,
                "referencia": r["referencia"],
                "versao_ref": "docker 29.9.2 / podman 4.9.3",
                "delonix_help_rc_99ecc796": r["delonix_help_rc"],
                "delonix_help_rc_agora": rc,
                "reconhecimento": "sim" if rc == 0 else "não",
                "estado": estado,
                "por_motor": per_engine,
                "teste": teste,
                "evidencia": evid,
                "nota": nota,
            })
    cols = ["dominio", "comando", "referencia", "versao_ref",
            "delonix_help_rc_99ecc796", "delonix_help_rc_agora", "reconhecimento",
            "estado", "por_motor", "teste", "evidencia", "nota"]
    with open(out, "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=cols)
        w.writeheader()
        w.writerows(rows)
    print(f"wrote {len(rows)} rows to {out}")


if __name__ == "__main__":
    main()
