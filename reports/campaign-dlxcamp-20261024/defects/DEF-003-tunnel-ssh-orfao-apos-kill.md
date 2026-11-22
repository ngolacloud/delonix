# DEF-003 — `net tunnel expose` leaves the `ssh -R` orphaned (public exposure persists) when the parent process is killed

- **Requirement/test:** TUNNEL-02 (§17 — stop/removal; "Delonix restart"), §19 (recovery/orphans)
- **Severity:** P2 (security — public exposure survives what the operator believes to be the shutdown)
- **Environment:** delonix v5.1.1 `350a6320…` (develop `558e5f20`), pinggy provider, rootless host

## Expected
Stopping a tunnel (closing the `delonix net tunnel expose` process) terminates the forwarding and closes the public exposure.

## Observed
- `net tunnel expose 18090 --provider pinggy` launches a child `ssh -p 443 ... -R 0:localhost:18090 free.pinggy.io`.
- When the parent `delonix net tunnel expose` process is killed (SIGTERM via `pkill -f`), the `ssh -R` child **survives** and the public URL **keeps serving the origin** (verified: `curl https://<id>.free.pinggy.net/index.html` → `dlxcamp-20261024-OK` AFTER the parent kill).
- Only an **explicit SIGKILL to the `ssh` process** closed the exposure.

## Minimal reproduction steps
```
delonix net tunnel expose 18090 --provider pinggy &   # note the URL
curl https://<id>.free.pinggy.net/           # serves the origin
pkill -f 'net tunnel expose 18090'            # "stop" the tunnel
curl https://<id>.free.pinggy.net/           # STILL serves (orphaned ssh -R alive)
pgrep -af 'ssh -p 443.*pinggy'                # orphaned process present
```

## Impact
An operator who terminates the tunnel process (or whose process dies) may believe the exposure is closed, when the orphaned `ssh -R` keeps it public. Risk of prolonged unintended exposure.

## Nuance / test limits
- The stop was by a non-graceful kill of the parent; graceful teardown (SIGINT/Ctrl+C in the foreground, or an eventual `tunnel stop`) may clean up correctly — it was not possible to exercise a `tunnel stop` (no such subcommand; the help only exposes `expose`/`apply`).
- Therefore the defect is: **non-graceful death of the parent does not take down the `ssh` child**. Confirm whether there is (or is missing) a process-group kill / `prctl(PDEATHSIG)` in the tunnel supervisor.

## Closure criterion
Killing the `delonix net tunnel expose` process (by any signal) also terminates the child `ssh -R` (e.g. placing it in the same process group and signalling the group, or `PR_SET_PDEATHSIG`), closing the exposure; ideally a `net tunnel stop`/`ls` would also exist.
