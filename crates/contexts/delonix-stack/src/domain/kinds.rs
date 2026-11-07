//! What each Kind IS — one table, instead of six lists that have to agree.
//!
//! Every column here already existed, each as a separate constant living beside
//! the code that consumed it: `KINDS` (the apply order), `CONVERGING_KINDS`,
//! `TEARDOWN_KINDS`, `kind_honors_namespace`, the `DECLARATIVOS` of the wait
//! test, and the arms of `presence()`. Nothing tied them together, and they
//! drifted — with measured symptoms, all of the same shape: a Kind answered one
//! list and not the others, and the gap was invisible because the wrong answer
//! still looked like a working command.
//!
//! - `Vm`/`FirewallPolicy`/`ShareVolume` got a reconciler adapter and stayed OUT
//!   of `CONVERGING_KINDS`, so `converge_and_stamp` skipped them — hidden
//!   because the old per-Kind apply is idempotent and converged by the wrong path.
//! - `NetworkRoute` entered `KINDS` and was applied for versions with no arm in
//!   `presence()`, so `ls`/`describe` called a resource the apply creates an
//!   «unsupported kind».
//! - The declarative Kinds never say `"yes"`, and `stack wait` read that as
//!   «absent», burning the whole `--timeout` on a stack that was entirely up.
//!
//! So the lists are now DERIVED from this table and the table is the only place
//! a Kind is described. Adding one without answering every column does not
//! compile; answering a column wrong is what the tests at the bottom are for.
//!
//! What is deliberately NOT here: `Desired.ownable`. It is decided inside each
//! Kind's `desired()`, which needs a document to run, so a copy in this table
//! would be a seventh list with nothing forcing it to agree with the real
//! value — the exact arrangement this module exists to remove.

/// The name of each Kind, in the one place a rename has to touch.
///
/// # Why these exist
///
/// Measured while renaming `Tunnel`→`Gateway`: the name of a Kind was a bare
/// string literal repeated **106 times across ten files**, plus a second
/// hand-kept list of them in `schema.rs`. Renaming one meant a careful sweep
/// with nothing to catch a site that was missed — and a missed site does not
/// fail loudly, it makes one code path stop recognising a Kind the rest of the
/// engine still serves.
///
/// That is the same defect this module already removed for the FACTS about a
/// Kind — six lists that had to agree and drifted — left standing for the NAME.
///
/// # Safe as `match` patterns
///
/// A `&'static str` const is a legal pattern, and a mistyped one degrades to a
/// catch-all BINDING rather than an error. That footgun is closed here by the
/// build itself: the binding makes every later arm unreachable, and this repo
/// runs with `-D warnings`. Verified with a throwaway program before adopting
/// the idiom, not assumed.
///
/// # What a rename now costs
///
/// One line here, one alias arm in `manifest::canonical_kind`, and one row in
/// the test that keeps old spellings loading. Not a sweep.
pub const SECRET: &str = "Secret";
/// The node's own admission ceiling (`<root>/policy.json`), given a manifest
/// form (M04 of `docs/roadmap/13-improvements-traceability.md`). Converges
/// like any other Kind — `stack apply` can raise or tighten it — but its
/// teardown is refused (see [`FACTS`]'s row and `no_teardown_reason` in
/// `cmd/stack.rs`): the ONLY way it comes down is `delonix policy unset`,
/// never a `stack destroy`/`--prune` that simply stopped declaring it.
pub const RUNTIME_POLICY: &str = "RuntimePolicy";
pub const NETWORK: &str = "Network";
pub const NETWORK_ROUTE: &str = "NetworkRoute";
pub const NETWORK_ZONE: &str = "NetworkZone";
/// The use of a storage pool the administrator declared (ADR-0067). A node
/// resource: it names a pool of the allowlist, never a device or a path.
pub const STORAGE_POOL: &str = "StoragePool";
pub const VOLUME: &str = "Volume";
/// An S3-compatible object store (ADR-0081): either provisioned on the node's
/// Ceph Object Gateway, or a reference to a foreign S3 store.
///
/// Renamed to **`Bucket`** by the catalog v1 (ADR-0086 D3; the remeasurement's
/// `ObjectStorage → Bucket` conversion). The identifier stays `OBJECT_STORAGE`
/// on purpose: a rename here is "one line + one alias arm + one test row, not a
/// sweep" (see the module header), so the dozens of `k::OBJECT_STORAGE` call
/// sites and the `objectstorage` module keep their handle while the *spelling a
/// user writes* becomes `Bucket`. `ObjectStorage` keeps loading as a silent
/// alias (`KIND_ALIASES`).
pub const OBJECT_STORAGE: &str = "Bucket";
pub const IMAGE: &str = "Image";
pub const APP: &str = "App";
pub const VM: &str = "VirtualMachine";
/// A system container on a remote provider: a whole userland with VM-like
/// semantics, not a `Container` (ADR-0058).
pub const SYSTEM_CONTAINER: &str = "SystemContainer";
pub const CONTAINER: &str = "Container";
pub const POD: &str = "Pod";
/// The Kind written for a container service (ADR-0086 D4). Lowers at load into
/// its `replicas` `Pod` INSTANCES — the Pod is the instance it creates, not a
/// thing written by hand — so it has no apply of its own (see `cmd::application`).
pub const APPLICATION: &str = "Application";
/// A run-to-completion workload (ADR-0086 D3). Lowers to one `Pod` whose
/// `restartPolicy` defaults to `OnFailure` — retried until it succeeds (see
/// `cmd::job`).
pub const JOB: &str = "Job";
pub const INGRESS: &str = "Ingress";
pub const FIREWALL_POLICY: &str = "NetworkPolicy";
pub const NETWORK_ACCESS_RULE: &str = "NetworkAccessRule";
/// The networking edge a `GatewayProvider` realizes (ADR-0051/0059).
///
/// Renamed to **`Router`** by the catalog v1 (ADR-0086 D3; the remeasurement's
/// `NetworkGateway → Router` conversion — `Router` is the catalog's name for
/// the connectivity edge, leaving `Gateway` for the Gateway-API entry point of
/// D4). As with `OBJECT_STORAGE`, the identifier stays `NETWORK_GATEWAY` so the
/// `network_gateway` module and every `k::NETWORK_GATEWAY` call site keep their
/// handle; only the spelling becomes `Router`, and `NetworkGateway` keeps
/// loading as a silent alias (`KIND_ALIASES`).
pub const NETWORK_GATEWAY: &str = "Router";
/// A single address claimed from an `IPPool` (ADR-0086 D3/D4c, ADR-0090), now
/// LIVE: a declarative claim of ONE address, recorded in the IPAM ledger under
/// `IPAddress/<name>` so nothing else leases it. Node-scoped, like the `IPPool`
/// it claims from.
pub const IP_ADDRESS: &str = "IPAddress";
/// A point-in-time copy of a `Volume` or `VirtualMachine` (ADR-0086 D3, D5d).
/// **Reserved**: declared at `v1beta1` with no apply of its own yet — snapshots
/// today are imperative (`vm snapshot`); the declarative Kind, its retention and
/// its `Bucket` destination land in a later phase under ADR-0090.
pub const SNAPSHOT: &str = "Snapshot";
/// Non-secret configuration data a workload consumes — the ConfigMap-shaped
/// sibling of `Secret` (ADR-0086 D3). **Reserved**: declared at `v1beta1` with
/// no apply of its own yet. Today non-secret data rides on `Secret`; the
/// cleartext store and its mount/env projection land in a later phase under
/// ADR-0090 (it is NOT lowered into `Secret` — that would encrypt data the
/// writer chose to keep in the clear).
pub const CONFIG: &str = "Config";
/// This node, as a resource one can read back (ADR-0086 D3, D7). **Reserved**:
/// declared at `v1beta1` with no apply of its own yet — the capacity is already
/// served by `NodeService.GetCapacity`, but what a declarative `kind: Node`
/// writes (labels, taints, schedulability) is deliberately left for the phase
/// that builds the uniform read-back surface (ResourceService, ADR-0087/0090).
/// Node-scoped, like the other node resources of D7.
pub const NODE: &str = "Node";
pub const HTTP_ROUTE: &str = "HTTPRoute";
pub const GATEWAY: &str = "Gateway";
pub const SERVICE: &str = "Service";
pub const IPPOOL: &str = "IPPool";
pub const WORKLOAD: &str = "Workload";
pub const DEPENDENCY: &str = "Dependency";
pub const SHARE_VOLUME: &str = "ShareVolume";
pub const STORAGE: &str = "Storage";
pub const STACK: &str = "Stack";
/// A pull source a node reconciles from on a timer (ADR-0021): a git
/// repository, a ref and a path, applied by a short-lived pass — never a
/// resident process.
pub const GITOPS_SOURCE: &str = "GitOpsSource";
/// The three Kinds a `Workload` becomes, in the slash spelling `lowers_to`
/// splits on. A `const` and not a literal for the same reason as the names it
/// joins: it stopped being true the moment `Vm` was renamed, and only a test
/// noticed.
pub const WORKLOAD_LOWERS_TO: &str = "Container/Pod/VirtualMachine";

