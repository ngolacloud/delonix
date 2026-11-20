# H1 finding — Delonix CRI sets `HOME=/root` for a non-root container user

**Severity:** medium (breaks client-go-based workloads that run as non-root).
**Surfaced by:** the Sonobuoy aggregator (`sonobuoy/sonobuoy:v0.57.5`) failing to start
on a Delonix-CRI node, during the H1 quick run.
**Mode:** 1 (Delonix as the node CRI, golden VM `delonix-vm-k8s:1.36`, k8s v1.36.5).

## What happens

A container whose process runs as a **non-root uid with no `/etc/passwd` entry** gets
`HOME=/root` under Delonix CRI. runc/crun (and therefore containerd/CRI-O) set `HOME` from
the user's passwd entry, defaulting to `/` when there is none — never `/root`.

Measured on this cluster with a plain busybox pod:

```
$ kubectl run homecheck --image=busybox:1.36 --restart=Never \
    --overrides='{"spec":{"securityContext":{"runAsUser":1000,"runAsGroup":3000}}}' \
    --command -- sh -c 'echo "HOME=[$HOME]"; id'
HOME=[/root]
uid=1000 gid=3000 groups=3000
```

runc would yield `HOME=/` here (uid 1000 has no passwd entry and busybox sets no `HOME`).

## Why it breaks a real workload

`/root` is mode `0700` owned by root, so a non-root process cannot traverse it. client-go's
default kubeconfig loading rules stat `$HOME/.kube/config`; with `HOME=/root` that stat
returns **EACCES**, which client-go treats as a fatal load error instead of the **ENOENT**
it would get at `/.kube/config` — the ENOENT is what makes it fall through to the in-cluster
config. So the Sonobuoy aggregator (runs as a non-root image user) died at startup:

```
level=error msg="getting kubeconfig: error loading config file \"/root/.kube/config\":
                 open /root/.kube/config: permission denied"
```

The same would hit any client-go tool (operators, controllers, CLIs) that runs non-root and
relies on the in-cluster fallback — a broad class of CNCF workloads.

## Scope / not a CRI-pull-or-run failure

Delonix CRI pulled the Sonobuoy image (6.5 s) and started/ran the container — the failure is
purely the injected `HOME`. The pull/run/aggregate pipeline itself is sound (the quick run
completes once `HOME` is corrected).

## Workaround used for the H1 quick proof

Inject `HOME=/tmp` into the aggregator container (and `--plugin-env e2e.HOME=/tmp`) so the
default-kubeconfig stat hits a traversable dir (ENOENT) and the in-cluster fallback applies.
This is a lab workaround, not a fix.

## Fix (out of scope here — delonix-cri)

Honor the container user's passwd home dir, and default to `/` (OCI/runc behaviour) when the
uid has no passwd entry — never hardcode `/root` for a non-root user. A follow-up for the CRI
crate, alongside the critest CRI-F1/F2 gaps.
