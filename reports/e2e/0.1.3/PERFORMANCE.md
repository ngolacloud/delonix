# PERFORMANCE.md — `delonix 0.1.3`, time per command group

Latency of `delonix <leaf> --help` (RUNS=20 per leaf, 366 leaves, 78
groups). Values in ms. `mean-of-means` = average of the leaf means in the group.
Base cold start: `--version` 9.874 ms mean, `--help` 9.886 ms mean.
Global mean: 9.12 ms · median 9.065 ms · min 7.785 · max 10.715.

| group | leaves | mean-of-means (ms) | p95 max (ms) | p99 max (ms) |
|---|--:|--:|--:|--:|
| `run` | 1 | 10.53 | 12.14 | 12.26 |
| `pull` | 1 | 10.27 | 11.73 | 11.75 |
| `completion` | 2 | 10.1 | 12.35 | 18.4 |
| `version` | 1 | 10.07 | 11.63 | 24.35 |
| `logout` | 1 | 10.05 | 11.96 | 13.34 |
| `migrate` | 1 | 10.02 | 11.32 | 11.69 |
| `rm` | 1 | 9.97 | 18.23 | 24.87 |
| `stop` | 1 | 9.93 | 12.35 | 12.53 |
| `hosts` | 1 | 9.85 | 11.37 | 11.83 |
| `systemcontainer` | 6 | 9.84 | 14.53 | 17.75 |
| `create` | 1 | 9.78 | 11.29 | 12.77 |
| `bucket` | 3 | 9.69 | 12.52 | 16.93 |
| `init` | 1 | 9.67 | 13.85 | 13.91 |
| `save` | 1 | 9.66 | 11.03 | 11.63 |
| `api-resources` | 1 | 9.65 | 11.61 | 12.59 |
| `get` | 1 | 9.65 | 11.23 | 11.34 |
| `delete` | 1 | 9.62 | 11.06 | 11.28 |
| `wait` | 1 | 9.61 | 10.85 | 11.77 |
| `mcp` | 3 | 9.58 | 13.46 | 13.86 |
| `logs` | 1 | 9.57 | 11.38 | 12.33 |
| `rename` | 1 | 9.57 | 11.24 | 11.41 |
| `backup` | 6 | 9.57 | 12.56 | 23.16 |
| `plan` | 1 | 9.53 | 10.7 | 11.12 |
| `push` | 1 | 9.51 | 12.42 | 12.51 |
| `pause` | 1 | 9.48 | 12.12 | 13.83 |
| `load` | 1 | 9.48 | 11.11 | 11.19 |
| `cp` | 1 | 9.46 | 10.31 | 10.89 |
| `policy` | 1 | 9.42 | 11.1 | 13.63 |
| `man` | 1 | 9.37 | 11.17 | 11.61 |
| `workload` | 4 | 9.36 | 12.12 | 14.19 |
| `oci-runtime` | 5 | 9.27 | 13.12 | 13.55 |
| `start` | 1 | 9.27 | 10.88 | 10.91 |
| `unpause` | 1 | 9.25 | 12.33 | 13.8 |
| `vm` | 39 | 9.25 | 12.68 | 24.77 |
| `top` | 1 | 9.24 | 10.35 | 10.76 |
| `system` | 20 | 9.24 | 13.56 | 15.26 |
| `diff` | 1 | 9.23 | 10.62 | 11.31 |
| `manifest` | 4 | 9.22 | 11.82 | 11.82 |
| `stack` | 11 | 9.21 | 12.02 | 23.03 |
| `kill` | 1 | 9.18 | 11.3 | 11.56 |
| `untag` | 1 | 9.18 | 10.28 | 14.39 |
| `network` | 21 | 9.16 | 15.43 | 19.02 |
| `container` | 35 | 9.14 | 12.51 | 24.07 |
| `rmi` | 1 | 9.12 | 10.54 | 11.45 |
| `net` | 36 | 9.12 | 12.5 | 21.33 |
| `serve` | 4 | 9.09 | 10.79 | 12.01 |
| `login` | 1 | 9.02 | 11.1 | 11.7 |
| `config` | 3 | 9.02 | 10.68 | 11.86 |
| `compatibility` | 2 | 9.01 | 10.45 | 19.33 |
| `restart` | 1 | 9.0 | 10.4 | 10.57 |
| `image` | 28 | 9.0 | 12.57 | 23.76 |
| `pod` | 19 | 8.98 | 16.23 | 25.78 |
| `cluster` | 22 | 8.96 | 14.98 | 18.38 |
| `volume` | 16 | 8.91 | 13.35 | 14.34 |
| `exec` | 1 | 8.89 | 10.51 | 10.8 |
| `info` | 1 | 8.84 | 11.77 | 12.01 |
| `apply` | 1 | 8.83 | 9.99 | 10.46 |
| `provider` | 6 | 8.82 | 12.57 | 18.55 |
| `describe` | 1 | 8.76 | 9.73 | 9.78 |
| `tag` | 1 | 8.71 | 10.6 | 11.31 |
| `secret` | 9 | 8.68 | 11.16 | 15.56 |
| `ps` | 1 | 8.67 | 10.47 | 10.75 |
| `attach` | 1 | 8.59 | 10.12 | 10.47 |
| `compose` | 5 | 8.59 | 10.69 | 10.71 |
| `stats` | 1 | 8.57 | 10.06 | 10.13 |
| `export` | 1 | 8.56 | 10.58 | 11.43 |
| `dashboard` | 1 | 8.54 | 9.68 | 9.96 |
| `drift` | 1 | 8.46 | 9.45 | 16.05 |
| `events` | 1 | 8.46 | 9.47 | 10.94 |
| `search` | 1 | 8.41 | 9.58 | 9.89 |
| `gitops` | 4 | 8.4 | 11.0 | 13.94 |
| `images` | 1 | 8.35 | 9.41 | 9.91 |
| `history` | 1 | 8.26 | 9.88 | 12.66 |
| `explain` | 1 | 8.25 | 9.47 | 9.48 |
| `port` | 1 | 8.19 | 9.14 | 9.77 |
| `build` | 1 | 8.12 | 9.26 | 9.66 |
| `import` | 1 | 8.11 | 9.54 | 9.94 |
| `commit` | 1 | 7.95 | 9.38 | 10.09 |


## Notes

- The measured time is **parse + `fork`/`exec`** (clap short-circuits on `--help`,
  before any effect), i.e. the startup latency of the CLI surface.
- Percentiles with n<20 per leaf are indicative; the concurrency probe (§5 of the
  report) generates a high n (8736 samples) for the profile under load.
- Reproduce: `DELONIX_BIN=target/release/delonix RUNS=20 bash scripts/cli_timing_profile.sh`.
