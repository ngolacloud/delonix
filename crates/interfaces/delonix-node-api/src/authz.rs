//! Authorization by the client certificate's identity, and the per-call audit
//! record (ADR-0098, A3 of the fleet-operability brief).
//!
//! A1 (ADR-0096) authenticates the TCP transport — a client certificate
//! chaining to the configured CA is required at the handshake — and A2
//! (ADR-0097) gives that certificate an identity: a subject CN that names the
//! member. A3, here, acts on *who* called:
//!
//! - [`CallerIdentity`] is read once per connection from the already-verified
//!   client leaf (subject CN, and any URI SANs as alternative identities).
//! - [`Policy`] is a declarative JSON file (`--authz-policy`): absent, the
//!   transport keeps A1/A2's posture ([`Policy::AllowAll`]); present, it is
//!   default-deny, granting named identities a set of operations each optionally
//!   scoped to namespaces. [`Policy::decide`] is the pure decision.
//! - [`AuditSink`] writes one JSON line per mutating call (and per denial) to a
//!   file and always to the `tracing` stream, carrying the identity, operation,
//!   namespace and outcome.
//!
//! The enforcement layer that calls this on every request lives in
//! [`crate::serve_tls_listener`]; this module is pure logic + the sink, with no
//! knowledge of axum or hyper, so the decision can be unit-tested directly.

use std::io::Write as _;
use std::sync::{Arc, Mutex};

use serde::Deserialize;

/// Which field of the certificate the matched identity came from — recorded in
/// the audit so an operator can see whether a rule matched the CN or a SAN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentitySource {
    /// The subject common name (A2's `node-enroll --cn`).
    Cn,
    /// A URI subject-alternative-name (a SPIFFE-style identity).
    Uri,
    /// No identity could be read from the leaf (no CN, no URI SAN).
    None,
}

impl IdentitySource {
    fn as_str(self) -> &'static str {
        match self {
            IdentitySource::Cn => "cn",
            IdentitySource::Uri => "uri",
            IdentitySource::None => "none",
        }
    }
}

/// The caller, as read from its mutual-TLS client certificate. The subject CN is
/// the primary identity (A2 names the member in it); URI SANs are accepted as
/// alternatives so a SPIFFE identity can be named by a rule without changing
/// this model. DNS/IP SANs name a host, not a principal, so they are not read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallerIdentity {
    /// The subject common name, if the leaf carries one.
    pub cn: Option<String>,
    /// URI subject-alternative-names, in the order the leaf lists them.
    pub uris: Vec<String>,
}

impl CallerIdentity {
    /// Parse the identity from an already-verified client leaf (DER). The chain
    /// was verified to the CA at the handshake (A1) before this runs, so a
    /// hostile or malformed leaf never reaches here; a leaf that parses but has
    /// neither a CN nor a URI SAN yields an empty identity (every name-bearing
    /// rule then misses it, so it is authorized for nothing under a policy).
    pub fn from_leaf_der(der: &[u8]) -> CallerIdentity {
        use x509_parser::extensions::GeneralName;
        use x509_parser::prelude::FromDer;

        let Ok((_, cert)) = x509_parser::certificate::X509Certificate::from_der(der) else {
            return CallerIdentity::default();
        };
        let cn = cert
            .subject()
            .iter_common_name()
            .next()
            .and_then(|a| a.as_str().ok())
            .map(str::to_string)
            .filter(|s| !s.is_empty());
        let mut uris = Vec::new();
        if let Ok(Some(san)) = cert.subject_alternative_name() {
            for gn in &san.value.general_names {
                if let GeneralName::URI(u) = gn {
                    uris.push((*u).to_string());
                }
            }
        }
        CallerIdentity { cn, uris }
    }

    /// The names a rule may match, most-specific first: the CN, then each URI
    /// SAN. Each carries where it came from, for the audit.
    fn names(&self) -> Vec<(&str, IdentitySource)> {
        let mut out = Vec::new();
        if let Some(cn) = &self.cn {
            out.push((cn.as_str(), IdentitySource::Cn));
        }
        for u in &self.uris {
            out.push((u.as_str(), IdentitySource::Uri));
        }
        out
    }

