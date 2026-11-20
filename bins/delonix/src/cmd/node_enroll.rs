//! Node identity by enrollment (ADR-0097, Phase A/A2) — the source and the sink
//! of the trust path A1 (ADR-0096) left to the operator.
//!
//! A1 serves the node contract over mutual TLS: a client is admitted only if its
//! certificate chains to a configured `--tls-client-ca`. A1 said nothing about
//! where that certificate comes from or how it is taken away. A2 adds both:
//!
//! - **`node-ca init`** — create a *node CA* (a key + self-signed cert). Its cert
//!   is the trust anchor an operator passes to every server as `--tls-client-ca`;
//!   its key lives only here, never on a node-API server.
//! - **`serve node-enroll`** — the enrollment authority: a server-TLS-only
//!   endpoint (`POST /v1/enroll`) that trades a **one-time token** for a
//!   **short-lived client certificate** signed from the member's **CSR**. The
//!   member pins the node CA to authenticate the authority; the token (not a
//!   client cert — the member has none yet) authenticates the member.
//! - **`node-enroll`** — the member side: generate a key pair + CSR, exchange the
//!   token, write `client.key` (never transmitted) and `client.crt`.
//! - **`node-ca revoke`** — list a certificate's serial in a CRL the node CA
//!   signs; a server loaded with it (`--tls-client-crl`) refuses that certificate
//!   at the handshake.
//!
//! Rotation needs no server restart: the server trusts the CA, so a fresh
//! certificate from the same CA is accepted at once and the old one expires on
//! its short clock. The node CA key is reloaded (`KeyPair::from_pem`) and the
//! issuer reconstructed from the canonical parameters `init` used, so a signed
//! leaf chains to the persisted `node-ca.crt` with no CA-cert parsing (ADR-0097
//! §7). Nothing here touches `proto/delonix/node/v1`.

use std::collections::HashSet;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use delonix_model::{Error, Result};
use rcgen::{
    date_time_ymd, BasicConstraints, CertificateParams, CertificateRevocationListParams,
    CertificateSigningRequestParams, DnType, ExtendedKeyUsagePurpose, IsCa, KeyIdMethod, KeyPair,
    KeyUsagePurpose, RevokedCertParams, SerialNumber,
};
use time::OffsetDateTime;

// ---------------------------------------------------------------------------
// The node CA / enrollment core
// ---------------------------------------------------------------------------

fn rcgen_err(context: &'static str) -> impl Fn(rcgen::Error) -> Error {
    move |e| Error::Runtime {
        context,
        message: e.to_string(),
    }
}

/// The node CA, held LIVE so it can sign CSRs and CRLs in one run. It is either
/// freshly generated ([`NodeCa::generate`]) or reloaded from the CA key
/// ([`NodeCa::reload`]); either way the issuer identity (subject DN, SKI) is the
/// canonical one [`ca_params`] builds, so a leaf it signs chains to the persisted
/// `node-ca.crt`.
pub(crate) struct NodeCa {
    cert: rcgen::Certificate,
    key: KeyPair,
}

/// The canonical CA parameters — IDENTICAL at `generate` and `reload` so the
/// reconstructed issuer matches the persisted certificate (same subject DN, same
/// key ⇒ same SKI with the default SHA-256 method). `keyCertSign + crlSign` make
/// it a CA that may also sign the CRL (`rcgen` refuses a CRL from a non-CRL-signer
/// issuer when key usages are set).
fn ca_params(cn: &str) -> Result<CertificateParams> {
    let mut p =
        CertificateParams::new(Vec::<String>::new()).map_err(rcgen_err("node CA params"))?;
    p.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    p.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    p.distinguished_name.push(DnType::CommonName, cn);
    Ok(p)
}

impl NodeCa {
    /// A new node CA. Returns the live CA plus its `(cert_pem, key_pem)` to persist.
    pub(crate) fn generate(cn: &str) -> Result<(Self, String, String)> {
        let params = ca_params(cn)?;
        let key = KeyPair::generate().map_err(rcgen_err("node CA key"))?;
        let cert = params
            .self_signed(&key)
            .map_err(rcgen_err("node CA cert"))?;
        let cert_pem = cert.pem();
        let key_pem = key.serialize_pem();
        Ok((NodeCa { cert, key }, cert_pem, key_pem))
    }

