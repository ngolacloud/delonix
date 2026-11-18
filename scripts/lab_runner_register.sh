#!/usr/bin/env bash
# Register (and provision) the self-hosted LAB runner — decision D1 of the
# maturity plan, the owner's step named in docs/lab.md and at the top of
# .github/workflows/lab.yml.
#
# WHY THIS SCRIPT EXISTS
#
# The hosted GitHub runner blocks unprivileged user namespaces, so the chaos
# job and the E2E battery there go green by skipping everything (the lie the
# chaos `userns` probe refuses). The only place they run for real is a
# self-hosted runner carrying the label `delonix-lab`. Until one is registered
# the nightly `lab.yml` waits in the queue — it never turns green on someone
# else's machine. Registering it was a prose checklist in docs/lab.md; this is
# that checklist as one idempotent command, so a host goes from bare to
# "lab.yml runs here" in a single run.
#
# WHAT IT DOES, in order (each step idempotent, re-runs are safe):
#   1. provisions the three runner-specific enablers that need root — cgroup
#      delegation, linger, and (on kernels that restrict it) an AppArmor userns
#      profile for the delonix binary — plus the subuid/subgid ranges;
#   2. downloads and configures the GitHub Actions runner with the
#      `delonix-lab` label and installs it as a service of this account;
#   3. runs scripts/lab_preflight.sh as the FINAL gate — it names every other
#      missing piece (KVM, firmware, slirp4netns, 80G free, …) and fails closed,
#      because a runner that registers but cannot run the battery is the same
#      lie one level down.
#
# WHAT IT DELIBERATELY DOES NOT DO: install the VM/build toolchain or the EDK2
# firmware. That is scripts/install.sh's job and the preflight's to verify; a
# broken installer must fail in its own test, not be hidden inside this one
# (the same separation the chaos workflow keeps between the environment probe
# and the engine preflight).
#
# USAGE
#   scripts/lab_runner_register.sh --url <repo-or-org-URL> --token <reg-token>
#   scripts/lab_runner_register.sh --url https://github.com/ngolacloud/delonix \
#       --token "$(gh api -X POST repos/ngolacloud/delonix/actions/runners/registration-token -q .token)"
#
# The registration token is short-lived and PER-OWNER: mint it in
# Settings → Actions → Runners → New self-hosted runner, or with the `gh` line
# above (needs the owner's repo-admin auth). This script never mints it — that
# is the owner's credential, not the runner host's.
#
# Flags:
#   --url URL         repo or org to register against (required)
#   --token TOKEN     runner registration token (required)
#   --name NAME       runner name (default: delonix-lab-<hostname>)
#   --labels CSV      extra labels to add alongside the defaults (optional)
#   --version VER     runner version, e.g. 2.323.0 (default: latest on GitHub)
#   --work DIR        runner work dir (default: _work)
#   --dir DIR         where to install the runner (default: ~/actions-runner)
#   --skip-preflight  do not run lab_preflight.sh at the end (NOT recommended)
#   --no-provision    skip the root provisioning (steps already done by hand)
set -euo pipefail

SELF_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

# ---- args -----------------------------------------------------------------
URL="" TOKEN="" NAME="" EXTRA_LABELS="" RUNNER_VERSION="" WORK="_work"
RUNNER_DIR="${HOME}/actions-runner" SKIP_PREFLIGHT=0 PROVISION=1
# The label the workflow's `runs-on: [self-hosted, linux, delonix-lab]` selects
# on, kept here as the single source of truth.
LAB_LABEL="delonix-lab"

die() { printf 'lab-runner: %s\n' "$1" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --url)            URL="${2:-}"; shift 2 ;;
    --token)          TOKEN="${2:-}"; shift 2 ;;
    --name)           NAME="${2:-}"; shift 2 ;;
    --labels)         EXTRA_LABELS="${2:-}"; shift 2 ;;
    --version)        RUNNER_VERSION="${2:-}"; shift 2 ;;
    --work)           WORK="${2:-}"; shift 2 ;;
    --dir)            RUNNER_DIR="${2:-}"; shift 2 ;;
    --skip-preflight) SKIP_PREFLIGHT=1; shift ;;
    --no-provision)   PROVISION=0; shift ;;
    -h|--help)        sed -n '2,52p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *)                die "unknown argument: $1 (try --help)" ;;
  esac