    /// The name and source to record in the audit for this caller (the CN if
    /// present, else the first URI, else none) — independent of what matched, so
    /// a denied caller is still named.
    fn primary(&self) -> (Option<&str>, IdentitySource) {
        if let Some(cn) = &self.cn {
            (Some(cn.as_str()), IdentitySource::Cn)
        } else if let Some(u) = self.uris.first() {
            (Some(u.as_str()), IdentitySource::Uri)
        } else {
            (None, IdentitySource::None)
        }
    }
}

/// The transport an authorized request arrived on. gRPC carries the namespace in
/// the protobuf body (not decoded by the enforcement layer), so a namespace-
/// scoped rule is fail-closed over gRPC (ADR-0098 §4); REST carries the
/// namespace as a URL path segment, read without a body decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Grpc,
    Rest,
}

impl Transport {
    fn as_str(self) -> &'static str {
        match self {
            Transport::Grpc => "grpc",
            Transport::Rest => "rest",
        }
    }
}

/// The target of a call for namespace matching: a node-wide operation (images,
/// VMs, operations — no `{namespace}` in its REST template), or a namespaced
/// one. The node scope is written `-` in the audit and is granted only by a `*`
/// namespace rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Node,
    Namespace(String),
}

impl Scope {
    /// The namespace as it appears in the audit and the denial message: the
    /// namespace itself, or `-` for a node-wide operation.
    pub fn label(&self) -> &str {
        match self {
            Scope::Node => "-",
            Scope::Namespace(ns) => ns,
        }
    }
}

/// A glob over an identity, operation or namespace: `*` (any), a `prefix-*`
/// (starts-with), or an exact string. The only wildcard is a single trailing
/// `*`; it is deliberately not a full glob — an authorization list reads wrong
/// when its patterns are clever.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pattern {
    Any,
    Prefix(String),
    Exact(String),
}

impl Pattern {
    fn parse(s: &str) -> Pattern {
        if s == "*" {
            Pattern::Any
        } else if let Some(prefix) = s.strip_suffix('*') {
            Pattern::Prefix(prefix.to_string())
        } else {
            Pattern::Exact(s.to_string())
        }
    }
    fn matches(&self, value: &str) -> bool {
        match self {
            Pattern::Any => true,
            Pattern::Prefix(p) => value.starts_with(p.as_str()),
            Pattern::Exact(e) => e == value,
        }
    }
}

/// An operation pattern: `*` (any), a group `Service.*`, or an exact
/// `Service.Method`. Named in the contract's own `Service.Method` vocabulary, so
/// a rule reads against `proto/` and the OpenAPI without a translation table.
#[derive(Debug, Clone, PartialEq, Eq)]
enum OpPattern {
    Any,
    Service(String),
    Exact(String),
}

impl OpPattern {
    fn parse(s: &str) -> OpPattern {
        if s == "*" {
            OpPattern::Any
        } else if let Some(service) = s.strip_suffix(".*") {
            OpPattern::Service(service.to_string())
        } else {
            OpPattern::Exact(s.to_string())
        }
    }
    /// `operation` is a canonical `Service.Method`.
    fn matches(&self, operation: &str) -> bool {
        match self {
            OpPattern::Any => true,
            OpPattern::Service(svc) => operation
                .split_once('.')
                .is_some_and(|(service, _)| service == svc),
            OpPattern::Exact(e) => e == operation,
        }
    }
}

/// One grant: an identity, the operations it may perform, and the namespaces it
/// may perform them in. Opaque — built only by [`Policy::from_json`]; its fields
/// and the pattern types stay private.
#[derive(Debug, Clone)]
pub struct Rule {
    identity: Pattern,
    operations: Vec<OpPattern>,
    namespaces: Vec<Pattern>,
}

