# Video script — Delonix Runtime

Recording script for the community launch video series. Six short episodes, each
self-contained, taking someone from "never heard of this" to "I can ship a real stack" — without
Docker installed, at any point. Every command below was run and confirmed on the development host
before going into the script.

Order designed for binge-watching (each assumes the previous one), but each video also works
standalone — someone who only searches "Kubernetes without Docker" on YouTube has to understand episode 05 without having
seen 01. That is why each script below repeats the essential context in the first 20 seconds (the
"hook").

## Overview

| # | Title | Focus | Duration |
|---|---|---|---|
| 01 | Delonix in 6 minutes | Install, run the first container, why no Docker | 5–7 min |
| 02 | Volumes, networks and build | Persistent data, rootless SDN, `delonix build` | 8–10 min |
| 03 | Declarative microVMs | `delonix vm`, golden image, cloud-init | 7–9 min |
| 04 | Stack and Compose | Declarative manifest + native `docker-compose.yml` | 9–11 min |
| 05 | Kubernetes without Docker | `cluster kubeadm` — a real end-to-end cluster | 10–13 min |
| 06 | Production: firewall and dashboard | Network security, dashboard, disk | 8–10 min |

---

## 01 — Delonix in 6 minutes (5–7 min)

The "hello world": install, run the first container, and the one idea that matters — a
container and microVM engine with no daemon, no root.

### Hook (first 15s)

> "This is Delonix running an nginx container — with no Docker installed on this machine, no root,
> no daemon running in the background at all." *(screen: `docker: command not found` followed by
> `delonix container run` working anyway.)*

### Objectives

- Install with one command (`install.sh`)
- Understand "rootless-first, daemonless" in one sentence, no jargon
- Run, see, and remove the first container

### On-screen script

**Say:** Installation — one script, no sudo (except the one-time subuid/subgid step the first time).

```bash
curl -fsSL https://ngolacloud.github.io/delonix/install.sh | sh
delonix --version
```

**Say:** The first container — publishes a port, with no privilege at all.

```bash
delonix container run -d --name web -p 8080:80 nginx
curl localhost:8080
# <title>Welcome to nginx!</title>
```

**Say:** `ps`/`logs`/`rm` — the docker vocabulary you already know, works the same way.

```bash
delonix container ls
delonix container logs web
delonix container rm -f web
```

### The line that sticks

"No daemon" means: every command is the whole process, start to finish — if `delonix`
died halfway, there would be no zombie daemon managing anything. "Rootless" means: your
normal user, no `sudo`, no `docker` group giving de facto root.

### Final CTA

"Next video: volumes that survive restarts, isolated networks between containers, and
`delonix build` building your own image — no special Dockerfile, it's the same as always."

---

## 02 — Volumes, networks and build (8–10 min)

Data that survives the container, containers that discover each other by name, and building a
custom image with a normal Dockerfile.

### Hook

> "I'm going to delete this container with the database inside — and the data will survive."
> *(screen: creates a volume, writes a file inside, removes the container, brings up another
> pointing at the same volume, the file is still there.)*

### Objectives

- Named volume — create, mount, survive an `rm`
- Custom network — two containers talking by NAME (internal DNS)
- `delonix build` — normal Dockerfile, zero new syntax

### On-screen script

**Say:** Named volume — the data lives outside the container's lifecycle.

```bash
delonix volume create data
delonix container run -d --name db -v data:/var/lib/postgresql/data postgres:16-alpine
delonix container rm -f db
# recreate it — the volume, not the container, is what held the data
delonix container run -d --name db2 -v data:/var/lib/postgresql/data postgres:16-alpine
```

**Say:** Custom network + DNS by name — the app talks to "db", not to a memorized IP.

```bash
delonix network create backend
delonix container run -d --name db --net backend postgres:16-alpine
delonix container run -it --rm --net backend alpine sh -c \
  "apk add -q bind-tools && nslookup db"
```

**Say:** Build — any Dockerfile, with nothing adapted.

```dockerfile
# Dockerfile
FROM alpine
RUN echo "built on Delonix" > /msg.txt
CMD ["cat", "/msg.txt"]
```

```bash
delonix build -t my-app:dev .
delonix container run --rm my-app:dev
```

### Point to underline

