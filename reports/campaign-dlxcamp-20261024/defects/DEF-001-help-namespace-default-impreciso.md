# DEF-001 — The `--help` text of the `--namespace` flag does not mention the `default` exception

- **Requirement/test:** SCOPE-01 (§8 isolation between scopes)
- **Severity:** P3 (documentation / user-facing text) — NOT a behaviour defect
- **Environment:** delonix v5.1.1 binary `55662f89…` (runtime from develop `558e5f20`), rootless host uid 1000

## Expected vs observed
`delonix run --help` describes `--namespace`:
> "Logical ISOLATION namespace (default `default`). Containers in different namespaces cannot reach each other (even on the same network); only a `kind: Dependency` crosses the boundary"

**Categorical** text. But ADR-0011 (dns-namespace-scoping) establishes, by design:
- `| namespace default | yes | reachable from any namespace, by design |` (line 50)
- "an open namespace is a shared space you opt into by not naming one — not a hole in isolation" (line 122)
- code `crates/adapters/delonix-sdn/src/infra.rs:9262`: "The default (shared) namespace. Reachable from every namespace by design"

## Minimal reproduction steps
```
run -d --name srv --net N -v app:/www:ro busybox httpd -f -p 80 -h /www      # namespace default
run -d --name web --namespace beta --net N busybox sleep 300
exec beta/web wget -qO- http://srv/   # REACHES (rc=0) — because default is shared by design
run -d --name web --namespace alpha --net N busybox httpd ...
exec beta/web ping <alpha_ip>         # 100% packet loss — isolation between named namespaces OK
```

## Impact
Low. The behaviour is correct and documented in ADR-0011; it is the `--help` text that teaches a wrong mental model ("cannot reach each other" with no exception), potentially leading someone to place sensitive workloads in `default` believing them to be isolated.

## Closure criterion
The `--help` for `--namespace` now mentions that the `default` namespace is a shared space reachable from any namespace (aligned with ADR-0011).

## Suspected cause
The clap field text was written before/independently of ADR-0011; documentation-vs-contract drift.
