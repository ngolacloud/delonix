#!/usr/bin/env bash
# CLI-parity conformance entry point. Two tiers, as the review asks:
#
#   fast         parser + TAB, no engine, no network — always runnable.
#   differential the priority flows replayed on docker/podman/delonix — needs
#                all three engines AND the busybox digest pulled into each.
#
# A conformance job must make a MISSING prerequisite VISIBLE and must never let
# an empty or all-skipped suite read as "conforme" (the review's explicit bar).
# So `differential` REFUSES to run unless all three engine binaries resolve: a
# skipped engine is a failure here, not a silent pass.
#
# Usage:
#   tests/cli-parity/run.sh fast          <delonix-bin>
#   tests/cli-parity/run.sh differential  <delonix-bin>   # DOCKER_BIN/PODMAN_BIN override
#   tests/cli-parity/run.sh all           <delonix-bin>
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
mode=${1:?usage: run.sh <fast|differential|all> <delonix-bin>}
BIN=${2:?path to the delonix binary built from THIS checkout}
BIN=$(readlink -f "$BIN")
[ -x "$BIN" ] || { echo "FAIL: '$BIN' is not an executable delonix binary" >&2; exit 2; }

DIGEST="busybox@sha256:bdf57e528e45e4433820e045b29b4597825a1c9e38353532d90a01445013f82e"

run_fast() {
    echo "== fast: parser/TAB (no engine) =="
    "$here/frequent-verbs.sh" "$BIN"
    "$here/tab-descriptions.sh" "$BIN"
}

require_engine() {
    local name=$1 bin=$2
    command -v "$bin" >/dev/null 2>&1 || {
        echo "FAIL[$name]: '$bin' not found — the differential suite needs docker, podman AND the" >&2
        echo "             delonix binary. A missing engine is a failed prerequisite, never a pass." >&2
        echo "             Install it (e.g. 'sudo apt-get install -y podman') and re-run, or run" >&2
        echo "             'tests/cli-parity/run.sh fast $BIN' for the engine-free tier." >&2
        return 1
    }
}

run_differential() {
    local docker=${DOCKER_BIN:-docker} podman=${PODMAN_BIN:-podman}
    echo "== differential: prerequisites =="
    require_engine docker "$docker"
    require_engine podman "$podman"
    echo "ok: docker=$(command -v "$docker") podman=$(command -v "$podman") delonix=$BIN"
    echo "== differential: aligning the pinned busybox digest across all engines =="
    "$docker" pull "$DIGEST" >/dev/null
    "$podman" pull "docker.io/library/$DIGEST" >/dev/null
    "$BIN" image pull "$DIGEST" >/dev/null
    echo "== differential: flow 1 (interactive container) =="
    DOCKER_BIN="$docker" PODMAN_BIN="$podman" DELONIX_BIN="$BIN" \
        python3 "$here/differential/flow1_interactive.py" "$here/evidence-flow1.jsonl"
    echo "== differential: flow 2 (pod, two members) =="
    DOCKER_BIN="$docker" PODMAN_BIN="$podman" DELONIX_BIN="$BIN" \
        python3 "$here/differential/flow2_pod.py" "$here/evidence-flow2.jsonl"
}

case "$mode" in
    fast) run_fast ;;
    differential) run_differential ;;
    all) run_fast; echo; run_differential ;;
    *) echo "unknown mode '$mode' (fast|differential|all)" >&2; exit 2 ;;
esac
echo "cli-parity[$mode]: PASS"
