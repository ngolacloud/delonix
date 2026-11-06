#!/usr/bin/env python3
"""Flow 1 — interactive container (priority flow #1).

Operator contract (changing ONLY the executable):
  * `run --rm -it <image> sh` in a real PTY: write+read a marker, `test -t 0`
    must see a TTY, exit with code 37, the host sees 37, `--rm` removed it.
  * stdin WITHOUT a TTY is still delivered, and the container sees no TTY.
  * stdout and stderr stay on separate streams when no TTY is attached.
  * a signal to a running container is observable in its exit.
  * `stats` yields a single sample and exits (no stream).
"""
from __future__ import annotations

import json
import sys

from harness import Step, available, run_step

DIGEST = ("busybox@sha256:"
          "bdf57e528e45e4433820e045b29b4597825a1c9e38353532d90a01445013f82e")


def main():
    engines = available(["docker", "podman", "delonix"])
    tag = "dlxpar-f1"
    records = []

    for engine, binpath in engines.items():
        name = f"{tag}-{engine}"
        # cleanup any stale fixture (outside the compared steps)
        import subprocess
        subprocess.run([binpath, "rm", "-f", name], capture_output=True)

        # Step 1: interactive PTY, marker + TTY check + exit 37
        s1 = Step(
            intent="run --rm -it IMG sh : marker, test -t 0, exit 37",
            argv=["run", "--rm", "-it", "--name", name, DIGEST, "sh"],
            tty=True,
            send=b"echo MARK_$$_READBACK; test -t 0 && echo TTY_YES || echo TTY_NO; exit 37\n",
            expect_exit=37,
            want=["MARK_", "READBACK", "TTY_YES"],
            timeout=40,
        )
        r1 = run_step(engine, binpath, s1)
        records.append(r1)

        # Step 2: --rm actually removed it
        s2 = Step(
            intent="ps -a : --rm container is gone",
            argv=["ps", "-a"],
            forbid=[name],
            expect_exit=0,
        )
        records.append(run_step(engine, binpath, s2))

        # Step 3: stdin WITHOUT a tty is delivered; container sees no tty
        s3 = Step(
            intent="run -i (no -t): stdin piped, no TTY inside",
            argv=["run", "--rm", "-i", "--name", f"{name}-notty", DIGEST,
                  "sh", "-c", "cat; echo; test -t 0 && echo HASTTY || echo NOTTY"],
            send=None,
            want=["PIPED_IN", "NOTTY"],
            forbid=["HASTTY"],
            expect_exit=0,
        )
        # feed stdin via plain subprocess
        import subprocess as sp
        p = sp.run([binpath, *s3.argv], input=b"PIPED_IN",
                   capture_output=True, timeout=40)
        merged = p.stdout.decode(errors="replace") + p.stderr.decode(errors="replace")
        rec = {
            "engine": engine, "intent": s3.intent,
            "argv": [binpath, *s3.argv], "tty": False, "exit": p.returncode,
            "stdout_stderr": merged[-2000:], "duration_s": None, "note": None,
            "ok": all(w in merged for w in s3.want) and all(f not in merged for f in s3.forbid) and p.returncode == 0,
            "reasons": [],
        }
        records.append(rec)

        # Step 4: stdout/stderr separation (no TTY)
        s4argv = ["run", "--rm", "--name", f"{name}-streams", DIGEST,
                  "sh", "-c", "echo TO_STDOUT; echo TO_STDERR >&2"]
        p = sp.run([binpath, *s4argv], capture_output=True, timeout=40)
        out = p.stdout.decode(errors="replace")
        err = p.stderr.decode(errors="replace")
        rec = {
            "engine": engine, "intent": "run: stdout/stderr on separate streams",
            "argv": [binpath, *s4argv], "tty": False, "exit": p.returncode,
            "stdout_stderr": f"STDOUT={out!r} STDERR={err!r}", "duration_s": None,
            "note": None,
            "ok": ("TO_STDOUT" in out and "TO_STDOUT" not in err
                   and "TO_STDERR" in err and "TO_STDERR" not in out
                   and p.returncode == 0),
            "reasons": [],
        }
        records.append(rec)

        # Step 5: stats single sample exits (no stream)
        s5 = Step(
            intent="stats: single sample, exits (no stream)",
            argv=["stats", "--no-stream"] if engine != "delonix" else ["stats"],
            expect_exit=0,
            timeout=20,
        )
        records.append(run_step(engine, binpath, s5))

    # report
    out_path = sys.argv[1] if len(sys.argv) > 1 else None
    if out_path:
        with open(out_path, "w") as fh:
            for r in records:
                fh.write(json.dumps(r) + "\n")

    print(f"\n{'ENGINE':8} {'OK':3} {'EXIT':5} INTENT")
    print("-" * 78)
    allok = True
    for r in records:
        mark = "ok" if r["ok"] else "XX"
        if not r["ok"]:
            allok = False
        print(f"{r['engine']:8} {mark:3} {str(r['exit']):5} {r['intent']}")
        if not r["ok"]:
            snippet = r["stdout_stderr"].replace("\n", "\\n")[:160]
            print(f"         └─ {snippet}")
    print("-" * 78)
    print("ALL OK" if allok else "DIVERGENCES PRESENT")
    return 0 if allok else 1


if __name__ == "__main__":
    sys.exit(main())
