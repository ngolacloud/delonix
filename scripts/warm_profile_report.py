#!/usr/bin/env python3
"""Relatório do perfil do plano morno (ADR-0095).

Lê o JSONL que o `delonix __poolbench` escreve (profiler + bench) e emparelha,
por corrida, a linha `parent` (spawn_total medido dentro do `spawn`) com a linha
`bench` imediatamente a seguir (wall da corrida). Dá, por fase (cold/warm):

    pre_spawn = wall − spawn_total

que é a MESMA definição do `spawn_profile_report.py` — o custo fora da mecânica
do container (processo + resolução de imagem/rootfs). O ponto do plano morno é
fazer o `pre_spawn` cair de ~58 ms (cold) para perto de 0 (warm).

Uso: warm_profile_report.py <ficheiro.jsonl>
"""
import json
import statistics
import sys


def load_pairs(path):
    """Por corrida a sequência no ficheiro é `prespawn`, `parent`, `bench`.

    Emparelha cada `bench` com a `parent` e a `prespawn` imediatamente anteriores.
    Devolve uma lista de dicts por corrida.
    """
    pending_parent = None
    pending_prespawn = None
    runs = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            role = rec.get("role")
            if role == "prespawn":
                pending_prespawn = rec
            elif role == "parent":
                pending_parent = rec
            elif role == "bench":
                runs.append(
                    {
                        "phase": rec.get("phase", "?"),
                        "wall_us": rec.get("wall_us"),
                        "spawn_total_us": pending_parent.get("total_us")
                        if pending_parent
                        else None,
                        "prespawn_total_us": pending_prespawn.get("total_us")
                        if pending_prespawn
                        else None,
                        "prespawn_phases": pending_prespawn.get("phases_us", {})
                        if pending_prespawn
                        else {},
                    }
                )
                pending_parent = None
                pending_prespawn = None
    return runs


def stats(values):
    vs = sorted(v for v in values if v is not None)
    if not vs:
        return None
    return {
        "n": len(vs),
        "min": vs[0] / 1000.0,
        "median": statistics.median(vs) / 1000.0,
        "max": vs[-1] / 1000.0,
    }


def phase_report(runs):
    """runs: lista de dicts de corrida. Devolve dict com as métricas por fase."""
    walls = [r["wall_us"] for r in runs]
    spawn_totals = [r["spawn_total_us"] for r in runs]
    prespawn_totals = [r["prespawn_total_us"] for r in runs]
    pre = [
        r["wall_us"] - r["spawn_total_us"]
        for r in runs
        if r["wall_us"] is not None and r["spawn_total_us"] is not None
    ]
    # Média por sub-fase do pre_spawn (policy_preflight, claims, resolve_rootfs, ...).
    subphases = {}
    for r in runs:
        for k, v in r.get("prespawn_phases", {}).items():
            subphases.setdefault(k, []).append(v)
    return {
        "wall": stats(walls),
        "spawn_total": stats(spawn_totals),
        "prespawn_total": stats(prespawn_totals),
        "pre_spawn": stats(pre),
        "subphases": {k: stats(v) for k, v in subphases.items()},
    }


def fmt(s):
    if s is None:
        return "    — sem dados —"
    return f"    min {s['min']:7.2f} | mediana {s['median']:7.2f} | max {s['max']:8.2f} ms  (n={s['n']})"


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        sys.exit(2)
    all_runs = load_pairs(sys.argv[1])
    by_phase = {}
    for r in all_runs:
        by_phase.setdefault(r["phase"], []).append(r)

    print("=== Perfil do plano morno (ADR-0095) — pre_spawn = wall − spawn_total ===\n")
    reports = {}
    for phase in ("cold", "warm"):
        rows = by_phase.get(phase, [])
        if not rows:
            print(f"[{phase}] sem corridas\n")
            continue
        r = phase_report(rows)
        reports[phase] = r
        print(f"[{phase}]  ({len(rows)} corridas)")
        print("  wall (processo→container pronto):")
        print(fmt(r["wall"]))
        print("  spawn_total (mecânica do container — clone/cgroup/mounts):")
        print(fmt(r["spawn_total"]))
        print("  >> pre_spawn (o que o plano morno ataca) = wall − spawn_total:")
        print(fmt(r["pre_spawn"]))
        print("     decomposto (cmd_run, medido): "
              f"prespawn_total {fmt(r['prespawn_total']).strip()}")
        for k in ("policy_preflight", "claims", "resolve_rootfs", "record_net"):
            s = r["subphases"].get(k)
            if s:
                print(f"       {k:16} {fmt(s).strip()}")
        print("     (o resto do pre_spawn = carga do processo + handshake/socket)")
        print()

    cold = reports.get("cold", {}).get("pre_spawn")
    warm = reports.get("warm", {}).get("pre_spawn")
    if cold and warm:
        drop = cold["median"] - warm["median"]
        pct = 100.0 * drop / cold["median"] if cold["median"] else 0.0
        print("=== Veredicto ===")
        print(f"  pre_spawn cold (mediana): {cold['median']:7.2f} ms")
        print(f"  pre_spawn warm (mediana): {warm['median']:7.2f} ms")
        print(f"  queda: {drop:7.2f} ms  ({pct:5.1f} %)")
        print("  baseline host 2026-11-18: pre_spawn ~57.9 ms (68 %) — comparar o cold acima.")


if __name__ == "__main__":
    main()
