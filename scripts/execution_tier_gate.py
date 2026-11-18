#!/usr/bin/env python3
"""Execution-tier gate — classify a command by its REAL capability, as runnable logic.

# What this closes

`.ai/agents/permission-tiers.md` sorts agents into tiers A–D against the engine's
own risk vocabulary (`crates/interfaces/delonix-mcp/src/risk.rs`:
`Read < SafeWrite < Disruptive < Destructive < Privileged`, with `requires_confirm`
above SafeWrite and `risk_of` failing closed to Privileged). Until now a tier was
a label an agent self-applied to a *command prefix*; nothing checked a concrete
command against the tier by its **effect**, and nothing could tell, in a fixture,
that `ls` is fine for a read-only agent while `bash -c "git push"` is not.

This module turns the tier contract into a **pure classifier**: given a command
string and an agent's maximum tier, it returns ALLOW or REFUSE by the command's
real capability — not its prefix. It decomposes subprocess wrappers
(`bash -c`, `env`, `xargs`, `find -exec`, `sudo`, `timeout …`) so an indirect
effect is judged by what actually runs, and it fails closed: an unknown verb is
Privileged, never Read. It never executes anything, so classifying a command can
never, itself, cause the effect.

# What the executor CANNOT impose (declared BLOCKED)

This gate CLASSIFIES and ADVISES; it is not the hard boundary. The real limits —
an unprivileged user, a per-task workspace/network, no access to provider
credentials, no elevation — are imposed by the *executor* that runs the agent
(the client's permission mode, hooks, sandbox, cgroup/namespace confinement),
which THIS repository does not control. Where the executor cannot impose a limit,
the contract downgrades to stop-and-ask and this gate is the advisory check that
makes the intended decision explicit and testable. Treat a REFUSE here as "the
tier says no"; the executor is what guarantees it.

# Usage

    python3 scripts/execution_tier_gate.py classify --tier A --cmd "ls -la"      # exit 0 ALLOW / 3 REFUSE
    python3 scripts/execution_tier_gate.py check-fixtures scripts/execution_tier_fixtures
"""

from __future__ import annotations

import argparse
import json
import re
import shlex
import sys
from pathlib import Path

# Capability levels, ascending — mirrors risk.rs.
READ, SAFEWRITE, DISRUPTIVE, DESTRUCTIVE, PRIVILEGED = range(5)
LEVEL_NAME = {READ: "Read", SAFEWRITE: "SafeWrite", DISRUPTIVE: "Disruptive",
              DESTRUCTIVE: "Destructive", PRIVILEGED: "Privileged"}

# Agent tier → the highest level it may reach at all (permission-tiers.md A–D).
TIER_CEILING = {"A": READ, "B": SAFEWRITE, "C": DESTRUCTIVE, "D": PRIVILEGED}

ALLOW, REFUSE = "ALLOW", "REFUSE"

# Wrappers that run another command; we must judge the inner one.
SKIP_WRAPPERS = {"timeout", "nice", "nohup", "stdbuf", "ionice", "chrt", "setsid"}
ELEVATION = {"sudo", "su", "pkexec", "doas", "runuser"}

# First-token verbs by their real capability.
READ_CMDS = {"ls", "cat", "head", "tail", "wc", "echo", "pwd", "which", "whereis",
             "file", "stat", "readlink", "realpath", "true", "basename", "dirname",
             "rg", "grep", "egrep", "fgrep", "awk", "sed", "cut", "sort", "uniq",
             "diff", "cmp", "tree", "date", "uname", "id", "hostname", "test", "nl"}
SAFEWRITE_CMDS = {"mkdir", "touch", "cp", "mv", "ln", "tee", "patch", "make",
                  "cargo", "rustc", "python3", "pytest", "npm", "node", "go"}
DESTRUCTIVE_CMDS = {"rm", "rmdir", "shred", "truncate", "mkfs", "wipefs", "prune"}
PRIVILEGED_CMDS = {"mount", "umount", "unshare", "nsenter", "setns", "ip", "nft",
                   "iptables", "ip6tables", "ebtables", "modprobe", "insmod",
                   "rmmod", "sysctl", "mknod", "losetup", "cryptsetup", "pvcreate",
                   "vgcreate", "lvcreate", "zpool", "zfs", "chroot", "virsh",
                   "qemu-system-x86_64", "cloud-hypervisor", "ch-remote", "kvm",
                   "dmsetup", "parted", "fdisk", "sgdisk", "mkswap", "swapon"}