    /// Reload the CA from its key PEM, reconstructing the issuer from the same
    /// canonical params `generate` used. Only the KEY is parsed — the persisted
    /// cert is not (no `x509-parser`, ADR-0097 §7).
    pub(crate) fn reload(key_pem: &str, cn: &str) -> Result<Self> {
        let key = KeyPair::from_pem(key_pem).map_err(rcgen_err("node CA key (PEM)"))?;
        let params = ca_params(cn)?;
        let cert = params
            .self_signed(&key)
            .map_err(rcgen_err("node CA issuer"))?;
        Ok(NodeCa { cert, key })
    }
}

/// Sign a member's CSR into a SHORT-lived client certificate. The CSR carries the
/// member's public key and subject; everything security-relevant is set HERE, not
/// trusted from the request: `clientAuth` EKU, an explicit random serial (so the
/// certificate can later be revoked by it), and a narrow validity window
/// (`not_before` skewed one minute back for clock drift). Returns the
/// `(cert_pem, serial_hex)`.
pub(crate) fn sign_csr(ca: &NodeCa, csr_pem: &str, ttl: Duration) -> Result<(String, String)> {
    let mut csr = CertificateSigningRequestParams::from_pem(csr_pem)
        .map_err(rcgen_err("client CSR (PEM)"))?;
    let now = now_unix();
    let not_before = offset(now - 60)?;
    let not_after = offset(now + ttl.as_secs() as i64)?;
    let serial = rand_bytes(16)?;
    csr.params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    csr.params.use_authority_key_identifier_extension = true;
    csr.params.not_before = not_before;
    csr.params.not_after = not_after;
    csr.params.serial_number = Some(SerialNumber::from_slice(&serial));
    let cert = csr
        .signed_by(&ca.cert, &ca.key)
        .map_err(rcgen_err("sign client CSR"))?;
    Ok((cert.pem(), hex_encode(&serial)))
}

/// A server certificate for the enrollment authority itself, signed by the node
/// CA so a member that pins `node-ca.crt` authenticates it. `serverAuth` EKU,
/// the given SANs (the addresses members reach it at).
fn issue_server_cert(ca: &NodeCa, sans: &[String]) -> Result<(String, String)> {
    let mut params = CertificateParams::new(sans.to_vec()).map_err(rcgen_err("authority cert"))?;
    params.is_ca = IsCa::NoCa;
    params
        .distinguished_name
        .push(DnType::CommonName, "delonix-node-enroll");
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.use_authority_key_identifier_extension = true;
    let now = now_unix();
    params.not_before = offset(now - 60)?;
    params.not_after = offset(now + 365 * 24 * 3600)?;
    let key = KeyPair::generate().map_err(rcgen_err("authority key"))?;
    let cert = params
        .signed_by(&key, &ca.cert, &ca.key)
        .map_err(rcgen_err("sign authority cert"))?;
    Ok((cert.pem(), key.serialize_pem()))
}

/// A CRL (PEM) signed by the node CA, listing `serials` (raw bytes). `crl_number`
/// is a monotonic sequence number; the validity window brackets «now».
pub(crate) fn build_crl(ca: &NodeCa, serials: &[Vec<u8>], crl_number: u64) -> Result<String> {
    let now = now_unix();
    let params = CertificateRevocationListParams {
        this_update: offset(now - 60)?,
        // A wide ceiling: the CRL stays valid long enough that a server reading it
        // on a cold start never sees it «expired»; a new revoke rewrites it.
        next_update: date_time_ymd(2100, 1, 1),
        crl_number: SerialNumber::from(crl_number),
        issuing_distribution_point: None,
        revoked_certs: serials
            .iter()
            .map(|s| RevokedCertParams {
                serial_number: SerialNumber::from_slice(s),
                revocation_time: offset(now).unwrap_or_else(|_| date_time_ymd(2020, 1, 1)),
                reason_code: None,
                invalidity_date: None,
            })
            .collect(),
        key_identifier_method: KeyIdMethod::Sha256,
    };
    params
        .signed_by(&ca.cert, &ca.key)
        .map_err(rcgen_err("sign CRL"))?
        .pem()
        .map_err(rcgen_err("CRL PEM"))
}