The network is not a cosmetic detail: two containers on the SAME network resolve each other by name (like
Kubernetes' CoreDNS) — it is the foundation for everything coming in episode 04 (Compose) and 05 (Kubernetes).

### Final CTA

"Container is only half the story — next video: real microVMs, with their own kernel,
ready to run Kubernetes."

---

## 03 — Declarative microVMs (7–9 min)

When a container is not enough — hypervisor isolation, its own kernel, ready to boot a
Kubernetes node without installing anything the first time it powers on.

### Hook

> "This is not a container — it's a full VM, with ITS OWN kernel, booting in seconds,
> without me touching an installer." *(screen: `delonix vm create dev` → `vm console dev` → working
> login.)*

### Objectives

- Practical difference between container vs. microVM (isolation, not performance)
- Create a VM with the golden image, enter via console and SSH
- Declarative cloud-init (hostname/SSH key without touching an ISO by hand)

### On-screen script

**Say:** A VM with the SSH key already injected — no default password in production.

```bash
delonix vm create dev --ssh-key ~/.ssh/id_ed25519.pub --hostname dev
delonix vm ls
# wait for the IP to appear, then:
ssh delonix@$(delonix vm ls | grep dev | awk '{print $4}')
```

**Say:** Serial console — for when the network is not even ready yet.

```bash
delonix vm console dev
# Ctrl+] to return to the host
```

**Say:** The golden image already ships with Kubernetes ready — this is the hook for episode 05.

```bash
ssh delonix@<ip> "kubeadm version && kubelet --version"
```

### Point to underline

"Golden image" = the VM is born with `kubeadm`/`kubelet`/`delonix-cri` already installed — booting a node
installs nothing, it just does `kubeadm init`/`join`. That's what makes the next video genuinely fast.

### Final CTA

"Container for everyday use, VM for serious isolation — next video, a way to describe
both (plus network, plus volumes) in a single YAML file, and the `docker-compose.yml` you already have
working without changing a line."

---

## 04 — Stack and Compose (9–11 min)

Two entry points to the same result: Delonix's native declarative manifest, and the
`docker-compose.yml` that already exists in your project.

### Hook

> "This `docker-compose.yml` was never written with Delonix in mind — and it's just going to work."
> *(screen: a real app+Postgres compose, `delonix compose up`, both services coming up in the
> right order.)*

### Objectives

- `delonix stack init` → a complete project ready to edit
- `stack plan`/`apply` — a declarative manifest that CONVERGES (live, without changing the PID)
- `compose up` — the same `docker-compose.yml`, with no manual translation

### On-screen script

**Say:** Scaffold — a complete project (volume + DB + app) in one command.

```bash
delonix stack init myapp && cd myapp
cat delonix-manifest.yaml
delonix stack apply --dry-run   # shows the plan, creates nothing
delonix stack apply
```

**Say:** Now the same result, but starting from an existing docker-compose.yml.

```bash
delonix compose config          # validates, shows the resolved plan
delonix compose up
delonix compose ps
delonix compose logs db
```

**Say:** Real `depends_on` — the app only starts once the DB actually responds.

```yaml
# in docker-compose.yml:
#   depends_on:
#     db:
#       condition: service_healthy
```

```bash
delonix compose down -v   # cleans everything, including named volumes
```

### Point to underline

It is not a shim that just pretends — `depends_on: service_healthy` genuinely waits for the real healthcheck (the
service's own or the image's own) before starting the next one. A cycle in the graph gives a clear error,
never a made-up order.

### Final CTA

"One container, one VM, one stack — what's missing is the biggest piece: a real Kubernetes cluster, from scratch, with no
Docker anywhere. That's the next video."

---

## 05 — Kubernetes without Docker (10–13 min)

The flagship video: one command, from zero to a `Ready` control-plane — VMs provisioned, `kubeadm`
running, automatic HA HAProxy.

### Hook

> "I'm going to create a Kubernetes cluster with two control-planes and three workers — and Docker will never
> show up in this demo, not once." *(screen: `kubectl get nodes` at the end, all `Ready`.)*

### Objectives

- `cluster kubeadm` — provisions VMs + bootstrap, a single command
- Automatic HA (>1 control-plane provisions a HAProxy on its own)
- `delonix-cri` as the runtime — no containerd, no Docker

