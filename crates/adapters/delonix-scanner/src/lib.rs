//! `delonix-scanner` — image vulnerability scanner (cloud-native).
//!
//! Two operations, without root and without running the image:
//! 1. **SBOM** — extracts the list of installed packages (Alpine `apk`, Debian/Ubuntu
//!    `dpkg`) by reading the *layers* directly from the CAS (without mounting anything);
//! 2. **match** — compares the SBOM against a database of *advisories* (CVE) and reports
//!    the vulnerable packages, with severity.
//!
//! Designed to run **before the build** (analyzing the `FROM` base) and on **already
//! existing images** — exactly like `trivy`/`grype`, but embedded in the engine.

use delonix_oci::{Image, ImageStore};
use serde::{Deserialize, Serialize};
use std::io::Read;

mod error;
pub mod pytree;

pub use error::{Error, Result};

/// The package manager that registered a package (determines the advisory source).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ecosystem {
    /// Alpine (`/lib/apk/db/installed`).
    Apk,
    /// Debian/Ubuntu (`/var/lib/dpkg/status`).
    Dpkg,
    /// Python/PyPI (`requirements.txt` of modules — e.g. Odoo modules).
    PyPi,
}

/// A package installed in the image.
#[derive(Serialize, Clone, Debug)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub ecosystem: Ecosystem,
}

/// Severity of a vulnerability (orderable for CI *gates*).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    /// Parses `"low"|"medium"|"high"|"critical"` (for `--fail-on`).
    pub fn parse(s: &str) -> Option<Severity> {
        match s.to_ascii_lowercase().as_str() {
            "low" => Some(Severity::Low),
            "medium" | "med" => Some(Severity::Medium),
            "high" => Some(Severity::High),
            "critical" | "crit" => Some(Severity::Critical),
            _ => None,
        }
    }
}

/// An *advisory* (CVE): "package X in ecosystem Y is vulnerable below
/// version `fixed`".
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Advisory {
    pub id: String,
    pub package: String,
    pub ecosystem: Ecosystem,
    /// First FIXED version: the image is vulnerable if `version < fixed`.
    pub fixed: String,
    pub severity: Severity,
    pub summary: String,
}

/// A vulnerability found in the image.
#[derive(Serialize, Clone, Debug)]
pub struct Finding {
    pub id: String,
    pub package: String,
    pub version: String,
    pub fixed: String,
    pub severity: Severity,
    pub summary: String,
}

// ---------------------------------------------------------------------------
// SBOM — extract packages from the layers (reading the CAS, without mounting or running)
// ---------------------------------------------------------------------------

/// Reads a *layer* (tar, optionally gzip) and returns the content of the first
/// file whose path ends in `suffix`, if any.
fn read_member(blob: &[u8], suffix: &str) -> Option<Vec<u8>> {
    let is_gzip = blob.len() >= 2 && blob[0] == 0x1f && blob[1] == 0x8b;
    let reader: Box<dyn Read> = if is_gzip {
        Box::new(flate2::read::GzDecoder::new(blob))
    } else {
        Box::new(blob)
    };
    let mut archive = tar::Archive::new(reader);
    for entry in archive.entries().ok()? {
        let mut entry = entry.ok()?;
        let path = entry.path().ok()?.to_string_lossy().into_owned();
        if path.trim_start_matches("./").ends_with(suffix) {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).ok()?;
            return Some(buf);
        }
    }
    None
}

/// Packages from an `apk` database (`P:` name, `V:` version, records separated
/// by a blank line).
fn parse_apk(db: &str) -> Vec<Package> {
    let mut out = Vec::new();
    let mut name = None;
    for line in db.lines() {
        if let Some(n) = line.strip_prefix("P:") {
            name = Some(n.trim().to_string());
        } else if let Some(v) = line.strip_prefix("V:") {
            if let Some(n) = name.take() {
                out.push(Package {
                    name: n,
                    version: v.trim().to_string(),
                    ecosystem: Ecosystem::Apk,
                });
            }
        }
    }
    out
}

