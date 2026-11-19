# Architecture: Scope of Each Repository

> ⚠️ **SUPERSEDED / HISTORICAL DOCUMENT — not canonical.**
>
> This is the record of a *spike* from 2026-10-15, prior to **ADR-0040**'s
> restructuring into layers. It describes a crate taxonomy and a product
> framing that **no longer match the engine**. In particular, do NOT take as
> true:
>
> - The phrase "PaaS execution engine … that runs on top of Kubernetes" (section 1).
>   Delonix is **daemonless, single-node, and consumer-agnostic**: it *consumes*
>   Kubernetes' CRI, it does not run on top of it nor does it have a notion of PaaS/tenant.
> - This document's crate names (`delonix-security-runtime`,
>   `delonix-scan`, `delonix-net`, `delonix-core`) — **do not exist**. The
>   real crates are `delonix-security`, `delonix-scanner`, `delonix-networking`,
>   `delonix-model`, in the layered tree `crates/{foundation,contexts,adapters,
>   providers,interfaces,assembly}`.
>
> **Canonical sources:** [`ARCHITECTURE.md`](../ARCHITECTURE.md) (root) and
> **ADR-0040** (`docs/adr/0040-*.md`). The map for contributors is
> [`docs/dev/architecture.md`](dev/architecture.md), which defers to those.
> The body below is preserved as written, only as a record of the spike.

**Date:** 2026-10-15  
**Status:** Superseded by ADR-0040 (historical spike; see banner above)  
**Decision:** Option B — pure runtime, observability in delonix-paas

---

## 1. delonix-runtime (This repository)

**What it is:** PaaS execution engine, written in Rust, that runs on top of Kubernetes. Responsible for securely admitting and running workloads (containers and VMs).

**Design principles (ADR-0026):**
- **Daemonless:** Every command is born, runs, dies. No resident processes.
- **Decision-only:** Evaluates admission policies. Does not monitor.
- **Stateless:** Emits events, but does not persist state locally.
- **Fail-closed:** Refuses by default; explicit-allow policy.
- **Minimal dependencies:** Only serde + delonix-core. No eBPF, YARA, or sensors.

### What IS in delonix-runtime

#### Security: Admission & Policy (✅ IMPLEMENTED)
- **delonix-security-runtime** (2703 LOC)
  - Admission control (container + VM paths)
  - Node policy enforcement (7 fields: denyPrivileged, denyHostNetwork, denyLatestTag, allowedRegistries, denyDevicePassthrough, denyLatestVmImage, allowedImageUrlHosts)
  - Event audit trail (events.jsonl with stable IDs)
  - Risk scoring and severity classification
  - PII/sensitive data redaction
  - Lint warnings on partial policies (POLICY-VM-PASSTHROUGH-OPEN, etc.)

#### Security: Supply Chain (✅ IMPLEMENTED)
- **delonix-scan** (939 LOC)
  - Cosign signature verification
  - SBOM generation + CVE scanning
  - Integrated as scan-on-pull gate

#### Security: Container Hardening (✅ IMPLEMENTED)
- **seccomp_profile.rs** (756 LOC)
  - OCI seccomp profiles
  - Capability filtering per workload
  - Architecture-specific policies

#### Security: Secrets (✅ IMPLEMENTED)
- **core/secret.rs + core/cred_vault.rs**
  - Encryption at rest (XChaCha20)
  - Vault integration

#### Networking: eBPF (⚠️ LIMITED)
- **delonix-net/src/bpf.rs**
  - Private egress control (RFC 1918, CGNAT blocking)
  - **Limitation:** Requires CAP_BPF + CAP_NET_ADMIN (not available in rootless mode, the primary deployment mode)

