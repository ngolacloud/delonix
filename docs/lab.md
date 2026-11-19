# The overnight lab

Decision D1 of [`docs/discovery/65_PLANO_MATURIDADE.md`](discovery/65_PLANO_MATURIDADE.md).

GitHub's hosted runner blocks unprivileged user namespaces. Because of that the
chaos workflow stays `skipped` and the E2E battery never runs in CI. The
[`.github/workflows/lab.yml`](../.github/workflows/lab.yml) workflow runs everything every
night on a self-hosted runner:

1. the host preflight;
2. the build;
3. the full E2E battery;
4. the SKIPs gate;
5. the chaos harness;
6. the performance gate.

## The host

What `scripts/lab_preflight.sh` requires. It names EVERYTHING that is missing and exits with
an error:

| Piece | Why |
|---|---|
| unprivileged user namespaces, `/etc/subuid` and `/etc/subgid` | the engine is rootless |
| `cpu cpuset io memory pids` delegated to `user@<uid>.service` | without them limits are refused, and the battery skips them |
| `br_netfilter` loaded and `bridge-nf-call-iptables=1` | without them per-namespace isolation is refused (D5) |
| `/dev/kvm` readable and writable; `virsh`, `qemu-*`, `cloud-hypervisor`, `cloud-localds`, `virt-customize`; `qemu:///system` reachable | the VM sections of the two local backends |
| the EDK2 `CLOUDHV.fd` in `/usr/local/share/delonix` | it is the only firmware that boots the project's images in Cloud Hypervisor |
| `slirp4netns`, `nft`, `ip`, `conntrack`, `newuidmap`, `protoc`, `wg` | rootless networking, encrypted overlay, and the build |
| 80 GiB free in the runner's home | images, VMs, and cargo's target |

Full delegation of the five controllers requires a drop-in as root:

```bash
sudo mkdir -p /etc/systemd/system/user@.service.d
printf '[Service]\nDelegate=cpu cpuset io memory pids\n' | sudo tee /etc/systemd/system/user@.service.d/delegate.conf
sudo systemctl daemon-reload && sudo systemctl restart user@$(id -u runner).service
```

`daemon-reload` alone is not enough, because a `user@` already running does not pick up new
controllers. This is measured in `AGENTS.md`.

## Registering the runner

One command, from the `runner` account on the lab host. The
[`scripts/lab_runner_register.sh`](../scripts/lab_runner_register.sh)
provisions whatever enablers are missing (cgroup delegation, linger,
`subuid`/`subgid`, and — on kernels that restrict it — the userns AppArmor
profile), downloads and configures the GitHub Actions runner with the
**`delonix-lab`** label, installs it as the account's service, and finishes by
running `lab_preflight.sh` as a final gate:

```bash
scripts/lab_runner_register.sh \
  --url https://github.com/ngolacloud/delonix \
  --token "$(gh api -X POST repos/ngolacloud/delonix/actions/runners/registration-token -q .token)"
```

The registration token is **ephemeral and belongs to the owner**: minted at *Settings → Actions →
Runners → New self-hosted runner* or with the `gh` line above (needs the repository's
admin account). The script never creates it — it is the owner's credential, not the
runner host's. Pass `--no-provision` if a root has already set up the enablers by
hand, and `--version` to pin the runner's version.

Confirm with a manual trigger:

```bash
gh workflow run lab.yml        # leaves the queue and starts on this host
```

A PR run of `chaos.yml` on a hosted runner still reports `skipped` by design (its
userns probe); the real chaos + E2E coverage runs here, through `lab.yml`.

## Declared SKIPs

`scripts/lab_allowed_skips.txt` starts empty. After the first night with the full host,
it prints the SKIPs that remain:

```bash
python3 scripts/lab_skip_gate.py --print /tmp/delonix-lab-e2e/results.jsonl
```

Each one left on the list carries the reason the lab cannot
perform that measurement: credentials for a remote provider, a GPU, or a second
node. The gate also fails when a declared line starts running. In that case the
line comes out in the same commit.
