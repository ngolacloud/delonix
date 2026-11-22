#!/usr/bin/env bash
# H0 — SPIFFE identity lab (ADR-0097). A real spire-server issues the node's
# X.509-SVID; the delonix-spiffe TLS layer accepts it over a real mutual-TLS
# handshake. Records the run under reports/interop/h0-spiffe/.
#
# No root, no persistent daemon: the spire-server is a child killed on exit.
set -euo pipefail

SPIRE_VER="1.16.0"
TRUST_DOMAIN="example.org"
SPIFFE_ID="spiffe://${TRUST_DOMAIN}/node"
CACHE="${DELONIX_SPIRE_CACHE:-$HOME/.cache/delonix-spire}"
BIN="$CACHE/spire-${SPIRE_VER}/bin/spire-server"

ROOT="$(git rev-parse --show-toplevel)"
REPORT_DIR="$ROOT/reports/interop/h0-spiffe"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/delonix-h0-spiffe.XXXXXX")"
SRV_PID=""

cleanup() {
  [ -n "$SRV_PID" ] && kill "$SRV_PID" 2>/dev/null || true
  [ -n "$SRV_PID" ] && wait "$SRV_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

say() { printf '>> %s\n' "$*"; }

# 1. Fetch spire-server (pinned) if absent.
if [ ! -x "$BIN" ]; then
  say "fetching SPIRE v${SPIRE_VER} into $CACHE"
  mkdir -p "$CACHE"
  tgz="spire-${SPIRE_VER}-linux-amd64-musl.tar.gz"
  curl -sSL -o "$CACHE/$tgz" \
    "https://github.com/spiffe/spire/releases/download/v${SPIRE_VER}/${tgz}"
  tar xzf "$CACHE/$tgz" -C "$CACHE"
fi
[ -x "$BIN" ] || { echo "spire-server not found at $BIN" >&2; exit 1; }

DATA="$WORK/data"; OUT="$WORK/out"; SOCK="$WORK/api.sock"
mkdir -p "$DATA" "$OUT"

# 2. A throwaway server whose own CA signs the SVID (no upstream authority).
cat > "$WORK/server.conf" <<EOF
server {
    bind_address = "127.0.0.1"
    bind_port = "8081"
    socket_path = "$SOCK"
    trust_domain = "$TRUST_DOMAIN"
    data_dir = "$DATA"
    log_level = "ERROR"
    ca_ttl = "24h"
    default_x509_svid_ttl = "1h"
}
plugins {
    DataStore "sql" { plugin_data { database_type = "sqlite3" connection_string = "$DATA/datastore.sqlite3" } }
    KeyManager "memory" { plugin_data {} }
    NodeAttestor "join_token" { plugin_data {} }
}
EOF

say "starting spire-server (trust domain ${TRUST_DOMAIN})"
"$BIN" run -config "$WORK/server.conf" >"$WORK/server.log" 2>&1 &
SRV_PID=$!
for _ in $(seq 1 100); do [ -S "$SOCK" ] && break; sleep 0.2; done
[ -S "$SOCK" ] || { echo "spire-server did not come up:" >&2; cat "$WORK/server.log" >&2; exit 1; }

# 3. Mint the node X.509-SVID off the SPIRE CA.
say "minting ${SPIFFE_ID}"
"$BIN" x509 mint -spiffeID "$SPIFFE_ID" -write "$OUT" -socketPath "$SOCK" >"$WORK/mint.log" 2>&1
[ -s "$OUT/svid.pem" ] && [ -s "$OUT/key.pem" ] && [ -s "$OUT/bundle.pem" ] \
  || { echo "mint produced no SVID:" >&2; cat "$WORK/mint.log" >&2; exit 1; }

# 4. The node TLS layer accepts the SPIRE-issued SVID over a real handshake.
say "running the delonix-spiffe handshake proof against the SPIRE SVID"
set +e
DELONIX_SPIFFE_SVID_CERT="$OUT/svid.pem" \
DELONIX_SPIFFE_SVID_KEY="$OUT/key.pem" \
DELONIX_SPIFFE_SVID_BUNDLE="$OUT/bundle.pem" \
  cargo test --manifest-path "$ROOT/Cargo.toml" -p delonix-spiffe --test handshake \
  spire_issued_svid_is_accepted -- --exact --ignored --nocapture >"$WORK/handshake.log" 2>&1
RC=$?
set -e
cat "$WORK/handshake.log"

# 5. Record the evidence (no private keys).
mkdir -p "$REPORT_DIR"
delonix_ver="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
sha="$(git -C "$ROOT" rev-parse --short=9 HEAD 2>/dev/null || echo unknown)"
openssl x509 -in "$OUT/svid.pem" -noout -text 2>/dev/null \
  | sed -n '/Issuer:/p;/Validity/,/Not After/p;/Subject:/p;/Subject Alternative/,+1p' \
  > "$REPORT_DIR/svid.txt" || echo "(openssl not available)" > "$REPORT_DIR/svid.txt"
cp "$WORK/handshake.log" "$REPORT_DIR/handshake.log"

result="PASS"; [ "$RC" -eq 0 ] || result="FAIL"
cat > "$REPORT_DIR/README.md" <<EOF
# H0 — SPIFFE identity: measured run

- Result: **${result}**
- SPIRE: spire-server v${SPIRE_VER} (its own CA, no upstream authority)
- Trust domain: \`${TRUST_DOMAIN}\` — node SVID \`${SPIFFE_ID}\`
- Delonix: v${delonix_ver} @ ${sha}
- Proof: \`cargo test -p delonix-spiffe --test handshake spire_issued_svid_is_accepted\`
  with \`DELONIX_SPIFFE_SVID_CERT/KEY/BUNDLE\` set to the SPIRE-minted material.

SPIRE minted the node X.509-SVID; \`delonix-spiffe\` built rustls server+client
configs from it and completed a mutual-TLS handshake, verifying the peer by its
\`spiffe://\` URI-SAN against the SPIRE bundle. The SVID's SAN is in
\`svid.txt\`; the console is in \`handshake.log\`. Private keys are not recorded.

Reproduce: \`tests/interop/spiffe/run.sh\`.
EOF

say "result: ${result} (evidence in reports/interop/h0-spiffe/)"
exit "$RC"
