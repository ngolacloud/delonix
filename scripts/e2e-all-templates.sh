#!/bin/bash
# End-to-end test: verify that EVERY project template `delonix init` ships is
# production-sound — generated, built and brought up healthy, with the VM image
# validated where the template supports one.
#
# Templates fall into two classes, because they do not all run the same way:
#
#   app    A single service that binds its own HTTP port and ships a VM image.
#          Runs from a lone `container run`. Full 5 phases: init → build →
#          run+health → VM build → VM structure.
#          (go node nestjs nextjs fastapi django laravel rust)
#
#   stack  Cannot be brought up by a lone `container run`: it needs something
#          the MANIFEST provides — a database (odoo, wordpress) or the TLS
#          certificate its server loads at startup (nginx, httpd, haproxy all
#          load an HTTPS vhost whose cert is mounted from `./tls`, so without
#          the manifest the process exits before the health port ever opens).
#          Brought up through `delonix stack apply` against the prod-shaped
#          manifest, whose image is `<name>:dev` (so the build is tagged
#          `:dev`). No VM image. 4 phases: init → build(:dev) →
#          stack apply+health → stack destroy.
#          (nginx httpd haproxy odoo wordpress)
set -u
B="${E2E_BINARY:?E2E_BINARY not set: pass E2E_BINARY=<delonix-binary>}"
W="${E2E_WORKDIR:?E2E_WORKDIR not set: pass E2E_WORKDIR=<work-directory>}"
TEMPLATES="${*:-go node nestjs nextjs fastapi django laravel rust nginx httpd haproxy odoo wordpress}"

# Per-template published port and health endpoint (kept in sync with each
# template's template.meta port=/health=).
declare -A TEMPLATE_PORT=(
  [go]=8000 [node]=8000 [nestjs]=3000 [nextjs]=3000 [fastapi]=8000 [django]=8000
  [laravel]=8000 [rust]=8000 [nginx]=8080 [httpd]=8080 [haproxy]=8080
  [odoo]=8069 [wordpress]=8080
)
declare -A TEMPLATE_HEALTH=(
  [go]="/api/v1/health/live" [node]="/api/v1/health/live" [nestjs]="/api/v1/health/live"
  [nextjs]="/api/v1/health/live" [fastapi]="/api/v1/health/live" [django]="/api/v1/health/live"
  [laravel]="/api/v1/health/live" [rust]="/api/v1/health/live"
  [nginx]="/healthz" [httpd]="/healthz" [haproxy]="/healthz"
  [odoo]="/web/health" [wordpress]="/wp-login.php"
)
# Class: app | stack  (anything unlisted defaults to app). `stack` templates are
# brought up through their manifest (they need a DB and/or the TLS cert it
# mounts); `app` templates run from a lone container and ship a VM image.
declare -A TEMPLATE_CLASS=(
  [nginx]=stack [httpd]=stack [haproxy]=stack
  [odoo]=stack [wordpress]=stack
)

mkdir -p "$W"
W=$(cd "$W" && pwd)

pass=0; fail=0
TIMEOUT=${E2E_TIMEOUT:-120}

echo "=========================================="
echo "E2E Test Suite: All Templates"
echo "=========================================="
echo "Binary:      $B"
echo "Work dir:    $W"
echo "Timeout:     ${TIMEOUT}s per phase"
echo "Templates:   $TEMPLATES"
echo ""

# Poll an HTTP health endpoint until it answers or the timeout elapses.
# $1 url  → returns 0 healthy, 1 timed out
wait_health() {
  local url="$1" elapsed=0
  while [ $elapsed -lt $TIMEOUT ]; do
    if curl -sf "$url" >/dev/null 2>&1; then return 0; fi
    sleep 1; elapsed=$((elapsed+1))
  done
  return 1
}

