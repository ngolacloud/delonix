#!/usr/bin/env python3
"""Capture terminal samples of the Delonix CLI and judge the layout mechanically.

Runs each command in a real PTY at a given width (so clap's wrapping is the one
an operator sees), and separately with stdout redirected to a pipe, in EN and in
pt. Reports, per sample, the facts a layout review needs and which a human
reading one screen cannot count reliably:

  - lines wider than the terminal (the wrap did not hold)
  - whether colour/ANSI is emitted when stdout is NOT a terminal
  - whether anything was written to stderr
  - whether stdout stayed empty (a command that says nothing to a pipe)
  - the column alignment of table output (ragged columns)
  - prompts/animations that would block CI (a read on stdin, a spinner)

It judges nothing about taste; it measures what is checkable, and leaves the
sample on disk so the report can quote it verbatim.
"""
from __future__ import annotations

import argparse
import json
import os
import pty
import re
import select
import subprocess
import sys
from pathlib import Path

ANSI = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")


def _reap(pid: int, fd: int) -> int:
    """Wait for the child and return its real exit code (or 128+signal)."""
    try:
        os.close(fd)
    except OSError:
        pass
    try:
        _, st = os.waitpid(pid, 0)
    except ChildProcessError:
        return 0
    if os.WIFEXITED(st):
        return os.WEXITSTATUS(st)
    if os.WIFSIGNALED(st):
        return 128 + os.WTERMSIG(st)
    return -1


def run_pty(bin_path: str, argv: list[str], cols: int, lang: str,
            timeout: float = 25.0) -> tuple[str, int]:
    """Run in a real PTY of `cols` columns; return (output, rc)."""
    env = dict(os.environ)
    env["COLUMNS"] = str(cols)
    env["LINES"] = "50"
    env["DELONIX_L18N"] = lang
    env.pop("NO_COLOR", None)
    pid, fd = pty.fork()
    if pid == 0:  # child
        try:
            import fcntl, struct, termios
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 50, cols, 0, 0))
            os.execvpe(bin_path, [bin_path] + argv, env)
        except Exception:
            os._exit(127)
    buf = b""
    import time
    t0 = time.monotonic()
    while time.monotonic() - t0 < timeout:
        r, _, _ = select.select([fd], [], [], 0.3)
        if r:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                # EIO on a pty means the child closed its end: it exited.
                # Reaping it here is what distinguishes "finished" from
                # "blocked"; without it every sample read as a timeout.
                return buf.decode(errors="replace"), _reap(pid, fd)
            if not chunk:
                return buf.decode(errors="replace"), _reap(pid, fd)
            buf += chunk
        else:
            wpid, st = os.waitpid(pid, os.WNOHANG)
            if wpid:
                # drain
                while True:
                    r2, _, _ = select.select([fd], [], [], 0.2)
                    if not r2:
                        break
                    try:
                        c2 = os.read(fd, 65536)
                    except OSError:
                        break
                    if not c2:
                        break
                    buf += c2
                try:
                    os.close(fd)
                except OSError:
                    pass
                if os.WIFEXITED(st):
                    return buf.decode(errors="replace"), os.WEXITSTATUS(st)
                if os.WIFSIGNALED(st):
                    return buf.decode(errors="replace"), 128 + os.WTERMSIG(st)
                return buf.decode(errors="replace"), -1
    try:
        os.kill(pid, 9)
        os.waitpid(pid, 0)
        os.close(fd)
    except Exception:
        pass
    return buf.decode(errors="replace") + "\n<TIMEOUT>", 124


def run_pipe(bin_path: str, argv: list[str], cols: int, lang: str,
             timeout: float = 25.0) -> tuple[str, str, int]:
    env = dict(os.environ)
    env["COLUMNS"] = str(cols)
    env["DELONIX_L18N"] = lang
    env.pop("NO_COLOR", None)
    try:
        p = subprocess.run([bin_path] + argv, capture_output=True, text=True,
                           timeout=timeout, env=env, input="")
        return p.stdout, p.stderr, p.returncode
    except subprocess.TimeoutExpired:
        return "", "<TIMEOUT>", 124


