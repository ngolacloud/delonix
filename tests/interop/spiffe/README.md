# H0 — SPIFFE identity (lab)

Proves campaign phase **H0**: a real **SPIRE** server is the CA that issues the
Delonix node's **X.509-SVID**, and the node's TLS layer (`delonix-spiffe`) accepts
it over a real mutual-TLS handshake.

This directory is the lab — it lives OUTSIDE `crates/`, `bins/` and `proto/`
because it names a product (SPIRE). The engine itself names only the standard
(SPIFFE / X.509-SVID); SPIRE appears here and in `docs/` only.

## What `run.sh` does

1. Fetches `spire-server` (v1.16.0, pinned) into `$DELONIX_SPIRE_CACHE`
   (default `~/.cache/delonix-spire`) if it is not already there.
2. Starts a throwaway `spire-server` with trust domain `example.org` and its own
   self-generated CA (no upstream authority — SPIRE IS the CA).
3. Mints a node X.509-SVID for `spiffe://example.org/node`
   (`spire-server x509 mint`), writing `svid.pem` / `key.pem` / `bundle.pem`.
4. Runs the `delonix-spiffe` handshake proof (`spire_issued_svid_is_accepted`)
   with `DELONIX_SPIFFE_SVID_CERT/KEY/BUNDLE` pointing at that material: the node
   presents the SPIRE SVID on both ends of a mutual-TLS handshake and verifies the
   peer by its `spiffe://` URI-SAN against the SPIRE bundle.
5. Records the run under `reports/interop/h0-spiffe/` (console, the SVID's SAN,
   the versions, the result). Private keys are NOT recorded.

## Run

```sh
tests/interop/spiffe/run.sh
```

Needs `cargo` and network access on first run (to fetch SPIRE). No root, no
daemon left behind — the server is a child process killed on exit.

## What it does NOT do

No SPIRE agent, no Workload API, no node-attestor, no SVID rotation (A2). The
SVID is minted directly off the server CA — enough to prove "SPIRE issues the
node certificate and the node accepts it", which is H0.