/// A set of enrollment tokens, held as salted SHA-256 hashes, each spendable at
/// most once. `consume` is the one-time gate: it returns true and removes the
/// token the first time, false forever after (ADR-0097 §3). In-memory for the
/// authority's run; persistence across restarts is out of scope for v1.
struct TokenStore {
    salt: Vec<u8>,
    hashes: Mutex<HashSet<[u8; 32]>>,
}

impl TokenStore {
    fn new(tokens: &[String]) -> Result<Self> {
        let salt = rand_bytes(16)?;
        let hashes = tokens.iter().map(|t| hash_token(&salt, t)).collect();
        Ok(TokenStore {
            salt,
            hashes: Mutex::new(hashes),
        })
    }

    fn consume(&self, token: &str) -> bool {
        let h = hash_token(&self.salt, token);
        self.hashes
            .lock()
            .map(|mut s| s.remove(&h))
            .unwrap_or(false)
    }

    fn remaining(&self) -> usize {
        self.hashes.lock().map(|s| s.len()).unwrap_or(0)
    }
}

fn hash_token(salt: &[u8], token: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(salt);
    h.update(token.as_bytes());
    h.finalize().into()
}

// ---------------------------------------------------------------------------
// Small dependency-free helpers (ADR-0097 keeps the crate set unchanged)
// ---------------------------------------------------------------------------

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn offset(unix_secs: i64) -> Result<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp(unix_secs).map_err(|e| Error::Runtime {
        context: "certificate time",
        message: e.to_string(),
    })
}

/// Cryptographically-random bytes from the kernel — no `rand`/`getrandom` crate
/// (Linux, the engine's platform).
fn rand_bytes(n: usize) -> Result<Vec<u8>> {
    let mut f = std::fs::File::open("/dev/urandom").map_err(|e| Error::Runtime {
        context: "/dev/urandom",
        message: e.to_string(),
    })?;
    let mut b = vec![0u8; n];
    f.read_exact(&mut b).map_err(|e| Error::Runtime {
        context: "/dev/urandom",
        message: e.to_string(),
    })?;
    Ok(b)
}

fn hex_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for byte in b {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

fn hex_decode(s: &str) -> Result<Vec<u8>> {
    let s = s.trim().trim_start_matches("0x");
    if s.is_empty() || !s.len().is_multiple_of(2) {
        return Err(Error::Invalid(format!(
            "'{s}' is not an even-length hex serial"
        )));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| Error::Invalid(format!("'{s}' is not hex")))
        })
        .collect()
}

fn read_pem(what: &str, path: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| Error::Invalid(format!("cannot read {what} '{path}': {e}")))
}