def ragged_columns(text: str) -> int:
    """Count rows whose column starts disagree with the header's.

    A table is aligned when every row begins its Nth field at the same column
    as the header. Measured on the first whitespace-separated header row.
    """
    lines = [l for l in text.splitlines() if l.strip()]
    if len(lines) < 3:
        return 0
    header = lines[0]
    if not re.match(r"^[A-Z][A-Z ]", header):
        return 0
    starts = [m.start() for m in re.finditer(r"\S+", header)]
    if len(starts) < 2:
        return 0
    bad = 0
    for row in lines[1:]:
        rs = [m.start() for m in re.finditer(r"\S+", row)]
        if len(rs) != len(starts):
            continue
        if any(abs(a - b) > 0 for a, b in zip(rs, starts)):
            bad += 1
    return bad


def judge(sample: dict) -> list[str]:
    notes: list[str] = []
    pty_txt = sample["pty"]["text"]
    pipe_out = sample["pipe"]["stdout"]
    pipe_err = sample["pipe"]["stderr"]
    cols = sample["cols"]

    plain = ANSI.sub("", pty_txt).replace("\r", "")
    over = [l for l in plain.splitlines() if len(l) > cols]
    if over:
        notes.append(f"{len(over)} linha(s) acima de {cols} colunas no PTY "
                     f"(maior: {max(len(l) for l in over)})")

    if ANSI.search(pipe_out) or ANSI.search(pipe_err):
        notes.append("emite ANSI com stdout redireccionado (sem NO_COLOR)")

    if not pipe_out.strip() and not pipe_err.strip() and sample["pipe"]["rc"] == 0:
        notes.append("rc=0 e nao escreveu nada em stdout nem stderr")

    if sample["pipe"]["rc"] != 0 and not pipe_err.strip():
        notes.append(f"rc={sample['pipe']['rc']} sem nada em stderr "
                     f"(o erro saiu em stdout)")

    r = ragged_columns(ANSI.sub("", pipe_out))
    if r:
        notes.append(f"{r} linha(s) de tabela desalinhadas do cabecalho")

    if sample["pipe"]["rc"] == 124 or sample["pty"]["rc"] == 124:
        notes.append("BLOQUEOU: timeout com stdin fechado (prompt ou animacao)")

    if re.search(r"[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]", pipe_out + pipe_err):
        notes.append("spinner braille emitido fora de um terminal")

    return notes


COMMANDS = [
    ["--help"],
    ["-h"],
    ["container", "--help"],
    ["container", "run", "--help"],
    ["container", "ps"],
    ["image", "ls"],
    ["volume", "ls"],
    ["network", "ls"],
    ["api-resources"],
    ["version"],
    ["system", "info"],
    ["stack", "plan", "--fields"],
    ["container", "stop", "nao-existe-aud"],
    ["container", "run", "--flag-que-nao-existe"],
    ["volume", "create"],
    ["get", "cascalhos"],
]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--root", required=True)
    args = ap.parse_args()
    out = Path(args.out); (out / "samples").mkdir(parents=True, exist_ok=True)
    os.environ["DELONIX_ROOT"] = args.root

    results = []
    for argv in COMMANDS:
        for cols in (80, 120):
            for lang in ("en", "pt"):
                pty_txt, pty_rc = run_pty(args.bin, argv, cols, lang)
                po, pe, prc = run_pipe(args.bin, argv, cols, lang)
                s = {
                    "argv": argv, "cols": cols, "lang": lang,
                    "pty": {"text": pty_txt, "rc": pty_rc},
                    "pipe": {"stdout": po, "stderr": pe, "rc": prc},
                }
                s["notes"] = judge(s)
                tag = ("_".join(argv).replace("/", "_").replace("-", "") or "root")
                fn = out / "samples" / f"{tag}.{cols}.{lang}.txt"
                fn.write_text(
                    f"# argv: {argv}  cols={cols} lang={lang}\n"
                    f"# pty rc={pty_rc} | pipe rc={prc}\n"
                    f"--- PTY ---\n{pty_txt}\n--- PIPE stdout ---\n{po}\n"
                    f"--- PIPE stderr ---\n{pe}\n--- notas ---\n" +
                    "\n".join(s["notes"]), encoding="utf-8")
                s["sample_file"] = str(fn)
                s.pop("pty"); s["pipe"] = {"rc": prc,
                                           "stdout_len": len(po),
                                           "stderr_len": len(pe)}
                results.append(s)

    json.dump(results, open(out / "ux.json", "w"), indent=1, ensure_ascii=False)
    withnotes = [r for r in results if r["notes"]]
    print(f"amostras: {len(results)}  com achados: {len(withnotes)}")
    import collections
    c = collections.Counter(n.split("(")[0].split(":")[0].strip()
                            for r in withnotes for n in r["notes"])
    for k, v in c.most_common():
        print(f"  {v:3}  {k}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
