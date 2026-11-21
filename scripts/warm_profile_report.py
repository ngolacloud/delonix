#!/usr/bin/env python3
"""Warm-plane profile report (ADR-0095).

Reads the JSONL that `delonix __poolbench` writes (profiler + bench) and pairs,
per run, the `parent` line (spawn_total measured inside the `spawn`) with the
`bench` line right after it (the run's wall). It gives, per phase (cold/warm):

    pre_spawn = wall − spawn_total

which is the SAME definition as in `spawn_profile_report.py` — the cost outside the
container mechanics (process + image/rootfs resolution). The point of the warm plane is
to make `pre_spawn` drop from ~58 ms (cold) to near 0 (warm).

Usage: warm_profile_report.py <file.jsonl>
"""
import json
import statistics
import sys


def load_pairs(path):
    """Per run the sequence in the file is `prespawn`, `parent`, `bench`.

    Pairs each `bench` with the `parent` and the `prespawn` immediately before it.
    Returns a list of dicts, one per run.
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
    """runs: list of per-run dicts. Returns a dict with the metrics per phase."""
    walls = [r["wall_us"] for r in runs]
    spawn_totals = [r["spawn_total_us"] for r in runs]
    prespawn_totals = [r["prespawn_total_us"] for r in runs]
    pre = [
        r["wall_us"] - r["spawn_total_us"]
        for r in runs
        if r["wall_us"] is not None and r["spawn_total_us"] is not None
    ]
    # Mean per pre_spawn sub-phase (policy_preflight, claims, resolve_rootfs, ...).
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
        return "    — no data —"
    return f"    min {s['min']:7.2f} | median {s['median']:7.2f} | max {s['max']:8.2f} ms  (n={s['n']})"


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        sys.exit(2)
    all_runs = load_pairs(sys.argv[1])
    by_phase = {}
    for r in all_runs:
        by_phase.setdefault(r["phase"], []).append(r)

    print("=== Warm-plane profile (ADR-0095) — pre_spawn = wall − spawn_total ===\n")
    reports = {}
    for phase in ("cold", "warm"):
        rows = by_phase.get(phase, [])
        if not rows:
            print(f"[{phase}] no runs\n")
            continue
        r = phase_report(rows)
        reports[phase] = r
        print(f"[{phase}]  ({len(rows)} runs)")
        print("  wall (process→container ready):")
        print(fmt(r["wall"]))
        print("  spawn_total (container mechanics — clone/cgroup/mounts):")
        print(fmt(r["spawn_total"]))
        print("  >> pre_spawn (what the warm plane attacks) = wall − spawn_total:")
        print(fmt(r["pre_spawn"]))
        print("     decomposed (cmd_run, measured): "
              f"prespawn_total {fmt(r['prespawn_total']).strip()}")
        for k in ("policy_preflight", "claims", "resolve_rootfs", "record_net"):
            s = r["subphases"].get(k)
            if s:
                print(f"       {k:16} {fmt(s).strip()}")
        print("     (the rest of pre_spawn = process load + handshake/socket)")
        print()

    cold = reports.get("cold", {}).get("pre_spawn")
    warm = reports.get("warm", {}).get("pre_spawn")
    if cold and warm:
        drop = cold["median"] - warm["median"]
        pct = 100.0 * drop / cold["median"] if cold["median"] else 0.0
        print("=== Verdict ===")
        print(f"  pre_spawn cold (median): {cold['median']:7.2f} ms")
        print(f"  pre_spawn warm (median): {warm['median']:7.2f} ms")
        print(f"  drop: {drop:7.2f} ms  ({pct:5.1f} %)")
        print("  host baseline 2026-11-18: pre_spawn ~57.9 ms (68 %) — compare with the cold above.")


if __name__ == "__main__":
    main()
