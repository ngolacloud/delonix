#!/usr/bin/env bash
# RT4 wrapper (ADR-0080): translate the runc-style CLI that the
# `opencontainers/runtime-tools` *validation suite* invokes into
# `delonix oci-runtime …`, so the upstream suite can measure Delonix itself
# (the peer of runc/crun) instead of only the reference runtime.
#
# This is the `RUNTIME=<wrapper>` referenced by `scripts/runtime-tools.sh`.
#
#   RUNTIME="$PWD/scripts/oci-runtime-rt4-wrapper.sh" ./scripts/runtime-tools.sh
#
# The suite (validation/util/container.go) calls the runtime with exactly:
#   create --pid-file <F> --bundle <D> <ID> | start <ID> | state <ID>
#   kill <ID> [SIGNAL] | delete [--force] <ID>
# Delonix differences the wrapper bridges:
#   * `create` has no `--pid-file` (we strip it and synthesise the file from the
#     container `state`, the way runc writes it on create);
#   * `--root` is a per-subcommand option, injected right after the subcommand;
#   * everything else maps 1:1.
set -uo pipefail

DELONIX="${DELONIX_BIN:-delonix}"
ROOT="${DELONIX_OCI_ROOT:-${XDG_RUNTIME_DIR:-/tmp}/delonix-rt4}"
mkdir -p "$ROOT" 2>/dev/null || true

sub="${1:-}"
shift || true

# The runtime-tools.sh banner probes `--version`; answer without touching state.
case "$sub" in
  --version | version | "")
    v="$("$DELONIX" --version 2>/dev/null | head -1)"
    echo "delonix-oci-runtime (RT4/ADR-0080) over ${v:-delonix}"
    exit 0
    ;;
esac

pidfile=""
args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --pid-file)   pidfile="${2:-}"; shift 2 ;;
    --pid-file=*) pidfile="${1#*=}"; shift ;;
    *)            args+=("$1"); shift ;;
  esac
done

"$DELONIX" oci-runtime "$sub" --root "$ROOT" "${args[@]}"
rc=$?

# Synthesise the pid-file runc writes on a successful create, so suite cases that
# read it behave identically against Delonix.
if [ "$sub" = create ] && [ -n "$pidfile" ] && [ "$rc" -eq 0 ] && [ "${#args[@]}" -gt 0 ]; then
  id="${args[${#args[@]}-1]}"
  pid="$("$DELONIX" oci-runtime state "$id" --root "$ROOT" 2>/dev/null \
          | sed -n 's/.*"pid"[[:space:]]*:[[:space:]]*\([0-9]\{1,\}\).*/\1/p' | head -1)"
  [ -n "$pid" ] && printf '%s' "$pid" > "$pidfile"
fi

exit "$rc"