#### Observability: Event Emission (✅ IMPLEMENTED)
- **core/events.rs**
  - Append-only, lock-free event log
  - Best-effort (errors don't block operations)
  - Non-evidence (daemonless design, editable locally)
  - Kind: "security" events with stable identifiers

#### Documentation (✅ IMPLEMENTED)
- **docs/guia-politica-de-seguranca.md** (164 LOC)
  - Complete policy.json guide with examples
  - Governance rules documented
  - Field validation and mode (enforce/warn)

- **docs/adr/0026-security-runtime-decision-crate.md**
  - Architecture decision: decision crate, not sensor platform
  - Boundary preservation (no tenancy, no approval, no continuous detection)
  - Consequences and alternatives considered

### What is NOT in delonix-runtime

#### ❌ Continuous Runtime Monitoring
- No eBPF sensors (would violate daemonless principle)
- No file integrity monitoring (resident process)
- No behavioral anomaly detection (resident process)
- No malware scanning (resident process)
- **Rationale:** ADR-0026, guardrail #1. Sensors are resident processes. delonix-runtime is ephemeral.

#### ❌ Multi-tenant Scoping
- No `tenant` field in policies, events, or scores
- No audit context per request
- No identity/approval workflows
- **Rationale:** ADR-0010 (Rejected), ADR-0003 (Proposed), ADR-0025 (Accepted). Tenancy lives in delonix-paas.
- **Test:** Build fails if `tenant`/`project`/`environment` appears in any file.

#### ❌ Observability Stack Components
- No Prometheus metrics (no /metrics endpoint)
- No OpenTelemetry exporters (no traces)
- No centralized logging infrastructure (Loki, ELK)
- No distributed tracing (Jaeger, Tempo)
- No alerting rules (Prometheus Alertmanager, PagerDuty)
- **Rationale:** Observability is orchestrator responsibility. Runtime emits events; paas aggregates.

#### ❌ Compliance Documentation
- No CIS Kubernetes Benchmark claims
- No NIST CSF mappings
- No compliance scanner integration
- **Rationale:** Compliance is measured at the paas layer (Kubernetes hardening, RBAC, NetworkPolicy). Runtime is a component, not a full stack.

#### ❌ eBPF in Rootless Mode
- Available() returns false in rootless (primary mode)
- CAP_BPF + CAP_NET_ADMIN not available
- **Rationale:** Kernel limitation. Measured in delonix-net/src/bpf.rs; ADR spike needed if this changes.

---

## 2. delonix-paas (Separate repository)

**What it is:** Kubernetes control plane for delonix-runtime. Manages tenancy, orchestration, observability, and compliance across a fleet of runtime instances.

### What CAN be in delonix-paas

#### Multi-tenancy & Identity
- Tenant scoping (policies, events, audit context per tenant)
- RBAC + ABAC (who can create/delete/inspect resources)
- Identity providers (OAuth2, SAML, internal)
- Audit logging with tenant context

#### Observability & Monitoring
- Prometheus metrics aggregation (from all runtime instances)
- OpenTelemetry integration (traces from runtime events)
- Loki log aggregation (from all runtime nodes)
- Distributed tracing (Jaeger, Tempo)
- Custom dashboards (security KPIs, resource usage)
- Real-time alerting (anomaly detection, quota violations)

#### Compliance & Validation
- CIS Kubernetes Benchmark enforcement (control plane hardening)
- NIST CSF mapping (measured against actual cluster state)
- Pod Security Standards validation
- RBAC audit (every role verified as least-privilege)
- Network Policy audit (explicit allow-lists, no default-allow)
- etcd encryption at rest verification
- Admission webhook validation

#### Policy Enforcement (Layer Above Runtime)
- Wrapper around delonix-security-runtime policies
- Tenant-scoped policy evaluation
- Policy versioning and rollout
- Approval workflows (requires identity)
- Automated remediation (requires tenant context)

#### Supply Chain (Augmenting Runtime)
- Registry credential management
- Image signature verification (cosign) — runtime does scan-on-pull, paas tracks inventory
- SBOM storage and lifecycle
- Dependency scanning across tenant workloads

### Consumer of delonix-runtime
- Imports `delonix-security-runtime` crate
- Consumes events from runtime (events.jsonl)
- Evaluates policies with tenant context
- Enriches audit trails with identity/approval info
- Enforces quotas, rate limits, cost models

**Reference:** `crates/delonix-core/src/policy_store.rs` already imports from delonix-security-runtime without linking private code back.

---

## 3. delonix-deploy (Future, or separate repository)

**What it is:** GitOps / Infrastructure-as-Code layer. Deploys and configures delonix-runtime + delonix-paas to Kubernetes clusters.

### What CAN be in delonix-deploy

#### Kubernetes Hardening
- RBAC definitions (ServiceAccount, Role, RoleBinding per component)
- NetworkPolicy manifests (ingress/egress rules, pod isolation)
- Pod Security Standards (RuntimeDefault seccomp, non-root, read-only FS)
- Admission webhooks (validation + mutation)
- Resource limits & requests (prevent noisy neighbor)

#### Observability Stack Deployment
- Prometheus Operator CRDs (ServiceMonitor, PrometheusRule)
- Loki deployment + Promtail DaemonSet
- Jaeger deployment + collector
- Grafana dashboards (security, operational)

#### Falco Runtime Detection (If Needed in Future)
- Falco DaemonSet with custom rules
- Syslog/webhook integration
- **Note:** Only if a future ADR approves resident sensors. Not in scope for current decision (Opção B).

#### Deployment Automation
- Helm charts or Kustomize overlays
- Multi-environment config (dev, staging, prod)
- Health checks, readiness probes
- Rolling updates, canary deployments

---

## Boundary Examples

### Example 1: Event from Runtime → Paas → Monitoring

```
delonix-runtime:
  Policy evaluation fails (e.g., denyPrivileged=true, user tries --privileged)
  ↓
  Emit to events.jsonl:
    {"ts": "2026-10-15T10:00:00Z", "kind": "security", "action": "ADM-PRIVILEGED", ...}

delonix-paas:
  Read events.jsonl (from all runtime nodes)
  ↓
  Enrich with tenant context:
    {"tenant": "acme-corp", "user": "alice@acme.com", "action": "ADM-PRIVILEGED", ...}
  ↓
  Ship to Loki + Prometheus:
    - Log to Loki (searchable, long-term storage)
    - Counter to Prometheus (rbac_denied_total{tenant="acme-corp"} += 1)

delonix-deploy:
  AlertRule in Prometheus:
    alert: HighRBACDenialRate
    expr: rate(rbac_denied_total[5m]) > 10
    notification: Slack, PagerDuty
```

### Example 2: Policy Compliance

```
delonix-runtime:
  Provides: delonix-security-runtime crate with policy fields and enforcement logic
  Does NOT: Claim compliance with CIS or NIST

delonix-paas:
  Consumes: delonix-security-runtime for tenant policy evaluation
  Audits: Kubernetes control plane against CIS Kubernetes Benchmark
  Measures: RBAC coverage, NetworkPolicy rules, etcd encryption
  Documents: "CIS-3.1.2 RBAC for kubelet — verified via kubectl describe clusterrolebinding"

delonix-deploy:
  Manifests: RBAC, NetworkPolicy, Pod Security Standards for both control plane and runtime
  Validation: Kyverno or admission webhooks ensure policies are deployed
```

### Example 3: What NOT to Do

```
❌ WRONG: Add /metrics endpoint to delonix-runtime
          (Violates daemonless principle, adds dependencies)

✅ RIGHT: Runtime emits events; paas scrapes and aggregates

---

❌ WRONG: Write CIS compliance docs in delonix-runtime/docs
          (Runtime is a component, not a full stack)

✅ RIGHT: delonix-paas/docs/ contains "CIS Benchmark Item X verified by <command>"
          Runtime only documents its own policy framework

---

❌ WRONG: Implement Falco rules in delonix-runtime
          (Violates daemonless, no resident sensors)

✅ RIGHT: If future ADR approves sensors, they go in delonix-deploy or separate operator
```

---

## Summary Table

| Aspect | delonix-runtime | delonix-paas | delonix-deploy |
|--------|-----------------|--------------|-----------------|
| **Design** | Daemonless, decision-only | Stateful control plane | GitOps layer |
| **Deployment** | Workload container | Control plane pod | Kustomize/Helm |
| **Multi-tenancy** | ❌ No | ✅ Yes | ✅ Manages |
| **Observability** | ✅ Events only | ✅ Full stack (Prom/Loki/Jaeger) | ✅ Deploys |
| **Security Admission** | ✅ Policy enforcement | ✅ Tenant wrapper | ✅ Validates |
| **Compliance Claims** | ❌ No | ✅ Measured | ✅ Deploys controls |
| **Runtime Sensors** | ❌ No (by design) | ⚠️ Future spike | ⚠️ Future deployment |

---

## Implications for Sprint 7+

### Sprint 7: Security Testing (delonix-runtime)
- Unit tests: RBAC matrix, policy enforcement, tenant isolation
- Integration tests: E2E threat scenarios
- CI gates: cargo audit, clippy, coverage

### Sprint 8: Observability Integration (delonix-paas)
- Consume runtime events + audit trail
- Aggregate into Prometheus, Loki, Jaeger
- Tenant-scoped dashboards and alerts
- Compliance audit automation

### Sprint 9: Deployment Hardening (delonix-deploy)
- RBAC, NetworkPolicy, PSS for all components
- Falco rules (if approved by ADR)
- Deployment validation (Kyverno, admission webhooks)

---

## References

- **ADR-0026:** Security runtime is decision crate, not sensor platform
- **ADR-0025:** Local MCP control surface (Accepted)
- **ADR-0010:** Remote management API (Rejected)
- **ADR-0003:** Tenancy-free capability model (Proposed)
- **docs/guia-politica-de-seguranca.md:** Policy framework guide
- **crates/delonix-core/src/policy_store.rs:** paas consumer of security-runtime

---

## Questions?

This document defines the architecture scope. If you have doubts about whether a feature belongs in runtime, paas, or deploy, refer to the **Boundary Examples** section or create an ADR.

**Last updated:** 2026-10-15 by Claude Code  
**Approved by:** (Pending Walter confirmation)