impl Rule {
    fn grants_operation(&self, operation: &str) -> bool {
        self.operations.iter().any(|p| p.matches(operation))
    }
    /// Whether this rule is namespace-scoped — i.e. it does NOT carry a `*`
    /// namespace. A namespace-scoped rule is fail-closed over gRPC (§4).
    fn namespace_scoped(&self) -> bool {
        !self.namespaces.iter().any(|p| matches!(p, Pattern::Any))
    }
    fn grants_namespace(&self, scope: &Scope) -> bool {
        match scope {
            // A node-wide operation is granted only by `*` (an explicit
            // namespace cannot name the node scope).
            Scope::Node => self.namespaces.iter().any(|p| matches!(p, Pattern::Any)),
            Scope::Namespace(ns) => self.namespaces.iter().any(|p| p.matches(ns)),
        }
    }
}

/// The effective policy. [`Policy::AllowAll`] is the A1/A2 posture kept when no
/// policy file is configured — an authenticated client is authorized for
/// everything. [`Policy::Rules`] is default-deny: a call is allowed only if a
/// rule grants it.
#[derive(Debug, Clone)]
pub enum Policy {
    AllowAll,
    Rules(Vec<Rule>),
}

/// The shape a policy file deserializes into (ADR-0098 §2).
#[derive(Debug, Deserialize)]
struct RawPolicy {
    /// Must be `"deny"` when a policy is present — the only safe default, and the
    /// guard against confusing "no policy" with "allow-all policy".
    default: String,
    #[serde(default)]
    rules: Vec<RawRule>,
}

#[derive(Debug, Deserialize)]
struct RawRule {
    identity: String,
    operations: Vec<String>,
    namespaces: Vec<String>,
}

impl Policy {
    /// Parse a policy from the JSON an operator wrote. `default` must be `deny`;
    /// a rule with empty operations or namespaces is refused (it would grant or
    /// deny nothing, which is a mistake, not a valid rule).
    pub fn from_json(src: &str) -> Result<Policy, String> {
        let raw: RawPolicy = serde_json::from_str(src)
            .map_err(|e| format!("authz policy is not valid JSON: {e}"))?;
        if raw.default != "deny" {
            return Err(format!(
                "authz policy `default` must be \"deny\" (got {:?}): the only safe default for an \
                 authorization list, and what tells \"no policy\" apart from an allow-all one",
                raw.default
            ));
        }
        let mut rules = Vec::with_capacity(raw.rules.len());
        for (i, r) in raw.rules.into_iter().enumerate() {
            if r.operations.is_empty() {
                return Err(format!(
                    "authz policy rule #{i} ({}) has no operations",
                    r.identity
                ));
            }
            if r.namespaces.is_empty() {
                return Err(format!(
                    "authz policy rule #{i} ({}) has no namespaces",
                    r.identity
                ));
            }
            rules.push(Rule {
                identity: Pattern::parse(&r.identity),
                operations: r.operations.iter().map(|s| OpPattern::parse(s)).collect(),
                namespaces: r.namespaces.iter().map(|s| Pattern::parse(s)).collect(),
            });
        }
        Ok(Policy::Rules(rules))
    }

    /// The pure decision: may `identity` perform `operation` (a canonical
    /// `Service.Method`) in `scope`, arriving over `transport`? A namespace-
    /// scoped rule does not apply over gRPC (§4): the body the layer would need
    /// to read the namespace from is not decoded, so such a grant is withheld,
    /// never guessed.
    pub fn decide(
        &self,
        identity: &CallerIdentity,
        operation: &str,
        scope: &Scope,
        transport: Transport,
    ) -> Decision {
        let Policy::Rules(rules) = self else {
            // No policy configured: A1/A2 posture, an authenticated client is
            // authorized for everything.
            return Decision::allow("no-policy");
        };
        // Scan every (name, rule) pair. Remember whether some rule matched the
        // identity+operation but was withheld only because it is namespace-
        // scoped over gRPC — that produces a more useful deny reason than a bare
        // "not authorized".
        let mut grpc_namespace_block = false;
        for (name, _src) in identity.names() {
            for rule in rules {
                if !rule.identity.matches(name) || !rule.grants_operation(operation) {
                    continue;
                }
                if rule.namespace_scoped() && transport == Transport::Grpc {
                    grpc_namespace_block = true;
                    continue;
                }
                if rule.grants_namespace(scope) {
                    return Decision::allow("rule");
                }
            }
        }
        if grpc_namespace_block {
            Decision::deny("namespace-scope-requires-rest")
        } else {
            Decision::deny("no-matching-rule")
        }
    }
}

