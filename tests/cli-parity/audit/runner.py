#!/usr/bin/env python3
"""Executor of the Delonix CLI audit matrix.

Reads a matrix of cases (JSONL), runs each one against a real binary with argv
passed as a LIST (never through a shell, so the boundary between the Delonix
parser and the shell is preserved), judges it against its declared oracle, and
writes one evidence record per execution.

Statuses: PASS, FAIL, BLOQUEADO, NAO_EXECUTADO.

A negative case passes only when BOTH the predicted rejection and the absence of
mutation are verified; a case that declares `no_mutation` and provides no probe
is reported BLOQUEADO rather than PASS, because an unverified claim of "nothing
changed" is the dishonesty this suite exists to avoid.

Usage:
  runner.py --matrix m.jsonl --bin /path/to/delonix --out DIR [--id CASE_ID]...
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import signal
import subprocess
import sys
import time
from pathlib import Path

REDACT = re.compile(
    r"(?i)(password|passwd|secret|token|authtoken|api[_-]?key|bearer)\s*[:=]\s*\S+"
)


def redact(text: str) -> str:
    """Redact credential-shaped values, keeping the key so evidence stays readable."""
    return REDACT.sub(r"\1=<REDACTED>", text or "")


class Probe:
    """A state probe: a command whose captured stdout is compared before/after."""

    def __init__(self, spec: dict, bin_path: str, env: dict):
        self.argv = spec["argv"]
        self.bin = bin_path
        self.env = env

    def read(self) -> str:
        try:
            p = subprocess.run(
                [self.bin] + self.argv,
                capture_output=True,
                text=True,
                timeout=60,
                env=self.env,
            )
            return p.stdout
        except Exception as exc:  # a probe that cannot read says so
            return f"<PROBE-ERROR:{exc}>"


def judge(case: dict, rc: int, sig: int | None, out: str, err: str,
          before: str | None, after: str | None) -> tuple[str, list[str]]:
    """Return (status, reasons). Every oracle clause that fails is reported."""
    fails: list[str] = []
    oracle = case.get("oracle", {})
    combined = out + err

    if "exit_code" in oracle:
        want = oracle["exit_code"]
        want_list = want if isinstance(want, list) else [want]
        if rc not in want_list:
            fails.append(f"exit_code: esperado {want_list}, observado {rc}")
    if "exit_code_not" in oracle:
        bad = oracle["exit_code_not"]
        bad_list = bad if isinstance(bad, list) else [bad]
        if rc in bad_list:
            fails.append(f"exit_code: nao devia ser {rc}")
    if oracle.get("no_signal") and sig is not None:
        fails.append(f"terminou por sinal {sig}")

    for needle in oracle.get("stdout_contains", []):
        if needle not in out:
            fails.append(f"stdout nao contem {needle!r}")
    for needle in oracle.get("stdout_not_contains", []):
        if needle in out:
            fails.append(f"stdout contem {needle!r} e nao devia")
    for needle in oracle.get("stderr_contains", []):
        if needle not in err:
            fails.append(f"stderr nao contem {needle!r}")
    for needle in oracle.get("output_contains", []):
        if needle not in combined:
            fails.append(f"saida nao contem {needle!r}")
    for needle in oracle.get("output_not_contains", []):
        if needle in combined:
            fails.append(f"saida contem {needle!r} e nao devia")
    for pat in oracle.get("output_matches", []):
        if not re.search(pat, combined, re.M):
            fails.append(f"saida nao casa /{pat}/")

    if oracle.get("stdout_json"):
        try:
            json.loads(out)
        except Exception as exc:
            fails.append(f"stdout nao e JSON valido: {exc}")

    if oracle.get("stdout_empty") and out.strip():
        fails.append("stdout devia estar vazio")
    if oracle.get("stderr_empty") and err.strip():
        fails.append("stderr devia estar vazio")

    if oracle.get("no_mutation"):
        if before is None or after is None:
            return "BLOQUEADO", ["no_mutation declarado sem sonda de estado"]
        if "<PROBE-ERROR" in (before + after):
            return "BLOQUEADO", ["a sonda de estado nao pode ler"]
        if before != after:
            fails.append("o estado mudou num caso que nao devia mutar")

    return ("FAIL" if fails else "PASS"), fails


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--matrix", required=True)
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--id", action="append", default=[])
    ap.add_argument("--group", action="append", default=[])
    ap.add_argument("--root", help="DELONIX_ROOT for the disposable environment")
    ap.add_argument("--net-runtime", help="DELONIX_NET_RUNTIME_DIR")
    args = ap.parse_args()

    out_dir = Path(args.out)
    (out_dir / "evidence").mkdir(parents=True, exist_ok=True)

    base_env = dict(os.environ)
    base_env["NO_COLOR"] = "1"
    base_env["COLUMNS"] = "200"
    base_env.setdefault("DELONIX_L18N", "en")
    if args.root:
        base_env["DELONIX_ROOT"] = args.root
    if args.net_runtime:
        base_env["DELONIX_NET_RUNTIME_DIR"] = args.net_runtime

    cases = []
    with open(args.matrix) as fh:
        for line in fh:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            cases.append(json.loads(line))

    wanted_ids = set(args.id)
    wanted_groups = set(args.group)
    if wanted_ids:
        cases = [c for c in cases if c["id"] in wanted_ids]
    if wanted_groups:
        cases = [c for c in cases if c.get("group") in wanted_groups]

    results_path = out_dir / "results.jsonl"
    tally: dict[str, int] = {}
    with open(results_path, "w") as res:
        for case in cases:
            cid = case["id"]
            if case.get("skip"):
                rec = {
                    "id": cid, "group": case.get("group"), "status": "NAO_EXECUTADO",
                    "reasons": [case["skip"]], "argv": case.get("argv"),
                }
                tally["NAO_EXECUTADO"] = tally.get("NAO_EXECUTADO", 0) + 1
                res.write(json.dumps(rec, ensure_ascii=False) + "\n")
                print(f"NAO_EXECUTADO {cid}: {case['skip']}")
                continue

            env = dict(base_env)
            env.update({k: str(v) for k, v in case.get("env", {}).items()})
            for k in case.get("env_unset", []):
                env.pop(k, None)

            probe = Probe(case["probe"], args.bin, env) if case.get("probe") else None
            before = probe.read() if probe else None

            # Precondition commands: a failure here BLOCKS the case, it never
            # fails it — the case has not been measured.
            blocked = None
            setup_log: list[dict] = []
            for pre in case.get("setup", []):
                p = subprocess.run([args.bin] + pre, capture_output=True, text=True,
                                   input=case.get("setup_stdin", ""),
                                   timeout=case.get("timeout", 120), env=env)
                # ALWAYS recorded, even when tolerated: a setup that failed
                # silently is how a case passes or fails for a reason nobody can
                # see afterwards. It cost one round of this audit.
                setup_log.append({
                    "argv": pre, "rc": p.returncode,
                    "stderr": redact(p.stderr.strip()[:300]),
                    "stdout": redact(p.stdout.strip()[:200]),
                    "tolerado": bool(case.get("setup_best_effort")),
                })
                if p.returncode != 0 and not case.get("setup_best_effort"):
                    blocked = (f"setup falhou: {' '.join(shlex.quote(a) for a in pre)}"
                               f" rc={p.returncode} {redact(p.stderr.strip()[:300])}")
                    break

            if blocked:
                rec = {"id": cid, "group": case.get("group"), "status": "BLOQUEADO",
                       "reasons": [blocked], "argv": case.get("argv"),
                       "setup": setup_log}
                tally["BLOQUEADO"] = tally.get("BLOQUEADO", 0) + 1
                res.write(json.dumps(rec, ensure_ascii=False) + "\n")
                print(f"BLOQUEADO {cid}: {blocked}")
                continue

            argv = case["argv"]
            t0 = time.monotonic()
            sig = None
            try:
                p = subprocess.run(
                    [args.bin] + argv,
                    input=case.get("stdin", ""),
                    capture_output=True,
                    text=True,
                    timeout=case.get("timeout", 60),
                    env=env,
                    cwd=case.get("cwd"),
                )
                rc, out, err = p.returncode, p.stdout, p.stderr
                if rc < 0:
                    sig = -rc
            except subprocess.TimeoutExpired as exc:
                rc = 124
                out = (exc.stdout or b"").decode(errors="replace") if isinstance(exc.stdout, bytes) else (exc.stdout or "")
                err = (exc.stderr or b"").decode(errors="replace") if isinstance(exc.stderr, bytes) else (exc.stderr or "")
                err += f"\n<TIMEOUT apos {case.get('timeout', 60)}s>"
            dur = time.monotonic() - t0

            after = probe.read() if probe else None

            status, reasons = judge(case, rc, sig, out, err, before, after)

            tolerated_fail = [e for e in setup_log
                              if e["rc"] != 0 and e["tolerado"]]
            if tolerated_fail and status == "FAIL":
                status = "BLOQUEADO"
                reasons = [
                    "setup tolerado FALHOU, por isso o caso nao mediu o que queria: "
                    + "; ".join(f"{' '.join(e['argv'])} rc={e['rc']} {e['stderr']}"
                                for e in tolerated_fail)
                ] + reasons

            # `compare_argv`: the case asserts two spellings answer the same.
            # A written promise of "verbatim" is only a promise if someone diffs it.
            cmp_out = None
            if case.get("compare_argv"):
                c2 = subprocess.run([args.bin] + case["compare_argv"],
                                    capture_output=True, text=True,
                                    timeout=case.get("timeout", 60), env=env)
                cmp_out = c2.stdout
                if cmp_out != out:
                    status = "FAIL"
                    reasons.append(
                        f"a saida difere de `{' '.join(case['compare_argv'])}`"
                    )

            ev_name = f"{cid}.txt"
            ev_path = out_dir / "evidence" / ev_name
            ev_path.write_text(
                f"# {cid}  {case.get('title','')}\n"
                f"# argv: {json.dumps([args.bin] + argv, ensure_ascii=False)}\n"
                f"# env delta: {json.dumps(case.get('env', {}))}\n"
                f"# rc={rc} sig={sig} dur={dur:.3f}s\n"
                f"--- stdout ---\n{redact(out)}\n--- stderr ---\n{redact(err)}\n"
                + (f"--- probe before ---\n{redact(before)}\n--- probe after ---\n{redact(after)}\n"
                   if probe else ""),
                encoding="utf-8",
            )

            for post in case.get("teardown", []):
                try:
                    subprocess.run([args.bin] + post, capture_output=True, text=True,
                                   timeout=120, env=env)
                except Exception:
                    pass

            rec = {
                "id": cid,
                "group": case.get("group"),
                "title": case.get("title"),
                "status": status,
                "reasons": reasons,
                "argv": argv,
                "env": case.get("env", {}),
                "expectation": case.get("expectation"),
                "source": case.get("source"),
                "rc": rc,
                "signal": sig,
                "duration_s": round(dur, 3),
                "stdout_head": redact(out)[:2000],
                "stderr_head": redact(err)[:2000],
                "state_changed": (before != after) if probe else None,
                "setup": setup_log,
                "evidence": str(ev_path),
            }
            tally[status] = tally.get(status, 0) + 1
            res.write(json.dumps(rec, ensure_ascii=False) + "\n")
            mark = {"PASS": "PASS", "FAIL": "FAIL", "BLOQUEADO": "BLOQ", "NAO_EXECUTADO": "NEXE"}[status]
            extra = f" :: {reasons[0]}" if reasons else ""
            print(f"{mark} {cid}{extra}")

    print("\n=== resumo ===")
    for k in ("PASS", "FAIL", "BLOQUEADO", "NAO_EXECUTADO"):
        print(f"{k}={tally.get(k,0)}")
    print(f"total={sum(tally.values())}")
    json.dump(tally, open(out_dir / "tally.json", "w"), indent=1)
    return 1 if tally.get("FAIL") else 0


if __name__ == "__main__":
    sys.exit(main())
