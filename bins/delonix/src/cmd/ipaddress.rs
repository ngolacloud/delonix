//! `kind: IPAddress` — a declarative claim of ONE specific address from an
//! `IPPool` (ADR-0086 D3/D4c, ADR-0090).
//!
//! The `IPPool` says which addresses exist; an `IPAddress` pins ONE of them to a
//! name so nothing else leases it. The claim is recorded in the live IPAM ledger
//! (`delonix_sdn::ipam`, the same registry `network ipam ls` shows and that the
//! container/VM allocator reserves against), under the claimant id
//! `IPAddress/<name>` — so a second `IPAddress` that names the same address is
//! REFUSED, the way two containers on one IP are.
//!
//! **Why a sidecar record beside the lease.** The IPAM registry holds only
//! `id -> ip`; it has no room for ownership, the last-applied spec or the
//! protect mark the reconciler stamps. So, exactly as `IPPool` keeps its own
//! `PoolFile`, an `IPAddress` keeps a small record of its own
//! (`<root>/ipaddress/<name>.json`) for that metadata, while the address claim
//! itself lives in the ledger. The record is written only AFTER a successful
//! reserve, so it never names a claim that did not take.
//!
//! The pool's prefix/CIDR — what the ledger keys the lease under — is derived
//! from the `IPPool`: the CIDR entry of the pool that contains the address, or,
//! for a pool written as ranges/singletons, the address's natural `/24`.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use super::kinds as k;
use super::manifest::{self, ManifestDoc};
use super::output::OutputFormat;
use delonix_model::{Error, Result};
use delonix_sdn::ipam;
use serde::{Deserialize, Serialize};

/// `spec` of `kind: IPAddress`.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct IpAddressSpec {
    /// The specific IPv4 address to claim. It must be one of the addresses the
    /// named `pool` owns.
    pub address: String,
    /// The `IPPool` the address is claimed from. The pool must already exist (it
    /// is applied before this, like an `HTTPRoute`'s pool), and the address must
    /// belong to it.
    pub pool: String,
    /// An optional note of what the address is reserved for — a workload name, a
    /// purpose. Purely descriptive; it changes nothing the engine does.
    #[serde(default)]
    pub owner: Option<String>,
}

pub const IP_ADDRESS_SPEC_FIELDS: &[&str] = &["address", "pool", "owner"];
pub const RECONCILED_IP_ADDRESS_FIELDS: &[&str] = &["address", "pool", "owner"];

/// One claim as persisted: the metadata beside the ledger lease (see the module
/// note on why the lease alone is not enough).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IpAddressFile {
    pub name: String,
    pub address: String,
    pub pool: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
}

fn dir() -> std::path::PathBuf {
    super::util::state_root().join("ipaddress")
}

fn path_in(d: &std::path::Path, name: &str) -> std::path::PathBuf {
    d.join(format!("{name}.json"))
}

/// The claimant id this Kind holds its lease under. Scoped by Kind, the way
/// `IPPool` scopes its claimants `HTTPRoute/<name>`: two different Kinds that
/// happened to share a name must never share a lease. Shared with
/// `prune::lease_owners`, so the IPAM reaper knows an `IPAddress`'s lease is
/// live and never reclaims it (the same way it keeps a `Service`'s VIP).
pub(crate) fn claimant_id(name: &str) -> String {
    format!("{}/{name}", k::IP_ADDRESS)
}

fn get_in(d: &std::path::Path, name: &str) -> Option<IpAddressFile> {
    serde_json::from_slice(&std::fs::read(path_in(d, name)).ok()?).ok()
}

/// Like [`get_in`], but a file that exists and does not parse is an ERROR, not
/// «no such claim»: the record points at a live lease, and reading a damaged
/// file as absent lets the next write forget which address this name holds.
fn read_in(d: &std::path::Path, name: &str) -> Result<Option<IpAddressFile>> {
    let path = path_in(d, name);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::Invalid(format!("ipaddress {name}: {e}"))),
    };
    serde_json::from_slice(&bytes).map(Some).map_err(|e| {
        Error::Invalid(format!(
            "ipaddress {name}: {} is damaged ({e}) — restore it or remove it by hand",
            path.display()
        ))
    })
}

