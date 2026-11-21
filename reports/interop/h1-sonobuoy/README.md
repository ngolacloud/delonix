# H1 — Sonobuoy against Delonix-CRI: measured run

**Result:** pipeline PROVEN (quick slice). Sonobuoy ran end to end on a cluster whose node
CRI is Delonix; the e2e plugin passed with 0 failures. This is the first measured conformance
step — **NOT** a certified-conformance claim (that is the `--mode certified-conformance`
follow-up, out of scope here).

## Environment

- Cluster: `delonix cluster kubeadm`, 1 node (control-plane, untainted), golden VM
  `delonix-vm-k8s:1.36` (Ubuntu 24.04, libvirt), bridge CNI, Mode 1.
- Node CRI: **`delonix://0.1.1`** · kubelet/k8s **v1.36.5** · apiserver v1.36.6.
- Sonobuoy: **v0.57.5**, `--mode quick`.

## What ran, and the proof

Delonix CRI pulled the Sonobuoy aggregator image and the Kubernetes conformance image and ran
the aggregator + both plugins. Result (`results-summary.txt`, `sonobuoy-status.txt`):

- **e2e plugin: passed — Passed 8, Failed 0, Skipped 7578.** The one real quick test
  (`quick-test.txt`): `[sig-node] Pods should be submitted and removed [NodeConformance]
  [Conformance]` — a pod-lifecycle conformance test, i.e. exactly the CRI path. The other 7
  "passed" are the suite framework steps (Report/Synchronized Before/After suite).
- **systemd-logs plugin: passed 1/1.**
- Node health 1/1 (100%), Pods 10/10 (100%).

(The "Errors detected in files" block in `results-summary.txt` is Sonobuoy grepping component
pod logs for the word "error" — normal apiserver/scheduler/etcd log noise, not test failures;
the e2e plugin status is `passed`, Failed 0.)

## The one real snag — a Delonix CRI finding

The Sonobuoy aggregator (a non-root image user) first failed to start: Delonix CRI sets
`HOME=/root` for a non-root uid, where runc uses the passwd home or `/`. client-go then hits
EACCES on `/root/.kube/config` instead of falling through to the in-cluster config. Detailed
in `finding-home-nonroot.md`. Worked around for this run with `HOME=/tmp`; the fix belongs to
the `delonix-cri` crate (a follow-up, with the critest CRI-F1/F2 gaps).

## Reproduce

```sh
delonix cluster kubeadm --name h1quick --control-plane 1 --workers 0 --memory 3G --vcpus 2
export KUBECONFIG=~/.local/share/delonix/clusters/h1quick-kubeconfig.yaml
# HOME=/tmp workaround for the finding above:
sonobuoy gen --mode quick --plugin-env e2e.HOME=/tmp | \
  <inject HOME=/tmp into the aggregator container> | kubectl apply -f -
sonobuoy retrieve && sonobuoy results <tarball>
delonix cluster destroy h1quick
```
