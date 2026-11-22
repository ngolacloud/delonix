# Delonix logging lab — centralising resource logs in Loki, Graylog and ELK

This lab proves, end to end and on one host, that the logs Delonix produces for
its resources (containers, VMs, …) can be **centralised** in the log-ingestion
tools teams actually run — Grafana **Loki**, **Graylog**, and the
**Elasticsearch/Kibana** (ELK) stack — so the data lands in one place for
decision-making.

It does so **without adding any network log client to the runtime**. That is
deliberate, and it is the architecture:

> `docs/ARCHITECTURE-SCOPE.md`: *"No centralized logging infrastructure (Loki,
> ELK). Observability is orchestrator responsibility. Runtime emits events;
> paas aggregates."*

The runtime's job is to **emit** well-structured logs to files; an external
**collector** tails them and **ships** them out. This lab wires a single
[Vector](https://vector.dev) collector to all three sinks at once. Swap Vector
for Fluent Bit, Promtail or the OpenTelemetry Collector and nothing on the
runtime side changes — that portability is the point.

```
 delonix resources                      collector (Vector)            sinks
 ─────────────────                      ──────────────────            ─────
 container stdout/stderr ─┐                                       ┌─▶ Loki ──▶ Grafana
   <root>/containers/<id>/log           tail + normalise          │
 VM serial console ───────┼──▶  delonix_kind / _id / _name  ──────┼─▶ Elasticsearch ▶ Kibana
   <root>/vms/<ns>/<name>.serial        + real timestamp          │
 runtime events ──────────┘                                       └─▶ Graylog (GELF/TCP)
   <root>/events.jsonl
```

## What the runtime provides (and this lab relies on)

| Resource | Where the log is | How to get it | Shape |
|---|---|---|---|
| **Container** | `<root>/containers/<id>/log` | always captured | raw, or CRI `"<ts> <stream> <F\|P> <body>"` with `--log-cri` |
| **VM** | `<root>/vms/<ns>/<name>.serial` | `vm create --capture-serial` | raw serial console; read it with `delonix vm logs <name>` |
| **Events** | `<root>/events.jsonl` | always | one JSON object per line |
| **Network** | host journald | nft `log prefix` (firewall rules) | kernel log — a journald source on the collector picks it up |

Clusters are not a separate source: a Delonix cluster node **is** a VM, so its
node logs flow through the VM leg above; in-cluster workload logs are Kubernetes'
own `kubectl logs` domain.

The collector identifies each line's origin from the file path
(`delonix_kind`, and the id or name), and enriches container ids with
name/image from a small CSV the lab generates from the runtime's records. Those
become the labels/fields you filter on in every sink.

## Run it

Requires `delonix` on `PATH` (or `DELONIX=/path/to/delonix`), `curl`, `python3`,
and enough RAM for two JVM search engines plus the demo workloads (~8–10 GiB).
Components share the host network namespace (the rootless default).

```bash
./run.sh      # pulls images, brings up all three stacks + Vector + demo workloads
./stop.sh     # tears everything down
```

Ports are chosen to not collide with the observability (OTel) lab:

| Service | URL | Login |
|---|---|---|
| Grafana (Loki + ES) | http://localhost:3001 | anonymous admin |
| Kibana (Elasticsearch) | http://localhost:5601 | — |
| Graylog | http://localhost:9000 | `admin` / `admin` |
| Loki API | http://localhost:3100 | — |
| Elasticsearch API | http://localhost:9200 | — |

## Verify the three sinks received the resources' logs

After ~30 s of ingestion:

```bash
# Loki — lines labelled by resource kind
curl -sG localhost:3100/loki/api/v1/query_range \
  --data-urlencode 'query={kind="container"}' | head -c 400
curl -sG localhost:3100/loki/api/v1/query_range \
  --data-urlencode 'query={kind="vm"}' | head -c 400

# Elasticsearch — the delonix-logs-* index
curl -s 'localhost:9200/delonix-logs-*/_search?q=delonix_kind:container&size=1'
curl -s 'localhost:9200/delonix-logs-*/_count?q=delonix_kind:vm'

# Graylog — open http://localhost:9000 -> Search, filter:
#   delonix_kind:container      delonix_kind:vm
```

In **Grafana** → Explore → Loki, `{kind=~".+"}` shows every resource's stream;
in **Kibana**, create a data view `delonix-logs-*` and filter on `delonix_kind`.

## Files

| File | Role |
|---|---|
| `run.sh` / `stop.sh` | bring the lab up / down (idempotent) |
| `vector.yaml` | the collector: sources (files) → normalise (remap) → 3 sinks |
| `grafana/provisioning/datasources/` | Loki + Elasticsearch datasources |
| `enrichment/containers.csv` | generated: container id → name/image |