/// The outcome of [`Policy::decide`]: allow or deny, with a short machine-stable
/// reason recorded in the audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub allow: bool,
    pub reason: &'static str,
}

impl Decision {
    fn allow(reason: &'static str) -> Decision {
        Decision {
            allow: true,
            reason,
        }
    }
    fn deny(reason: &'static str) -> Decision {
        Decision {
            allow: false,
            reason,
        }
    }
}

/// Whether an operation (`Service.Method`) changes state — the set that is
/// audited whether allowed or denied (ADR-0098 §5). The read half of the
/// contract is not audited when allowed; a *denied* read is audited by the
/// layer regardless of this. The classification spans both encodings (the REST
/// `MUTATIONS` set is a subset — it lists only the RPCs that read the two
/// mutation headers), so it is kept here, next to the authorization it serves.
pub fn is_mutating(operation: &str) -> bool {
    const MUTATIONS: &[&str] = &[
        "NetworkService.CreateNetwork",
        "NetworkService.DeleteNetwork",
        "NetworkService.ConnectContainer",
        "NetworkService.DisconnectContainer",
        "VolumeService.CreateVolume",
        "VolumeService.DeleteVolume",
        "VirtualMachineService.CreateVirtualMachine",
        "VirtualMachineService.DeleteVirtualMachine",
        "VirtualMachineService.StartVirtualMachine",
        "VirtualMachineService.StopVirtualMachine",
        "VirtualMachineService.PauseVirtualMachine",
        "VirtualMachineService.ResumeVirtualMachine",
        "VirtualMachineService.CreateSnapshot",
        "VirtualMachineService.RestoreSnapshot",
        "VirtualMachineService.DeleteSnapshot",
        "ContainerService.CreateContainer",
        "ContainerService.UpdateContainer",
        "ContainerService.DeleteContainer",
        "ContainerService.StartContainer",
        "ContainerService.StopContainer",
        "ContainerService.KillContainer",
        "PodService.CreatePod",
        "PodService.DeletePod",
        "StackService.ApplyStack",
        "StackService.DestroyStack",
        "ImageService.PullImage",
        "ImageService.DeleteImage",
        "ImageService.PushImage",
        "ImageService.BuildImage",
        "ImageService.ScanImage",
        "StorageService.CreateObjectStoreUser",
        "OperationService.CancelOperation",
        // The node scheduling write surface (ADR-0101, A6).
        "NodeService.SetNodeLabels",
        "NodeService.SetNodeTaints",
        "NodeService.SetSchedulability",
    ];
    MUTATIONS.contains(&operation)
}

/// One audit line (ADR-0098 §5). Serialized to a single JSON object per line.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AuditRecord {
    /// RFC 3339, UTC.
    pub ts: String,
    /// The caller's name (CN, else first URI), or null if the leaf bore none.
    pub identity: Option<String>,
    /// `cn` / `uri` / `none`.
    pub identity_source: &'static str,
    /// The canonical `Service.Method`.
    pub operation: String,
    /// The target namespace, or `-` for a node-wide operation.
    pub namespace: String,
    /// `grpc` / `rest`.
    pub transport: &'static str,
    /// `allow` / `deny`.
    pub decision: &'static str,
    /// The decision reason (`rule`, `no-matching-rule`, `namespace-scope-requires-rest`, `no-policy`).
    pub reason: &'static str,
    /// The client's address.
    pub remote: String,
}

