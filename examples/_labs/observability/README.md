# Delonix observability lab (dogfooding)

A full **Prometheus + Grafana + OpenTelemetry** stack where every component runs
**as a Delonix-managed container**, scraping and tracing the Delonix runtime
itself. It proves the three observability legs end to end:

1. **Structured logs** — `tracing` (stderr; `DELONIX_LOG_FORMAT=json` for ingest).
2. **Prometheus metrics** — the shared registry (`delonix_telemetry::metrics`),
   served over TCP by `delonix-mgmt`.
3. **OTLP traces** — `cri.*` spans exported over OTLP/HTTP to a collector.

```
 delonix-cri ──OTLP/HTTP:4318──▶ otel-collector ──OTLP:14317──▶ Jaeger :16686
 delonix-mgmt ──/metrics:9199──◀── Prometheus :9091 ──────────▶ Grafana :3000
```

## Prerequisites

- `delonix` on PATH (rootless is fine).
- `crictl` (only to generate `cri.*` spans).

## 1. Start the runtime servers with telemetry on

The rich `/metrics` (resource gauges + per-workload series) is served over TCP
by `delonix-mgmt` when `DELONIX_MGMT_METRICS_ADDR` is set. Traces are exported
when `DELONIX_OTLP_ENDPOINT` is set.

```bash
export DELONIX_ROOT="$HOME/.local/share/delonix"   # same root the CLI uses
export DELONIX_MGMT_METRICS_ADDR=0.0.0.0:9199
export DELONIX_OTLP_ENDPOINT=http://127.0.0.1:4318
delonix serve api &                                 # management API + TCP /metrics
```

## 2. Bring up the observability stack

```bash
./run.sh          # pulls images, starts jaeger/otel-collector/prometheus/grafana
```

URLs:

| Service    | URL                     | Check                                         |
|------------|-------------------------|-----------------------------------------------|
| Grafana    | http://localhost:3000   | dashboard **Delonix / Delonix Runtime**       |
| Prometheus | http://localhost:9091   | **Status → Targets**: `delonix-runtime` is UP |
| Jaeger     | http://localhost:16686  | service **delonix-cri**                       |

## 3. Generate load and traces

```bash
# metrics: run a couple of containers / a VM so the gauges move
delonix run -d --name load-1 --net <some-net> alpine:3.19 sh -c 'while true; do wget -qO- http://example.com; sleep 3; done'

# traces: exercise the CRI server (spans cri.run_pod_sandbox, cri.create_container, …)
export CONTAINER_RUNTIME_ENDPOINT=unix:///run/delonix-cri.sock
crictl runp pod-config.json
crictl create <pod> ctr-config.json pod-config.json
crictl start <ctr>
```

Note: a container's per-interface network bytes are only counted when it has its
own netns (a bridge network). Containers on the host netns (the rootless default
with no `--net`) are reported under `delonix_network_unmeasured_containers`.

## Tear down

```bash
./stop.sh
```

## Notes on the wiring

- Components share the **host network namespace** (rootless default), so every
  service binds a host port and `localhost` wiring just works — no `-p` needed.
- The otel-collector is started with a `--` separator
  (`delonix run … IMAGE -- --config=…`) because `delonix run` owns a `--config`
  flag of its own; without the separator it captures the container's `--config`.
- Jaeger's OTLP intake is moved to `:14317/:14318` so it does not clash with the
  collector's `:4318` on the shared netns.
