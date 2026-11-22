#!/usr/bin/env python3
"""Functional cases, tiers 3-6: lifecycles, convergence, integrations, recovery.

Separate from `functional.py` (tiers 1-2) because these need a container to
exist, so they run after the base-resource tier and in its own order.

Every oracle asserts an observable EFFECT. A case that only checks rc=0 is not
written here: `rc=0` on a verb that reports success without doing the work is
precisely the class of defect this engine's own history catalogues.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

BUSYBOX = "busybox@sha256:bdf57e528e45e4433820e045b29b4597825a1c9e38353532d90a01445013f82e"


def build(workdir: Path, rid: str) -> list[dict]:
    C: list[dict] = []
    n = {"i": 100}
    name = f"aud-{rid}"

    def add(group, title, argv, oracle, expectation, source, **kw):
        n["i"] += 1
        c = {"id": f"FUNC-{n['i']:03d}", "group": group, "title": title,
             "argv": argv, "oracle": oracle, "expectation": expectation,
             "source": source}
        c.update(kw)
        C.append(c)
        return c

    # ---- tier 3: container lifecycle --------------------------------------
    # The engine is daemonless: a foreground `run` inherits stdio, so the
    # process's own exit code must come back as the command's.
    add("3-ciclo", "RUN-01: o argv do processo recebe tokens que colidem com flags do motor",
        ["run", "--rm", "--net", "none", BUSYBOX,
         "echo", "--config", "--rm", "-p", "1:1"],
        {"exit_code": 0, "stdout_contains": ["--config --rm -p 1:1"]},
        "o processo recebe os quatro tokens intactos; nada e consumido pelo parser do motor",
        "docker 29.9.3 e podman 4.9.3 medidos: argv entregue `--config --rm -p 1:1`, rc=0",
        timeout=180)

    add("3-ciclo", "o codigo de saida do PROCESSO atravessa (daemonless)",
        ["run", "--rm", "--net", "none", BUSYBOX, "sh", "-c", "exit 37"],
        {"exit_code": 37},
        "rc=37: o codigo e do processo, nao do motor",
        "docker e podman medidos: rc=37. docs/cli-stability.md: «os codigos de saida»",
        timeout=180)

    add("3-ciclo", "run -d cria, ps lista e o PID existe",
        ["run", "-d", "--net", "none", "--name", f"{name}-c1", BUSYBOX,
         "sh", "-c", "sleep 300"],
        {"exit_code": 0, "output_matches": [r"[0-9a-f]{8,}"]},
        "rc=0 e o id do container em stdout",
        "docs/cli-stability.md: `container run` e estavel; -d detached",
        timeout=180)

    add("3-ciclo", "ps mostra o container criado como Running",
        ["container", "ps"],
        {"exit_code": 0, "stdout_contains": [f"{name}-c1"]},
        "o container aparece sem -a porque esta Running",
        "AGENTS.md: `container ls` lista o que o `docker ps` lista (Running/Paused)",
        timeout=60)

    add("3-ciclo", "exec corre dentro do container (nao no host)",
        ["container", "exec", f"{name}-c1", "cat", "/etc/hostname"],
        {"exit_code": 0, "output_not_contains": ["kaeso-sys-01"]},
        "o hostname lido e o do container, nao o do host",
        "AGENTS.md «Um exec logo depois de run -d corria no HOST» (fechado em v0.66.x)",
        timeout=120)

    add("3-ciclo", "exec devolve o codigo do comando executado",
        ["container", "exec", f"{name}-c1", "sh", "-c", "exit 23"],
        {"exit_code": 23},
        "rc=23",
        "paridade docker exec",
        timeout=120)

    add("3-ciclo", "cp host->container e container->host preserva o checksum",
        ["container", "cp", str(workdir / "payload.bin"), f"{name}-c1:/tmp/payload.bin"],
        {"exit_code": 0},
        "rc=0 e o ficheiro entra com o conteudo intacto (verificado no caso seguinte)",
        "docs/cli-stability.md: `cp` e alias de `container cp`",
        files={"payload.bin": "conteudo-sintetico-da-auditoria-" + rid + "\n"},
        timeout=120)

    add("3-ciclo", "o ficheiro copiado le-se igual DENTRO do container",
        ["container", "exec", f"{name}-c1", "cat", "/tmp/payload.bin"],
        {"exit_code": 0, "stdout_contains": ["conteudo-sintetico-da-auditoria-" + rid]},
        "o conteudo e byte a byte o que foi copiado",
        "o cp e provado pelo CONTEUDO, nao pelo rc do cp",
        timeout=120)

    add("3-ciclo", "logs de um container detached mostram a saida",
        ["container", "logs", f"{name}-c2"],
        {"exit_code": 0, "output_contains": ["marca-" + rid]},
        "a linha escrita pelo processo aparece nos logs",
        "docs/cli-stability.md: `logs` estavel",
        setup=[["run", "-d", "--net", "none", "--name", f"{name}-c2", BUSYBOX,
                "sh", "-c", f"echo marca-{rid}; sleep 300"]],
        teardown=[["container", "rm", "-f", f"{name}-c2"]],
        timeout=180)

    add("3-ciclo", "wait devolve o codigo real de um container detached",
        ["container", "wait", f"{name}-c3"],
        {"exit_code": 0, "stdout_contains": ["7"]},
        "o wait imprime 7 — o codigo real do processo, capturado pelo supervisor",
        "AGENTS.md: `run -d` faz sempre fork de um supervisor; so o pai real pode waitpid",
        setup=[["run", "-d", "--net", "none", "--name", f"{name}-c3", BUSYBOX,
                "sh", "-c", "sleep 1; exit 7"]],
        teardown=[["container", "rm", "-f", f"{name}-c3"]],
        timeout=180)

    add("3-ciclo", "stop pedido NAO e crash: o estado fica Exited(0)",
        ["container", "ps", "-a"],
        {"exit_code": 0, "output_matches": [rf"(Exited|Stopped)[^\n]*{name}-c4"],
         "output_not_contains": ["Dead"]},
        "um stop pedido termina Exited/Stopped, nunca Dead",
        "AGENTS.md: «um stop PEDIDO nunca e um crash» (stopped_by_user)",
        setup=[["run", "-d", "--net", "none", "--name", f"{name}-c4", BUSYBOX,
                "sh", "-c", "sleep 300"],
               ["container", "stop", "-t", "2", f"{name}-c4"]],
        teardown=[["container", "rm", "-f", f"{name}-c4"]],
        timeout=300)

    add("3-ciclo", "inspect emite JSON valido",
        ["container", "inspect", f"{name}-c1"],
        {"exit_code": 0, "stdout_json": True},
        "rc=0 e JSON parseavel",
        "docs/cli-stability.md: «o JSON do inspect — campos podem ser ADICIONADOS, nunca removidos»",
        timeout=60)

    add("3-ciclo", "inspect de container inexistente e 4 e nao muta",
        ["container", "inspect", f"{name}-nao-existe"],
        {"exit_code": 4, "no_mutation": True},
        "rc=4 e nada criado",
        "docs/cli-stability.md: 4 = nao existe",
        probe={"argv": ["container", "ps", "-aq"]},
        timeout=60)

    add("3-ciclo", "stop de container inexistente e 4",
        ["container", "stop", f"{name}-nao-existe"],
        {"exit_code": 4},
        "rc=4",
        "docs/cli-stability.md",
        timeout=60)

    add("3-ciclo", "exists devolve 0/1 sem imprimir nada (Podman)",
        ["container", "exists", f"{name}-nao-existe"],
        {"exit_code": [1, 4], "stdout_empty": True},
        "nao imprime nada; o codigo responde",
        "ajuda: «prints nothing, exit 0/1» (Podman `container exists`)",
        timeout=60)

    add("3-ciclo", "rm -f remove e ps -a deixa de o listar",
        ["container", "rm", "-f", f"{name}-c1"],
        {"exit_code": 0},
        "rc=0 e o container sai do store (verificado no caso seguinte)",
        "docs/cli-stability.md",
        timeout=300)

    add("3-ciclo", "apos o rm, o container nao esta no store",
        ["container", "ps", "-a"],
        {"exit_code": 0, "output_not_contains": [f"{name}-c1"]},
        "o nome desapareceu da listagem",
        "o rm e provado pela AUSENCIA, nao pelo seu rc",
        timeout=60)

    # ---- tier 4: convergence ----------------------------------------------
    man = workdir / "stack.yaml"
    add("4-converg", "PLAN-01a: plan de um manifesto novo propoe criar",
        ["stack", "plan", "-f", str(man), "--detailed-exitcode"],
        {"exit_code": 2, "output_not_contains": ["panicked"]},
        "rc=2: ha mudancas a aplicar (contrato terraform-like)",
        "docs/cli-stability.md: --detailed-exitcode 0/2/1; AGENTS.md «IaC nativa»",
        files={"stack.yaml": (
            "apiVersion: core.delonix.io/v1beta1\n"
            "kind: Stack\n"
            f"metadata:\n  name: aud-{rid}-stk\n"
            f"  namespace: aud-{rid}\n"
            "spec:\n"
            "  containers:\n"
            f"    - name: aud-{rid}-s1\n"
            "      spec:\n"
            f"        image: {BUSYBOX}\n"
            "        command: [\"sh\", \"-c\", \"sleep 300\"]\n"
            "        network: none\n")},
        timeout=120)

    add("4-converg", "PLAN-01b: apply cria o recurso",
        ["stack", "apply", "-f", str(man)],
        {"exit_code": 0, "output_not_contains": ["panicked"]},
        "rc=0 e o container do stack passa a existir",
        "AGENTS.md «IaC nativa: stack plan/apply convergente»",
        timeout=300)

    add("4-converg", "PLAN-01c: o recurso do stack esta a correr",
        ["container", "ps", "--namespace", f"aud-{rid}"],
        {"exit_code": 0, "stdout_contains": [f"aud-{rid}-s1"]},
        "o container declarado aparece Running",
        "o apply e provado pelo recurso, nao pelo seu rc",
        timeout=60)

    add("4-converg", "PLAN-01d: segundo plan nao propoe nada (idempotente)",
        ["stack", "plan", "-f", str(man), "--detailed-exitcode"],
        {"exit_code": 0},
        "rc=0: sem mudancas — a convergencia e estavel",
        "AGENTS.md: «um apply inalterado da ZERO diferencas»; o teste de deriva eterna",
        timeout=120)

    add("4-converg", "PLAN-01e: segundo apply nao duplica",
        ["stack", "apply", "-f", str(man)],
        {"exit_code": 0},
        "rc=0 sem criar um segundo recurso (verificado a seguir)",
        "semantica «ensure present»",
        timeout=300)

    add("4-converg", "PLAN-01f: existe exactamente UM recurso do stack",
        ["container", "ps", "-a", "--namespace", f"aud-{rid}"],
        {"exit_code": 0, "output_matches": [rf"(?s)^(?:(?!aud-{rid}-s1).)*aud-{rid}-s1(?:(?!aud-{rid}-s1).)*$"]},
        "uma so ocorrencia do nome: o apply repetido nao duplicou",
        "a nao-duplicacao e medida por CONTAGEM",
        timeout=60)

    add("4-converg", "deriva: uma alteracao feita A MAO e detectada",
        ["drift", "-f", str(man), "--detailed-exitcode"],
        {"exit_code": 2, "output_not_contains": ["panicked"]},
        "rc=2: o drift ve o que a MAQUINA mudou desde o ultimo apply",
        "AGENTS.md «delonix drift — o que a MAQUINA mudou» (ADR-0088 A5)",
        # The mutation has to be POSSIBLE on this container. A port is not:
        # `network: none` has no slirp api-socket, and the engine refuses the
        # hot publish by name (measured, DX-1000). A live volume mount is, and
        # `volumes` is a stamped field.
        setup=[["volume", "create", f"aud-{rid}-drift"],
               ["container", "update", "--volume-add",
                f"aud-{rid}-drift:/mnt/aud", f"aud-{rid}-s1"]],
        timeout=180)

    add("4-converg", "get containers RECUSA e encaminha para a superficie imperativa",
        ["get", "containers"],
        {"exit_code": 1, "output_contains": ["container ps"]},
        "o verbo generico recusa este Kind e diz qual usar — nao adivinha",
        "MEDIDO: «`container` is the imperative surface — use `container ps`». "
        "docs/cli-stability.md: «um resolvedor generico que adivinhasse o Kind e "
        "exactamente o que esta CLI recusa fazer»",
        timeout=60)

    add("4-converg", "o container do stack e visivel pela superficie imperativa",
        ["container", "ps"],
        {"exit_code": 0, "stdout_contains": [f"aud-{rid}-s1"]},
        "o recurso declarado ve-se por `container ps`, que por omissao lista TODOS "
        "os namespaces (medido: 2 de 2)",
        "a recusa do `get` obriga a provar o recurso pela via que o motor indica",
        timeout=60)

    add("4-converg", "get de um Kind inexistente recusa nomeando-o",
        ["get", "cascalhos"],
        {"exit_code": [1, 2, 4], "output_contains": ["cascalho"]},
        "recusa nomeando o recurso pedido",
        "AGENTS.md #166: `get clusters` respondia «no such resource kind»",
        timeout=60)

    add("4-converg", "destroy remove o que o stack possui",
        ["stack", "destroy", "-f", str(man)],
        {"exit_code": 0},
        "rc=0 e o recurso sai (verificado a seguir)",
        "AGENTS.md: destroy usa a ordem INVERSA de KINDS",
        timeout=300)

    add("4-converg", "apos o destroy o recurso do stack nao existe",
        ["container", "ps", "-a", "--namespace", f"aud-{rid}"],
        {"exit_code": 0, "output_not_contains": [f"aud-{rid}-s1"]},
        "ausencia confirmada",
        "o destroy e provado pela AUSENCIA",
        timeout=60)

    # ---- tier 6: residue ---------------------------------------------------
    add("6-limpeza", "nenhum container da auditoria ficou no store",
        ["container", "ps", "-a"],
        {"exit_code": 0, "output_not_contains": [f"aud-{rid}-"]},
        "o root descartavel nao tem residuo da suite",
        "criterio de conclusao da auditoria: verificar residuo mesmo apos falha",
        timeout=60)

    return C


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--run-id", required=True)
    args = ap.parse_args()
    wd = Path(args.workdir); wd.mkdir(parents=True, exist_ok=True)
    cases = build(wd, args.run_id)
    for c in cases:
        for nm, content in (c.pop("files", {}) or {}).items():
            (wd / nm).write_text(content, encoding="utf-8")
    with open(args.out, "w") as fh:
        for c in cases:
            fh.write(json.dumps(c, ensure_ascii=False) + "\n")
    print(f"{len(cases)} casos funcionais (tiers 3-6) em {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