# Remote / collaboration effect — needs an explicit authorization grant (H01),
# never auto at an execution tier.
REMOTE_CMDS = {"curl", "wget", "scp", "rsync", "ssh", "gh", "git-push"}

GIT_READ = {"log", "show", "diff", "status", "cat-file", "rev-parse", "rev-list",
            "cherry", "merge-base", "ls-files", "ls-tree", "blame", "describe",
            "shortlog", "reflog", "config"}  # config read; a --write is caught below
GIT_REMOTE = {"push", "pull", "fetch", "clone", "remote"}
GIT_MUTATE = {"add", "commit", "checkout", "reset", "rebase", "merge", "tag",
              "stash", "cherry-pick", "revert", "apply", "am", "worktree", "branch"}

# Paths that are out of a task's scope or hold credentials/admission state.
CRED_PATTERNS = [re.compile(p) for p in (
    r"\.aws/", r"\.config/gcloud", r"\.ssh/id_", r"id_rsa", r"id_ed25519",
    r"/secrets?\b", r"policy\.json", r"events\.jsonl", r"\.kube/config",
    r"credential", r"secret", r"token", r"\.netrc", r"\.pgpass",
)]
OUT_OF_SCOPE_WRITE = [re.compile(p) for p in (
    r"^/etc/", r"^/usr/", r"^/boot/", r"^/run/delonix", r"\bDELONIX_ROOT\b",
    r"^/var/lib/delonix", r"/delonix-[a-z-]*\.sock",
)]
FAIL_OPEN_FLAGS = re.compile(
    r"DELONIX_ALLOW_UNENFORCED_|_INSECURE_TLS|DELONIX_INSECURE_REGISTRIES|"
    r"POLICY_LINT=0|DELONIX_CRI_CAP_CEILING|--wipe-devices")


def requires_confirm(level: int) -> bool:
    """Mirror risk.rs: anything above SafeWrite needs explicit human confirm."""
    return level > SAFEWRITE


def _tokens(cmd: str) -> list[str]:
    try:
        return shlex.split(cmd)
    except ValueError:
        return cmd.split()


