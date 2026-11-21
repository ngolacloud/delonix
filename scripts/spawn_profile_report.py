#!/usr/bin/env python3
"""Relatório do perfil de arranque a frio do delonix (lê o directório de scripts/spawn_profile.sh).

Decompõe o WALL-CLOCK de cada `container run` em:

    wall = pre_spawn (arranque do processo + resolução de imagem/rootfs no CLI)
         + spawn_total (clone + cgroup + rede-hook + mounts, do profiler)

Separa a 1ª corrida (FRIA: paga overlay/cgroup frios) da mediana das restantes
(QUENTE). Nomeia o maior consumidor — a pergunta que o ADR-0095 faz antes de
desenhar o plano morno/pool.

Uma mediana sem dispersão esconde a corrida que foi o dobro, por isso cada número
sai com min/mediana/max.
"""
import json
import os
import sys
from statistics import median

PARENT_ORDER = ["pre_spawn", "clone", "cgroup", "net", "mounts"]
PARENT_LABEL = {
    "pre_spawn": "pre_spawn (arranque proc. + imagem/rootfs no CLI)",
    "clone": "clone (namespaces)",
    "cgroup": "cgroup (setup_cgroup, pai)",
    "net": "rede (hook on_started: CNI/slirp)",
    "mounts": "mounts (filho: overlay+pivot+/proc,/sys,/dev)",
}


def load_dir(path, mode):
    f = os.path.join(path, f"{mode}.jsonl")
    rows = []
    if os.path.exists(f):
        with open(f) as fh:
            for line in fh:
                line = line.strip()
                if line:
                    rows.append(json.loads(line))
    return rows


def enrich(rows):
    """Acrescenta `pre_spawn` = wall - spawn_total a cada corrida."""
    for r in rows:
        if "wall_us" in r:
            r["phases_us"] = dict(r.get("phases_us", {}))
            r["phases_us"]["pre_spawn"] = max(0, r["wall_us"] - r["total_us"])
            r["wall_total_us"] = r["wall_us"]
    return rows


def stats_ms(vals):
    return min(vals) / 1000.0, median(vals) / 1000.0, max(vals) / 1000.0


def phase_table(rows, total_key):
    """Imprime a tabela de fases para `rows`, usando `total_key` (wall vs spawn)."""
    if not rows:
        print("    (sem amostras)")
        return
    totals = [r[total_key] for r in rows]
    tmn, tmd, tmx = stats_ms(totals)
    print(f"    total: min {tmn:.2f} / MEDIANA {tmd:.2f} / max {tmx:.2f} ms  (n={len(rows)})")
    print(f"    {'fase':<48} {'min':>8} {'mediana':>9} {'max':>8} {'%med':>7}")
    print(f"    {'-'*48} {'-'*8} {'-'*9} {'-'*8} {'-'*7}")
    meds = {}
    keys = PARENT_ORDER + [k for r in rows for k in r["phases_us"] if k not in PARENT_ORDER]
    seen = set()
    for key in keys:
        if key in seen:
            continue
        seen.add(key)
        vals = [r["phases_us"][key] for r in rows if key in r["phases_us"]]
        if not vals:
            continue
        mn, md, mx = stats_ms(vals)
        meds[key] = md
        pct = (md / tmd * 100.0) if tmd else 0.0
        print(f"    {PARENT_LABEL.get(key, key):<48} {mn:>7.2f} {md:>8.2f} {mx:>7.2f} {pct:>6.1f}%")
    if meds:
        top = max(meds, key=meds.get)
        print(f"    => MAIOR CONSUMIDOR: {PARENT_LABEL.get(top, top)} "
              f"({meds[top]:.2f} ms, {meds[top]/tmd*100:.1f}% do total)")


def report_mode(path, mode, title):
    rows = enrich(load_dir(path, mode))
    if not rows:
        print(f"\n### {title}: sem dados (modo saltado?)")
        return None
    cold, warm = rows[0], rows[1:]
    print(f"\n### {title}")
    print(f"  [1ª CORRIDA — FRIA] wall={cold['wall_us']/1000:.2f} ms "
          f"(spawn={cold['total_us']/1000:.2f}, pre_spawn={cold['phases_us']['pre_spawn']/1000:.2f})")
    print("  [WALL-CLOCK por fase — excluindo a 1ª corrida]")
    phase_table(warm, "wall_total_us")
    return rows


def process_start(path):
    f = os.path.join(path, "process-start.us")
    if os.path.exists(f):
        vals = [int(x) for x in open(f).read().split() if x.strip().isdigit()]
        if vals:
            return median(vals) / 1000.0
    return None


def child_sub(path, mode):
    """Mediana das sub-fases do filho (setup_rootfs/confine_tail) do stderr."""
    f = os.path.join(path, f"{mode}.child.txt")
    sr, ct = [], []
    if os.path.exists(f):
        for line in open(f):
            # ...total=X.XXms | setup_rootfs=Y.YYms confine_tail=Z.ZZms
            for tok in line.split("|")[-1].split():
                if tok.startswith("setup_rootfs="):
                    sr.append(float(tok.split("=")[1].rstrip("ms")))
                elif tok.startswith("confine_tail="):
                    ct.append(float(tok.split("=")[1].rstrip("ms")))
    return (median(sr) if sr else None), (median(ct) if ct else None)


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "."
    ps = process_start(path)
    if ps is not None:
        print(f"### Arranque puro do processo (exec+linker+parse): mediana {ps:.2f} ms")
        print("    (parte do pre_spawn; o resto do pre_spawn é resolução de imagem + preparação do rootfs no CLI)")
    host = report_mode(path, "host", "HOST (--net host) — linha 4a do doc (~91 ms)")
    sr, ct = child_sub(path, "host")
    if sr is not None:
        print(f"  [FILHO, sub-fases DENTRO de `mounts`] setup_rootfs≈{sr:.2f} ms, confine_tail≈{ct:.2f} ms")
    iso = report_mode(path, "isolated", "ISOLADO (--net rede, plano de pé) — linha 4b do doc (~216 ms)")

    if host and iso:
        hw = median([r["wall_us"] for r in host[1:]]) / 1000.0
        iw = median([r["wall_us"] for r in iso[1:]]) / 1000.0
        print("\n### DELTA host -> isolado (wall-clock, quente)")
        print(f"  host    ~ {hw:.2f} ms")
        print(f"  isolado ~ {iw:.2f} ms   (delta {iw-hw:+.2f} ms)")
        print("  O custo da rede isolada é o veth/IPAM/nft/nsenter montado em torno do")
        print("  spawn (orquestração do CLI), NÃO a fase `net` do on_started — por isso")
        print("  mede-se por parede, como o doc faz nas linhas 4b/4c.")


if __name__ == "__main__":
    main()