pub const CLUSTER: &str = "KubernetesCluster";

/// The area a Kind acts on. Printed as a column, so the names are short and the
/// three network ones are split: they answer different questions and a single
/// `network` label would hide that `NetworkRoute` opens a PATH while
/// `FirewallPolicy` decides whether traffic is ALLOWED along it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// Runs code: containers, pods, VMs.
    Compute,
    /// Holds bytes: volumes and shares.
    Storage,
    /// Whether a path exists between two addresses.
    NetConnectivity,
    /// Whether traffic along an existing path is permitted.
    NetPolicy,
    /// How something outside the node gets in.
    NetExposure,
    /// Content consumed by a workload: images, secrets.
    Artifact,
    /// Composes or drives other Kinds; not a resource of its own.
    Composition,
    /// The node's own ceiling on what may run, checked at admission — before
    /// any of the domains above get a chance to act. Distinct from
    /// `NetPolicy` (which governs traffic already flowing) and from
    /// `Artifact` (which governs what is pulled): this one governs whether a
    /// `container run`/`vm create` is allowed to happen at all.
    Security,
}

impl Domain {
    /// Deliberately NOT translated: these are identifiers a script can grep for,
    /// like the Kind names beside them in the same table. The words that ARE
    /// prose (`primary`/`declarative`/…) go through `po::t` at the point of
    /// printing.
    pub fn label(self) -> &'static str {
        match self {
            Domain::Compute => "compute",
            Domain::Storage => "storage",
            Domain::NetConnectivity => "net-conn",
            Domain::NetPolicy => "net-policy",
            Domain::NetExposure => "net-exposure",
            Domain::Artifact => "artifact",
            Domain::Composition => "composition",
            Domain::Security => "security",
        }
    }
}

/// What a document of this Kind becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// Has its own apply and survives the `load`.
    Primary,
    /// Rewritten into another Kind at load time as a convenience, not because it
    /// is going away.
    Sugar(&'static str),
    /// Expands into the documents it contains.
    Aggregate,
    /// A foreign schema this engine accepts verbatim, compiled onto another
    /// Kind's mechanism. It DOES survive the load — that is what separates it
    /// from [`Form::Sugar`], which is rewritten into its target and disappears.
    Compat(&'static str),
    /// Still primary — own apply, survives the load — but a successor is
    /// announced and this spelling is on its way out.
    ///
    /// The engine once had a `Deprecated` variant beside this one — REWRITTEN
    /// at load, so the writer got the successor's behaviour for free. It was
    /// removed with the last three Kinds that used it, because a variant with
    /// no constructor is dead weight and `-D warnings` says so. Bring it back
    /// when a Kind needs rewriting rather than announcing: the distinction is
    /// real, and the reason a `Sunset` Kind is NOT rewritten is that rewriting
    /// it would change what the engine DOES.
    ///
    /// `Container` is the case that forced this variant. Lowering it to a
    /// one-container `Pod` looks like a rename and is not: a Pod always builds
    /// a shared netns and its members join it through the `--pod` re-exec, so
    /// every declarative container would silently change its runtime shape —
    /// an extra netns holder each, and a different network path. The name half
    /// was solvable (`pod.rs` honours a member's own name), the netns half is
    /// not. So it is announced, not rewritten, and a future major removes it
    /// once manifests have moved.
    Sunset(&'static str),
    /// Declared in the catalog but with no apply of its own yet. The name,
    /// group and version are reserved at `v1beta1` — a group born beta is
    /// *expected* to hold Kinds that are not live yet (ADR-0086 D2: `v1`
    /// requires every Kind of the group proven live over two releases, which is
    /// precisely what a `Reserved` Kind is not). It survives nothing and lowers
    /// to nothing: it is out of the stack cycle, never applied, never observable,
    /// and its typed schema is deliberately absent (an `untyped_hint` says so).
    ///
    /// Distinct from `Sunset` (which HAS an apply and is on its way out) and from
    /// `Sugar` (which is rewritten into a target that does the work): a
    /// `Reserved` Kind has no target and no work — the work is a later campaign
    /// phase. `IPAddress` and `Snapshot` are the first two (ADR-0086 D3, built in
    /// phases 7–11 under ADR-0090); promoting their group to `v1` is refused by
    /// the gate until they are live.
    Reserved,
}

impl Form {
    /// The Kind this one hands over to, whether by lowering or by announcement.
    /// Used by the gate that keeps a target from naming a Kind that never
    /// existed.
    pub fn successor(self) -> Option<&'static str> {
        match self {
            Form::Sunset(k) => Some(k),
            other => other.lowers_to(),
        }
    }

    /// The Kind a document of this one ends up as, if it is not itself.
    pub fn lowers_to(self) -> Option<&'static str> {
        match self {
            Form::Sugar(k) | Form::Compat(k) => Some(k),
            // `Sunset` is deliberately not here: it does not lower, it is
            // merely announced. Its successor is checked by the same test,
            // through `successor`. `Reserved` lowers to nothing — it has no
            // target at all, which is what separates it from `Sunset`.
            Form::Primary | Form::Aggregate | Form::Sunset(_) | Form::Reserved => None,
        }
    }
}