impl AuditRecord {
    /// Assemble a record from a decided call. `ts` is stamped now (UTC, RFC 3339).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: &CallerIdentity,
        operation: &str,
        scope: &Scope,
        transport: Transport,
        decision: &Decision,
        remote: &str,
    ) -> AuditRecord {
        let (name, source) = identity.primary();
        AuditRecord {
            ts: now_rfc3339(),
            identity: name.map(str::to_string),
            identity_source: source.as_str(),
            operation: operation.to_string(),
            namespace: scope.label().to_string(),
            transport: transport.as_str(),
            decision: if decision.allow { "allow" } else { "deny" },
            reason: decision.reason,
            remote: remote.to_string(),
        }
    }
}

/// Current time as RFC 3339, UTC. A clock read that fails (a system before the
/// epoch) falls back to the epoch rather than panicking — an audit line with a
/// wrong timestamp is better than a dropped one.
fn now_rfc3339() -> String {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

/// Where audit lines go: always a `tracing` INFO event on a stable target, and —
/// when a path is configured — appended as one JSON line to a file. The file is
/// held behind a mutex so concurrent connection tasks write whole lines; opening
/// it in append mode means two processes (a cold-exit overlapping a restart,
/// ADR-0041) never truncate each other's log.
#[derive(Clone)]
pub struct AuditSink {
    file: Option<Arc<Mutex<std::fs::File>>>,
}

impl std::fmt::Debug for AuditSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditSink")
            .field("file", &self.file.as_ref().map(|_| "<open>"))
            .finish()
    }
}

impl AuditSink {
    /// A sink that only emits to the `tracing` stream (no file).
    pub fn tracing_only() -> AuditSink {
        AuditSink { file: None }
    }