/// Write a file, refusing to clobber unless `force`. `secret` makes it `0600`.
fn write_file(path: &str, contents: &str, secret: bool, force: bool) -> Result<()> {
    if !force && std::path::Path::new(path).exists() {
        return Err(Error::Invalid(format!(
            "'{path}' already exists — pass --force to overwrite"
        )));
    }
    std::fs::write(path, contents)
        .map_err(|e| Error::Invalid(format!("cannot write '{path}': {e}")))?;
    if secret {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// `delonix node-ca …`
// ---------------------------------------------------------------------------

#[derive(Debug, clap::Subcommand)]
pub enum NodeCaCmd {
    /// Create a node CA: a key and its self-signed certificate.
    ///
    /// The CERT is the trust anchor you pass to every server as `serve node-api
    /// --tls-client-ca`; the KEY stays with the enrollment authority (`serve
    /// node-enroll`) and no node-API server ever holds it (ADR-0097).
    Init {
        /// The CA's subject common name — the SAME value `serve node-enroll` and
        /// `node-ca revoke` must be given, since the issuer is reconstructed from it.
        #[arg(long, default_value = "delonix-node-ca")]
        cn: String,
        /// Where to write the CA certificate (PEM).
        #[arg(long, value_name = "PEM", default_value = "node-ca.crt")]
        out_cert: String,
        /// Where to write the CA private key (PEM, written `0600`).
        #[arg(long, value_name = "PEM", default_value = "node-ca.key")]
        out_key: String,
        /// Overwrite existing output files.
        #[arg(long)]
        force: bool,
    },
    /// Revoke client-certificate serials into a CRL the node CA signs.
    ///
    /// Serials accumulate in a sidecar (`<out>.serials`) so each run lists every
    /// serial revoked so far; point `serve node-api --tls-client-crl` at the CRL.
    Revoke {
        /// The node CA private key (PEM) — the CRL is signed with it.
        #[arg(long, value_name = "PEM", default_value = "node-ca.key")]
        ca_key: String,
        /// The CA subject common name (must match `node-ca init --cn`).
        #[arg(long = "cn", default_value = "delonix-node-ca")]
        cn: String,
        /// A client-certificate serial to revoke (hex, as `node-enroll` printed).
        /// Repeatable.
        #[arg(long = "serial", value_name = "HEX", required = true)]
        serials: Vec<String>,
        /// Where to write the CRL (PEM).
        #[arg(long, value_name = "PEM", default_value = "node-ca.crl")]
        out: String,
    },
}

pub fn run_node_ca(cmd: NodeCaCmd) -> Result<()> {
    match cmd {
        NodeCaCmd::Init {
            cn,
            out_cert,
            out_key,
            force,
        } => {
            let (_ca, cert_pem, key_pem) = NodeCa::generate(&cn)?;
            write_file(&out_key, &key_pem, true, force)?;
            write_file(&out_cert, &cert_pem, false, force)?;
            println!(
                "node CA created (CN={cn})\n  certificate (trust anchor): {out_cert}\n  private key (keep with the enrollment authority): {out_key}\n\
                 Pass {out_cert} to every node as `serve node-api --tls-client-ca`."
            );
            Ok(())
        }
        NodeCaCmd::Revoke {
            ca_key,
            cn,
            serials,
            out,
        } => {
            let ca = NodeCa::reload(&read_pem("--ca-key", &ca_key)?, &cn)?;
            // Validate the serials given, then accumulate with any already in the
            // sidecar, so the CRL always lists everything revoked so far.
            for s in &serials {
                hex_decode(s)?;
            }
            let sidecar = format!("{out}.serials");
            let mut all: Vec<String> = Vec::new();
            if let Ok(existing) = std::fs::read_to_string(&sidecar) {
                all.extend(existing.lines().map(|l| l.trim().to_string()));
            }
            all.extend(serials.iter().map(|s| s.trim().to_lowercase()));
            all.retain(|s| !s.is_empty());
            all.sort();
            all.dedup();
            let decoded: Vec<Vec<u8>> = all.iter().map(|s| hex_decode(s)).collect::<Result<_>>()?;
            let crl = build_crl(&ca, &decoded, now_unix() as u64)?;
            write_file(&sidecar, &format!("{}\n", all.join("\n")), false, true)?;
            write_file(&out, &crl, false, true)?;
            println!(
                "CRL written: {out} ({} serial(s) revoked)\n  Point `serve node-api --tls-client-crl {out}` at it; it takes effect on the node's next (socket-activated) start.",
                decoded.len()
            );
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// `delonix node-enroll …` (the member side)
// ---------------------------------------------------------------------------

#[derive(Debug, clap::Args)]
pub struct NodeEnrollArgs {
    /// The enrollment authority address: `host:port` or `tcp://host:port`.
    #[arg(long)]
    server: String,
    /// The node CA certificate (PEM) to PIN for the authority's server identity —
    /// this is what stops a man-in-the-middle from harvesting the token.
    #[arg(long, value_name = "PEM")]
    ca: String,
    /// The one-time enrollment token.
    #[arg(long)]
    token: String,
    /// The subject common name = this member's identity in its certificate.
    #[arg(long)]
    cn: String,
    /// A subject alternative name (IP or DNS) for the client certificate.
    /// Repeatable; usually unnecessary for a client identity.
    #[arg(long = "san", value_name = "SAN")]
    sans: Vec<String>,
    /// Where to write the generated private key (PEM, `0600`) — it never leaves
    /// this host.
    #[arg(long, value_name = "PEM", default_value = "client.key")]
    out_key: String,
    /// Where to write the issued client certificate (PEM).
    #[arg(long, value_name = "PEM", default_value = "client.crt")]
    out_cert: String,
    /// Where to also write the CA bundle the authority returned (PEM).
    #[arg(long, value_name = "PEM")]
    out_ca: Option<String>,
    /// Overwrite existing output files.
    #[arg(long)]
    force: bool,
}

pub fn run_node_enroll(args: NodeEnrollArgs) -> Result<()> {
    // Generate the key pair and the CSR here; the private key is never sent.
    let key = KeyPair::generate().map_err(rcgen_err("member key"))?;
    let mut params =
        CertificateParams::new(args.sans.clone()).map_err(rcgen_err("member CSR params"))?;
    params.distinguished_name.push(DnType::CommonName, &args.cn);
    let csr_pem = params
        .serialize_request(&key)
        .map_err(rcgen_err("member CSR"))?
        .pem()
        .map_err(rcgen_err("member CSR PEM"))?;

    let host = args
        .server
        .strip_prefix("tcp://")
        .unwrap_or(&args.server)
        .trim_end_matches('/');
    let url = format!("https://{host}/v1/enroll");
    let ca_pem = read_pem("--ca", &args.ca)?;

    // Trust ONLY the pinned node CA for the authority's server certificate.
    let ca_cert = reqwest::Certificate::from_pem(ca_pem.as_bytes())
        .map_err(|e| Error::Invalid(format!("--ca is not a valid PEM certificate: {e}")))?;
    let client = reqwest::blocking::Client::builder()
        .tls_built_in_root_certs(false)
        .add_root_certificate(ca_cert)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| Error::Runtime {
            context: "enrollment client",
            message: e.to_string(),
        })?;

    let body = serde_json::json!({ "token": args.token, "csr_pem": csr_pem });
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .map_err(|e| Error::Unavailable(format!("enrollment request to {url} failed: {e}")))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        let detail = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str().map(str::to_string)))
            .unwrap_or(text);
        return Err(Error::Invalid(format!(
            "enrollment refused (HTTP {}): {detail}",
            status.as_u16()
        )));
    }
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| Error::Runtime {
        context: "enrollment response",
        message: format!("not JSON: {e}"),
    })?;
    let cert_pem = v
        .get("cert_pem")
        .and_then(|c| c.as_str())
        .ok_or_else(|| Error::Runtime {
            context: "enrollment response",
            message: "no cert_pem field".into(),
        })?;
    let serial = v.get("serial").and_then(|s| s.as_str()).unwrap_or("?");

    write_file(&args.out_key, &key.serialize_pem(), true, args.force)?;
    write_file(&args.out_cert, cert_pem, false, args.force)?;
    if let Some(out_ca) = &args.out_ca {
        if let Some(ca_returned) = v.get("ca_pem").and_then(|c| c.as_str()) {
            write_file(out_ca, ca_returned, false, args.force)?;
        }
    }
    println!(
        "enrolled as CN={} (serial {serial})\n  private key: {}\n  certificate: {}\n  Call the node with `--cert {} --key {} --cacert {}`.",
        args.cn, args.out_key, args.out_cert, args.out_cert, args.out_key, args.ca
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// `delonix serve node-enroll …` (the authority side) — called by cmd::serve
// ---------------------------------------------------------------------------

/// Everything `serve node-enroll` needs. The authority holds the node CA key and
/// the valid tokens; it never runs a node-API server.
pub struct AuthorityConfig {
    /// `tcp://host:port` to listen on.
    pub addr: String,
    /// The node CA key (PEM path) — reloaded to sign leaves/CRLs.
    pub ca_key: String,
    /// The node CA cert (PEM path) — echoed to members in the response.
    pub ca_cert: String,
    /// The CA subject common name (must match `node-ca init --cn`).
    pub cn: String,
    /// Valid one-time tokens, each spendable once.
    pub tokens: Vec<String>,
    /// A file of additional tokens, one per line (`#` comments and blanks skipped).
    pub token_file: Option<String>,
    /// SANs for the authority's own server certificate — the addresses members
    /// reach it at (127.0.0.1 and localhost are always included).
    pub sans: Vec<String>,
    /// Lifetime of the client certificates this authority issues.
    pub cert_ttl: Duration,
    /// An explicit server identity (PEM paths); when absent it is auto-issued
    /// from the node CA for `sans`.
    pub server_cert: Option<String>,
    pub server_key: Option<String>,
}

struct Authority {
    ca_key_pem: String,
    ca_cert_pem: String,
    cn: String,
    cert_ttl: Duration,
    tokens: TokenStore,
}

pub fn serve_authority(cfg: AuthorityConfig) -> Result<()> {
    let hostport = cfg
        .addr
        .strip_prefix("tcp://")
        .ok_or_else(|| Error::Invalid(format!("--addr '{}' must be tcp://host:port", cfg.addr)))?
        .to_string();

    let ca_key_pem = read_pem("--ca-key", &cfg.ca_key)?;
    let ca_cert_pem = read_pem("--ca-cert", &cfg.ca_cert)?;
    // Reload once up front to fail fast on a bad key, and to issue the server cert.
    let ca = NodeCa::reload(&ca_key_pem, &cfg.cn)?;

    let mut tokens = cfg.tokens.clone();
    if let Some(path) = &cfg.token_file {
        let text = read_pem("--token-file", path)?;
        for line in text.lines() {
            let t = line.trim();
            if !t.is_empty() && !t.starts_with('#') {
                tokens.push(t.to_string());
            }
        }
    }
    if tokens.is_empty() {
        return Err(Error::Invalid(
            "no enrollment tokens given — pass at least one --token or a --token-file".into(),
        ));
    }
    let token_store = TokenStore::new(&tokens)?;

    // The authority's own server identity: supplied, or auto-issued from the CA.
    let (server_cert_pem, server_key_pem) =
        match (&cfg.server_cert, &cfg.server_key) {
            (Some(c), Some(k)) => (read_pem("--server-cert", c)?, read_pem("--server-key", k)?),
            (None, None) => {
                let mut sans = cfg.sans.clone();
                for d in ["127.0.0.1", "localhost"] {
                    if !sans.iter().any(|s| s == d) {
                        sans.push(d.to_string());
                    }
                }
                issue_server_cert(&ca, &sans)?
            }
            _ => return Err(Error::Invalid(
                "--server-cert and --server-key must be given together (or neither, to auto-issue)"
                    .into(),
            )),
        };

    let config = server_only_tls(&server_cert_pem, &server_key_pem)?;
    let state = Arc::new(Authority {
        ca_key_pem,
        ca_cert_pem,
        cn: cfg.cn,
        cert_ttl: cfg.cert_ttl,
        tokens: token_store,
    });

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| Error::Runtime {
            context: "tokio",
            message: e.to_string(),
        })?;
    println!(
        "enrollment authority listening on https://{hostport}/v1/enroll (mutual-TLS-free, token-authenticated)\n  issuing client certificates valid for {}s, signed by CN={}; {} token(s) loaded.",
        state.cert_ttl.as_secs(),
        state.cn,
        state.tokens.remaining(),
    );
    rt.block_on(serve_loop(hostport, config, state))
}

fn server_only_tls(
    cert_pem: &str,
    key_pem: &str,
) -> Result<Arc<tokio_rustls::rustls::ServerConfig>> {
    use tokio_rustls::rustls::pki_types::pem::PemObject;
    use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use tokio_rustls::rustls::ServerConfig;
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(cert_pem.as_bytes())
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| Error::Invalid(format!("authority server cert (PEM): {e}")))?;
    let key = PrivateKeyDer::from_pem_slice(key_pem.as_bytes())
        .map_err(|e| Error::Invalid(format!("authority server key (PEM): {e}")))?;
    // Server-authenticated only: the member has no client certificate yet (that
    // is the point of enrolling), so no client verifier (ADR-0097).
    let mut cfg = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| Error::Invalid(format!("authority server cert/key mismatch: {e}")))?;
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}

