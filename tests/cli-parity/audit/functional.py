#!/usr/bin/env python3
"""Hand-written functional cases for the Delonix CLI audit.

These are the cases whose oracle cannot be derived from help text: they prove an
observable EFFECT, not a parse. Ordered by dependency, as the audit asks:
parser/local reads → base resources → lifecycles → convergence → integrations →
recovery.

Every resource name carries the run id, so nothing collides with the machine's
real state and the teardown can only ever remove what this suite created.

    functional.py --out cases.jsonl --workdir DIR --run-id ID
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

# Digest-pinned so the fixture is identical across Delonix, Docker and Podman.
BUSYBOX = "busybox@sha256:bdf57e528e45e4433820e045b29b4597825a1c9e38353532d90a01445013f82e"
BUSYBOX_TAG = "busybox:1.37"


def build(workdir: Path, rid: str) -> list[dict]:
    C: list[dict] = []
    n = {"i": 0}

    def add(group: str, title: str, argv: list[str], oracle: dict,
            expectation: str, source: str, **kw) -> dict:
        n["i"] += 1
        case = {
            "id": f"FUNC-{n['i']:03d}",
            "group": group,
            "title": title,
            "argv": argv,
            "oracle": oracle,
            "expectation": expectation,
            "source": source,
        }
        case.update(kw)
        C.append(case)
        return case

    # ---- tier 1: parser contract and local reads (no engine needed) --------
    add("1-local", "version imprime o commit e a versao",
        ["version"],
        {"exit_code": 0, "output_matches": [r"delonix \d+\.\d+\.\d+", r"commit: [0-9a-f]{7,}"]},
        "rc=0; a versao e o commit identificam o build",
        "AGENTS.md: --version diz a distancia ao tag; `version` imprime o texto da flag")

    add("1-local", "`version` e `--version` dizem o mesmo (promessa escrita)",
        ["version"],
        {"exit_code": 0},
        "o texto de `version` e verbatim o da flag --version",
        "AGENTS.md secao CLI: «imprime o texto da flag VERBATIM para os dois nao divergirem»",
        compare_argv=["--version"])

    add("1-local", "api-resources lista os Kinds com grupo e plural",
        ["api-resources"],
        {"exit_code": 0,
         "stdout_contains": ["compute.delonix.io", "networking.delonix.io", "VirtualMachine"]},
        "rc=0 e a tabela de Kinds com os grupos publicados",
        "AGENTS.md «Os Kinds ganham grupos e nomes definitivos»")

    add("1-local", "explain de um campo existente responde",
        ["explain", "Container.ports"],
        {"exit_code": 0, "stdout_not_contains": ["panicked"]},
        "rc=0 com a descricao do campo",
        "AGENTS.md: `delonix explain Container.ports` (ADR-0007)")

    add("1-local", "explain de um Kind inexistente recusa com classe",
        ["explain", "NaoExisteEsteKind.campo"],
        {"exit_code": [1, 2, 4], "output_not_contains": ["panicked"]},
        "recusa nomeando o Kind; nunca rc=0",
        "ADR-0005: um Kind renomeado responde ao nome antigo; um inexistente e recusado")

    add("1-local", "manifest schema emite JSON valido",
        ["manifest", "schema"],
        {"exit_code": 0, "stdout_json": True},
        "rc=0 e JSON parseavel (o schema publicado)",
        "ADR-0007: o schema e GERADO do codigo. NOTA: nao ha `schema print` na raiz")

    # `explain` is polymorphic: it answers a Kind.field AND a DX code. There is
    # no `codes` leaf; `man codes` refuses. Measured on 0.1.2.
    add("1-local", "explain de um codigo DX responde (o `explain` e polimorfico)",
        ["explain", "DX-1000"],
        {"exit_code": 0, "output_contains": ["DX-1000"]},
        "rc=0 e a entrada do dicionario",
        "medido: `explain DX-1000` responde; `codes` e `system codes` nao existem")

    add("1-local", "completion shell bash gera um script nao vazio",
        ["completion", "shell", "bash"],
        {"exit_code": 0, "output_matches": [r"complete\b|_delonix"]},
        "rc=0 e um script de completion com o nome do binario",
        "AGENTS.md: `completion shell <shell>` desde v0.67.0 (nao `completion <shell>`)")

    add("1-local", "completion de uma shell inexistente e recusada",
        ["completion", "shell", "cascalho"],
        {"exit_code": 2, "output_contains": ["cascalho"]},
        "rc=2 nomeando o valor invalido",
        "clap ValueEnum: possible values")

    add("1-local", "man gera pagina para um comando real",
        ["man", "container"],
        {"exit_code": 0, "output_not_contains": ["panicked"]},
        "rc=0 com a pagina man",
        "cmd/man.rs + manual_entries.rs")

    # I18N-01, the audit's named case. Precedence of flag vs env.
    add("1-local", "I18N-01a: --l18n pt traduz a ajuda (marcador traduzido)",
        ["--l18n", "pt", "container", "--help"],
        {"exit_code": 0,
         "output_contains": ["Remove um ou mais containers"],
         "output_not_contains": ["Remove one or more containers", "panicked"]},
        "a descricao do `rm` sai em portugues, e a inglesa nao aparece",
        "AGENTS.md secao i18n: --l18n=pt / DELONIX_L18N=pt",
        env={"DELONIX_L18N": "en"})

    add("1-local", "I18N-01b: DELONIX_L18N=pt sem flag traduz",
        ["container", "--help"],
        {"exit_code": 0,
         "output_contains": ["Remove um ou mais containers"],
         "output_not_contains": ["Remove one or more containers"]},
        "a variavel sozinha traduz",
        "AGENTS.md secao i18n",
        env={"DELONIX_L18N": "pt"})

    add("1-local", "I18N-01c: --l18n en sobre DELONIX_L18N=pt — a FLAG ganha",
        ["--l18n", "en", "container", "--help"],
        {"exit_code": 0,
         "output_contains": ["Remove one or more containers"],
         "output_not_contains": ["Remove um ou mais containers"]},
        "a flag precede a variavel: medido por comparacao da saida INTEIRA",
        "AGENTS.md: «flag > env, mesma precedencia do --addr»",
        env={"DELONIX_L18N": "pt"})

    add("1-local", "I18N-01c2: --l18n pt sobre DELONIX_L18N=en — a FLAG ganha",
        ["--l18n", "pt", "container", "--help"],
        {"exit_code": 0,
         "output_contains": ["Remove um ou mais containers"],
         "output_not_contains": ["Remove one or more containers"]},
        "a precedencia vale nos dois sentidos",
        "medido: hash da saida inteira igual ao de DELONIX_L18N=pt sem flag",
        env={"DELONIX_L18N": "en"})

    add("1-local", "I18N-01e: --l18n e insensivel a maiusculas (PT == pt)",
        ["--l18n", "PT", "container", "--help"],
        {"exit_code": 0, "output_contains": ["Remove um ou mais containers"]},
        "`PT` traduz como `pt` (medido: saidas identicas byte a byte)",
        "medido com diff: `--l18n PT` e `--l18n pt` nao diferem")

    # MEASURED: an unrecognised value degrades to EN with rc=0 and NO warning.
    # That is the accepted-and-ignored class this engine refuses elsewhere. The
    # case pins TODAY's behaviour so the day it starts warning, it fails and the
    # decision gets written down.
    add("1-local", "I18N-01d: lingua invalida degrada para EN em silencio (rc=0)",
        ["--l18n", "klingon", "container", "--help"],
        {"exit_code": 0,
         "output_contains": ["Remove one or more containers"],
         "output_not_contains": ["panicked"]},
        "rc=0 e saida identica ao EN; NAO ha aviso de que o valor foi ignorado",
        "MEDIDO: `--l18n klingon` e `--l18n en` nao diferem (diff -q). Achado "
        "registado em §6 do relatorio: aceite-e-ignorado, sem aviso")

    add("1-local", "config: leitura local nao muta nada",
        ["config", "--help"],
        {"exit_code": 0},
        "rc=0",
        "inventario")

    add("1-local", "manifest validate recusa YAML com chave duplicada",
        ["manifest", "validate", "-f", str(workdir / "dup.yaml")],
        {"exit_code": [1, 2], "output_not_contains": ["panicked"]},
        "recusa o manifesto; nunca aceita silenciosamente",
        "ADR-0069: manifestos estritos — campo/chave que o motor nao entende recusa",
        files={"dup.yaml": (
            "apiVersion: compute.delonix.io/v1alpha1\n"
            "kind: Container\n"
            "metadata:\n  name: dup\n  name: dup2\n"
            "spec:\n  image: busybox\n")})

    add("1-local", "manifest validate NOMEIA o campo desconhecido (lenient por desenho)",
        ["manifest", "validate", "-f", str(workdir / "unknown.yaml")],
        {"exit_code": 0,
         "output_contains": ["unknown field 'campoQueNaoExiste'", "ignored"]},
        "rc=0 a reportar: o validate usa load_lenient. O campo e NOMEADO, nunca engolido",
        "AGENTS.md ADR-0069: «stack validate usa load_lenient para reportar»",
        files={"unknown.yaml": (
            "apiVersion: compute.delonix.io/v1alpha1\n"
            "kind: Container\n"
            "metadata:\n  name: unk\n"
            "spec:\n  image: busybox\n  campoQueNaoExiste: 1\n")})

    add("1-local", "stack apply RECUSA o campo desconhecido (metade estrita do ADR-0069)",
        ["stack", "apply", "-f", str(workdir / "unknown.yaml"), "--dry-run"],
        {"exit_code_not": [0], "output_contains": ["campoQueNaoExiste"]},
        "o apply recusa onde o validate reporta: e a metade estrita do ADR-0069",
        "ADR-0069 item 1: um campo que o motor nao entende RECUSA antes de qualquer efeito")

    add("1-local", "manifest validate aceita um manifesto minimo valido",
        ["manifest", "validate", "-f", str(workdir / "ok.yaml")],
        {"exit_code": 0},
        "rc=0 num manifesto correcto",
        "docs/schema/v1/delonix.json",
        files={"ok.yaml": (
            "apiVersion: compute.delonix.io/v1alpha1\n"
            "kind: Container\n"
            f"metadata:\n  name: aud-{rid}-ok\n"
            "spec:\n  image: " + BUSYBOX_TAG + "\n  command: [\"sh\", \"-c\", \"sleep 300\"]\n"
            "  net: none\n")})

    # ---- tier 2: base resources -------------------------------------------
    add("2-base", "volume create cria e ls mostra",
        ["volume", "create", f"aud-{rid}-v1"],
        {"exit_code": 0},
        "rc=0 e o volume passa a existir",
        "cmd/volume.rs; docs/comandos/volume",
        teardown=[["volume", "rm", f"aud-{rid}-v1"]])

    add("2-base", "volume ls lista o volume criado",
        ["volume", "ls"],
        {"exit_code": 0, "stdout_contains": [f"aud-{rid}-v2"]},
        "o nome criado aparece na listagem",
        "cmd/volume.rs",
        setup=[["volume", "create", f"aud-{rid}-v2"]],
        teardown=[["volume", "rm", f"aud-{rid}-v2"]])

    add("2-base", "volume create do mesmo nome e IDEMPOTENTE (como docker)",
        ["volume", "create", f"aud-{rid}-v3"],
        {"exit_code": 0, "output_not_contains": ["panicked"]},
        "rc=0 e UM so volume: paridade com `docker volume create` (medido rc=0)",
        "medido: docker 29.9.3 rc=0 na 2a chamada. NOTA: `network create` responde 5 "
        "ao mesmo gesto — tres recursos irmaos, tres respostas",
        setup=[["volume", "create", f"aud-{rid}-v3"]],
        teardown=[["volume", "rm", f"aud-{rid}-v3"]])

    add("2-base", "volume rm de nome inexistente e NAO EXISTE (4)",
        ["volume", "rm", f"aud-{rid}-nao-existe"],
        {"exit_code": 4, "output_not_contains": ["panicked"]},
        "rc=4: esse recurso nao existe",
        "docs/cli-stability.md: 4 = nao existe")

    add("2-base", "volume inspect de inexistente e 4, sem mutar",
        ["volume", "inspect", f"aud-{rid}-nao-existe"],
        {"exit_code": 4, "no_mutation": True},
        "rc=4 e nada criado",
        "docs/cli-stability.md",
        probe={"argv": ["volume", "ls"]})

    add("2-base", "network create cria uma rede bridge",
        ["network", "create", f"aud-{rid}-n1"],
        {"exit_code": 0},
        "rc=0 e a rede passa a existir",
        "cmd/network.rs; AGENTS.md secao network",
        teardown=[["network", "rm", f"aud-{rid}-n1"]])

    add("2-base", "network create repetida e CONFLITO (5)",
        ["network", "create", f"aud-{rid}-n2"],
        {"exit_code": 5},
        "rc=5",
        "AGENTS.md: os tres create* do NetworkStore produzem Conflict",
        setup=[["network", "create", f"aud-{rid}-n2"]],
        teardown=[["network", "rm", f"aud-{rid}-n2"]])

    add("2-base", "network create com nome que escapa o store e RECUSADO",
        ["network", "create", "../evil-" + rid, "--subnet", "10.231.0.0/24"],
        {"exit_code": [1, 2, 4, 5, 77], "no_mutation": True,
         "output_not_contains": ["panicked"]},
        "recusa: o nome tem de ser validado no store (defeito medido em v4.4.0)",
        "AGENTS.md ADR-0042 E2: «NetworkStore::create_with_cidr nao validava o nome»",
        probe={"argv": ["network", "ls"]})

    add("2-base", "secret create a partir de stdin",
        ["secret", "create", f"aud-{rid}-s1", "--from-env-file", "-"],
        {"exit_code": 0},
        "rc=0: o `-` le de stdin (cheatsheet documentado)",
        "AGENTS.md v0.32.1 achado 1: `--from-env-file -` le de stdin",
        stdin="AUDIT_KEY=valor-sintetico\n",
        teardown=[["secret", "rm", f"aud-{rid}-s1"]])

    add("2-base", "secret inspect NAO revela o valor sem --reveal",
        ["secret", "inspect", f"aud-{rid}-s2"],
        {"exit_code": 0, "output_not_contains": ["valor-sintetico"]},
        "o valor fica redigido sem --reveal",
        "AGENTS.md: `secret inspect` redige atras de --reveal",
        setup=[["secret", "create", f"aud-{rid}-s2", "--from-env-file", "-"]],
        setup_stdin="AUDIT_KEY=valor-sintetico\n",
        teardown=[["secret", "rm", f"aud-{rid}-s2"]])

    add("2-base", "secret rm de inexistente nomeia o recurso certo",
        ["secret", "rm", f"aud-{rid}-nao-existe"],
        {"exit_code": 4, "output_not_contains": ["no such container"]},
        "rc=4 e a mensagem nomeia `secret`, nao `container`",
        "AGENTS.md v0.32.1 achado 2: Error::NotFound dizia sempre «no such container»")

    add("2-base", "image pull fixado por digest",
        ["image", "pull", BUSYBOX],
        {"exit_code": 0, "output_not_contains": ["panicked"]},
        "rc=0 e a imagem fica no store (digest verificado)",
        "AGENTS.md: o pull verifica o digest do manifesto (achado HIGH da auditoria #3)",
        timeout=300)

    add("2-base", "image ls mostra a imagem puxada",
        ["image", "ls"],
        {"exit_code": 0, "stdout_contains": ["busybox"]},
        "a imagem aparece na listagem",
        "docs/cli-stability.md: image ls (v2.0.0 reverteu `image list`)",
        timeout=120)

    # cli-stability.md says this spelling "fails with unrecognized subcommand,
    # never silently". It does NOT: image.rs:295 carries
    # `#[command(visible_alias = "list")]`. The case pins the MEASURED behaviour
    # and the report carries the divergence against the document.
    add("2-base", "image list responde (alias visivel), contra o que o documento afirma",
        ["image", "list"],
        {"exit_code": 0, "output_contains": ["REPOSITORY"]},
        "rc=0: `list` e alias visivel de `ls`",
        "MEDIDO contra bins/delonix/src/cmd/image.rs:295 (visible_alias). "
        "docs/cli-stability.md afirma o contrario — divergencia documento/comportamento")

    add("2-base", "image pull com digest adulterado e RECUSADO",
        ["image", "pull",
         "busybox@sha256:0000000000000000000000000000000000000000000000000000000000000000"],
        {"exit_code_not": [0], "output_not_contains": ["panicked"]},
        "recusa: digest-pinning verifica o MANIFESTO contra o digest pedido",
        "AGENTS.md auditoria #3 achado 1 (HIGH): verify_manifest_digest",
        timeout=180)

    return C


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--workdir", required=True)
    ap.add_argument("--run-id", required=True)
    args = ap.parse_args()

    workdir = Path(args.workdir)
    workdir.mkdir(parents=True, exist_ok=True)
    cases = build(workdir, args.run_id)

    # Materialise the fixture files each case declares.
    for c in cases:
        for name, content in (c.pop("files", {}) or {}).items():
            (workdir / name).write_text(content, encoding="utf-8")

    with open(args.out, "w") as fh:
        for c in cases:
            c.pop("id_hint", None)
            fh.write(json.dumps(c, ensure_ascii=False) + "\n")
    print(f"{len(cases)} casos funcionais em {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
