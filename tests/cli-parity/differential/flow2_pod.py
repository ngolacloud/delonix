#!/usr/bin/env python3
"""Flow 2 — a pod with two members (priority flow #2).

The operator contract: a named pod holding two containers that share a network
namespace, so one reaches the other over `localhost`; list and inspect the
members; tear the pod down.

The two engines reach it by DIFFERENT routes, and that divergence is the
finding, recorded rather than hidden:

  * Podman is IMPERATIVE — `pod create --name`, then `run --pod` per member,
    then `pod stop/start/restart/rm`.
  * Delonix is DECLARATIVE — a `kind: Pod` manifest with both members, applied
    with `pod create -f`; the pod is listed with `get pods`, inspected with
    `describe pod`, and removed with `delete pod`. There is no
    `pod create --name` and no `pod stop/start/restart` verb (ADR-0006): the
    pod's shape lives in the manifest, and its lifecycle in the generic verbs.

What BOTH must deliver, and what this script asserts for each: two members up,
member-to-member traffic over localhost carrying a sentinel, both members
visible in the listing, and a clean teardown.
"""
from __future__ import annotations

import json
import subprocess as sp
import sys
import time

BB = "busybox:1.36"
BB_PODMAN = "docker.io/library/busybox:1.36"
SENTINEL = "POD_LOCALHOST_OK"
PORT = "8081"


def cap(argv, timeout=60, **kw):
    t0 = time.monotonic()
    p = sp.run(argv, capture_output=True, timeout=timeout, **kw)
    return {
        "argv": argv,
        "exit": p.returncode,
        "out": p.stdout.decode(errors="replace"),
        "err": p.stderr.decode(errors="replace"),
        "dur": round(time.monotonic() - t0, 3),
    }


def rec(records, engine, intent, r, ok, reasons=None):
    records.append({
        "engine": engine, "intent": intent, "argv": r["argv"],
        "exit": r["exit"], "stdout_stderr": (r["out"] + r["err"])[-1500:],
        "duration_s": r["dur"], "ok": ok, "reasons": reasons or [],
    })


# ---- Podman: imperative two-member pod -------------------------------------

def podman_flow(binp, records):
    e = "podman"
    pod = "dlxpar-f2-pm"
    # cleanup
    sp.run([binp, "pod", "rm", "-f", pod], capture_output=True)

    r = cap([binp, "pod", "create", "--name", pod])
    rec(records, e, "pod create --name (imperative)", r, r["exit"] == 0)

    # member b: an httpd on localhost:PORT serving a sentinel
    serve = (f"mkdir -p /www; echo {SENTINEL} > /www/probe; "
             f"exec httpd -f -p {PORT} -h /www")
    r = cap([binp, "run", "-d", "--pod", pod, "--name", f"{pod}-b", BB_PODMAN,
             "sh", "-c", serve])
    rec(records, e, "run --pod : member b (httpd on localhost)", r, r["exit"] == 0)

    # member a: sleeps, used to curl member b over localhost
    r = cap([binp, "run", "-d", "--pod", pod, "--name", f"{pod}-a", BB_PODMAN,
             "sleep", "300"])
    rec(records, e, "run --pod : member a", r, r["exit"] == 0)

    time.sleep(1.5)
    # localhost comms: member a reaches member b on 127.0.0.1:PORT
    r = cap([binp, "exec", f"{pod}-a", "wget", "-qO-",
             f"http://localhost:{PORT}/probe"])
    rec(records, e, "localhost comms between members", r,
        SENTINEL in r["out"], [] if SENTINEL in r["out"] else ["no sentinel"])

    # list members
    r = cap([binp, "ps", "-a", "--pod", "--format", "{{.Names}} {{.Pod}}"])
    both = f"{pod}-a" in r["out"] and f"{pod}-b" in r["out"]
    rec(records, e, "list members of the pod", r, both)

    # inspect
    r = cap([binp, "pod", "inspect", pod])
    rec(records, e, "pod inspect", r, r["exit"] == 0 and pod in r["out"])

    # lifecycle: stop/start/restart
    for verb in ("stop", "start", "restart"):
        r = cap([binp, "pod", verb, pod], timeout=60)
        rec(records, e, f"pod {verb}", r, r["exit"] == 0)

    # teardown
    r = cap([binp, "pod", "rm", "-f", pod])
    gone = cap([binp, "pod", "ps", "--format", "{{.Name}}"])
    rec(records, e, "pod rm + confirm gone", r,
        r["exit"] == 0 and pod not in gone["out"])


# ---- Delonix: declarative two-member pod -----------------------------------