fn list_in(d: &std::path::Path) -> Vec<IpAddressFile> {
    let mut v: Vec<IpAddressFile> = std::fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok())
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

fn write_in(d: &std::path::Path, f: &IpAddressFile) -> Result<()> {
    std::fs::create_dir_all(d).map_err(|e| Error::Invalid(format!("ipaddress: {e}")))?;
    let json = serde_json::to_vec_pretty(f).map_err(|e| Error::Invalid(e.to_string()))?;
    delonix_state::write_atomic(&path_in(d, &f.name), &json)
        .map_err(|e| Error::Invalid(format!("ipaddress: {e}")))
}

pub fn address_get(name: &str) -> Option<IpAddressFile> {
    get_in(&dir(), name)
}

pub fn address_list() -> Vec<IpAddressFile> {
    list_in(&dir())
}

// ---- pool resolution ------------------------------------------------------------

/// Parses the `address` field as a plain IPv4 address.
fn parse_address(address: &str) -> Result<Ipv4Addr> {
    address.trim().parse::<Ipv4Addr>().map_err(|_| {
        Error::Invalid(format!(
            "IPAddress: '{address}' is not an IPv4 address (IPv6 is not supported)"
        ))
    })
}

/// The ledger prefix the lease is keyed under, derived from the `pool`.
///
/// The CIDR entry of the pool that CONTAINS the address is the right key — it is
/// the subnet the address lives in, and the allocator keys container leases the
/// same way, so an `IPAddress` and a container on the same subnet share one
/// registry and cannot both take the address. A pool written only as ranges or
/// singletons has no CIDR to borrow; there the address's natural `/24` is used,
/// which `ipam::reserve` accepts and keys the lease under deterministically.
fn pool_prefix_for(pool: &super::ippool::PoolFile, addr: Ipv4Addr) -> String {
    let want = u32::from(addr);
    for entry in &pool.addresses {
        if entry.contains('/') {
            if let Some(c) = delonix_sdn::Cidr::parse(entry) {
                if c.contains(want) {
                    return c.to_string_cidr();
                }
            }
        }
    }
    let o = addr.octets();
    format!("{}.{}.{}.0/24", o[0], o[1], o[2])
}

/// Resolves the pool and checks the address belongs to it, returning the ledger
/// prefix to reserve under. The containment check against the pool's EXPANDED
/// addresses is what makes "claimed from this pool" true rather than decorative.
fn resolve(spec: &IpAddressSpec) -> Result<(Ipv4Addr, String)> {
    let addr = parse_address(&spec.address)?;
    let pool = super::ippool::pool_get(&spec.pool).ok_or_else(|| {
        Error::NotFound(format!(
            "IPAddress: IPPool '{}' (declare a `kind: IPPool` named '{}')",
            spec.pool, spec.pool
        ))
    })?;
    let owned = super::ippool::expand(&pool.addresses)?;
    if !owned.contains(&addr) {
        return Err(Error::Invalid(format!(
            "IPAddress: {addr} is not an address of IPPool '{}'",
            spec.pool
        )));
    }
    Ok((addr, pool_prefix_for(&pool, addr)))
}

// ---- the Kind -------------------------------------------------------------------

fn spec_fields(spec: &IpAddressSpec) -> BTreeMap<String, String> {
    let mut f = BTreeMap::new();
    f.insert("address".into(), spec.address.clone());
    f.insert("pool".into(), spec.pool.clone());
    f.insert("owner".into(), spec.owner.clone().unwrap_or_default());
    f
}

pub(crate) fn desired(doc: &ManifestDoc) -> Result<super::reconcile::Desired> {
    let spec: IpAddressSpec = manifest::spec_of(doc)?;
    Ok(super::reconcile::Desired {
        kind: k::IP_ADDRESS.into(),
        name: doc.metadata.name.clone(),
        fields: spec_fields(&spec),
        converges: true,
        ownable: true,
    })
}