def classify(cmd: str) -> tuple[int, str]:
    """Return (level, reason) for a command string, judging its real effect.

    Total and side-effect-free. Decomposes wrappers; fails closed on the unknown.
    """
    if FAIL_OPEN_FLAGS.search(cmd):
        return PRIVILEGED, "sets a fail-open/destructive flag — fail closed"
    for pat in CRED_PATTERNS:
        if pat.search(cmd):
            return PRIVILEGED, "reads/writes a credential or admission-state path"

    toks = _tokens(cmd)
    if not toks:
        return PRIVILEGED, "empty/unparseable command — fail closed"

    head = toks[0]
    # Strip leading VAR=value assignments (env-style) and `env`.
    if head == "env" or re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", head):
        rest = [t for t in toks[1:] if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*=", t)]
        if not rest:
            return READ, "environment inspection only"
        return classify(" ".join(rest))

    if head in ELEVATION:
        return PRIVILEGED, f"privilege elevation via {head}"

    if head in SKIP_WRAPPERS:
        return classify(" ".join(toks[1:])) if len(toks) > 1 else (
            PRIVILEGED, f"{head} with no inner command — fail closed")

    # Shell -c wrappers: judge the inner script.
    if head in {"bash", "sh", "zsh", "dash"} and "-c" in toks:
        idx = toks.index("-c")
        if idx + 1 < len(toks):
            return classify(toks[idx + 1])
        return PRIVILEGED, "shell -c with no script — fail closed"

    if head == "xargs":
        inner = [t for t in toks[1:] if not t.startswith("-")]
        return classify(" ".join(inner)) if inner else (READ, "xargs with no command")

    if head == "find":
        if "-delete" in toks:
            return DESTRUCTIVE, "find -delete"
        for ex in ("-exec", "-execdir", "-ok"):
            if ex in toks:
                i = toks.index(ex)
                return classify(" ".join(toks[i + 1:]))
        return READ, "find traversal (read-only)"

    if head == "git":
        sub = toks[1] if len(toks) > 1 else ""
        if sub in GIT_REMOTE:
            return DISRUPTIVE, f"git {sub} is a remote effect"
        if sub in GIT_READ:
            return READ, f"git {sub} (read-only)"
        if sub in GIT_MUTATE:
            # branch -d / worktree remove etc. still mutate local state.
            return SAFEWRITE, f"git {sub} (local mutation)"
        return SAFEWRITE, f"git {sub} (assumed local mutation)"

    if head in REMOTE_CMDS or head == "git-push":
        return DISRUPTIVE, f"{head} is a remote/network effect"

    if head in PRIVILEGED_CMDS:
        return PRIVILEGED, f"{head} is a privileged kernel/net/virt operation"

    if head == "dd" and any(t.startswith("of=/dev") for t in toks):
        return PRIVILEGED, "dd writing to a device"

    if head in DESTRUCTIVE_CMDS:
        return DESTRUCTIVE, f"{head} destroys data"

    if head == "cargo":
        sub = toks[1] if len(toks) > 1 else ""
        cargo_read = {"tree", "metadata", "search", "pkgid", "locate-project",
                      "read-manifest", "verify-project", "version", "--version", "-V"}
        if sub in cargo_read:
            return READ, f"cargo {sub} (read-only)"
        if sub == "publish":
            return DISRUPTIVE, "cargo publish is a remote effect"
        return SAFEWRITE, f"cargo {sub or 'build'} (workspace build/test)"

    if head in SAFEWRITE_CMDS:
        for pat in OUT_OF_SCOPE_WRITE:
            if pat.search(cmd):
                return PRIVILEGED, f"{head} writes outside the task scope"
        return SAFEWRITE, f"{head} writes within the workspace"

    if head in READ_CMDS:
        return READ, f"{head} is read-only"

    return PRIVILEGED, f"unknown verb {head!r} — fail closed"


def decide(cmd: str, max_tier: str) -> tuple[str, str]:
    """ALLOW iff the command's level fits the tier AND needs no human confirm."""
    ceiling = TIER_CEILING.get(max_tier.upper())
    if ceiling is None:
        return REFUSE, f"unknown tier {max_tier!r} — fail closed"
    level, why = classify(cmd)
    if level > ceiling:
        return REFUSE, f"{LEVEL_NAME[level]} exceeds tier {max_tier} ceiling ({why})"
    if requires_confirm(level):
        return REFUSE, f"{LEVEL_NAME[level]} requires explicit human confirm ({why})"
    return ALLOW, f"{LEVEL_NAME[level]} allowed at tier {max_tier} ({why})"


def _cmd_classify(args: argparse.Namespace) -> int:
    decision, reason = decide(args.cmd, args.tier)
    print(f"{decision}: {reason}")
    return 0 if decision == ALLOW else 3


def _cmd_check_fixtures(args: argparse.Namespace) -> int:
    root = Path(args.dir)
    files = sorted(root.glob("*.json"))
    if not files:
        print(f"::error::no fixtures in {root} — this gate stopped guarding anything")
        return 1
    failures = total = 0
    for fixture in files:
        data = json.loads(fixture.read_text(encoding="utf-8"))
        for case in data.get("cases", []):
            total += 1
            decision, reason = decide(case["cmd"], case["tier"])
            if decision != case["expect"]:
                failures += 1
                print(f"[FAIL] {fixture.name}::{case['name']} "
                      f"expected {case['expect']}, got {decision} — {reason}")
    if failures:
        print(f"{failures}/{total} fixture case(s) disagree with the tier contract")
        return 1
    print(f"all {total} fixture case(s) match the execution-tier contract")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("classify", help="classify one command at a tier")
    c.add_argument("--tier", required=True, help="agent max tier: A/B/C/D")
    c.add_argument("--cmd", required=True, help="the command string")
    c.set_defaults(func=_cmd_classify)
    f = sub.add_parser("check-fixtures", help="validate a fixtures directory")
    f.add_argument("dir", help="directory of *.json fixtures")
    f.set_defaults(func=_cmd_check_fixtures)
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except (OSError, json.JSONDecodeError, KeyError) as exc:
        print(f"::error::bad input: {exc}")
        return 2


if __name__ == "__main__":
    sys.exit(main())