### On-screen script

**Say:** One command: provisions the VMs, generates the SSH key, boots the cluster.

```bash
delonix cluster kubeadm --name lab --control-plane 2 --workers 3
# `kind create cluster`-style progress — each step closes with a ✓
```

**Say (while it's coming up, without going silent):** explain what's happening behind the scenes —
it provisions 5+1 VMs (the +1 is the HAProxy, automatic with >1 control-plane), prepares each host,
runs `kubeadm init`, joins the remaining control-planes and the workers.

**Say:** The cluster is ready — pull the kubeconfig and confirm.

```bash
export KUBECONFIG=~/.local/share/delonix/clusters/lab-kubeconfig.yaml
kubectl get nodes -o wide
# NAME       STATUS   ROLES           VERSION
# lab-cp1    Ready    control-plane   v1.34.0
# lab-cp2    Ready    control-plane   v1.34.0
# lab-w1     Ready    <none>          v1.34.0
# ...
```

**Say:** Acid test — deploy a real app on the cluster.

```bash
kubectl create deployment web --image=nginx --replicas=3
kubectl get pods -o wide
```

### Point to underline

Every node's runtime is `delonix-cri` — there is no containerd nor Docker anywhere in the stack. And it is
idempotent "Terraform-style, but with no state file": run the same command again and it only
fixes what's missing.

### Final CTA

"A running cluster is only half the story — in the last video: firewall between containers, the
htop-style dashboard, and how to see where your disk is going before it's too late."

---

## 06 — Production: firewall and dashboard (8–10 min)

What changes when this stops being a demo — isolating traffic between containers, observing everything on
a single screen, and not letting the disk fill up silently.

### Hook

> "This database is going to stop responding to ANY container except the app — in two
> lines." *(screen: a `curl` failing after `net ingress policy deny`.)*

### Objectives

- Per-container L4 firewall (`net ingress`/`egress`)
- Interactive dashboard — RAM/network/disk KPIs on one screen
- `system df`/`prune` — not letting disk fill up silently

### On-screen script

**Say:** Block everything by default, only let in what's actually needed.

```bash
delonix net ingress policy db deny
delonix net ingress allow db tcp/5432 --from 10.219.0.0/16
delonix net ingress ls db
```

**Say:** The dashboard — not a snapshot, it's live (the runtime's equivalent of `htop`).

```bash
delonix dashboard
# arrow keys to navigate, `m` toggles the containers/memory graph
```

**Say:** It also scripts/Grafana-integrates — plain JSON, no TUI.

```bash
delonix dashboard --json | jq '.tiles'
```

**Say:** Disk — the silent villain of any rootless runtime (per-container rootfs).

```bash
delonix system df
delonix system prune   # only removes orphaned/unreferenced items — asks for confirmation
```

### Point to underline

"Last rule wins" in the firewall — changing an `allow`/`deny` for the SAME target replaces the
previous one, never piling up an invisible shadow behind it.

### Final CTA

"This closes the base series — installation, containers, VMs, stacks, Kubernetes, and production. From
here, the full documentation and each group's `--help` take you the rest of the way. If you're left with
any question, leave it in the comments."

---

## Production notes

- **Terminal:** large font (≥18pt), consistent light or dark theme across ALL videos
  (brand recognition), short prompt (avoid `user@hostname:~/long/path$` eating the screen).
- **Before recording each episode:** run the command script once dry, off-recording —
  every command in this document has already been tested on this host, but the local STATE
  (containers/networks already existing) may differ; clearing the environment before each recording avoids an
  `already exists` live.
- **Live errors are not bad luck — they're content:** if something fails during
  recording, don't cut — showing the error message and what it says to do works in the product's favour (clear
  error messages is an explicit goal of this project).
- **Caption/overlay:** each "Say" block above is the voice guide, not text to read — say it in
  your own words over what the screen shows.
- **Consistent thumbnail/title:** "Delonix Runtime — Ep. 0X: \<title\>", same orange palette
  (`#e8590c`) as the site/dashboard, so the series is recognisable in the channel list.

---

*Script prepared for Delonix Runtime's community launch — all commands were
run and confirmed on this host before going into the script.*