pub(crate) fn actual() -> Result<Vec<super::reconcile::Actual>> {
    Ok(address_list()
        .into_iter()
        .map(|a| {
            let mut f = BTreeMap::new();
            f.insert("address".into(), a.address.clone());
            f.insert("pool".into(), a.pool.clone());
            f.insert("owner".into(), a.owner.clone().unwrap_or_default());
            super::reconcile::Actual {
                kind: k::IP_ADDRESS.into(),
                name: a.name.clone(),
                fields: f,
                owner: a.labels.get(super::reconcile::STACK_LABEL).cloned(),
                last_applied: a
                    .annotations
                    .get(super::reconcile::LAST_APPLIED)
                    .and_then(|raw| super::reconcile::decode_last_applied(raw)),
                protect: a.labels.contains_key(super::reconcile::PROTECT),
            }
        })
        .collect())
}

pub(crate) fn stamp(
    name: &str,
    stack: &str,
    fields: &BTreeMap<String, String>,
    protected: bool,
) -> Result<()> {
    let d = dir();
    let mut a = read_in(&d, name)?.ok_or_else(|| Error::NotFound(format!("ipaddress: {name}")))?;
    a.labels
        .insert(super::reconcile::STACK_LABEL.into(), stack.into());
    if protected {
        a.labels
            .insert(super::reconcile::PROTECT.into(), "true".into());
    }
    a.labels
        .insert(super::reconcile::MANAGED_BY.into(), "delonix".into());
    super::reconcile::stamp_annotations(&mut a.annotations, fields, k::IP_ADDRESS);
    write_in(&d, &a)
}

/// Removes a claim: releases its lease in the ledger, then drops the record.
/// This is the delete/prune path — an `IPAddress` IS the lease, so taking it
/// down GIVES the address back (unlike an `IPPool`, which refuses removal while
/// anyone still holds one of its addresses).
pub(crate) fn remove_for_replace(name: &str) -> Result<()> {
    let d = dir();
    let Some(a) = read_in(&d, name)? else {
        return Ok(());
    };
    if let Ok(addr) = parse_address(&a.address) {
        if let Some(pool) = super::ippool::pool_get(&a.pool) {
            ipam::release(&pool_prefix_for(&pool, addr), &claimant_id(name));
        } else {
            // The pool is gone; release wherever the address's natural /24 keyed it.
            let o = addr.octets();
            ipam::release(
                &format!("{}.{}.{}.0/24", o[0], o[1], o[2]),
                &claimant_id(name),
            );
        }
    }
    let _ = std::fs::remove_file(path_in(&d, name));
    Ok(())
}

fn apply_one(doc: &ManifestDoc) -> Result<()> {
    let spec: IpAddressSpec = manifest::spec_of(doc)?;
    let name = &doc.metadata.name;
    let (addr, prefix) = resolve(&spec)?;
    let id = claimant_id(name);
    let d = dir();
    let existing = read_in(&d, name)?;
    // If this name already held a DIFFERENT address or sat in another pool's
    // prefix, give the old lease back before taking the new one — otherwise a
    // re-pointed claim would leak its previous address.
    if let Some(prev) = &existing {
        if let Ok(prev_addr) = parse_address(&prev.address) {
            if prev_addr != addr || prev.pool != spec.pool {
                if let Some(prev_pool) = super::ippool::pool_get(&prev.pool) {
                    ipam::release(&pool_prefix_for(&prev_pool, prev_addr), &id);
                }
            }
        }
    }
    // The live claim. Refuses if the address is already leased to another id
    // (another IPAddress, or a container) — the reservation this Kind exists to
    // make hold.
    ipam::reserve(&prefix, &id, &addr.to_string()).map_err(Error::from)?;
    write_in(
        &d,
        &IpAddressFile {
            name: name.clone(),
            address: addr.to_string(),
            pool: spec.pool.clone(),
            owner: spec.owner.clone(),
            // Preserve any stamp a previous apply wrote; `stamp()` re-writes it.
            labels: existing
                .as_ref()
                .map(|p| p.labels.clone())
                .unwrap_or_default(),
            annotations: existing
                .as_ref()
                .map(|p| p.annotations.clone())
                .unwrap_or_default(),
        },
    )?;
    println!(
        "{}",
        super::po::tf(
            "ipaddress/{name}: {address} claimed from IPPool {pool}",
            &[
                ("name", name),
                ("address", &addr.to_string()),
                ("pool", &spec.pool),
            ],
        )
    );
    Ok(())
}

