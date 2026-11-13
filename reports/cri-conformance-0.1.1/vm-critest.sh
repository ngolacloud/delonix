#!/usr/bin/env bash
# VM-side critest harness — delonix-cri ROOTFUL + CNI, on the golden k8s VM.
# Run as root inside the VM:  sudo FOCUS='...' bash vm-critest.sh
#
# Binaries expected in /usr/local/bin: delonix, delonix-cri, critest, crictl.
# Env: FOCUS / SKIP (ginkgo), GINKGO_TIMEOUT (default 25m), KEEP=1 (leave server up).
set -uo pipefail
export PATH=/usr/local/bin:/usr/local/sbin:/usr/sbin:/sbin:/usr/bin:/bin:$PATH

CRI_BIN=/usr/local/bin/delonix-cri
CLI_BIN=/usr/local/bin/delonix
SOCK=/run/delonix-cri.sock
GINKGO_TIMEOUT="${GINKGO_TIMEOUT:-25m}"
LOG="${LOG:-/tmp/critest-$$.log}"

[ "$(id -u)" = 0 ] || { echo "ERRO: precisa de root (sudo)"; exit 2; }
for b in "$CRI_BIN" "$CLI_BIN" critest crictl; do
  command -v "$b" >/dev/null || [ -x "$b" ] || { echo "ERRO: falta $b"; exit 2; }
done
[ -d /opt/cni/bin ] || { echo "ERRO: faltam plugins CNI em /opt/cni/bin"; exit 2; }

# --- Server environment: CNI on, conformance-only seccomp nil, wide cap ceiling ---
export DELONIX_CNI=1 CNI_PATH=/opt/cni/bin
export DELONIX_CRI_SECCOMP_NIL=unconfined
export DELONIX_CRI_CAP_CEILING=all

# --- cgroup v2 delegation: pull our procs off the root of the cgroup-ns and
#     enable the controllers so delonix.slice receives memory/cpu/io/pids. ---
if [ -w /sys/fs/cgroup/cgroup.subtree_control ]; then
  mkdir -p /sys/fs/cgroup/init
  echo $$ > /sys/fs/cgroup/init/cgroup.procs 2>/dev/null || true
  echo "+cpu +memory +io +pids" > /sys/fs/cgroup/cgroup.subtree_control 2>/dev/null || true
fi
echo "==> cgroup2 subtree_control: $(cat /sys/fs/cgroup/cgroup.subtree_control 2>/dev/null)"

# --- CNI conflist: bridge + host-local + portmap. NO isDefaultGateway (default
#     route already present → EEXIST). Pod IP routable inside the VM netns. ---
mkdir -p /etc/cni/net.d
cat > /etc/cni/net.d/10-delonix-bridge.conflist <<'JSON'
{
  "cniVersion": "1.0.0",
  "name": "delonixnet",
  "plugins": [
    { "type": "bridge", "bridge": "cni0", "isGateway": true, "ipMasq": true, "hairpinMode": true,
      "ipam": { "type": "host-local", "ranges": [[{ "subnet": "10.88.0.0/16" }]], "routes": [{ "dst": "0.0.0.0/0" }] } },
    { "type": "portmap", "capabilities": { "portMappings": true } }
  ]
}
JSON

# --- AppArmor: the apparmor specs reference two node profiles that must be loaded
#     (the runtime fail-closes a reference to an unloaded profile). ---
if command -v apparmor_parser >/dev/null && [ -d /sys/kernel/security/apparmor ]; then
  echo "==> loading critest AppArmor profiles (apparmor_parser)"
  AA="$(mktemp)"
  cat >"$AA" <<'AAEOF'
#include <tunables/global>
profile cri-validate-apparmor-test-deny-write flags=(attach_disconnected) {
  #include <abstractions/base>
  file,
  deny /** w,
}
profile cri-validate-apparmor-test-audit-write flags=(attach_disconnected) {
  #include <abstractions/base>
  file,
  audit /** w,
}
AAEOF
  apparmor_parser -r -W "$AA" \
    && echo "   loaded: $(aa-status --json 2>/dev/null | grep -o 'cri-validate-apparmor-test-[a-z-]*' | sort -u | tr '\n' ' ')" \
    || echo "   WARNING: apparmor_parser failed — apparmor specs will fail"
  rm -f "$AA"
else
  echo "==> no apparmor_parser/securityfs — apparmor specs may fail"
fi

cleanup() {
  [ "${KEEP:-}" = 1 ] && { echo "KEEP=1 → servidor em pé (socket $SOCK)"; return; }
  [ -n "${SRV_PID:-}" ] && kill "$SRV_PID" 2>/dev/null
  DELONIX_ROOT="${DELONIX_ROOT:-}" "$CLI_BIN" netns down >/dev/null 2>&1 || true
  rm -f "$SOCK"
}
trap cleanup EXIT

echo "==> arrancar delonix-cri (socket=$SOCK, CAP_CEILING=all, CNI=1, SECCOMP_NIL=unconfined)"
rm -f "$SOCK"
DELONIX_CRI_ADDR="unix://$SOCK" setsid "$CRI_BIN" >"$LOG.server" 2>&1 </dev/null &
SRV_PID=$!
for _ in $(seq 1 60); do [ -S "$SOCK" ] && break; sleep 0.3; done
[ -S "$SOCK" ] || { echo "ERRO: socket não abriu"; tail -40 "$LOG.server"; exit 3; }

echo "==> smoke: crictl version"
crictl --runtime-endpoint "unix://$SOCK" version || { tail -40 "$LOG.server"; exit 3; }

echo "==> critest (timeout ginkgo=$GINKGO_TIMEOUT) FOCUS='${FOCUS:-}' SKIP='${SKIP:-}'"
set -x
critest \
  -runtime-endpoint "unix://$SOCK" \
  -image-endpoint "unix://$SOCK" \
  -ginkgo.timeout="$GINKGO_TIMEOUT" \
  -ginkgo.no-color \
  ${FOCUS:+-ginkgo.focus="$FOCUS"} \
  ${SKIP:+-ginkgo.skip="$SKIP"} \
  ${GINKGO_EXTRA:-} \
  2>&1 | tee "$LOG"
rc=${PIPESTATUS[0]}
set +x
echo "==> critest rc=$rc (log=$LOG; server=$LOG.server)"
echo "=== server log tail ==="; tail -30 "$LOG.server"
exit "$rc"