/// Packages from a `dpkg/status` (stanzas with `Package:` and `Version:`).
fn parse_dpkg(db: &str) -> Vec<Package> {
    let mut out = Vec::new();
    let mut name = None;
    for line in db.lines() {
        if let Some(n) = line.strip_prefix("Package:") {
            name = Some(n.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Version:") {
            if let Some(n) = name.clone() {
                out.push(Package {
                    name: n,
                    version: v.trim().to_string(),
                    ecosystem: Ecosystem::Dpkg,
                });
            }
        } else if line.is_empty() {
            name = None;
        }
    }
    out
}

/// Extracts the SBOM of an image, reading the *layers* from the top down to the
/// base (the most recent package database wins).
pub fn extract_sbom(images: &ImageStore, image: &Image) -> Result<Vec<Package>> {
    let mut apk_db: Option<String> = None;
    let mut dpkg_db: Option<String> = None;
    for digest in image.layers.iter().rev() {
        let blob = images.cas().read(digest)?;
        if apk_db.is_none() {
            if let Some(b) = read_member(&blob, "lib/apk/db/installed") {
                apk_db = Some(String::from_utf8_lossy(&b).into_owned());
            }
        }
        if dpkg_db.is_none() {
            if let Some(b) = read_member(&blob, "var/lib/dpkg/status") {
                dpkg_db = Some(String::from_utf8_lossy(&b).into_owned());
            }
        }
    }
    let mut pkgs = Vec::new();
    if let Some(db) = apk_db {
        pkgs.extend(parse_apk(&db));
    }
    if let Some(db) = dpkg_db {
        pkgs.extend(parse_dpkg(&db));
    }
    if pkgs.is_empty() {
        return Err(Error::EmptySbom);
    }
    pkgs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(pkgs)
}

// ---------------------------------------------------------------------------
// SBOM as a document (SPDX 2.3) — M03 of the 13-improvements programme
// ---------------------------------------------------------------------------

/// SPDX requires unique ids over a restricted alphabet: letters, digits, `.`,
/// `-`. A package name with `+`/`:` (there are some, in `dpkg`'s epoch-bearing
/// versions) would otherwise produce an invalid id, and a validator rejects
/// the whole document over it. Same rule `scripts/sbom.py` already applies to
/// the engine's own SBOM — kept identical so the two documents this project
/// publishes read the same way.
fn spdx_safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// The `purl` (package URL) type for an ecosystem — the field a consumer
/// actually queries a vulnerability database with.
fn purl(pkg: &Package) -> String {
    let ecosystem = match pkg.ecosystem {
        Ecosystem::Apk => "apk/alpine",
        Ecosystem::Dpkg => "deb",
        Ecosystem::PyPi => "pypi",
    };
    format!("pkg:{ecosystem}/{}@{}", pkg.name, pkg.version)
}

/// Builds an SPDX 2.3 document for an image's SBOM — the same shape
/// `scripts/sbom.py` already gives the engine's OWN dependency tree, so the
/// two SBOMs this project produces (the binary's, the image's) read
/// identically to whatever already consumes the first one.
///
/// **Promises** the packages this store can already see: `apk`/`dpkg`
/// entries read straight from the layers, name+version+ecosystem, with a
/// `purl` a vulnerability database can query. **Does not promise** licence
/// data (`NOASSERTION` — nothing here reads a licence file) nor a signed
/// attestation of HOW the image was built; that is a SEPARATE, harder
/// problem (SLSA-style provenance needs a signing key, the way `image sign`
/// already has one for the image itself) and is not something a package
/// listing can honestly claim on its own.
///
/// Deterministic on purpose, like the engine's own SBOM: no generation
/// timestamp (`created` is fixed), so building the SAME image twice gives
/// the SAME SBOM byte for byte — the document namespace is derived from the
/// image id instead, which already changes when the content does.
pub fn sbom_spdx(image: &Image, packages: &[Package]) -> serde_json::Value {
    use serde_json::json;

    let name = image
        .repo_tags
        .first()
        .cloned()
        .unwrap_or_else(|| image.short_id());
    let root_id = format!("SPDXRef-Package-{}", spdx_safe(&image.short_id()));

    let mut spdx_packages = vec![json!({
        "SPDXID": root_id,
        "name": name,
        "versionInfo": image.id,
        "downloadLocation": "NOASSERTION",
        "licenseConcluded": "NOASSERTION",
        "licenseDeclared": "NOASSERTION",
        "filesAnalyzed": false,
    })];
    let mut relationships = vec![json!({
        "spdxElementId": "SPDXRef-DOCUMENT",
        "relationshipType": "DESCRIBES",
        "relatedSpdxElement": root_id,
    })];

    for p in packages {
        let pid = format!(
            "SPDXRef-Package-{}-{}",
            spdx_safe(&p.name),
            spdx_safe(&p.version)
        );
        spdx_packages.push(json!({
            "SPDXID": pid,
            "name": p.name,
            "versionInfo": p.version,
            "downloadLocation": "NOASSERTION",
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
            "filesAnalyzed": false,
            "externalRefs": [{
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": purl(p),
            }],
        }));
        relationships.push(json!({
            "spdxElementId": root_id,
            "relationshipType": "CONTAINS",
            "relatedSpdxElement": pid,
        }));
    }

    json!({
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": format!("{name}-sbom"),
        "documentNamespace": format!(
            "https://github.com/ngolacloud/delonix/image-sbom/{}",
            image.short_id()
        ),
        "creationInfo": {
            "creators": ["Tool: delonix-scanner", "Organization: ngolacloud"],
            "created": "1970-01-01T00:00:00Z",
            "comment": "Generated by reading the image's own layers (apk/dpkg package \
                databases), without mounting or running it. Covers installed OS packages \
                only — does not cover application-level dependencies (pip/npm/…) unless \
                they were captured by a package manager this scanner reads, and claims no \
                signed build provenance.",
        },
        "packages": spdx_packages,
        "relationships": relationships,
    })
}

// ---------------------------------------------------------------------------
// Version comparison (apk/dpkg style, simplified)
// ---------------------------------------------------------------------------

/// Splits a version into alternating sequences of digits / non-digits.
fn tokens(v: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut digit = false;
    for c in v.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_digit() != digit && !cur.is_empty() {
                out.push((digit, std::mem::take(&mut cur)));
            }
            digit = c.is_ascii_digit();
            cur.push(c);
        } else if !cur.is_empty() {
            out.push((digit, std::mem::take(&mut cur)));
            cur.clear();
        }
    }
    if !cur.is_empty() {
        out.push((digit, cur));
    }
    out
}