done

[ -n "$URL" ]   || die "missing --url (the repo or org to register against)"
[ -n "$TOKEN" ] || die "missing --token (mint one as the owner; see --help)"
[ -n "$NAME" ]  || NAME="${LAB_LABEL}-$(hostname -s)"
[ "$(id -u)" -ne 0 ] || die "run as the runner account, NOT root — the script sudos only the steps that need it"

# `sudo` only where root is unavoidable; the rest runs as the runner account so
# the installed service belongs to it (a runner started as root would run every
# job's build as root too).
RUN_ROOT="sudo"
command -v sudo >/dev/null 2>&1 || die "sudo not found — needed for the cgroup/linger/AppArmor provisioning (or pass --no-provision if a root already did it)"

# ---- 1. provision the runner-specific enablers ----------------------------
provision() {
  local uid; uid=$(id -u)
  local user; user=$(id -un)
  echo "== provisioning host enablers (cgroup delegation, linger, subuid/subgid, userns) =="

  # Full cgroup-controller delegation to the user manager. Without it the
  # battery's resource limits are silently inert (memory.max=max), which reads
  # as "environment" and SKIPs — the lab's cardinal failure. daemon-reload is
  # not enough: a user@ already running does not pick up new controllers, so
  # restart it (documented in docs/testing/lab-requisitos.md).
  local dropin=/etc/systemd/system/user@.service.d/delegate.conf
  if ! grep -qs 'Delegate=cpu cpuset io memory pids' "$dropin" 2>/dev/null; then
    $RUN_ROOT mkdir -p /etc/systemd/system/user@.service.d
    printf '[Service]\nDelegate=cpu cpuset io memory pids\n' | $RUN_ROOT tee "$dropin" >/dev/null
    $RUN_ROOT systemctl daemon-reload
    $RUN_ROOT systemctl restart "user@${uid}.service"
    echo "  cgroup delegation drop-in written and user@${uid} restarted"
  else
    echo "  cgroup delegation already configured"
  fi

  # Linger keeps the user manager (and the runner service) alive with no login
  # session — a runner spends most of its life with nobody logged in.
  if ! loginctl show-user "$user" -p Linger 2>/dev/null | grep -q 'Linger=yes'; then
    $RUN_ROOT loginctl enable-linger "$user"
    echo "  linger enabled for $user"
  else
    echo "  linger already enabled"
  fi

  # subuid/subgid ranges: rootless maps the container's uids through newuidmap.
  for id in subuid subgid; do
    if ! grep -q "^${user}:" "/etc/$id" 2>/dev/null; then
      printf '%s:100000:65536\n' "$user" | $RUN_ROOT tee -a "/etc/$id" >/dev/null
      echo "  /etc/$id range added for $user"
    else
      echo "  /etc/$id range already present"
    fi
  done

  # Recent Ubuntu (23.10+) sets kernel.apparmor_restrict_unprivileged_userns=1:
  # a binary WITHOUT an AppArmor profile cannot create user namespaces and the
  # engine dies at unshare(). Write the same `userns` profile install.sh does,
  # scoped to the delonix binary the runner will build. Best-effort: on a host
  # that does not restrict userns this is a no-op.
  if [ "$(sysctl -n kernel.apparmor_restrict_unprivileged_userns 2>/dev/null || echo 0)" = 1 ]; then
    if command -v apparmor_parser >/dev/null 2>&1; then
      # The release build the lab job produces; the profile names the path it
      # will run from, matching the chaos workflow's AppArmor step.
      local binpath="${RUNNER_DIR}/_work/delonix/delonix/target/release/delonix"
      printf 'abi <abi/4.0>,\ninclude <tunables/global>\nprofile delonix %s flags=(unconfined) {\n  userns,\n}\n' \
        "$binpath" | $RUN_ROOT tee /etc/apparmor.d/delonix >/dev/null
      $RUN_ROOT apparmor_parser -r /etc/apparmor.d/delonix
      echo "  AppArmor userns profile installed for $binpath"
    else
      echo "  WARNING: kernel restricts userns but apparmor_parser is absent — the engine will fail at unshare(); install apparmor" >&2
    fi
  else
    echo "  host does not restrict userns — no AppArmor profile needed"
  fi
}

