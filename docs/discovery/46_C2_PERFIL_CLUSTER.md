# 46/C2 — Timing profile of cluster creation

| Field | Value |
|---|---|
| Date | 2026-08-10 |
| Command | `delonix cluster create --name perfc2 --workers 0` (kind mode, 1 control-plane) |
| Host | rootless, 5 production containers running, under `systemd-run --user --scope -p Delegate=yes` |
| Image | `kindest/node:v1.34.0` (424.8 MiB), **already local** — the `pull` does not enter into any of these numbers |
| Method | One real run, with each progress line timestamped from the start |

---

## 0. The premise of the request is wrong, and it needs to be said

The original request talks about having "a Kubernetes cluster up in a few
**milliseconds**." That is physically impossible, and not for lack of optimization: `kubeadm init`
**waits** for etcd, the apiserver, the controller-manager, and the scheduler to become healthy, and a kind node boots
`systemd` and `containerd` inside it before any of that begins. Nothing this engine does
reduces a wait for another process's *readiness* to milliseconds.

What can be done — and is what this document delivers — is to **measure where the time is**,
shorten what is ours, and declare the floor.

## 1. The measured profile

```
    0.0s  ✓ Ensuring node image (kindest/node:v1.34.0)
    2.6s  (node container created)
   79.4s  ✓ Preparing nodes (1)
   79.4s  ✓ Writing configuration
  541.7s  ✓ Starting control-plane
  542.0s  ✓ Installing CNI (kindnet)
  560.5s  ✓ Waiting for control-plane to be Ready
  560.5s  kubeconfig written, context set
```

| Stage | Duration | Share | Whose time it is |
|---|---|---|---|
| Ensure the node image | ~0 s | — | ours (already local) |
| Create the node container | 2.6 s | 0.5 % | **ours** |
| Prepare the node | 76.8 s | 14 % | mixed: our `run` + `systemd`/`containerd` booting inside |
| Write the configuration | ~0 s | — | ours |
| **`kubeadm init`** | **462.3 s** | **82 %** | **kubeadm's** |
| Install the CNI | 0.3 s | 0.05 % | ours (applies a manifest) |
| Wait for `Ready` | 18.5 s | 3 % | kubelet's/CNI's |
| **Total** | **560.5 s** | | |

**`kubeadm init` dominates with 82 %.** Everything the engine controls — creating the container, writing
the configuration, applying the CNI — adds up to less than 3 seconds out of 560.

## 2. The real gain obtained, and why it is small HERE

This same session's performance fix (the rootfs no longer being extracted twice on
`--net <custom-network>`, see the `perf:` commit) applies to each node, because a kind node **is** a
`container run` on a custom network. Measured with the node image itself:

| | `container run` of `kindest/node` |
|---|---|
| before | 5,226 ms |
| after | **2,762 ms** (−48 %) |

But it is necessary to be honest about what that is worth in the cluster: **~2.5 s saved out of 560 s = 0.45 %**.
I did suggest, before measuring the stage, that the fix would noticeably shorten cluster
creation. **It does not.** The node's `container run` is ~2.7 s out of a 76.8 s stage, and
that stage is 14 % of a total dominated by `kubeadm`. The fix is excellent for
`container run` (−46 % on a 431 MB image) and nearly irrelevant for `cluster create`.

In an N-node cluster the gain scales (~2.5 s per node), and remains small next to the 462 s.

## 3. The theoretical floor, declared

With the node image already local and the engine costing <3 s, the floor is **how long kubeadm and kubelet
take to become healthy**. On this machine that was 462 s + 18 s. A typical kind run does the same
work in 30–60 s, which says this host is well below normal — likely
CPU contention (5 production containers running, plus parallel Rust builds during the
measurement). **I did not isolate the inside of the 462 s** and I say so rather than attribute it: doing so requires
another run with `kubeadm` verbosity, i.e. another ~9 minutes, and I did not do it.

What is known of the inside, and it is little: the control-plane images **are** in the
node's containerd (`kube-apiserver-amd64`, `etcd`, `coredns` — the set that `kindest/node` ships), and all
the components only showed up running near the END of the window. So the ~7 minutes were
spent **before** the static pods came up, not downloading images.

## 4. Candidates, and what is NOT worth it

- **Not worth it:** optimizing node container creation, writing the configuration, or
  applying the CNI: together they add up to less than 3 s. Optimizing 0.5 % is noise.
- **Worth investigating:** the inside of the 462 s before anything else — it is 82 % of the
  problem, and without knowing whether it is this host's contention or something structural, any downstream
  work is a guess. It is the next measurement, not the next fix.
- **Parallelizing nodes** (for `--workers N`) only attacks the 76.8 s stage and only when N > 1; the
  first control-plane's `kubeadm init` is sequential by nature, and the `join`s depend on the
  token it produces.

## 5. Hygiene

The `perfc2` cluster was removed at the end. The host was left with the 5 production containers and nothing
else.