pub fn apply(docs: &[ManifestDoc]) -> Result<()> {
    for doc in manifest::of_kind(docs, k::IP_ADDRESS) {
        apply_one(doc)?;
    }
    Ok(())
}

pub(crate) fn converge_doc(doc: &ManifestDoc) -> Result<()> {
    apply_one(doc)
}

pub(crate) fn presence_of(doc: &ManifestDoc) -> (String, String) {
    let name = &doc.metadata.name;
    match address_get(name) {
        None => ("no".into(), "-".into()),
        Some(a) => {
            // Confirm the lease is really in the ledger, not just the record.
            let live = parse_address(&a.address).ok().and_then(|addr| {
                super::ippool::pool_get(&a.pool)
                    .map(|p| pool_prefix_for(&p, addr))
                    .and_then(|prefix| ipam::lookup(&prefix, &claimant_id(name)))
            });
            match live {
                Some(ip) => ("yes".into(), ip),
                None => ("yes".into(), a.address.clone()),
            }
        }
    }
}

pub fn spec_with_defaults(doc: &ManifestDoc) -> Result<serde_yaml::Value> {
    let spec: IpAddressSpec = manifest::spec_of(doc)?;
    serde_yaml::to_value(spec).map_err(|e| Error::Invalid(format!("dry-run: {e}")))
}

#[derive(Serialize)]
struct LsRow {
    name: String,
    address: String,
    pool: String,
    owner: Option<String>,
    stack: Option<String>,
}

pub(crate) fn cmd_ls(format: OutputFormat) -> Result<()> {
    let format = super::config::resolve_output(&super::util::state_root(), format);
    let rows: Vec<LsRow> = address_list()
        .into_iter()
        .map(|a| LsRow {
            stack: a.labels.get(super::reconcile::STACK_LABEL).cloned(),
            name: a.name,
            address: a.address,
            pool: a.pool,
            owner: a.owner,
        })
        .collect();
    if format == OutputFormat::Json {
        return super::output::print_json(&rows);
    }
    let mut t = super::output::Table::new(&["NAME", "ADDRESS", "POOL", "OWNER", "STACK"]);
    for r in &rows {
        t.row(vec![
            r.name.clone(),
            r.address.clone(),
            r.pool.clone(),
            r.owner.clone().unwrap_or_else(|| "-".into()),
            r.stack.clone().unwrap_or_else(|| "-".into()),
        ]);
    }
    t.drop_uninformative().print();
    Ok(())
}

pub(crate) fn cmd_describe(names: &[String]) -> Result<()> {
    for name in names {
        let Some(a) = address_get(name) else {
            return Err(Error::NotFound(format!("ipaddress: {name}")));
        };
        let mut d = super::output::Describe::new();
        d.field("Name", &a.name);
        d.field("Address", &a.address);
        d.field("Pool", &a.pool);
        d.field_opt("Owner", a.owner.as_ref());
        d.field_opt("Stack", a.labels.get(super::reconcile::STACK_LABEL));
        d.print();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(addrs: &[&str]) -> super::super::ippool::PoolFile {
        super::super::ippool::PoolFile {
            name: "edge".into(),
            addresses: addrs.iter().map(|s| s.to_string()).collect(),
            announce: "local".into(),
            ..Default::default()
        }
    }

    #[test]
    fn a_cidr_pool_gives_the_containing_cidr_as_the_prefix() {
        let p = pool(&["198.51.100.0/24"]);
        assert_eq!(
            pool_prefix_for(&p, "198.51.100.42".parse().unwrap()),
            "198.51.100.0/24"
        );
    }

    #[test]
    fn a_range_pool_falls_back_to_the_address_natural_24() {
        let p = pool(&["203.0.113.10-203.0.113.20"]);
        assert_eq!(
            pool_prefix_for(&p, "203.0.113.15".parse().unwrap()),
            "203.0.113.0/24"
        );
    }

    #[test]
    fn a_non_ipv4_address_is_refused_by_reason() {
        let e = parse_address("2001:db8::1").unwrap_err().to_string();
        assert!(e.contains("IPv4"), "{e}");
    }
}
