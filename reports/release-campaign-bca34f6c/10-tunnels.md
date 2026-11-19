# 10 — Tunnels (net tunnel expose)

SUT: `delonix 5.1.1` @ `bca34f6c3`. Synthetic app: nginx-unprivileged on `127.0.0.1:18100`. Verb: `delonix net tunnel expose <port> --provider <p>`.

| # | Case (§17) | Observed | State |
|---|---|---|---|
| TUNNEL-1 pinggy | public HTTP + origin | URL `https://gtzfv-….run.pinggy-free.link`; **public fetch HTTP 200 (896 B)**; nginx log shows `GET /?relcprobe=pinggy7a3f9 200` with XFF `102.130.192.246` (pinggy edge) → **public→origin round-trip proven** | **PASSED** |
| TUNNEL-1 ngrok | establishment | URL `https://repose-unsworn-aviator.ngrok-free.dev` (host has a cached ngrok authtoken) | **PASSED** (establishment) |
| TUNNEL-1 cloudflare | public HTTP | quick URL `https://yacht-norm-assists-claims.trycloudflare.com` assigned; **public fetch times out (curl rc=124)**, origin never receives the request | **FAILED at the edge (environment)** |
| TUNNEL-origin | origin readiness proof | pinggy/ngrok report `(origin ready)` — the `probe_origin` probe (commit 06df2d56) runs after the agent (re)starts | **PASSED** |
| TUNNEL-2 stop/remove | `delete gateway <name>` | `tunnel-18100`/`tunnel-19999: removed`; child processes (ngrok/ssh) reaped; pre-existing registrations (6) intact | **PASSED** |

## Rigor notes

- **cloudflare edge unreliable in this sandbox:** the tunnel ESTABLISHES (URL assigned, agent running),
  but the edge→origin path times out (HTTP 525/timeout) — consistent with a prior project observation;
  classified as an **environment limitation**, not a product defect. The 1st occurrence recorded (rc=124),
  with no retries hiding it.
- **"origin unavailable" test invalidated by me:** I exposed port 19999 assuming it free, but there is
  a pre-existing listener (not mine, owned by another uid/root) on `0.0.0.0:19999` — so the origin
  was NOT dead and the `(origin ready)` was **correct**. The `Unavailable` path of `probe_origin`
  is covered by unit tests (`tunnel.rs:2716,2720`). **Not a defect.**
- **Stop method:** stopping with `timeout`/`pkill` of the wrapper leaves orphan `ssh -R`/`ngrok` children;
  the clean way is `delonix delete gateway <name>` (reaps the children). Operational note, not a confirmed
  defect (I did not test direct SIGTERM to the wrapper in isolation).
- **`net tunnel` keeps a persistent list of registrations** (`get gateway`) — the campaign removed only
  its own (`tunnel-18100/19999`); the 6 pre-existing ones (`kambacode-*`, `nokubiko-*`, `devfast-*`) preserved.

## Not tested

Custom domain/named tunnel (requires token/account — out of scope without credentials), reconnection after network
drop, TCP/other protocols, localhost resolution with a remote agent.
