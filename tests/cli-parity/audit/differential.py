#!/usr/bin/env python3
"""Differential comparison: Delonix vs Docker vs Podman, per INTENTION.

Each case states an intention and gives the three complete invocations, run on
equivalent fixtures (the same digest-pinned image, the same content). It keeps
TWO verdicts per case — Delonix–Docker and Delonix–Podman — and compares
syntax and operational result separately, as the audit asks: a combination that
works in one comparator and fails in the other keeps both results.

Mode asymmetry is recorded per case and never papered over: on this host
Docker is ROOTFUL and Delonix is measured ROOTLESS, so a difference may come
from the mode rather than the engine.

Classification of each divergence:
  defeito          contradicts Delonix's own contract
  incompat         breaks a parity Delonix promised
  intencional      a documented deliberate difference
  lacuna           functionality absent
  sem-equivalente  the comparator has no counterpart
  inconclusivo     could not be decided here
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
import time
from pathlib import Path

DIGEST = ("busybox@sha256:bdf57e528e45e4433820e045b29b4597825a1c9e38353532d"
          "90a01445013f82e")


def run(argv: list[str], timeout: int = 180, env_extra: dict | None = None,
        stdin: str = "") -> dict:
    env = dict(os.environ)
    env["NO_COLOR"] = "1"
    env.update(env_extra or {})
    t0 = time.monotonic()
    try:
        p = subprocess.run(argv, capture_output=True, text=True,
                           timeout=timeout, env=env, input=stdin)
        rc, out, err = p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired:
        rc, out, err = 124, "", "<TIMEOUT>"
    except FileNotFoundError as exc:
        rc, out, err = 127, "", f"<NAO ENCONTRADO: {exc}>"
    return {"argv": argv, "rc": rc, "stdout": out, "stderr": err,
            "dur": round(time.monotonic() - t0, 3)}


def cases(dlx: str, rid: str, root: str) -> list[dict]:
    """Each case: intention + the three argvs + how to read the result."""
    N = f"dif-{rid}"
    dlx_env = {"DELONIX_ROOT": root}

    def c(cid, intent, d_argv, k_argv, p_argv, read, note="", mode_asym=False,
          timeout=180, cleanup=None, setup=None):
        return {"id": cid, "intencao": intent,
                "delonix": d_argv, "docker": k_argv, "podman": p_argv,
                "leitura": read, "nota": note, "assimetria_de_modo": mode_asym,
                "timeout": timeout, "cleanup": cleanup or {},
                "setup": setup or {}, "delonix_env": dlx_env}

    return [
        c("DIF-01", "O utilizador escreve uma flag que nao existe em `run`",
          [dlx, "run", "--flag-que-nao-existe"],
          ["docker", "run", "--flag-que-nao-existe"],
          ["podman", "run", "--flag-que-nao-existe"],
          "a flag tem de ser recusada COMO FLAG; nenhum motor deve tratar-la como operando",
          timeout=60),

        c("DIF-02", "O utilizador escreve uma flag que nao existe em `exec`",
          [dlx, "exec", "--flag-que-nao-existe"],
          ["docker", "exec", "--flag-que-nao-existe"],
          ["podman", "exec", "--flag-que-nao-existe"],
          "idem: recusada como flag, nunca como nome de container",
          timeout=60),

        c("DIF-03", "Passar ao processo um argumento que colide com uma flag do motor",
          [dlx, "run", "--rm", "--net", "none", DIGEST, "echo", "--config", "--rm", "-p", "1:1"],
          ["docker", "run", "--rm", DIGEST, "echo", "--config", "--rm", "-p", "1:1"],
          ["podman", "run", "--rm", DIGEST, "echo", "--config", "--rm", "-p", "1:1"],
          "os quatro tokens tem de chegar intactos ao processo",
          note="`--net none` no delonix evita o holder de rede; nao altera o argv"),

        c("DIF-04", "O codigo de saida do processo atravessa o motor",
          [dlx, "run", "--rm", "--net", "none", DIGEST, "sh", "-c", "exit 37"],
          ["docker", "run", "--rm", DIGEST, "sh", "-c", "exit 37"],
          ["podman", "run", "--rm", DIGEST, "sh", "-c", "exit 37"],
          "rc=37 nos tres"),

        c("DIF-05", "Parar/remover um container que nao existe",
          [dlx, "stop", f"{N}-nao-existe"],
          ["docker", "stop", f"{N}-nao-existe"],
          ["podman", "stop", f"{N}-nao-existe"],
          "classe do erro: delonix promete 4; docker/podman usam 1/125",
          note="divergencia de CODIGO documentada em docs/cli-stability.md",
          timeout=60),

        c("DIF-06", "Criar um volume com um nome ja tomado",
          [dlx, "volume", "create", f"{N}-v"],
          ["docker", "volume", "create", f"{N}-v"],
          ["podman", "volume", "create", f"{N}-v"],
          "delonix promete 5 (conflito); docker e idempotente (rc=0)",
          note="o caso corre DUAS vezes: a segunda e a que conta",
          timeout=60,
          cleanup={"delonix": [dlx, "volume", "rm", f"{N}-v"],
                   "docker": ["docker", "volume", "rm", f"{N}-v"],
                   "podman": ["podman", "volume", "rm", f"{N}-v"]}),

        c("DIF-07", "Listar containers quando nao ha nenhum",
          [dlx, "container", "ps"],
          ["docker", "ps"],
          ["podman", "ps"],
          "cabecalho presente, sem linhas; rc=0",
          mode_asym=True, timeout=60),

        c("DIF-08", "`run -dit` (detached + interactivo + tty), a primeira linha de muitos scripts",
          [dlx, "run", "-dit", "--net", "none", "--name", f"{N}-dit", DIGEST, "sh"],
          ["docker", "run", "-dit", "--name", f"{N}-dit", DIGEST, "sh"],
          ["podman", "run", "-dit", "--name", f"{N}-dit", DIGEST, "sh"],
          "os tres aceitam: paridade. A declaracao de conflito esta na ExecArgs, nao na RunArgs",
          note="MEDIDO: delonix rc=0, docker rc=0, podman rc=0. A primeira leitura "
               "atribuiu o conflito ao `run` e estava errada",
          cleanup={"delonix": [dlx, "container", "rm", "-f", f"{N}-dit"],
                   "docker": ["docker", "rm", "-f", f"{N}-dit"],
                   "podman": ["podman", "rm", "-f", f"{N}-dit"]}),

        c("DIF-09", "Publicar uma porta e ler qual ficou publicada",
          [dlx, "container", "port", f"{N}-p"],
          ["docker", "port", f"{N}-p"],
          ["podman", "port", f"{N}-p"],
          "a porta publicada e legivel pelo mesmo verbo nos tres",
          mode_asym=True, timeout=120,
          setup={"delonix": [dlx, "run", "-d", "-p", "18190:80", "--name", f"{N}-p",
                             DIGEST, "sh", "-c", "sleep 120"],
                 "docker": ["docker", "run", "-d", "-p", "18191:80", "--name", f"{N}-p",
                            DIGEST, "sh", "-c", "sleep 120"],
                 "podman": ["podman", "run", "-d", "-p", "18192:80", "--name", f"{N}-p",
                            DIGEST, "sh", "-c", "sleep 120"]},
          cleanup={"delonix": [dlx, "container", "rm", "-f", f"{N}-p"],
                   "docker": ["docker", "rm", "-f", f"{N}-p"],
                   "podman": ["podman", "rm", "-f", f"{N}-p"]}),

        c("DIF-13", "`exec -dit`: destacado + interactivo + tty no mesmo comando",
          [dlx, "exec", "-dit", f"{N}-x", "sh", "-c", "true"],
          ["docker", "exec", "-dit", f"{N}-x", "sh", "-c", "true"],
          ["podman", "exec", "-dit", f"{N}-x", "sh", "-c", "true"],
          "docker e podman aceitam; o delonix declara detach conflicts_with_all "
          "[interactive, tty] na ExecArgs e RECUSA",
          note="docs/cli-stability.md lista -i/-t como estaveis no `exec` e nao "
               "documenta o conflito com -d",
          timeout=120,
          setup={"delonix": [dlx, "run", "-d", "--net", "none", "--name", f"{N}-x",
                             DIGEST, "sh", "-c", "sleep 120"],
                 "docker": ["docker", "run", "-d", "--name", f"{N}-x",
                            DIGEST, "sh", "-c", "sleep 120"],
                 "podman": ["podman", "run", "-d", "--name", f"{N}-x",
                            DIGEST, "sh", "-c", "sleep 120"]},
          cleanup={"delonix": [dlx, "container", "rm", "-f", f"{N}-x"],
                   "docker": ["docker", "rm", "-f", f"{N}-x"],
                   "podman": ["podman", "rm", "-f", f"{N}-x"]}),

        c("DIF-10", "Inspeccionar uma imagem por digest e obter JSON",
          [dlx, "image", "inspect", DIGEST],
          ["docker", "image", "inspect", DIGEST],
          ["podman", "image", "inspect", DIGEST],
          "os tres emitem JSON valido com uma entrada: paridade",
          note="MEDIDO: `image inspect` existe no delonix e da JSON. A primeira "
               "leitura usou `image describe` (a forma HUMANA) e concluiu "
               "divergencia onde nao ha"),

        c("DIF-14", "`image describe`: a forma humana que o docker nao tem",
          [dlx, "image", "describe", DIGEST],
          ["docker", "image", "describe", DIGEST],
          ["podman", "image", "describe", DIGEST],
          "sem equivalente: docker/podman nao tem um `describe` de imagem",
          note="docs/cli-stability.md: «inspect e diff sao TYPE-SCOPED»"),

        c("DIF-11", "`--rm` remove mesmo o container depois de sair",
          [dlx, "container", "ps", "-a"],
          ["docker", "ps", "-a"],
          ["podman", "ps", "-a"],
          "nenhum dos tres deve listar o container do DIF-04 (corrido com --rm)",
          timeout=60),

        c("DIF-12", "`logs` de um container detached",
          [dlx, "container", "logs", f"{N}-l"],
          ["docker", "logs", f"{N}-l"],
          ["podman", "logs", f"{N}-l"],
          "a linha escrita pelo processo aparece nos tres",
          timeout=120,
          setup={"delonix": [dlx, "run", "-d", "--net", "none", "--name", f"{N}-l",
                             DIGEST, "sh", "-c", f"echo marca-{rid}; sleep 120"],
                 "docker": ["docker", "run", "-d", "--name", f"{N}-l",
                            DIGEST, "sh", "-c", f"echo marca-{rid}; sleep 120"],
                 "podman": ["podman", "run", "-d", "--name", f"{N}-l",
                            DIGEST, "sh", "-c", f"echo marca-{rid}; sleep 120"]},
          cleanup={"delonix": [dlx, "container", "rm", "-f", f"{N}-l"],
                   "docker": ["docker", "rm", "-f", f"{N}-l"],
                   "podman": ["podman", "rm", "-f", f"{N}-l"]}),
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--delonix", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--run-id", required=True)
    ap.add_argument("--root", required=True)
    args = ap.parse_args()

    out = Path(args.out); out.mkdir(parents=True, exist_ok=True)
    rows = []
    for case in cases(args.delonix, args.run_id, args.root):
        rec = {k: case[k] for k in ("id", "intencao", "leitura", "nota",
                                     "assimetria_de_modo")}
        for engine in ("delonix", "docker", "podman"):
            env = case["delonix_env"] if engine == "delonix" else {}
            su = (case.get("setup") or {}).get(engine)
            if su:
                sr = run(su, timeout=case["timeout"], env_extra=env)
                rec.setdefault("setup", {})[engine] = {
                    "rc": sr["rc"],
                    "erro": (sr["stderr"] or "").strip()[:200],
                }
            r = run(case[engine], timeout=case["timeout"], env_extra=env)
            if case["id"] == "DIF-06":  # the SECOND call is the one that counts
                r = run(case[engine], timeout=case["timeout"], env_extra=env)
            rec[engine] = {
                "argv": " ".join(shlex.quote(a) for a in case[engine]),
                "rc": r["rc"],
                "primeira_linha": (r["stdout"] or r["stderr"]).strip().splitlines()[:1],
                "stdout": r["stdout"][:1500],
                "stderr": r["stderr"][:1500],
                "dur": r["dur"],
            }
        for engine, cl in (case["cleanup"] or {}).items():
            run(cl, timeout=120,
                env_extra=case["delonix_env"] if engine == "delonix" else {})
        rows.append(rec)
        d, k, p = rec["delonix"]["rc"], rec["docker"]["rc"], rec["podman"]["rc"]
        print(f"{rec['id']}  delonix={d:<4} docker={k:<4} podman={p:<4}  {rec['intencao'][:52]}")

    json.dump(rows, open(out / "differential.json", "w"), indent=1,
              ensure_ascii=False)
    print(f"\n{len(rows)} casos em {out / 'differential.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