DELONIX_POD_MANIFEST = f"""\
apiVersion: compute.delonix.io/v1alpha1
kind: Pod
metadata:
  name: dlxpar-f2-dlx
spec:
  containers:
    - name: b
      image: {BB}
      command: ["sh", "-c"]
      args: ["mkdir -p /www; echo {SENTINEL} > /www/probe; exec httpd -f -p {PORT} -h /www"]
    - name: a
      image: {BB}
      command: ["sleep", "300"]
"""


def delonix_flow(binp, records):
    e = "delonix"
    pod = "dlxpar-f2-dlx"
    import tempfile, os
    sp.run([binp, "delete", "pod", pod, "-f"], capture_output=True)
    mf = tempfile.NamedTemporaryFile("w", suffix=".yaml", delete=False)
    mf.write(DELONIX_POD_MANIFEST)
    mf.close()

    # DIVERGENCE 1: no `pod create --name` — declarative only.
    r = cap([binp, "pod", "create", "-f", mf.name], timeout=90)
    rec(records, e, "pod create -f (declarative kind: Pod)", r, r["exit"] == 0)

    time.sleep(1.5)
    # localhost comms: member a reaches member b over localhost
    r = cap([binp, "pod", "exec", pod, "--container", "a", "--",
             "wget", "-qO-", f"http://localhost:{PORT}/probe"], timeout=40)
    rec(records, e, "localhost comms between members", r,
        SENTINEL in r["out"], [] if SENTINEL in r["out"] else ["no sentinel"])

    # list members (generic verb)
    r = cap([binp, "get", "pods"])
    rec(records, e, "list members of the pod (get pods)", r,
        pod in r["out"])

    # inspect (generic verb)
    r = cap([binp, "describe", "pod", pod])
    rec(records, e, "describe pod (inspect)", r, r["exit"] == 0 and pod in r["out"])

    # Pod lifecycle verbs — added in the CLI-parity review so the Podman
    # spelling works on delonix too (operating the declaratively-created pod).
    for verb in ("stop", "start", "restart"):
        r = cap([binp, "pod", verb, pod], timeout=60)
        rec(records, e, f"pod {verb}", r, r["exit"] == 0)
        if verb != "stop":
            time.sleep(1.0)

    # teardown (generic verb)
    r = cap([binp, "delete", "pod", pod, "-f"], timeout=60)
    gone = cap([binp, "get", "pods"])
    rec(records, e, "delete pod + confirm gone", r,
        r["exit"] == 0 and pod not in gone["out"])

    # DIVERGENCE 3 (regression guard): `run --pod <absent>` — the imperative
    # Podman spelling — must FAIL with an ACTIONABLE error naming the
    # declarative flow, never the old cryptic "ingress infra is down".
    sp.run([binp, "rm", "-f", "dlxpar-f2-join"], capture_output=True)
    r = cap([binp, "run", "-d", "--pod", "dlxpar-no-such-pod", "--name",
             "dlxpar-f2-join", BB, "sleep", "30"])
    msg = r["out"] + r["err"]
    actionable = (r["exit"] != 0 and "kind: Pod" in msg
                  and "pod create -f" in msg
                  and "ingress infra is down" not in msg)
    rec(records, e, "run --pod <absent>: actionable error (not cryptic)", r,
        actionable, [] if actionable else ["error not actionable"])
    sp.run([binp, "rm", "-f", "dlxpar-f2-join"], capture_output=True)
    os.unlink(mf.name)


def main():
    import shutil, os
    records = []
    podman = os.environ.get("PODMAN_BIN", "podman")
    delonix = os.environ.get("DELONIX_BIN", "delonix")
    podman = shutil.which(podman) or (podman if os.access(podman, os.X_OK) else None)
    delonix = delonix if (os.path.isabs(delonix) and os.access(delonix, os.X_OK)) else shutil.which(delonix)

    if podman:
        podman_flow(podman, records)
    else:
        print("# SKIP podman: not found", file=sys.stderr)
    if delonix:
        delonix_flow(delonix, records)
    else:
        print("# SKIP delonix: not found", file=sys.stderr)

    out_path = sys.argv[1] if len(sys.argv) > 1 else None
    if out_path:
        with open(out_path, "w") as fh:
            for r in records:
                fh.write(json.dumps(r) + "\n")

    print(f"\n{'ENGINE':8} {'OK':3} {'EXIT':5} INTENT")
    print("-" * 82)
    allok = True
    for r in records:
        mark = "ok" if r["ok"] else "XX"
        if not r["ok"]:
            allok = False
        print(f"{r['engine']:8} {mark:3} {str(r['exit']):5} {r['intent']}")
        if not r["ok"]:
            print(f"         └─ {r['stdout_stderr'].replace(chr(10),' ')[:160]}")
    print("-" * 82)
    print("ALL OK" if allok else "DIVERGENCES PRESENT (see per-row)")
    return 0 if allok else 1


if __name__ == "__main__":
    sys.exit(main())
