# H0 — SPIFFE identity: measured run

- Result: **PASS**
- SPIRE: spire-server v1.16.0 (its own CA, no upstream authority)
- Trust domain: `example.org` — node SVID `spiffe://example.org/node`
- Delonix: v0.1.2 @ 2220d815c
- Proof: `cargo test -p delonix-spiffe --test handshake spire_issued_svid_is_accepted`
  with `DELONIX_SPIFFE_SVID_CERT/KEY/BUNDLE` set to the SPIRE-minted material.

SPIRE minted the node X.509-SVID; `delonix-spiffe` built rustls server+client
configs from it and completed a mutual-TLS handshake, verifying the peer by its
`spiffe://` URI-SAN against the SPIRE bundle. The SVID's SAN is in
`svid.txt`; the console is in `handshake.log`. Private keys are not recorded.

Reproduce: `tests/interop/spiffe/run.sh`.