/// `true` if `a < b` (version comparison). Numeric tokens compare as
/// numbers; the rest, lexicographically; whoever ends first is "smaller".
pub fn version_lt(a: &str, b: &str) -> bool {
    let (ta, tb) = (tokens(a), tokens(b));
    for i in 0..ta.len().max(tb.len()) {
        match (ta.get(i), tb.get(i)) {
            (None, Some(_)) => return true,  // a ended -> a < b
            (Some(_), None) => return false, // b ended -> a > b
            (Some((da, sa)), Some((db, sb))) => {
                let ord = if *da && *db {
                    sa.parse::<u64>()
                        .unwrap_or(0)
                        .cmp(&sb.parse::<u64>().unwrap_or(0))
                } else {
                    sa.cmp(sb)
                };
                if ord != std::cmp::Ordering::Equal {
                    return ord == std::cmp::Ordering::Less;
                }
            }
            (None, None) => break,
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Advisory database + matching
// ---------------------------------------------------------------------------

/// The *advisory* database (loaded from JSON; in production it would sync from
/// OSV / NVD / Alpine secdb).
pub struct AdvisoryDb {
    advisories: Vec<Advisory>,
}

impl AdvisoryDb {
    /// Loads the database from JSON (`[ {id, package, ecosystem, fixed, ...} ]`).
    pub fn load(json: &str) -> Result<Self> {
        let advisories: Vec<Advisory> = serde_json::from_str(json).map_err(Error::AdvisoryDb)?;
        Ok(Self { advisories })
    }

    /// Number of loaded advisories.
    pub fn len(&self) -> usize {
        self.advisories.len()
    }
    pub fn is_empty(&self) -> bool {
        self.advisories.is_empty()
    }

    /// Cross-references the SBOM with the advisories: a package is vulnerable if
    /// there is an advisory of the same ecosystem/name with `version < fixed`.
    pub fn scan(&self, sbom: &[Package]) -> Vec<Finding> {
        let mut out = Vec::new();
        for adv in &self.advisories {
            for pkg in sbom {
                if pkg.ecosystem == adv.ecosystem
                    && pkg.name == adv.package
                    && version_lt(&pkg.version, &adv.fixed)
                {
                    out.push(Finding {
                        id: adv.id.clone(),
                        package: pkg.name.clone(),
                        version: pkg.version.clone(),
                        fixed: adv.fixed.clone(),
                        severity: adv.severity,
                        summary: adv.summary.clone(),
                    });
                }
            }
        }
        // most severe first
        out.sort_by_key(|f| std::cmp::Reverse(f.severity));
        out
    }
}

// ---------------------------------------------------------------------------
// Ingestion of real feeds (OSV) + database staleness
// ---------------------------------------------------------------------------

/// Maps the OSV `ecosystem` (e.g. `Alpine:v3.18`, `Debian:11`, `Ubuntu:22.04`)
/// to the package ecosystem we know how to cross-reference. Returns `None` for
/// the ones we don't support (npm, Go, …) — they are ignored, not turned into junk.
fn map_osv_ecosystem(s: &str) -> Option<Ecosystem> {
    if s.starts_with("Alpine") {
        Some(Ecosystem::Apk)
    } else if s.starts_with("Debian") || s.starts_with("Ubuntu") {
        Some(Ecosystem::Dpkg)
    } else if s == "PyPI" {
        Some(Ecosystem::PyPi)
    } else {
        None
    }
}

/// Extracts a severity from an OSV object (vuln or affected) from the
/// `database_specific.severity` / `ecosystem_specific.severity` (string). The CVSS
/// vector is NOT interpreted (it would require computing the score) — anything
/// without a label falls back to the caller's default.
fn osv_severity_label(obj: &serde_json::Value) -> Option<Severity> {
    for key in ["database_specific", "ecosystem_specific"] {
        if let Some(s) = obj
            .get(key)
            .and_then(|d| d.get("severity"))
            .and_then(|v| v.as_str())
        {
            if let Some(sev) = Severity::parse(s) {
                return Some(sev);
            }
        }
    }
    None
}

/// First FIXED version (`{"fixed": …}`) in the `ranges[].events[]` of an
/// OSV `affected` object.
fn osv_first_fixed(affected: &serde_json::Value) -> Option<String> {
    for range in affected.get("ranges")?.as_array()? {
        let Some(events) = range.get("events").and_then(|e| e.as_array()) else {
            continue;
        };
        for ev in events {
            if let Some(f) = ev.get("fixed").and_then(|v| v.as_str()) {
                return Some(f.to_string());
            }
        }
    }
    None
}

/// Converts ONE OSV vuln object into advisories — one per affected package
/// with a first-fixed version, for the ecosystems we cross-reference (the rest
/// ignored) — appending to `out`. Shared by the array/`{vulns}` feed path and
/// the per-file zip path, so the two feed shapes map a record identically.
fn push_osv_vuln(vuln: &serde_json::Value, out: &mut Vec<Advisory>) {
    let id = vuln.get("id").and_then(|x| x.as_str()).unwrap_or("");
    if id.is_empty() {
        return;
    }
    let summary = vuln
        .get("summary")
        .and_then(|x| x.as_str())
        .or_else(|| vuln.get("details").and_then(|x| x.as_str()))
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    let vuln_sev = osv_severity_label(vuln);
    let Some(affected) = vuln.get("affected").and_then(|x| x.as_array()) else {
        return;
    };
    for aff in affected {
        let Some(pkg) = aff.get("package") else {
            continue;
        };
        let name = pkg.get("name").and_then(|x| x.as_str()).unwrap_or("");
        let eco = pkg.get("ecosystem").and_then(|x| x.as_str()).unwrap_or("");
        let (Some(ecosystem), false) = (map_osv_ecosystem(eco), name.is_empty()) else {
            continue;
        };
        let Some(fixed) = osv_first_fixed(aff) else {
            continue;
        };
        let severity = osv_severity_label(aff)
            .or(vuln_sev)
            .unwrap_or(Severity::Medium);
        out.push(Advisory {
            id: id.to_string(),
            package: name.to_string(),
            ecosystem,
            fixed,
            severity,
            summary: summary.clone(),
        });
    }
}

/// Converts an **OSV** feed (an array of vulns, or `{"vulns":[…]}`) into the
/// internal advisory database. Best-effort: each vuln generates one advisory per
/// (package, ecosystem, first-fixed-version) for the supported ecosystems
/// (Alpine→apk, Debian/Ubuntu→dpkg); the rest are ignored. Without a severity
/// label, it assumes `Medium`. Pure function — testable without the network.
pub fn advisories_from_osv(json: &str) -> Result<Vec<Advisory>> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(Error::OsvNotJson)?;
    let vulns = if let Some(arr) = v.as_array() {
        arr.clone()
    } else if let Some(arr) = v.get("vulns").and_then(|x| x.as_array()) {
        arr.clone()
    } else {
        return Err(Error::OsvShape);
    };
    let mut out = Vec::new();
    for vuln in &vulns {
        push_osv_vuln(vuln, &mut out);
    }
    Ok(out)
}

/// Minimal reader for the ZIP container OSV distributes its per-ecosystem
/// export as (`<ecosystem>/all.zip`: one JSON record per vuln). Only the two
/// things that archive actually uses are handled — STORED (0) and DEFLATE (8)
/// — read through the CENTRAL DIRECTORY, whose sizes and offsets are always
/// correct (a local header may defer them to a trailing data descriptor).
/// No zip64: OSV's exports are a few thousand small files, far under the
/// u16/u32 limits the base format carries; a zip that needs zip64 is refused
/// rather than mis-read. Returns the decompressed bytes of every `.json`
/// member. Zero new dependencies — the deflate decoder is the `flate2` the
/// crate already uses for layers. Pure (no network, no fs), so the whole
/// OSV→advisory path stays testable without either.
fn zip_json_members(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    fn u16le(b: &[u8], o: usize) -> Option<usize> {
        Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]) as usize)
    }
    fn u32le(b: &[u8], o: usize) -> Option<usize> {
        Some(
            u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?])
                as usize,
        )
    }
    let bad = |m: &str| Error::OsvZip(m.to_string());

    // End of Central Directory record (signature PK\x05\x06), 22 bytes plus an
    // optional trailing comment — scan backwards for it over the last 64 KiB.
    const EOCD: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    if data.len() < 22 {
        return Err(bad("too small to be a zip"));
    }
    let scan_from = data.len().saturating_sub(22 + 0xffff);
    let eocd = (scan_from..=data.len() - 22)
        .rev()
        .find(|&i| data[i..i + 4] == EOCD)
        .ok_or_else(|| bad("no end-of-central-directory record (not a zip?)"))?;
    let count = u16le(data, eocd + 10).ok_or_else(|| bad("truncated EOCD"))?;
    let cd_off = u32le(data, eocd + 16).ok_or_else(|| bad("truncated EOCD"))?;
    if count == 0xffff || cd_off == 0xffff_ffff {
        return Err(bad("zip64 is not supported"));
    }

    let mut out = Vec::with_capacity(count);
    let mut p = cd_off;
    for _ in 0..count {
        // Central-directory file header (signature PK\x01\x02), 46 bytes fixed.
        if data.get(p..p + 4) != Some(&[0x50, 0x4b, 0x01, 0x02]) {
            return Err(bad("bad central-directory header"));
        }
        let method = u16le(data, p + 10).ok_or_else(|| bad("truncated CD header"))?;
        let comp_size = u32le(data, p + 20).ok_or_else(|| bad("truncated CD header"))?;
        let name_len = u16le(data, p + 28).ok_or_else(|| bad("truncated CD header"))?;
        let extra_len = u16le(data, p + 30).ok_or_else(|| bad("truncated CD header"))?;
        let comment_len = u16le(data, p + 32).ok_or_else(|| bad("truncated CD header"))?;
        let lho = u32le(data, p + 42).ok_or_else(|| bad("truncated CD header"))?;
        let name = data
            .get(p + 46..p + 46 + name_len)
            .ok_or_else(|| bad("truncated file name"))?;
        let is_json = name.ends_with(b".json");
        p += 46 + name_len + extra_len + comment_len;
        if !is_json {
            continue;
        }

        // Local file header (signature PK\x03\x04), 30 bytes fixed, then its
        // OWN name+extra (the extra length can differ from the CD's), then the
        // file data of `comp_size` bytes.
        if data.get(lho..lho + 4) != Some(&[0x50, 0x4b, 0x03, 0x04]) {
            return Err(bad("bad local file header"));
        }
        let lname = u16le(data, lho + 26).ok_or_else(|| bad("truncated local header"))?;
        let lextra = u16le(data, lho + 28).ok_or_else(|| bad("truncated local header"))?;
        let start = lho + 30 + lname + lextra;
        let comp = data
            .get(start..start + comp_size)
            .ok_or_else(|| bad("truncated file data"))?;
        let bytes = match method {
            0 => comp.to_vec(),
            8 => {
                let mut buf = Vec::new();
                flate2::read::DeflateDecoder::new(comp)
                    .read_to_end(&mut buf)
                    .map_err(|e| bad(&format!("deflate failed: {e}")))?;
                buf
            }
            m => return Err(bad(&format!("unsupported compression method {m}"))),
        };
        out.push(bytes);
    }
    Ok(out)
}