/// How `stack ls`/`wait` can tell whether the resource is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// A store of its own answers yes/no.
    Registry,
    /// Derived from something else — a Pod is its labelled members.
    Derived,
    /// Nothing to read back: the resource is a directive applied to a target,
    /// not state. `presence()` answers `-`, and **that is not «absent»** — the
    /// distinction `is_pending` exists to make.
    Declarative,
    /// Never reaches `presence()`: it does not survive the load, or is not a
    /// local resource at all.
    NotObservable,
}

/// Whether `metadata.namespace` means anything on a Kind.
///
/// Three states and not a bool, because `Volume` genuinely has three answers:
/// none for a plain volume, real for one with a `share:` block. Modelling that
/// as `true` would make the load stop warning about a namespace that does
/// nothing on every ordinary volume; as `false`, it would warn «namespace has no
/// effect» on a share, whose namespace decides which directory its data lives
/// in — a warning that is not merely useless but wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Namespaced {
    /// Never — the resource is global to the node.
    Never,
    /// Always.
    Always,
    /// Depends on the document. `honors_namespace` answers `true` so nothing is
    /// warned away wholesale, and the Kind's own apply decides what to do with it.
    PerDocument,
}

/// Everything the rest of the code asks about a Kind.
#[derive(Debug, Clone, Copy)]
pub struct KindFacts {
    pub kind: &'static str,
    /// The lowercase plural a caller types (`delonix get pods`). Its own field
    /// and not derived by appending `s`: `Dependency`→`dependencies` and
    /// `Ingress`→`ingresses` do not follow that rule, and a resolver that
    /// guessed would answer «no such resource» for a Kind that is right there.
    pub plural: &'static str,
    /// Accepted abbreviations. Deliberately sparse — a shortname exists when it
    /// is unambiguous AND worth typing; inventing one per Kind only multiplies
    /// the ways two Kinds can collide. Uniqueness across the whole registry is
    /// a TEST, not a convention: a duplicate would silently shadow, which is
    /// the defect class this module exists to have removed.
    pub short: &'static [&'static str],
    /// The `apiVersion` a manifest writes for this Kind.
    ///
    /// A column and not a constant, even though every row says the same thing
    /// today. The CLI restructuring splits it per domain
    /// (`compute.delonix.io/…`, `networking.delonix.io/…`), and a Kind whose
    /// version lives in a shared `const` cannot be moved one at a time — which
    /// is the only way that migration can be reviewed. `delonix.io/v1` also has
    /// to keep LOADING afterwards (ADR-0020, and the promise in
    /// `docs/cli-stability.md`), so the old spelling stays reachable rather
    /// than being renamed away.
    pub api_version: &'static str,
    pub domain: Domain,
    pub form: Form,
    /// Applied by `stack apply`. **The order of the rows below is the order of
    /// the apply**, and `destroy` derives its own by reversing it.
    pub in_stack: bool,
    /// The key of the `kind: Stack` `spec` that groups this Kind, or `""` when
    /// the Kind cannot be grouped (and then [`stack_group_absent_reason`] says
    /// why — a Kind that is silently absent reads as «nobody got round to it»).
    ///
    /// **This column governs the expansion.** `expand_stack`, the schema, the
    /// unknown-field warning and the generated docs all read it, and there is no
    /// second list of groups anywhere. There used to be one: it was written by
    /// hand, and `NetworkRoute`, `NetworkAccessRule`, `Service` and `App` were
    /// applied by `stack apply` and could not be put inside a Stack.
    ///
    /// It is NOT the same question as `in_stack`: a `Dependency` is lowered into
    /// a `NetworkPolicy` and a `Workload` into a `Container`/`Pod`/`VirtualMachine`,
    /// so neither is applied as itself — and both are things a person writes
    /// inside a Stack.
    pub stack_group: &'static str,
    /// A changed field is really applied, rather than «ensure present».
    pub converges: bool,
    /// `destroy_one` removes it, so `--prune` and `destroy` can promise it.
    pub teardown: bool,
    /// Whether `metadata.namespace` does something on this Kind.
    pub namespaced: Namespaced,
    pub presence: Presence,
}