[ "$PROVISION" -eq 1 ] && provision

# ---- 2. download and configure the Actions runner -------------------------
echo "== configuring the GitHub Actions runner (label: ${LAB_LABEL}) =="
mkdir -p "$RUNNER_DIR"
cd "$RUNNER_DIR"

arch_tag() {
  case "$(uname -m)" in
    x86_64)           echo x64 ;;
    aarch64|arm64)    echo arm64 ;;
    *)                die "unsupported arch $(uname -m) for the GitHub runner" ;;
  esac
}

if [ ! -x ./config.sh ]; then
  local_ver="$RUNNER_VERSION"
  if [ -z "$local_ver" ]; then
    # The latest published runner; `"tag_name": "v2.323.0"` → `2.323.0`.
    # NOTE: do NOT pipe curl into `grep -m1`/`head` here — under `set -o
    # pipefail` the early-closing reader makes curl's interrupted write a fatal
    # error 23, which aborts the whole script. `sed -n …p` reads all of curl's
    # output (tag_name appears once in the /releases/latest payload), so the
    # producer never gets SIGPIPE.
    local_ver=$(curl -fsSL https://api.github.com/repos/actions/runner/releases/latest \
      | sed -nE 's/.*"tag_name":[[:space:]]*"v?([^"]+)".*/\1/p') \
      || die "could not resolve the latest runner version — pass --version"
    [ -n "$local_ver" ] || die "GitHub returned no runner version — pass --version"
  fi
  tarball="actions-runner-linux-$(arch_tag)-${local_ver}.tar.gz"
  echo "  downloading ${tarball}"
  curl -fsSL -o "$tarball" \
    "https://github.com/actions/runner/releases/download/v${local_ver}/${tarball}" \
    || die "download of $tarball failed"
  tar xzf "$tarball"
  rm -f "$tarball"
fi

# `delonix-lab` always; the defaults (self-hosted, linux, <arch>) are added by
# config.sh itself, and `--labels` only ADDS to them. Any extra labels the
# caller passed ride along.
labels="$LAB_LABEL"
[ -n "$EXTRA_LABELS" ] && labels="${labels},${EXTRA_LABELS}"

if [ -f .runner ]; then
  echo "  a runner is already configured here (.runner present) — leaving it; run ./config.sh remove first to re-register"
else
  ./config.sh --unattended --replace \
    --url "$URL" --token "$TOKEN" \
    --name "$NAME" --labels "$labels" --work "$WORK"
fi

# Install + start as a service of THIS account (svc.sh reads the configured
# runner), so it survives reboots and logouts.
if [ -x ./svc.sh ]; then
  $RUN_ROOT ./svc.sh install "$(id -un)"
  $RUN_ROOT ./svc.sh start
  $RUN_ROOT ./svc.sh status || true
fi

# ---- 3. final gate: the host can actually run the battery -----------------
if [ "$SKIP_PREFLIGHT" -eq 0 ]; then
  echo "== lab preflight (final gate) =="
  bash "${SELF_DIR}/lab_preflight.sh"
fi

cat <<EOF

lab-runner: ${NAME} registered with label '${LAB_LABEL}' against ${URL}.
Confirm the nightly job now has somewhere to run:
    gh workflow run lab.yml        # should leave the queue and start on this host
A hosted PR run of chaos.yml still reports 'skipped' by design (its userns
probe); the real chaos + E2E coverage runs here, through lab.yml.
EOF