for template in $TEMPLATES; do
  echo "──────────────────────────────────────────"
  echo "Testing template: $template"
  echo "──────────────────────────────────────────"

  proj="$W/e2e-$template"
  if [ -e "$proj" ]; then
    echo "SKIP  $template: $proj already exists"
    continue
  fi

  class=${TEMPLATE_CLASS[$template]:-app}
  port=${TEMPLATE_PORT[$template]:-8000}
  health=${TEMPLATE_HEALTH[$template]:-"/api/v1/health/live"}

  # Phase 1: Init
  echo -n "  [init] delonix init ... "
  if ! "$B" init -t "$template" --name "e2e-$template" "$proj" </dev/null >"$W/e2e-$template.init.log" 2>&1; then
    echo "FAIL"
    echo "        $(tail -1 "$W/e2e-$template.init.log")"
    fail=$((fail+1)); continue
  fi
  echo "OK"

  # Phase 2: Build. `stack` templates must be tagged :dev — the manifest the
  # stack brings up references `<name>:dev`.
  tag="e2e-$template:latest"; [ "$class" = stack ] && tag="e2e-$template:dev"
  echo -n "  [build] delonix build ($tag) ... "
  t0=$(date +%s)
  if ! (cd "$proj" && "$B" build -t "$tag" . >"$W/e2e-$template.build.log" 2>&1); then
    secs=$(( $(date +%s) - t0 ))
    echo "FAIL (${secs}s)"
    echo "        $(tail -3 "$W/e2e-$template.build.log" | head -1)"
    fail=$((fail+1)); continue
  fi
  echo "OK ($(( $(date +%s) - t0 ))s)"

  # Phase 3: Run + health — path depends on class.
  ok=1
  if [ "$class" = stack ]; then
    echo -n "  [stack] stack apply + health ($port$health) ... "
    t0=$(date +%s)
    if ! (cd "$proj" && "$B" stack apply -f delonix-manifest.yaml >"$W/e2e-$template.apply.log" 2>&1); then
      echo "FAIL (apply)"
      echo "        $(tail -1 "$W/e2e-$template.apply.log")"
      ok=0
    elif wait_health "http://localhost:$port$health"; then
      echo "OK ($(( $(date +%s) - t0 ))s)"
    else
      echo "FAIL (${TIMEOUT}s timeout)"; ok=0
    fi
    (cd "$proj" && "$B" stack destroy -f delonix-manifest.yaml >/dev/null 2>&1) || true
  else
    echo -n "  [run] container run + health ($port$health) ... "
    t0=$(date +%s)
    cid=$("$B" container run --detach --net host -p "$port" "$tag" 2>"$W/e2e-$template.run.log" || echo "")
    if [ -z "$cid" ]; then
      echo "FAIL (start)"; ok=0
    elif wait_health "http://localhost:$port$health"; then
      echo "OK ($(( $(date +%s) - t0 ))s)"
    else
      echo "FAIL (${TIMEOUT}s timeout)"
      "$B" container logs "$cid" 2>/dev/null | grep -iE "listening|error" | head -2 | sed 's/^/        /' || true
      ok=0
    fi
    [ -n "$cid" ] && "$B" container rm -f "$cid" >/dev/null 2>&1 || true
  fi
  if [ $ok = 0 ]; then fail=$((fail+1)); rm -rf "$proj" "$W/e2e-$template"*.log; continue; fi

  # Phases 4-5: VM — only for `app` templates (template.meta vm_support=yes).
  # stack templates ship no VM image, so these phases do not apply.
  if [ "$class" = app ]; then
    echo -n "  [vm-build] VM build (syntax) ... "
    vm_ok=1
    if [ -f "$proj/VMfile" ]; then
      head -1 "$proj/VMfile" | grep -q "^FROM" || { echo "FAIL (VMfile)"; vm_ok=0; }
    elif [ -f "$proj/vm.yaml" ]; then
      python3 -c "import yaml; yaml.safe_load(open('$proj/vm.yaml'))" 2>/dev/null || { echo "FAIL (vm.yaml)"; vm_ok=0; }
    else
      echo "FAIL (no VM files)"; vm_ok=0
    fi
    if [ $vm_ok = 1 ]; then
      if [ ! -f "$proj/cloud-init/user-data" ]; then echo "FAIL (cloud-init)"; vm_ok=0
      elif ! grep -q "ExecStart=" "$proj/cloud-init/user-data"; then echo "FAIL (systemd)"; vm_ok=0
      fi
    fi
    [ $vm_ok = 1 ] && echo "OK"

    echo -n "  [vm-struct] VM structure ... "
    if [ ! -f "$proj/.delonixignore" ]; then echo "FAIL (.delonixignore)"; vm_ok=0; else echo "OK"; fi

    if [ $vm_ok = 0 ]; then fail=$((fail+1)); rm -rf "$proj" "$W/e2e-$template"*.log; continue; fi
  else
    echo "  [vm] skipped — $class template ships no VM image (expected)"
  fi

  echo ""
  echo "✓ $template: PASS ($class)"
  pass=$((pass+1))
  rm -rf "$proj" "$W/e2e-$template"*.log
done

echo ""
echo "=========================================="
echo "Results: $pass PASS, $fail FAIL"
echo "=========================================="
[ $fail = 0 ] && echo "✓ All templates passed!" || echo "✗ Some failed"
exit $([ $fail = 0 ] && echo 0 || echo 1)