/// The table. Rows are in APPLY ORDER for everything with `in_stack: true` —
/// see [`KindFacts::in_stack`] — and the rest follow.
const FACTS: &[KindFacts] = &[
    KindFacts {
        // First in apply order, deliberately: `run_layers` (cmd/stack.rs) applies
        // it before Container/VM/Pod, so a stricter ceiling declared in the SAME
        // manifest already governs the workloads the rest of that same apply is
        // about to create.
        kind: RUNTIME_POLICY,
        plural: "runtimepolicies",
        short: &["policy"],
        api_version: "policy.delonix.io/v1beta1",
        domain: Domain::Security,
        form: Form::Primary,
        in_stack: true,
        stack_group: "runtimePolicies",
        converges: true,
        // `no_teardown_reason` in `cmd/stack.rs` names why: lowering the node's
        // ceiling must never be a side effect of a manifest simply no longer
        // mentioning it.
        teardown: false,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: SECRET,
        plural: "secrets",
        short: &["sec"],
        api_version: "core.delonix.io/v1beta1",
        domain: Domain::Artifact,
        form: Form::Primary,
        in_stack: true,
        stack_group: "secrets",
        // Converges per DOCUMENT (`secret::desired`): one that carries its
        // values (`stringData`) is compared by a keyed fingerprint, never by
        // decrypting for display; one that reads `fromEnv`/`fromEnvFile` stays
        // ensure-present, and `not_converged_reason` says why.
        converges: true,
        teardown: false,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        // ADR-0086 D3 / ADR-0090: the ConfigMap-shaped sibling of `Secret`, now
        // LIVE. Same lifecycle as `Secret` — its own apply, converges per
        // document, read back by `get configs`/`describe config`, removed by
        // `delete config` — but the data is stored in CLEARTEXT (never through
        // the cred vault). That cleartext is the whole point against a Secret.
        // Applied right after `Secret` (both are consumed by the workloads
        // below) and, like `Secret`, deliberately not ownable/stamped, so no
        // stack prunes it (ADR-0090: shared content carries no owner).
        kind: CONFIG,
        plural: "configs",
        short: &["cfg"],
        api_version: "core.delonix.io/v1beta1",
        // Consumed by a workload, like `Secret` and `Image` beside it.
        domain: Domain::Artifact,
        form: Form::Primary,
        in_stack: true,
        stack_group: "configs",
        // Converges per DOCUMENT (`config_kind::desired`), exactly as `Secret`:
        // one that carries its values (`data`) is compared by a content
        // fingerprint; one that reads `fromEnv`/`fromEnvFile` stays
        // ensure-present, and `not_converged_reason` says why.
        converges: true,
        teardown: false,
        // Not namespaced, matching `Secret` beside it (both are node-global
        // key/value stores keyed by name).
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: NETWORK,
        plural: "networks",
        short: &["net"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetConnectivity,
        form: Form::Primary,
        in_stack: true,
        stack_group: "networks",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: NETWORK_ROUTE,
        plural: "networkroutes",
        short: &["nr"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetConnectivity,
        form: Form::Primary,
        in_stack: true,
        stack_group: "networkRoutes",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: NETWORK_ZONE,
        plural: "networkzones",
        short: &["nz"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetConnectivity,
        // Superseded by `Network` + `spec.provider.proxmox` (ADR-0070): the
        // networks naming a zone are folded into this Kind's document at load,
        // so it stays the executor and stops being the way to write it.
        form: Form::Sunset(NETWORK),
        in_stack: true,
        stack_group: "networkZones",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Never,
        // Its own registry (ADR-0049 addendum, mirrors NETWORK_GATEWAY): the
        // Proxmox cluster this realizes against carries no `delonix.io/stack`
        // label of its own to stamp — the zone/vnets are PENDING config in
        // `/cluster/sdn/*`, not a local resource.
        presence: Presence::Registry,
    },
    KindFacts {
        // ADR-0067. BEFORE `Volume`: a volume with `spec.pool` is allocated in
        // a pool that has to be in use by then, and the teardown — the reverse
        // of this order — removes the volumes before it stops using the pool.
        kind: STORAGE_POOL,
        plural: "storagepools",
        short: &["sp"],
        api_version: "storage.delonix.io/v1beta1",
        domain: Domain::Storage,
        form: Form::Primary,
        in_stack: true,
        stack_group: "storagePools",
        converges: true,
        teardown: true,
        // A pool is a node resource. Who divides it among anyone is whoever
        // consumes the engine, with volumes and sizes.
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: VOLUME,
        plural: "volumes",
        short: &["vol"],
        api_version: "storage.delonix.io/v1beta1",
        domain: Domain::Storage,
        form: Form::Primary,
        in_stack: true,
        stack_group: "volumes",
        converges: true,
        teardown: true,
        // A plain volume is global and a `share:` one is scoped by namespace,
        // so this is the one Kind whose answer comes from the DOCUMENT.
        namespaced: Namespaced::PerDocument,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: OBJECT_STORAGE,
        plural: "buckets",
        short: &["bkt"],
        api_version: "storage.delonix.io/v1beta1",
        domain: Domain::Storage,
        form: Form::Primary,
        in_stack: true,
        stack_group: "objectStorages",
        converges: true,
        // An object store holds a tenant's S3 data; removing it because a
        // manifest stopped naming it would orphan buckets and objects. Removal
        // is explicit — `no_teardown_reason` (cmd/stack.rs) names how. Same
        // reasoning as Image/App just below.
        teardown: false,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: IMAGE,
        plural: "images",
        short: &["img"],
        api_version: "core.delonix.io/v1beta1",
        domain: Domain::Artifact,
        form: Form::Primary,
        in_stack: true,
        stack_group: "images",
        converges: true,
        // Shared content-addressed cache: removing it because one stack stopped
        // declaring it would pull it from under the others.
        teardown: false,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: APP,
        plural: "apps",
        short: &[],
        api_version: "core.delonix.io/v1beta1",
        domain: Domain::Artifact,
        form: Form::Primary,
        in_stack: true,
        stack_group: "apps",
        converges: true,
        // An App's output is an image — shared content-addressed cache, owned
        // by no stack. Same reasoning, same wording, as Image's row above.
        teardown: false,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: VM,
        plural: "virtualmachines",
        short: &["vm"],
        api_version: "compute.delonix.io/v1beta1",
        domain: Domain::Compute,
        form: Form::Primary,
        in_stack: true,
        stack_group: "vms",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Always,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: SYSTEM_CONTAINER,
        plural: "systemcontainers",
        short: &["sc"],
        // A provider-specific Kind (ADR-0086 D6): it lives in the `proxmox`
        // provider group with its own version, NOT a domain group — which keeps
        // the six domain groups free of provider specifics (ADR-0070/0071). The
        // first provider-group Kind; `v1alpha1` because the provider track moves
        // on its own cadence, independent of the domain groups' `v1beta1`.
        api_version: "proxmox.delonix.io/v1alpha1",
        domain: Domain::Compute,
        form: Form::Primary,
        in_stack: true,
        stack_group: "systemContainers",
        converges: true,
        teardown: true,
        // The provider's node has no namespace of this engine's to put it in:
        // the engine's isolation does not reach it (ADR-0058 T7).
        namespaced: Namespaced::Never,
        // Its own registry holds the provider's locator; what the container
        // IS is read back from the provider.
        presence: Presence::Registry,
    },
    KindFacts {
        kind: CONTAINER,
        plural: "containers",
        short: &[],
        api_version: "compute.delonix.io/v1beta1",
        domain: Domain::Compute,
        // ADR-0086 D4: the sunset successor is `Application` (not `Pod`) — the
        // service Kind a single container becomes with `replicas: 1`. Announced,
        // not rewritten (`Sunset`), so a `kind: Container` still applies as a
        // Container; the announcement points at `Application`.
        form: Form::Sunset(APPLICATION),
        in_stack: true,
        stack_group: "containers",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Always,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: POD,
        plural: "pods",
        short: &["po"],
        api_version: "compute.delonix.io/v1beta1",
        domain: Domain::Compute,
        form: Form::Primary,
        in_stack: true,
        stack_group: "pods",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Always,
        presence: Presence::Derived,
    },
    KindFacts {
        // ADR-0032: a selector-matched set, DNS round-robin, no VIP. Placed
        // right after the compute Kinds it selects — a Service's own registry
        // entry is independent of them, but the match count printed on
        // `apply` is only meaningful once the workloads it names already
        // exist.
        kind: SERVICE,
        plural: "services",
        short: &["svc"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetConnectivity,
        form: Form::Primary,
        in_stack: true,
        stack_group: "services",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Always,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: INGRESS,
        plural: "ingresses",
        short: &["ing"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetExposure,
        form: Form::Compat(HTTP_ROUTE),
        in_stack: true,
        stack_group: "ingress",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Never,
        presence: Presence::Declarative,
    },
    KindFacts {
        kind: FIREWALL_POLICY,
        plural: "networkpolicies",
        short: &["np"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetPolicy,
        form: Form::Primary,
        in_stack: true,
        stack_group: "firewallPolicies",
        converges: true,
        teardown: false,
        namespaced: Namespaced::Never,
        presence: Presence::Declarative,
    },
    KindFacts {
        kind: NETWORK_ACCESS_RULE,
        plural: "networkaccessrules",
        short: &["nar"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetPolicy,
        form: Form::Primary,
        in_stack: true,
        stack_group: "networkAccessRules",
        converges: true,
        // Unlike `FIREWALL_POLICY`: this Kind's rule carries its own `origin`,
        // a durable identity independent of anything else on the target
        // container — so a stack CAN own and prune it.
        teardown: true,
        namespaced: Namespaced::Never,
        presence: Presence::Declarative,
    },
    KindFacts {
        kind: NETWORK_GATEWAY,
        plural: "routers",
        short: &["rtr"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetConnectivity,
        form: Form::Primary,
        in_stack: true,
        stack_group: "networkGateways",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Never,
        // Its own registry (ADR-0051 Phase 3): the appliance a `GatewayProvider`
        // talks to carries no `delonix.io/stack` label of its own to stamp — the
        // same reason `Service`'s record lives in ITS own registry rather than
        // on a target container.
        presence: Presence::Registry,
    },
    KindFacts {
        // ADR-0046 D3: a reservation ledger routes claim addresses from. It sits
        // BEFORE `HTTPRoute` because a route with `spec.pool` claims at apply time and
        // the pool has to exist by then.
        kind: IPPOOL,
        plural: "ippools",
        short: &["pool"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetExposure,
        form: Form::Primary,
        in_stack: true,
        stack_group: "ipPools",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        // ADR-0086 D3/D4c, ADR-0090: a declarative claim of ONE address from the
        // `IPPool` above, now LIVE. The claim is recorded in the IPAM ledger
        // (`cmd::ipaddress`) so nothing else leases it — its own apply, converges
        // per document, read back by `get ipaddresses`/`describe ipaddress`,
        // released by `delete ipaddress`. Applied right AFTER `IPPool`, which it
        // claims from (the pool has to exist by then — the same reason `HTTPRoute`
        // follows the pool). Node-scoped (D7), like the pool.
        kind: IP_ADDRESS,
        plural: "ipaddresses",
        short: &["ip"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetExposure,
        form: Form::Primary,
        in_stack: true,
        stack_group: "ipAddresses",
        // Converges per document: `ipaddress::converge_doc` re-reserves and
        // rewrites the record in place, so a changed `address`/`pool` is applied
        // without a destroy-and-recreate.
        converges: true,
        teardown: true,
        // Node-scoped like the `IPPool` it claims from (D7) — an IPAM claim is a
        // property of the node, not of one tenant.
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        kind: HTTP_ROUTE,
        plural: "httproutes",
        short: &["hr"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetExposure,
        form: Form::Primary,
        in_stack: true,
        stack_group: "httpRoutes",
        converges: true,
        teardown: true,
        namespaced: Namespaced::Never,
        presence: Presence::Declarative,
    },
    KindFacts {
        kind: GATEWAY,
        plural: "gateways",
        short: &["gw"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetExposure,
        form: Form::Primary,
        in_stack: true,
        stack_group: "gateways",
        converges: true,
        teardown: false,
        namespaced: Namespaced::Never,
        presence: Presence::Registry,
    },
    KindFacts {
        // ADR-0021. LAST in apply order, after everything it could pull: the
        // timer it installs may fire as soon as it is written, and the stack it
        // reconciles into should already be what this same apply declared.
        // `destroy` reverses the order, so the timer goes first and nothing
        // reconciles while the rest of the stack comes down.
        kind: GITOPS_SOURCE,
        plural: "gitopssources",
        short: &["gos"],
        api_version: "core.delonix.io/v1beta1",
        domain: Domain::Composition,
        form: Form::Primary,
        in_stack: true,
        stack_group: "gitopsSources",
        converges: true,
        teardown: true,
        // A source applies INTO a namespace, and two tenants may each name
        // theirs `production`.
        namespaced: Namespaced::Always,
        presence: Presence::Registry,
    },
    // --- Not resources of the stack: they become one of the above, or are not
    // local resources at all. ---
    KindFacts {
        // ADR-0086 D4: the service Kind. Lowers at load into its `replicas` `Pod`
        // INSTANCES (`cmd::application`), so like `Workload` it is out of the stack
        // cycle and not observable — the Pods it becomes are.
        kind: APPLICATION,
        plural: "applications",
        short: &[],
        api_version: "compute.delonix.io/v1beta1",
        domain: Domain::Compute,
        form: Form::Sugar(POD),
        in_stack: false,
        stack_group: "applications",
        converges: false,
        teardown: false,
        namespaced: Namespaced::Always,
        presence: Presence::NotObservable,
    },
    KindFacts {
        // ADR-0086 D3: run-to-completion. Lowers to one `Pod` (`cmd::job`), so
        // like `Application`/`Workload` it is out of the stack cycle and not
        // observable — the Pod it becomes is.
        kind: JOB,
        plural: "jobs",
        short: &[],
        api_version: "compute.delonix.io/v1beta1",
        domain: Domain::Compute,
        form: Form::Sugar(POD),
        in_stack: false,
        stack_group: "jobs",
        converges: false,
        teardown: false,
        namespaced: Namespaced::Always,
        presence: Presence::NotObservable,
    },
    KindFacts {
        kind: WORKLOAD,
        plural: "workloads",
        short: &["wl"],
        api_version: "compute.delonix.io/v1beta1",
        domain: Domain::Compute,
        form: Form::Sugar(WORKLOAD_LOWERS_TO),
        in_stack: false,
        stack_group: "workloads",
        converges: false,
        teardown: false,
        // Carried onto the child it lowers to.
        namespaced: Namespaced::Always,
        presence: Presence::NotObservable,
    },
    KindFacts {
        kind: DEPENDENCY,
        plural: "dependencies",
        short: &["dep"],
        api_version: "networking.delonix.io/v1beta1",
        domain: Domain::NetPolicy,
        form: Form::Sugar(FIREWALL_POLICY),
        in_stack: false,
        stack_group: "dependencies",
        converges: false,
        teardown: false,
        namespaced: Namespaced::Never,
        // It has an arm in `presence()` (answering `-`) even though the load
        // lowers it away, so a document that somehow reaches `ls` is described
        // rather than called unsupported.
        presence: Presence::Declarative,
    },
    KindFacts {
        // ADR-0086 D3/D5d. RESERVED: declared at `v1beta1` with no apply yet —
        // snapshots today are imperative (`vm snapshot`). The declarative Kind,
        // its retention and its `Bucket` destination land in a later phase under
        // ADR-0090. Out of the stack cycle, no group, not observable.
        kind: SNAPSHOT,
        plural: "snapshots",
        short: &["snap"],
        api_version: "storage.delonix.io/v1beta1",
        domain: Domain::Storage,
        form: Form::Reserved,
        in_stack: false,
        stack_group: "",
        converges: false,
        teardown: false,
        // Provisional, as for `IPAddress` above: `Never` until the apply exists.
        namespaced: Namespaced::Never,
        presence: Presence::NotObservable,
    },
    KindFacts {
        // ADR-0086 D3/D7. RESERVED: this node as a readable resource, declared at
        // `v1beta1` with no apply yet — `NodeService.GetCapacity` already serves
        // the capacity, but the declarative Kind's read-back/labels/taints land
        // with the ResourceService (ADR-0087/0090). Node-scoped (D7), out of the
        // stack cycle, no group, not observable.
        kind: NODE,
        plural: "nodes",
        short: &[],
        api_version: "core.delonix.io/v1beta1",
        // The compute host itself — the substrate the compute Kinds run on.
        domain: Domain::Compute,
        form: Form::Reserved,
        in_stack: false,
        stack_group: "",
        converges: false,
        teardown: false,
        // Genuinely node-scoped (D7), not merely provisional like the others.
        namespaced: Namespaced::Never,
        presence: Presence::NotObservable,
    },
    KindFacts {
        kind: STACK,
        plural: "stacks",
        short: &[],
        api_version: "core.delonix.io/v1beta1",
        domain: Domain::Composition,
        form: Form::Aggregate,
        in_stack: false,
        stack_group: "",
        converges: false,
        teardown: false,
        // Propagated to every child it expands into.
        namespaced: Namespaced::Always,
        presence: Presence::NotObservable,
    },
    KindFacts {
        kind: CLUSTER,
        plural: "kubernetesclusters",
        // "clusters" is not the plural (that is "kubernetesclusters", the
        // literal pluralization every other Kind here follows) — it is the
        // shortname that matches the CLI group's own name (`cluster
        // create`/`cluster apply`/...) and the one this repo's own release
        // notes (v0.69.0) and built-in manual already promised as the
        // replacement for `cluster ls`/`cluster delete`. Without it, `delonix
        // get clusters` — the exact command documented — failed with "no such
        // resource kind 'clusters'", a real regression shipped in v1.0.0.
        short: &["kc", "clusters"],
        api_version: "cluster.delonix.io/v1beta1",
        domain: Domain::Composition,
        form: Form::Primary,
        // Deliberately outside the stack cycle: it is a remote procedure over
        // SSH against hosts that already exist, not a resource of this node.
        in_stack: false,
        stack_group: "",
        converges: false,
        teardown: false,
        namespaced: Namespaced::Never,
        presence: Presence::NotObservable,
    },
];

/// Facts for a CANONICAL kind name (`canonical_kind` has already run at the call
/// site), or `None` for a name this engine does not know.
pub fn facts(kind: &str) -> Option<&'static KindFacts> {
    FACTS.iter().find(|f| f.kind == kind)
}

/// Every Kind in the table, in the order it is written.
pub fn all() -> impl Iterator<Item = &'static KindFacts> {
    FACTS.iter()
}

/// The Kinds `stack apply` handles, **in apply order**. Was the `KINDS`
/// constant.
pub fn stack_kinds() -> impl DoubleEndedIterator<Item = &'static str> {
    FACTS.iter().filter(|f| f.in_stack).map(|f| f.kind)
}

/// Every group a `kind: Stack` accepts, as `(group, kind)`, **in table order**.
///
/// The Kinds that are applied as themselves come first, in apply order (the
/// table is written that way), then the two that are lowered into another Kind.
/// The expansion iterates this and never a list of its own.
pub fn stack_groups() -> impl Iterator<Item = (&'static str, &'static str)> {
    FACTS
        .iter()
        .filter(|f| !f.stack_group.is_empty())
        .map(|f| (f.stack_group, f.kind))
}

/// Old spellings of a group key, as `(spelling, canonical group)`.
///
/// A rename does not change what the document MEANS, so the old key keeps
/// loading in silence — the rule `KIND_ALIASES` already follows for the Kinds
/// themselves, and for the same reason: warning would train people to ignore
/// warnings. `tunnels:` is what the Gateway group was called while the Kind was
/// still `Tunnel`.
pub const STACK_GROUP_ALIASES: &[(&str, &str)] = &[("tunnels", "gateways")];

/// The group a Stack key names, resolving an old spelling; `None` when the key
/// is not a group at all (a typo).
pub fn resolve_stack_group(key: &str) -> Option<&'static str> {
    stack_groups()
        .map(|(g, _)| g)
        .find(|g| *g == key)
        .or_else(|| {
            STACK_GROUP_ALIASES
                .iter()
                .find(|(old, _)| *old == key)
                .map(|(_, new)| *new)
        })
}

/// Every key a Stack `spec` accepts: the groups plus their old spellings.
pub fn stack_spec_fields() -> Vec<&'static str> {
    stack_groups()
        .map(|(g, _)| g)
        .chain(STACK_GROUP_ALIASES.iter().map(|(old, _)| *old))
        .collect()
}

/// Why a Kind cannot be grouped in a Stack — `None` when it can.
///
/// The same discipline as `not_converged_reason`-style columns: a Kind either
/// has a group or writes down why not. Without it, the next Kind added to the
/// table would silently join the ones a Stack cannot hold.
pub fn stack_group_absent_reason(kind: &str) -> Option<&'static str> {
    let f = facts(kind)?;
    if !f.stack_group.is_empty() {
        return None;
    }
    Some(match kind {
        STACK => {
            "a Stack does not nest: the children would need a namespace and an owner of \
             their own, and the reconciler identifies a resource by (kind, name) — two \
             stacks reaching the same child through a third is a second writer"
        }
        CLUSTER => {
            "a KubernetesCluster is a remote procedure over SSH against hosts that already \
             exist, not a resource of this node, so `stack apply` never runs it — grouping \
             it would promise an apply that does not happen"
        }
        SNAPSHOT | NODE => {
            "a Reserved Kind has no apply yet (ADR-0086 D3, built under ADR-0090), so \
             `stack apply` never runs it — grouping it would promise an apply that does \
             not happen"
        }
        _ => return None,
    })
}

/// Whether the Kind belongs to the stack cycle at all.
pub fn in_stack(kind: &str) -> bool {
    facts(kind).is_some_and(|f| f.in_stack)
}

/// Whether a changed field is really applied. Was `CONVERGING_KINDS.contains`.
pub fn converges(kind: &str) -> bool {
    facts(kind).is_some_and(|f| f.converges)
}

/// Whether `destroy_one` removes it. Was `TEARDOWN_KINDS.contains`.
pub fn has_teardown(kind: &str) -> bool {
    facts(kind).is_some_and(|f| f.teardown)
}

/// Whether `metadata.namespace` does anything here.
pub fn honors_namespace(kind: &str) -> bool {
    facts(kind).is_some_and(|f| f.namespaced != Namespaced::Never)
}

/// The Kinds `metadata.namespace` actually does something on, sorted.
///
/// DERIVED and not written out, because a hand-kept copy of this list is
/// exactly what this module exists to have ended. The warning that consumes it
/// said "only Container, Pod and Vm" while the table already had SEVEN — so
/// someone who wrote `namespace:` on a `kind: Stack` was being told, by a
/// warning ABOUT namespaces, that Stack has none. The condition next to it was
/// right all along (`honors_namespace`, off this same table); only the sentence
/// had drifted, which is the quiet way a message becomes wrong.
///
/// Deprecated spellings are left out: `ShareVolume` honors the field and is
/// rewritten into `Volume` at load time, so naming both would advertise a
/// spelling we are retiring. Sugar and aggregates stay — `Workload` and `Stack`
/// are things a person legitimately writes.
pub fn namespaced_kinds() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = all()
        .filter(|f| f.namespaced != Namespaced::Never)
        .filter(|f| !matches!(f.form, Form::Sunset(_)))
        .map(|f| f.kind)
        .collect();
    out.sort_unstable();
    out
}

/// The Kinds that are node-scoped — they carry no namespace, because they are a
/// property of the node itself and not of any one tenant (ADR-0086 D7). Every
/// OTHER Kind is namespaced, and its name is unique WITHIN the namespace.
///
/// This const is the source of truth for the first half of D7; the second — the
/// name-unique-per-namespace relayout, where the store key (`vms/<name>.json`)
/// and the pod netns (`pod-<name>`) become namespace-qualified, reversing
/// ADR-0069 Pending #2 — is a structural change still to land (it touches the
/// live VM adapter and needs a VM-capable environment to prove). Until then this
/// list guards the node-scoped set from drift (`is_node_scoped` and the test),
/// so the decision is recorded where the relayout will read it.
pub const D7_NODE_SCOPED: &[&str] = &[
    NODE,
    STORAGE_POOL,
    IPPOOL,
    IP_ADDRESS,
    NETWORK_GATEWAY,
    RUNTIME_POLICY,
];

/// Whether a Kind is node-scoped per ADR-0086 D7. The membership is the
/// decision (`D7_NODE_SCOPED`); `Node` is on the list even though it is still
/// `Form::Reserved` (declared, no apply yet), because being a node resource is
/// about WHAT it is, not whether its apply is built. The test cross-checks that
/// every Kind on the list really carries `Namespaced::Never`, so the list and
/// the table cannot disagree.
pub fn is_node_scoped(kind: &str) -> bool {
    facts(kind).is_some_and(|f| D7_NODE_SCOPED.contains(&f.kind))
}

// There is deliberately no `is_declarative(kind)` helper. `stack wait` decides
// on the presence MARKER the store actually returned (`-`), not on what the
// table says the Kind should return: a second path to the same answer would let
// the two disagree exactly when a Kind's presence changes — which is what
// happened to `NetworkRoute` when it gained a record of its own. The column is
// here to be DISPLAYED and to be asserted against `presence()` in the tests.

/// The domain label for display, or `-` for a Kind this engine does not know
/// (`stack ls` prints what the manifest says, including a typo).
pub fn domain_label(kind: &str) -> &'static str {
    facts(kind).map(|f| f.domain.label()).unwrap_or("-")
}

#[cfg(test)]
mod tests {

    // Regression for a message that had drifted: the warning named three Kinds
    // ("only Container, Pod and Vm") while the table honored seven. Asserted
    // against the TABLE and not against a written-out list, so the day a Kind
    // becomes namespaced the sentence follows it without anyone remembering.
    #[test]
    fn the_namespaced_list_is_the_table_and_hides_the_deprecated_spelling() {
        let got = namespaced_kinds();
        for kind in &got {
            assert!(
                honors_namespace(kind),
                "{kind} is advertised as namespaced and the table disagrees"
            );
        }
        for f in all() {
            if f.namespaced != Namespaced::Never && !matches!(f.form, Form::Sunset(_)) {
                assert!(
                    got.contains(&f.kind),
                    "{} honors the namespace and the message never names it",
                    f.kind
                );
            }
        }
        // ShareVolume honors it AND is rewritten into Volume at load time —
        // naming it would advertise the spelling we are retiring.
        assert!(
            !got.contains(&"ShareVolume"),
            "a deprecated spelling leaked in"
        );
        // The exact shape of the drift this closes: the old sentence.
        assert!(
            got.len() > 3,
            "back down to the stale trio — the list stopped coming off the table"
        );
    }
    use super::*;

    /// **A Kind the stack applies must be a Kind a Stack can hold.** Four were
    /// not — `NetworkRoute`, `NetworkAccessRule`, `Service`, `App` — because the
    /// group list was written by hand next to the table instead of coming off it.
    #[test]
    fn every_kind_applied_by_the_stack_has_a_group() {
        for f in all().filter(|f| f.in_stack) {
            assert!(
                !f.stack_group.is_empty(),
                "{} is applied by `stack apply` and has no group in `kind: Stack`",
                f.kind
            );
        }
    }

    /// The reverse: a Kind with no group owes the reason, and a Kind with a
    /// group owes none.
    #[test]
    fn a_kind_without_a_group_says_why() {
        for f in all() {
            let reason = stack_group_absent_reason(f.kind);
            assert_eq!(
                f.stack_group.is_empty(),
                reason.is_some(),
                "{}: the group column and stack_group_absent_reason disagree",
                f.kind
            );
        }
    }

    /// Two Kinds under one key would merge their children into one list and
    /// hand the wrong spec to one of them.
    #[test]
    fn a_group_key_is_unique_and_no_alias_shadows_it() {
        let mut seen = std::collections::BTreeSet::new();
        for (g, _) in stack_groups() {
            assert!(seen.insert(g), "group '{g}' is used by two Kinds");
        }
        for (old, new) in STACK_GROUP_ALIASES {
            assert!(!seen.contains(old), "alias '{old}' shadows a real group");
            assert!(
                seen.contains(new),
                "alias '{old}' points at '{new}', not a group"
            );
        }
    }

    /// A group is the Kind's plural in lowerCamelCase, so nobody has to look it
    /// up: `networkRoutes` for `NetworkRoute`. Three are older than the rename of
    /// their Kind and keep the key manifests already write — renaming a group
    /// breaks every published Stack, and the Kind rename did not.
    #[test]
    fn a_group_is_the_plural_of_its_kind() {
        const KEPT: &[(&str, &str)] = &[
            (INGRESS, "ingress"),
            (VM, "vms"),
            (FIREWALL_POLICY, "firewallPolicies"),
            // Renamed by the catalog v1 (ADR-0086 D3): the Kind is now `Bucket`/
            // `Router`, but the Stack group keeps its published key so existing
            // Stacks keep loading — the same treatment the three above got, where
            // the group key (`vms`, `ingress`, `firewallPolicies`) stays put
            // while the Kind's own plural moved on.
            (OBJECT_STORAGE, "objectStorages"),
            (NETWORK_GATEWAY, "networkGateways"),
        ];
        for f in all().filter(|f| !f.stack_group.is_empty()) {
            if KEPT.contains(&(f.kind, f.stack_group)) {
                continue;
            }
            assert_eq!(
                f.stack_group.to_ascii_lowercase(),
                f.plural,
                "{}: the group is not the plural of the Kind",
                f.kind
            );
        }
    }

    /// The two Kinds that are lowered still have a group, and a Stack is not one.
    #[test]
    fn lowered_kinds_are_groupable_and_the_aggregate_is_not() {
        for kind in [WORKLOAD, DEPENDENCY] {
            assert!(!facts(kind).unwrap().stack_group.is_empty(), "{kind}");
        }
        assert!(stack_group_absent_reason(STACK).is_some());
        assert!(stack_group_absent_reason(CLUSTER).is_some());
        assert_eq!(resolve_stack_group("tunnels"), Some("gateways"));
        assert_eq!(resolve_stack_group("contaienrs"), None);
    }

    /// Two rows for one Kind means half the code reads one and half the other.
    #[test]
    fn nenhum_kind_aparece_duas_vezes() {
        let mut seen = std::collections::BTreeSet::new();
        for f in all() {
            assert!(seen.insert(f.kind), "{} está duplicado na tabela", f.kind);
        }
    }

    /// The reversal `destroy` relies on is only meaningful if the stack Kinds
    /// are contiguous at the head of the table: a row inserted between them
    /// changes the apply order without anyone editing an «order» anywhere.
    #[test]
    fn os_kinds_do_stack_sao_um_prefixo_contiguo() {
        let first_out = FACTS.iter().position(|f| !f.in_stack).unwrap();
        assert!(
            FACTS[first_out..].iter().all(|f| !f.in_stack),
            "um Kind do stack está DEPOIS de um que não é — a ordem de apply mudou em silêncio"
        );
        assert_eq!(stack_kinds().count(), first_out);
    }

    /// A Kind that does not survive the load has no apply of its own, so it
    /// cannot be part of the stack cycle — and if it were, `apply` would look
    /// for documents that no longer exist.
    #[test]
    fn um_kind_que_baixa_para_outro_nao_pertence_ao_ciclo_do_stack() {
        for f in all() {
            if matches!(f.form, Form::Sugar(_) | Form::Aggregate) {
                assert!(
                    !f.in_stack,
                    "{} é reescrito no load e está no ciclo do stack",
                    f.kind
                );
                assert!(
                    !f.converges,
                    "{} é reescrito no load e diz convergir",
                    f.kind
                );
            }
        }
        // A `Compat` DOES survive — that is the whole difference, and the
        // `Ingress` proves it by being applied like any other.
        assert!(in_stack("Ingress"));
    }

    /// The pairing that `TEARDOWN_KINDS` and `no_teardown_reason` used to keep
    /// by hand: promising to remove something the destroy refuses would fail
    /// HALFWAY, after the rest of the stack is already gone.
    #[test]
    fn so_um_kind_convergente_tem_teardown() {
        for f in all() {
            if f.teardown {
                assert!(
                    f.converges,
                    "{} tem teardown e não converge — o `--prune` promete o que o destroy recusa",
                    f.kind
                );
            }
        }
    }

    /// A Kind outside the stack cycle is never applied, so it can neither
    /// converge nor be torn down. Catches the copy-paste of a row.
    #[test]
    fn fora_do_ciclo_nao_converge_nem_e_removido() {
        for f in all().filter(|f| !f.in_stack) {
            assert!(!f.converges, "{} converge fora do ciclo", f.kind);
            assert!(!f.teardown, "{} tem teardown fora do ciclo", f.kind);
        }
    }

    /// A Kind the `apply` applies has to be observable SOMEHOW — `NotObservable`
    /// is the mark of a document that never reaches `presence()`, and a Kind in
    /// the cycle that carried it would fall through to `_ => ("?", "unsupported
    /// kind")`: printed by `ls`/`describe`, and counted as pending forever by
    /// `wait`. That is exactly what `NetworkRoute` did for several versions.
    ///
    /// The paired check lives in `stack.rs`, where `presence()` can be called:
    /// this one says the column is filled in, that one says it is TRUE.
    #[test]
    fn um_kind_do_ciclo_do_stack_tem_presenca_observavel() {
        for f in all().filter(|f| f.in_stack) {
            assert_ne!(
                f.presence,
                Presence::NotObservable,
                "{} é aplicado pelo stack e não tem presença — cairia no «unsupported kind»",
                f.kind
            );
        }
    }

    /// ADR-0086 D7: exactly the Kinds in `D7_NODE_SCOPED` are node-scoped, and
    /// each really carries `Namespaced::Never` in the table. This locks the
    /// first (and today true) half of D7 — the node-scoped set — from drift: a
    /// node resource accidentally made namespaced, or a tenant Kind slipped into
    /// the node-scoped list. The second half (every OTHER Kind namespaced, name
    /// unique per namespace, the `vms/<name>.json`/`pod-<name>` relayout that
    /// reverses ADR-0069 #2) is a structural change still to land and is NOT
    /// asserted here.
    #[test]
    fn the_d7_node_scoped_set_is_exactly_the_node_resources() {
        for k in D7_NODE_SCOPED {
            assert_eq!(
                facts(k).unwrap().namespaced,
                Namespaced::Never,
                "{k} is in D7_NODE_SCOPED but the table does not mark it node-scoped"
            );
            assert!(is_node_scoped(k), "{k}");
        }
        // Nothing OUTSIDE the list answers `is_node_scoped` — the helper is the
        // list, so a sixth node resource has to be added here on purpose.
        for f in all() {
            assert_eq!(
                is_node_scoped(f.kind),
                D7_NODE_SCOPED.contains(&f.kind),
                "{} disagrees with D7_NODE_SCOPED",
                f.kind
            );
        }
    }

    /// The lowering target has to be a Kind that exists — a typo here sends the
    /// reader looking for a Kind the engine never had.
    #[test]
    fn o_destino_de_uma_reducao_existe() {
        for f in all() {
            let Some(to) = f.form.successor() else {
                continue;
            };
            // `Workload` names three, and the slash is the honest spelling.
            for k in to.split('/') {
                assert!(
                    facts(k).is_some(),
                    "{} baixa para {k}, que não existe",
                    f.kind
                );
            }
        }
    }
}
