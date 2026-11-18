#!/usr/bin/env python3
"""Skill router gate — the E2E skill routes by domain, and harness-check reuses real targets.

# What this closes

The E2E skill is a ~2.5k-line playbook; reading it whole for one task blows the
budget. H05 adds `.ai/skills/delonix-runtime-e2e-quality/ROUTER.md`, which maps a
task to ONE domain (linux-oci, docker-podman, cri-cni, vm-storage, iac-recovery,
cve-mcp, paas) and the sections/tier/external-dependency it loads, plus a
`delonix-harness-check` skill that reuses the Makefile/scripts/workflows instead of
inventing a parallel check. This gate keeps that honest:

1. The router covers every required domain, and routing a subject lands on one.
2. A task that depends on the PaaS records an **external** dependency (the engine
   knows no consumer).
3. The maintained skills (architecture, triage, security, release) and the E2E and
   harness-check skills all exist with a valid `## Skill Name`.
4. `harness-check` points at **real** targets: Makefile targets that exist and
   scripts that exist — not an invented parallel suite.

Pure logic (routing) + repo checks; no build, no git.

  python3 scripts/skill_router_gate.py                       # report + validate
  python3 scripts/skill_router_gate.py route "a CRI conformance task"
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS = ROOT / ".ai" / "skills"
E2E_ROUTER = SKILLS / "delonix-runtime-e2e-quality" / "ROUTER.md"
HARNESS_CHECK = SKILLS / "delonix-harness-check" / "SKILL.md"
MAKEFILE = ROOT / "Makefile"

REQUIRED_DOMAINS = ["linux-oci", "docker-podman", "cri-cni", "vm-storage",
                    "iac-recovery", "cve-mcp", "paas"]
MAINTAINED_SKILLS = ["delonix-architecture-review", "delonix-bug-triage",
                     "delonix-security-review", "delonix-release",
                     "delonix-runtime-e2e-quality", "delonix-harness-check"]
EXTERNAL_DEP = {"paas": "ngolacloud-control-plane"}

# Routing keywords, ordered — the first domain whose keyword matches wins.
_ROUTE = [
    ("paas", ("paas", "control-plane", "control plane")),
    ("cri-cni", ("cri", "cni", "kubelet", "kubernetes", "kaas", "pod network")),
    ("docker-podman", ("docker", "podman", "compose", "quadlet", "libpod")),
    ("vm-storage", ("vm", "microvm", "hypervisor", "volume", "snapshot", "storage", "pool")),
    ("iac-recovery", ("stack", "iac", "gitops", "drift", "reboot", "recovery", "upgrade")),
    ("cve-mcp", ("cve", "sbom", "supply", "mcp", "scan", "provenance", "autonomy")),
    ("linux-oci", ("oci", "rootfs", "namespace", "cgroup", "seccomp", "capabilit", "linux")),
]


def route(topic: str) -> str:
    """Pure: map a task subject to one domain (default linux-oci/Core)."""
    low = topic.lower()
    for domain, keys in _ROUTE:
        if any(k in low for k in keys):
            return domain
    return "linux-oci"


def external_dependency(domain: str) -> str | None:
    return EXTERNAL_DEP.get(domain)


def _makefile_targets() -> set[str]:
    if not MAKEFILE.exists():
        return set()
    return set(re.findall(r"^([a-z][a-z0-9_-]*):", MAKEFILE.read_text(encoding="utf-8"), re.M))


def check() -> list[str]:
    errors: list[str] = []

    # 1. Router exists and covers every required domain.
    if not E2E_ROUTER.exists():
        return [f"{E2E_ROUTER} missing — the E2E router"]
    router_text = E2E_ROUTER.read_text(encoding="utf-8")
    listed = set(re.findall(r"\*\*([a-z-]+)\*\*", router_text))
    for dom in REQUIRED_DOMAINS:
        if dom not in listed:
            errors.append(f"router does not cover domain: {dom}")

    # 2. Routing lands on a listed domain, and paas records an external dep.
    if route("a CRI conformance run") != "cri-cni":
        errors.append("routing a CRI task did not land on cri-cni")
    if external_dependency(route("a PaaS control-plane task")) is None:
        errors.append("a PaaS task must record an external dependency")

    # 3. Skills exist with a valid name block.
    for skill in MAINTAINED_SKILLS:
        path = SKILLS / skill / "SKILL.md"
        if not path.exists():
            errors.append(f"missing skill: {skill}/SKILL.md")
            continue
        text = path.read_text(encoding="utf-8")
        if "## Skill Name" not in text or f"`{skill}`" not in text:
            errors.append(f"{skill}/SKILL.md: no '## Skill Name' with `{skill}`")

    # 4. harness-check reuses real targets (Makefile) and real scripts.
    if HARNESS_CHECK.exists():
        hc = HARNESS_CHECK.read_text(encoding="utf-8")
        targets = _makefile_targets()
        for mk in re.findall(r"`make ([a-z-]+)", hc):
            if mk not in targets:
                errors.append(f"harness-check references unknown make target: {mk}")
        for script in sorted(set(re.findall(r"`?(scripts/[\w./-]+\.py)", hc))):
            if not (ROOT / script).exists():
                errors.append(f"harness-check references missing script: {script}")

    return errors


def _cmd_route(args: argparse.Namespace) -> int:
    d = route(args.topic)
    dep = external_dependency(d)
    print(f"domain={d}" + (f" external={dep}" if dep else ""))
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd")
    r = sub.add_parser("route", help="route a task subject to a domain")
    r.add_argument("topic")
    r.set_defaults(func=_cmd_route)
    args = parser.parse_args(argv)
    if getattr(args, "func", None):
        return args.func(args)
    errors = check()
    if errors:
        print("FAIL — skill router:")
        for e in errors:
            print(f"  - {e}")
        return 1
    print(f"OK — router covers {len(REQUIRED_DOMAINS)} domains, "
          f"{len(MAINTAINED_SKILLS)} skills present, harness-check reuses real targets")
    return 0


if __name__ == "__main__":
    sys.exit(main())
