# Networking and CNI — `fa8c6297`

Sources: E2E battery (`raw/e2e.log`), chaos (`raw/chaos.log`), G-008 fault injection, critest.

| Area | Evidence | State |
|---|---|---|
| Bridge/veth, IP per container, concurrent attach (8) and scale (30), distinct IPs | chaos `concurrent-attach`, `scale` | PASS |
| Namespace isolation for containers and pods | chaos `namespace-isolation`, `pod-namespace-isolation` | PASS |
| Holder: pin kill, total death, control restart, wedge, slirp kill, idempotency | chaos (6 scenarios) | PASS |
| Ownership across roots (pid recycled from another root) | chaos `posse-destrutiva/*` | PASS |
| NetworkRoute prune/destroy with no orphans | chaos `stack-netroute` | PASS |
| Port publishing, ingress/egress, internal DNS | E2E | PASS |
| **Anti-spoof fails open** when the nft insert fails | injection with shim, control with the rule | **FAIL (G-008)** |
| Port mapping/multi-container seen from the host (critest) | pod IP in the holder's netns, unreachable from the host | ROOTLESS LIMITATION |
| CRI port-forward (2 specs) | timeout; no cause logged | UNKNOWN (G-031) |
| CNI ADD/DEL/CHECK with external plugins (`DELONIX_CNI=1`) | — | NOT TESTED |
| IPv6 / dual-stack | off by design (fail-closed, v0.37.1) | DELIBERATE DIVERGENCE |
| WireGuard/overlay inter-node | requires 2 nodes | ENVIRONMENT BLOCKED |
| PaaS forwards reaper with host address / without registration | code reading of `delonix-paas` origin/main | FAIL (G-036, G-037) |
| Host restart | — | NOT TESTED |
