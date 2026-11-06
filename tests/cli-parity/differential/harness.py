#!/usr/bin/env python3
"""Differential CLI executor for Docker / Podman / Delonix.

The central question of this suite: can an operator take a habitual
Docker/Podman flow, change ONLY the executable name to `delonix`, and observe
the same contract? Each flow is a sequence of operator STEPS. A step names an
intent and an argv; the runner replays it against every selected engine,
capturing argv, stdout, stderr, exit code and wall-clock duration. Setup and
cleanup run OUTSIDE the compared steps. A PTY is used for interactive steps.

Engines are selected with --engines (default: all available). A binary path can
be overridden per engine via env: DOCKER_BIN, PODMAN_BIN, DELONIX_BIN. Fixtures
are isolated per run with a unique tag so a shared daemon is never disturbed;
`prune`/context flows are gated behind --allow-destructive because they need a
disposable engine, never a personal one.

Records are written as JSONL to the path given by --out (default: stdout). The
exit code is 0 only when every asserted step held for every engine.
"""
from __future__ import annotations

import argparse
import json
import os
import pty
import select
import shutil
import subprocess
import sys
import time
import uuid

# ---- engine registry -------------------------------------------------------

ENGINES = {
    "docker": os.environ.get("DOCKER_BIN", "docker"),
    "podman": os.environ.get("PODMAN_BIN", "podman"),
    "delonix": os.environ.get("DELONIX_BIN", "delonix"),
}


def resolve(engine: str) -> str | None:
    """Absolute path to the engine binary, or None if it is not runnable."""
    cand = ENGINES[engine]
    if os.path.isabs(cand) or "/" in cand:
        return cand if os.access(cand, os.X_OK) else None
    return shutil.which(cand)


def available(selected: list[str]) -> dict[str, str]:
    out = {}
    for e in selected:
        p = resolve(e)
        if p:
            out[e] = p
        else:
            print(f"# SKIP engine {e}: binary not found", file=sys.stderr)
    return out


# ---- step execution --------------------------------------------------------


class Step:
    """One operator action, captured identically across engines."""

    def __init__(self, intent, argv, *, tty=False, send=None,
                 timeout=30, expect_exit=None, want=None, forbid=None):
        self.intent = intent          # human label, stable across engines
        self.argv = argv              # list WITHOUT the engine binary
        self.tty = tty                # run under a PTY
        self.send = send              # bytes to write to a PTY stdin
        self.timeout = timeout
        self.expect_exit = expect_exit  # asserted exit code, or None
        self.want = want or []        # substrings that MUST appear in output
        self.forbid = forbid or []    # substrings that must NOT appear


def _run_plain(binpath, argv, timeout):
    t0 = time.monotonic()
    try:
        p = subprocess.run([binpath, *argv], capture_output=True,
                           timeout=timeout)
        return (p.returncode, p.stdout.decode(errors="replace"),
                p.stderr.decode(errors="replace"), time.monotonic() - t0, None)
    except subprocess.TimeoutExpired as e:
        return (124, (e.stdout or b"").decode(errors="replace"),
                (e.stderr or b"").decode(errors="replace"),
                time.monotonic() - t0, "timeout")


def _run_pty(binpath, argv, send, timeout):
    """Run under a real PTY, feeding `send` once the child is up. stdout and
    stderr are merged on the PTY (that is what a terminal does)."""
    t0 = time.monotonic()
    pid, fd = pty.fork()
    if pid == 0:  # child
        try:
            os.execv(binpath, [binpath, *argv])
        except Exception as exc:  # noqa: BLE001
            sys.stderr.write(f"exec failed: {exc}\n")
            os._exit(127)
    buf = bytearray()
    sent = False
    deadline = time.monotonic() + timeout
    while True:
        if not sent and send is not None and (time.monotonic() - t0) > 0.6:
            try:
                os.write(fd, send)
            except OSError:
                pass
            sent = True
        r, _, _ = select.select([fd], [], [], 0.2)
        if fd in r:
            try:
                chunk = os.read(fd, 4096)
            except OSError:
                break
            if not chunk:
                break
            buf.extend(chunk)
        if time.monotonic() > deadline:
            try:
                import signal
                os.kill(pid, signal.SIGKILL)
            except OSError:
                pass
            _, status = os.waitpid(pid, 0)
            return (124, buf.decode(errors="replace"), "",
                    time.monotonic() - t0, "timeout")
        # child exit check
        try:
            wpid, status = os.waitpid(pid, os.WNOHANG)
        except OSError:
            break
        if wpid == pid:
            # drain remaining
            while True:
                r, _, _ = select.select([fd], [], [], 0.1)
                if fd not in r:
                    break
                try:
                    chunk = os.read(fd, 4096)
                except OSError:
                    break
                if not chunk:
                    break
                buf.extend(chunk)
            code = os.waitstatus_to_exitcode(status)
            return (code, buf.decode(errors="replace"), "",
                    time.monotonic() - t0, None)
    _, status = os.waitpid(pid, 0)
    return (os.waitstatus_to_exitcode(status), buf.decode(errors="replace"),
            "", time.monotonic() - t0, None)


def run_step(engine, binpath, step: Step):
    if step.tty:
        rc, out, err, dur, note = _run_pty(binpath, step.argv, step.send,
                                          step.timeout)
    else:
        rc, out, err, dur, note = _run_plain(binpath, step.argv, step.timeout)
    merged = out + err
    ok = True
    reasons = []
    if step.expect_exit is not None and rc != step.expect_exit:
        ok = False
        reasons.append(f"exit {rc} != {step.expect_exit}")
    for w in step.want:
        if w not in merged:
            ok = False
            reasons.append(f"missing {w!r}")
    for f in step.forbid:
        if f in merged:
            ok = False
            reasons.append(f"forbidden {f!r}")
    return {
        "engine": engine,
        "intent": step.intent,
        "argv": [binpath, *step.argv],
        "tty": step.tty,
        "exit": rc,
        "stdout_stderr": merged[-4000:],
        "duration_s": round(dur, 3),
        "note": note,
        "ok": ok,
        "reasons": reasons,
    }