async fn serve_loop(
    hostport: String,
    config: Arc<tokio_rustls::rustls::ServerConfig>,
    state: Arc<Authority>,
) -> Result<()> {
    use hyper_util::rt::TokioIo;
    let listener = tokio::net::TcpListener::bind(&hostport)
        .await
        .map_err(|e| Error::Runtime {
            context: "bind",
            message: e.to_string(),
        })?;
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    loop {
        let (socket, _peer) = match listener.accept().await {
            Ok(x) => x,
            Err(e) => {
                eprintln!("enroll: accept() failed, retrying: {e}");
                continue;
            }
        };
        let acceptor = acceptor.clone();
        let state = state.clone();
        tokio::spawn(async move {
            match acceptor.accept(socket).await {
                Ok(tls) => {
                    let io = TokioIo::new(tls);
                    let svc = hyper::service::service_fn(move |req| handle(req, state.clone()));
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(io, svc)
                        .await;
                }
                // A client that fails the TLS handshake (e.g. it did not pin the
                // node CA) is dropped quietly; it never reaches `/v1/enroll`.
                Err(_e) => {}
            }
        });
    }
}

async fn handle(
    req: hyper::Request<hyper::body::Incoming>,
    state: Arc<Authority>,
) -> std::result::Result<
    hyper::Response<http_body_util::Full<bytes::Bytes>>,
    std::convert::Infallible,
