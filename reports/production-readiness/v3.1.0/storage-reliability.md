# Storage and volumes under failure — measured against `fa8c6297`

Isolated state (`/tmp/dxs`), 2026-09-15. Log: `raw/storage-failure.txt`.

| Scenario | Result | State |
|---|---|---|
| `volume rm` with a container using the volume | refused («in use by container(s): c1 (running)»), data intact | PASS |
| `backup create container c1` | 1.2 KiB archive with `volumes/v1` | PASS |
| `backup restore` over a running container without `--force` | refused with a reason | PASS |
| `backup inspect` of an archive truncated to 50% | **rc=0**, lists volumes | PARTIAL (G-038) |
| `backup restore --force` of a truncated archive | rc=1 `failed to unpack …container.json`; **same PID** (did not restart); current data intact | PASS (fail-closed) |
| Valid restore after `stop` | `volume v1 restored`, original data back | PASS |
| Restore temporaries after a failure | `/tmp/dxs/tmp` empty | PASS |
| Disk full (chaos) | only the tmp→fsync→rename pattern; real exhaustion skipped | PARTIAL |
| Write failure (chaos) | rc=77, zero temporaries | PASS |
| Destroy of a partially applied stack (chaos) | takes the orphan, spares what belongs to others | PASS |
| Volume snapshots, quotas, sparse | — | NOT TESTED |
| NFS/CIFS/TrueNAS | requires CAP_SYS_ADMIN / appliance | ENVIRONMENT BLOCKED |
| Interruption mid-backup (SIGKILL) | — | NOT TESTED |
| Malicious symlink in the volume during backup | — | NOT TESTED |

No irreversible operation was observed presenting itself as transactional: the failed restore
did not touch the state, and `--force` only stops the resource when the archive is readable.