    /// A sink that also appends to `path`, creating it if absent. An error
    /// opening the file is returned to the caller (the bin refuses to start with
    /// an audit path it cannot write — a silent drop would be the worst outcome
    /// for an audit log).
    pub fn to_file(path: &str) -> std::io::Result<AuditSink> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(AuditSink {
            file: Some(Arc::new(Mutex::new(file))),
        })
    }

    /// Record one line: the `tracing` event always, and the file when present.
    /// A file write that fails is logged (at `warn`) and does not fail the call —
    /// the request has already been decided; losing the file line must not turn
    /// an allowed call into an error, and the `tracing` event still carries it.
    pub fn record(&self, rec: &AuditRecord) {
        tracing::info!(
            target: "delonix_node_api::audit",
            ts = %rec.ts,
            identity = rec.identity.as_deref().unwrap_or("-"),
            identity_source = rec.identity_source,
            operation = %rec.operation,
            namespace = %rec.namespace,
            transport = rec.transport,
            decision = rec.decision,
            reason = rec.reason,
            remote = %rec.remote,
            "node-api call audited"
        );
        if let Some(file) = &self.file {
            match serde_json::to_string(rec) {
                Ok(mut line) => {
                    line.push('\n');
                    if let Ok(mut f) = file.lock() {
                        if let Err(e) = f.write_all(line.as_bytes()) {
                            tracing::warn!(error = %e, "audit log write failed");
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, "audit record did not serialize"),
            }
        }
    }
}

/// The authorization bundle the TLS serve path carries (ADR-0098): the policy in
/// effect and the audit sink. Given for a unix listener it is refused (symmetry
/// with TLS material, ADR-0096 §3) — there is no client certificate to read an
/// identity from on the local socket.
#[derive(Debug, Clone)]
pub struct Authz {
    pub policy: Policy,
    pub audit: AuditSink,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(json: &str) -> Policy {
        Policy::from_json(json).expect("valid policy")
    }

    fn cn(name: &str) -> CallerIdentity {
        CallerIdentity {
            cn: Some(name.to_string()),
            uris: Vec::new(),
        }
    }

    #[test]
    fn a_default_other_than_deny_is_refused() {
        let err = Policy::from_json(r#"{"default":"allow","rules":[]}"#).unwrap_err();
        assert!(err.contains("must be \"deny\""), "{err}");
    }

    #[test]
    fn an_empty_operations_or_namespaces_rule_is_refused() {
        assert!(Policy::from_json(
            r#"{"default":"deny","rules":[{"identity":"x","operations":[],"namespaces":["*"]}]}"#
        )
        .is_err());
        assert!(Policy::from_json(
            r#"{"default":"deny","rules":[{"identity":"x","operations":["*"],"namespaces":[]}]}"#
        )
        .is_err());
    }

    #[test]
    fn allow_all_authorizes_everything() {
        let p = Policy::AllowAll;
        let d = p.decide(
            &cn("whoever"),
            "VirtualMachineService.DeleteVirtualMachine",
            &Scope::Node,
            Transport::Grpc,
        );
        assert!(d.allow);
        assert_eq!(d.reason, "no-policy");
    }

    #[test]
    fn default_deny_refuses_an_unmentioned_identity() {
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"ctl","operations":["*"],"namespaces":["*"]}]}"#,
        );
        let d = p.decide(
            &cn("stranger"),
            "NodeService.GetNodeInfo",
            &Scope::Node,
            Transport::Rest,
        );
        assert!(!d.allow);
        assert_eq!(d.reason, "no-matching-rule");
    }

    #[test]
    fn a_star_rule_grants_every_operation_and_scope_on_both_transports() {
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"ctl","operations":["*"],"namespaces":["*"]}]}"#,
        );
        assert!(
            p.decide(
                &cn("ctl"),
                "ContainerService.CreateContainer",
                &Scope::Namespace("team-a".into()),
                Transport::Grpc
            )
            .allow
        );
        assert!(
            p.decide(
                &cn("ctl"),
                "VolumeService.DeleteVolume",
                &Scope::Node,
                Transport::Rest
            )
            .allow
        );
    }

    #[test]
    fn an_operation_group_grants_only_its_service() {
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"net","operations":["NetworkService.*"],"namespaces":["*"]}]}"#,
        );
        assert!(
            p.decide(
                &cn("net"),
                "NetworkService.CreateNetwork",
                &Scope::Node,
                Transport::Rest
            )
            .allow
        );
        assert!(
            !p.decide(
                &cn("net"),
                "VolumeService.DeleteVolume",
                &Scope::Node,
                Transport::Rest
            )
            .allow
        );
    }

    #[test]
    fn a_namespace_scoped_rule_is_enforced_on_rest() {
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"sch","operations":["ContainerService.*"],"namespaces":["team-a"]}]}"#,
        );
        // In its namespace: allowed.
        assert!(
            p.decide(
                &cn("sch"),
                "ContainerService.CreateContainer",
                &Scope::Namespace("team-a".into()),
                Transport::Rest
            )
            .allow
        );
        // In another namespace: denied.
        let d = p.decide(
            &cn("sch"),
            "ContainerService.CreateContainer",
            &Scope::Namespace("team-b".into()),
            Transport::Rest,
        );
        assert!(!d.allow);
        assert_eq!(d.reason, "no-matching-rule");
    }

    #[test]
    fn a_namespace_scoped_rule_is_failclosed_over_grpc() {
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"sch","operations":["ContainerService.*"],"namespaces":["team-a"]}]}"#,
        );
        let d = p.decide(
            &cn("sch"),
            "ContainerService.CreateContainer",
            &Scope::Namespace("team-a".into()),
            Transport::Grpc,
        );
        assert!(
            !d.allow,
            "a namespace-scoped grant must not apply over gRPC"
        );
        assert_eq!(d.reason, "namespace-scope-requires-rest");
    }

    #[test]
    fn a_star_namespace_still_grants_over_grpc() {
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"sch","operations":["ContainerService.*"],"namespaces":["*"]}]}"#,
        );
        assert!(
            p.decide(
                &cn("sch"),
                "ContainerService.CreateContainer",
                &Scope::Namespace("team-a".into()),
                Transport::Grpc
            )
            .allow
        );
    }

    #[test]
    fn a_prefix_identity_glob_matches() {
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"scheduler-*","operations":["*"],"namespaces":["*"]}]}"#,
        );
        assert!(
            p.decide(
                &cn("scheduler-7"),
                "NodeService.GetNodeInfo",
                &Scope::Node,
                Transport::Rest
            )
            .allow
        );
        assert!(
            !p.decide(
                &cn("backup-1"),
                "NodeService.GetNodeInfo",
                &Scope::Node,
                Transport::Rest
            )
            .allow
        );
    }

    #[test]
    fn a_uri_san_can_be_the_matched_identity() {
        let id = CallerIdentity {
            cn: None,
            uris: vec!["spiffe://delonix/node/7".into()],
        };
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"spiffe://delonix/node/*","operations":["*"],"namespaces":["*"]}]}"#,
        );
        assert!(
            p.decide(
                &id,
                "NodeService.GetNodeInfo",
                &Scope::Node,
                Transport::Rest
            )
            .allow
        );
    }

    #[test]
    fn mutations_are_classified_across_both_encodings() {
        assert!(is_mutating("ContainerService.CreateContainer"));
        assert!(is_mutating("VirtualMachineService.StartVirtualMachine"));
        assert!(is_mutating("StackService.ApplyStack"));
        assert!(!is_mutating("NodeService.GetNodeInfo"));
        assert!(!is_mutating("ContainerService.ListContainers"));
        assert!(!is_mutating("StackService.PlanStack"));
    }

    #[test]
    fn an_audit_record_is_one_valid_json_object_with_the_expected_fields() {
        let id = cn("tenant-a-scheduler");
        let d = Decision::allow("rule");
        let rec = AuditRecord::new(
            &id,
            "ContainerService.CreateContainer",
            &Scope::Namespace("team-a".into()),
            Transport::Rest,
            &d,
            "192.168.122.41:54122",
        );
        let line = serde_json::to_string(&rec).unwrap();
        assert!(!line.contains('\n'));
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["identity"], "tenant-a-scheduler");
        assert_eq!(v["identity_source"], "cn");
        assert_eq!(v["operation"], "ContainerService.CreateContainer");
        assert_eq!(v["namespace"], "team-a");
        assert_eq!(v["transport"], "rest");
        assert_eq!(v["decision"], "allow");
        assert_eq!(v["reason"], "rule");
        assert!(v["ts"].as_str().unwrap().contains('T'));
    }

    #[test]
    fn a_node_scope_is_granted_only_by_a_star_namespace() {
        // A namespace-scoped rule does not grant a node-wide operation.
        let p = policy(
            r#"{"default":"deny","rules":[{"identity":"x","operations":["*"],"namespaces":["team-a"]}]}"#,
        );
        assert!(
            !p.decide(
                &cn("x"),
                "ImageService.PullImage",
                &Scope::Node,
                Transport::Rest
            )
            .allow
        );
    }

    #[test]
    fn from_leaf_der_reads_the_subject_cn_and_uri_san() {
        use rcgen::{CertificateParams, DnType, KeyPair, SanType};
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.distinguished_name.push(DnType::CommonName, "node-7");
        params.subject_alt_names.push(SanType::URI(
            rcgen::Ia5String::try_from("spiffe://delonix/node/7").unwrap(),
        ));
        let key = KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let id = CallerIdentity::from_leaf_der(cert.der().as_ref());
        assert_eq!(id.cn.as_deref(), Some("node-7"));
        assert_eq!(id.uris, vec!["spiffe://delonix/node/7".to_string()]);
        // A DER that is not a certificate yields an empty identity, never a panic.
        assert_eq!(
            CallerIdentity::from_leaf_der(b"not a cert"),
            CallerIdentity::default()
        );
    }
}