/// Converts an **OSV ZIP export** (`<ecosystem>/all.zip`, one JSON record per
/// `.json` member) into advisories. A member that is not valid JSON, or not an
/// OSV record, is skipped — one bad file in a 1700-file export must not sink
/// the whole sync; a wholly unreadable zip (not the files, the container) is
/// the hard error [`Error::OsvZip`]. Pure function.
pub fn advisories_from_osv_zip(zip: &[u8]) -> Result<Vec<Advisory>> {
    let members = zip_json_members(zip)?;
    let mut out = Vec::new();
    for raw in &members {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(raw) {
            push_osv_vuln(&v, &mut out);
        }
    }
    Ok(out)
}

/// `true` if the advisory database is STALE (synced more than `max_age_days`
/// ago, or never synced). Used to warn that a scan with no findings may not
/// be trustworthy. Pure function.
pub fn db_is_stale(synced_unix: Option<u64>, now_unix: u64, max_age_days: u64) -> bool {
    match synced_unix {
        None => true,
        Some(t) => now_unix.saturating_sub(t) > max_age_days.saturating_mul(86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(id: &str, tags: &[&str]) -> Image {
        Image {
            id: id.to_string(),
            repo_tags: tags.iter().map(|t| t.to_string()).collect(),
            layers: vec![],
            config: delonix_oci::ImageConfig::default(),
            created_unix: 0,
        }
    }

    #[test]
    fn sbom_spdx_names_every_package_with_a_purl() {
        let img = image("sha256:abc123", &["myapp:latest"]);
        let pkgs = vec![
            Package {
                name: "busybox".into(),
                version: "1.36.1-r20".into(),
                ecosystem: Ecosystem::Apk,
            },
            Package {
                name: "libc6".into(),
                version: "2.36-9".into(),
                ecosystem: Ecosystem::Dpkg,
            },
        ];
        let doc = sbom_spdx(&img, &pkgs);
        assert_eq!(doc["spdxVersion"], "SPDX-2.3");
        let packages = doc["packages"].as_array().unwrap();
        // The image itself, plus the two packages.
        assert_eq!(packages.len(), 3);
        let purls: Vec<&str> = packages
            .iter()
            .filter_map(|p| p["externalRefs"][0]["referenceLocator"].as_str())
            .collect();
        assert_eq!(
            purls,
            vec!["pkg:apk/alpine/busybox@1.36.1-r20", "pkg:deb/libc6@2.36-9"]
        );
    }

    #[test]
    fn sbom_spdx_is_deterministic_for_the_same_image() {
        let img = image("sha256:same", &["x:1"]);
        let pkgs = vec![Package {
            name: "a".into(),
            version: "1".into(),
            ecosystem: Ecosystem::Apk,
        }];
        assert_eq!(sbom_spdx(&img, &pkgs), sbom_spdx(&img, &pkgs));
    }

    #[test]
    fn sbom_spdx_falls_back_to_the_short_id_when_untagged() {
        let img = image(
            "sha256:deadbeefcafebabe0000000000000000000000000000000000000000000000",
            &[],
        );
        let doc = sbom_spdx(&img, &[]);
        assert_eq!(doc["packages"][0]["name"], "deadbeefcafe");
    }

    #[test]
    fn spdx_safe_replaces_characters_outside_the_allowed_alphabet() {
        assert_eq!(spdx_safe("libssl1.1:amd64"), "libssl1.1-amd64");
        assert_eq!(spdx_safe("pkg+epoch:1.0"), "pkg-epoch-1.0");
    }

    #[test]
    fn version_comparison() {
        assert!(version_lt("1.36.1-r20", "1.37.0")); // busybox vulnerable
        assert!(!version_lt("1.3.1-r0", "1.3")); // zlib already fixed
        assert!(version_lt("3.1.7", "3.1.8"));
        assert!(!version_lt("3.1.8-r1", "3.1.8"));
        assert!(version_lt("2.36-9", "2.37"));
    }

    #[test]
    fn parses_apk_db() {
        let db = "C:Q1x\nP:busybox\nV:1.36.1-r20\nA:x86_64\n\nP:zlib\nV:1.3.1-r0\n";
        let pkgs = parse_apk(db);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].name, "busybox");
        assert_eq!(pkgs[0].version, "1.36.1-r20");
    }

    #[test]
    fn parses_dpkg_status() {
        let db = "Package: libc6\nStatus: install ok installed\nVersion: 2.36-9\n\nPackage: bash\nVersion: 5.2-15\n";
        let pkgs = parse_dpkg(db);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[1].name, "bash");
    }

    #[test]
    fn matches_only_vulnerable() {
        let sbom = vec![
            Package {
                name: "busybox".into(),
                version: "1.36.1-r20".into(),
                ecosystem: Ecosystem::Apk,
            },
            Package {
                name: "zlib".into(),
                version: "1.3.1-r0".into(),
                ecosystem: Ecosystem::Apk,
            },
        ];
        let db = AdvisoryDb::load(
            r#"[
              {"id":"CVE-A","package":"busybox","ecosystem":"Apk","fixed":"1.37.0","severity":"high","summary":"x"},
              {"id":"CVE-B","package":"zlib","ecosystem":"Apk","fixed":"1.3","severity":"medium","summary":"y"}
            ]"#,
        )
        .unwrap();
        let f = db.scan(&sbom);
        assert_eq!(f.len(), 1); // only busybox
        assert_eq!(f[0].id, "CVE-A");
    }

    #[test]
    fn osv_converte_alpine_e_ignora_ecossistemas_nao_suportados() {
        // An OSV feed: one Alpine vuln (convertible) + one npm (ignored).
        let feed = r#"{"vulns":[
          {
            "id":"CVE-2023-4567","summary":"busybox oops",
            "database_specific":{"severity":"HIGH"},
            "affected":[{
              "package":{"ecosystem":"Alpine:v3.18","name":"busybox"},
              "ranges":[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"1.36.1-r5"}]}]
            }]
          },
          {
            "id":"GHSA-xxxx","summary":"left-pad",
            "affected":[{"package":{"ecosystem":"npm","name":"left-pad"},
              "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.3.0"}]}]}]
          }
        ]}"#;
        let advs = advisories_from_osv(feed).unwrap();
        assert_eq!(advs.len(), 1); // only the Alpine one
        assert_eq!(advs[0].id, "CVE-2023-4567");
        assert_eq!(advs[0].package, "busybox");
        assert_eq!(advs[0].ecosystem, Ecosystem::Apk);
        assert_eq!(advs[0].fixed, "1.36.1-r5");
        assert_eq!(advs[0].severity, Severity::High);
    }

    #[test]
    fn osv_aceita_array_e_default_medium_sem_rotulo() {
        let feed = r#"[{
          "id":"CVE-1","details":"debian glibc",
          "affected":[{"package":{"ecosystem":"Debian:12","name":"libc6"},
            "ranges":[{"type":"ECOSYSTEM","events":[{"fixed":"2.36-9+deb12u1"}]}]}]
        }]"#;
        let advs = advisories_from_osv(feed).unwrap();
        assert_eq!(advs.len(), 1);
        assert_eq!(advs[0].ecosystem, Ecosystem::Dpkg);
        assert_eq!(advs[0].severity, Severity::Medium); // no label → default
                                                        // the converted advisories cross-reference against a real SBOM.
        let db = AdvisoryDb::load(&serde_json::to_string(&advs_json(&advs)).unwrap()).unwrap();
        let sbom = vec![Package {
            name: "libc6".into(),
            version: "2.36-9".into(),
            ecosystem: Ecosystem::Dpkg,
        }];
        assert_eq!(db.scan(&sbom).len(), 1);
    }

    // helper: Advisory is not Serialize; rebuilds the JSON for the round-trip.
    fn advs_json(advs: &[Advisory]) -> Vec<serde_json::Value> {
        advs.iter().map(|a| serde_json::json!({
            "id": a.id, "package": a.package,
            "ecosystem": match a.ecosystem { Ecosystem::Apk => "Apk", Ecosystem::Dpkg => "Dpkg", Ecosystem::PyPi => "PyPi" },
            "fixed": a.fixed,
            "severity": match a.severity { Severity::Low=>"low", Severity::Medium=>"medium", Severity::High=>"high", Severity::Critical=>"critical" },
            "summary": a.summary,
        })).collect()
    }

    /// Builds a minimal ZIP of `(name, bytes)` members — STORED (`deflate =
    /// false`) or DEFLATE (`true`, the method OSV's exports actually use) — so
    /// the zip path is tested without the network and without a `zip`
    /// dependency. CRCs are left zero: the reader does not check them.
    fn build_zip(files: &[(&str, &[u8])], deflate: bool) -> Vec<u8> {
        use std::io::Write;
        let mut out = Vec::new();
        let mut central = Vec::new();
        let mut local_offsets = Vec::new();
        let (method, flag): (u16, u16) = if deflate { (8, 1 << 3) } else { (0, 0) };
        let _ = flag; // flags left 0 below; data-descriptor flag not used
        for (name, data) in files {
            local_offsets.push(out.len() as u32);
            let stored: Vec<u8> = if deflate {
                let mut e =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(data).unwrap();
                e.finish().unwrap()
            } else {
                data.to_vec()
            };
            // local file header
            out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
            out.extend_from_slice(&[20, 0, 0, 0]); // version needed + flags
            out.extend_from_slice(&method.to_le_bytes());
            out.extend_from_slice(&[0, 0, 0, 0]); // time + date
            out.extend_from_slice(&[0, 0, 0, 0]); // crc32 (unchecked)
            out.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&[0, 0]); // extra len
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&stored);
            // central-directory header, for the same member
            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            central.extend_from_slice(&[20, 0, 20, 0, 0, 0]); // made-by + needed + flags
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0]); // time + date
            central.extend_from_slice(&[0, 0, 0, 0]); // crc32
            central.extend_from_slice(&(stored.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // extra + comment + disk
            central.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // internal + external attrs
            central.extend_from_slice(local_offsets.last().unwrap().to_le_bytes().as_ref());
            central.extend_from_slice(name.as_bytes());
        }
        let cd_off = out.len() as u32;
        let cd_size = central.len() as u32;
        out.extend_from_slice(&central);
        // end of central directory
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        out.extend_from_slice(&[0, 0, 0, 0]); // this disk + cd-start disk
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&(files.len() as u16).to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0, 0]); // comment len
        out
    }

    const OSV_ALPINE: &str = r#"{"id":"CVE-2023-4567","summary":"busybox oops",
      "database_specific":{"severity":"HIGH"},
      "affected":[{"package":{"ecosystem":"Alpine:v3.18","name":"busybox"},
        "ranges":[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"1.36.1-r5"}]}]}]}"#;
    const OSV_NPM: &str = r#"{"id":"GHSA-xxxx","summary":"left-pad",
      "affected":[{"package":{"ecosystem":"npm","name":"left-pad"},
        "ranges":[{"type":"SEMVER","events":[{"fixed":"1.3.0"}]}]}]}"#;

    #[test]
    fn osv_zip_stored_converte_membros_json_e_ignora_nao_json() {
        let zip = build_zip(
            &[
                ("CVE-2023-4567.json", OSV_ALPINE.as_bytes()),
                ("GHSA-xxxx.json", OSV_NPM.as_bytes()), // npm → ignored by the mapper
                ("README.txt", b"not json, not a .json member"),
            ],
            false,
        );
        let advs = advisories_from_osv_zip(&zip).unwrap();
        assert_eq!(advs.len(), 1); // only the Alpine one
        assert_eq!(advs[0].id, "CVE-2023-4567");
        assert_eq!(advs[0].package, "busybox");
        assert_eq!(advs[0].ecosystem, Ecosystem::Apk);
        assert_eq!(advs[0].fixed, "1.36.1-r5");
        assert_eq!(advs[0].severity, Severity::High);
    }

    #[test]
    fn osv_zip_deflate_le_o_metodo_que_o_osv_usa() {
        // DEFLATE (method 8) is what the real `all.zip` uses; the stored test
        // above would pass even if the inflate path were broken.
        let zip = build_zip(&[("CVE-2023-4567.json", OSV_ALPINE.as_bytes())], true);
        let advs = advisories_from_osv_zip(&zip).unwrap();
        assert_eq!(advs.len(), 1);
        assert_eq!(advs[0].fixed, "1.36.1-r5");
    }

    #[test]
    fn osv_zip_recusa_o_que_nao_e_zip() {
        // A JSON feed, an HTML error page, an empty body: none is a zip, and
        // the reader says so (it does not silently return zero advisories).
        for bad in [&b"not a zip at all"[..], b"<html>502</html>", b"", b"PK"] {
            assert!(
                matches!(advisories_from_osv_zip(bad), Err(Error::OsvZip(_))),
                "{bad:?} should be OsvZip"
            );
        }
    }

    /// The CI build-failing gate (G1c): the scanner, exercised END TO END on a
    /// REAL image layer (a gzipped tar written to the CAS, read back by
    /// `extract_sbom` — not a hand-built `Package` list), must catch a known
    /// CVE in a known-vulnerable base. If a refactor ever breaks layer reading,
    /// apk parsing, or matching, `cargo test --workspace` fails here and the
    /// build stops — which is exactly "an image scan in CI that fails the build
    /// on a known CVE". The `--fail-on high` decision that turns this into a
    /// non-zero exit is covered by the CLI's own `admission_rejects` test.
    #[test]
    fn image_scan_catches_a_planted_cve_in_a_real_layer() {
        use std::io::Write;

        // A real layer: an apk database with the busybox alpine:3.12 ships
        // (1.31.1-r22), known-vulnerable to CVE-2022-48174 (fixed 1.37.0).
        let apk_db = "C:Q1x\nP:busybox\nV:1.31.1-r22\nA:x86_64\n\nP:zlib\nV:1.2.12-r0\n";
        let mut tar_buf = Vec::new();
        {
            let mut ar = tar::Builder::new(&mut tar_buf);
            let mut header = tar::Header::new_gnu();
            header.set_path("lib/apk/db/installed").unwrap();
            header.set_size(apk_db.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            ar.append(&header, apk_db.as_bytes()).unwrap();
            ar.finish().unwrap();
        }
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(&tar_buf).unwrap();
        let layer = gz.finish().unwrap();

        let dir = tempfile::tempdir().unwrap();
        let store = ImageStore::open(dir.path()).unwrap();
        let digest = store.cas().write(&layer).unwrap();
        let img = Image {
            id: "sha256:planted".into(),
            repo_tags: vec!["vuln:test".into()],
            layers: vec![digest],
            config: delonix_oci::ImageConfig::default(),
            created_unix: 0,
        };

        // SBOM read from the layer in the CAS — the path `delonix image scan` runs.
        let sbom = extract_sbom(&store, &img).unwrap();
        assert!(
            sbom.iter()
                .any(|p| p.name == "busybox" && p.version == "1.31.1-r22"),
            "extract_sbom must read busybox out of the real layer"
        );

        // The planted CVE is a real advisory (the shape a synced OSV feed yields).
        let db = AdvisoryDb::load(
            r#"[{"id":"CVE-2022-48174","package":"busybox","ecosystem":"Apk","fixed":"1.37.0","severity":"high","summary":"busybox ash stack overflow"}]"#,
        )
        .unwrap();
        let findings = db.scan(&sbom);
        assert_eq!(findings.len(), 1, "the scan must catch the planted CVE");
        assert_eq!(findings[0].id, "CVE-2022-48174");
        assert_eq!(findings[0].severity, Severity::High);

        // The build-failing decision: the worst severity trips a `--fail-on high` gate.
        let worst = findings.iter().map(|f| f.severity).max().unwrap();
        assert!(worst >= Severity::High);
    }

    #[test]
    fn staleness_deteta_nunca_e_antigo() {
        let dia = 86_400u64;
        assert!(db_is_stale(None, 100 * dia, 30)); // never synced
        assert!(db_is_stale(Some(0), 40 * dia, 30)); // 40 days > 30
        assert!(!db_is_stale(Some(20 * dia), 40 * dia, 30)); // 20 days <= 30
    }
}