> {
    use http_body_util::BodyExt;
    let reply = |code: u16, json: serde_json::Value| {
        let body = http_body_util::Full::new(bytes::Bytes::from(json.to_string()));
        hyper::Response::builder()
            .status(code)
            .header("content-type", "application/json")
            .body(body)
            .unwrap()
    };

    if req.method() != hyper::Method::POST || req.uri().path() != "/v1/enroll" {
        return Ok(reply(
            404,
            serde_json::json!({ "error": "POST /v1/enroll only" }),
        ));
    }
    // Bound the body — a bootstrap surface must not read an unbounded request.
    let limited = http_body_util::Limited::new(req.into_body(), 64 * 1024);
    let bytes = match limited.collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => {
            return Ok(reply(
                413,
                serde_json::json!({ "error": "request too large" }),
            ))
        }
    };
    let payload: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => {
            return Ok(reply(
                400,
                serde_json::json!({ "error": format!("invalid JSON: {e}") }),
            ))
        }
    };
    let token = payload.get("token").and_then(|t| t.as_str()).unwrap_or("");
    let csr_pem = payload
        .get("csr_pem")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    if token.is_empty() || csr_pem.is_empty() {
        return Ok(reply(
            400,
            serde_json::json!({ "error": "both token and csr_pem are required" }),
        ));
    }
    // One-time: the token is spent here, before any signing, so a replay — even
    // of a valid token — is refused.
    if !state.tokens.consume(token) {
        return Ok(reply(
            401,
            serde_json::json!({ "error": "unknown or already-used enrollment token" }),
        ));
    }
    // Reload the CA per request (cheap; avoids sharing rcgen types across tasks).
    let ca = match NodeCa::reload(&state.ca_key_pem, &state.cn) {
        Ok(ca) => ca,
        Err(e) => {
            return Ok(reply(
                500,
                serde_json::json!({ "error": format!("CA unavailable: {e}") }),
            ))
        }
    };
    match sign_csr(&ca, csr_pem, state.cert_ttl) {
        Ok((cert_pem, serial)) => {
            println!("enroll: issued a client certificate (serial {serial})");
            Ok(reply(
                200,
                serde_json::json!({
                    "cert_pem": cert_pem,
                    "ca_pem": state.ca_cert_pem,
                    "serial": serial,
                }),
            ))
        }
        Err(e) => Ok(reply(
            400,
            serde_json::json!({ "error": format!("cannot sign CSR: {e}") }),
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_spendable_exactly_once() {
        let store = TokenStore::new(&["alpha".into(), "beta".into()]).unwrap();
        assert_eq!(store.remaining(), 2);
        assert!(
            store.consume("alpha"),
            "first use of a known token succeeds"
        );
        assert!(!store.consume("alpha"), "a second use is refused");
        assert!(
            !store.consume("never-issued"),
            "an unknown token is refused"
        );
        assert_eq!(store.remaining(), 1);
    }

    #[test]
    fn hex_round_trips_and_rejects_garbage() {
        assert_eq!(hex_encode(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
        assert_eq!(
            hex_decode("DEADbeef").unwrap(),
            vec![0xde, 0xad, 0xbe, 0xef]
        );
        assert_eq!(hex_decode("0xff").unwrap(), vec![0xff]);
        assert!(hex_decode("abc").is_err(), "odd length is rejected");
        assert!(hex_decode("zz").is_err(), "non-hex is rejected");
    }

    #[test]
    fn reload_reconstructs_an_issuer_that_signs_leaves_chaining_to_the_persisted_cert() {
        // Generate a CA, persist its key, reload it, and sign a CSR. The reloaded
        // issuer must produce a leaf whose issuer DN equals the persisted CA's
        // subject (same CN) — the chain property ADR-0097 §7 depends on.
        let (_ca, ca_cert_pem, ca_key_pem) = NodeCa::generate("delonix-node-ca").unwrap();
        assert!(ca_cert_pem.contains("BEGIN CERTIFICATE"));
        let reloaded = NodeCa::reload(&ca_key_pem, "delonix-node-ca").unwrap();

        // A member CSR.
        let key = KeyPair::generate().unwrap();
        let mut p = CertificateParams::new(Vec::<String>::new()).unwrap();
        p.distinguished_name.push(DnType::CommonName, "member-1");
        let csr = p.serialize_request(&key).unwrap().pem().unwrap();

        let (cert_pem, serial) = sign_csr(&reloaded, &csr, Duration::from_secs(60)).unwrap();
        assert!(cert_pem.contains("BEGIN CERTIFICATE"));
        assert_eq!(serial.len(), 32, "16-byte serial, hex");

        // The reloaded issuer's subject matches the persisted CA's subject, so the
        // leaf chains to the `node-ca.crt` an operator configured (verified at the
        // handshake in delonix-node-api's tls_mtls CRL/mTLS tests).
        let reissued_cert_pem = ca_params("delonix-node-ca")
            .unwrap()
            .self_signed(&KeyPair::from_pem(&ca_key_pem).unwrap())
            .unwrap()
            .pem();
        assert!(reissued_cert_pem.contains("BEGIN CERTIFICATE"));
    }

    #[test]
    fn build_crl_lists_each_revoked_serial() {
        let (ca, _c, _k) = NodeCa::generate("delonix-node-ca").unwrap();
        let s1 = vec![0xDEu8, 0xAD];
        let s2 = vec![0xBEu8, 0xEF];
        let crl = build_crl(&ca, &[s1, s2], 7).unwrap();
        assert!(crl.contains("BEGIN X509 CRL"), "a PEM CRL: {crl}");
    }

    #[test]
    fn sign_csr_rejects_a_non_csr() {
        let (ca, _c, _k) = NodeCa::generate("delonix-node-ca").unwrap();
        let e = sign_csr(&ca, "not a csr", Duration::from_secs(60)).unwrap_err();
        assert!(e.to_string().contains("CSR"), "{e}");
    }
}
