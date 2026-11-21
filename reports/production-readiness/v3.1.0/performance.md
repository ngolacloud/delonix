# Performance — NOT RUN in this audit

Reason: the host is a production one (Docker and Podman active, real user containers, several build
sessions in parallel — load 1m oscillated between 2.9 and 18 during the session). A benchmark here would
not have an equivalent configuration across Docker, Podman and Delonix nor controlled noise; `scripts/bench.sh` refuses
by default above the load threshold, and forcing it would produce numbers not fit for publication.

Latest existing evidence (STALE, not reused as a result):

| Measurement | Number | Version |
|---|---|---|
| `run --rm` (4a) Docker / Podman / Delonix | 299 / 277 / 80 ms | v0.63.1 |
| Main table | 208 / 268 / 91 ms | v0.53.0 |

Treat as a **hypothesis** until running `scripts/bench.sh` against the candidate commit on a dedicated VM, with
median, p95, p99, variance and raw samples.
