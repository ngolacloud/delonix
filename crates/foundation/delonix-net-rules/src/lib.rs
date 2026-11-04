//! PURE network rules — what can be computed without touching the kernel.
//!
//! `delonix-net-RULES`, not `-model`: these are RULES — functions and one value
//! type — not a typed domain model of a network.
//!
//! Part of what `delonix-sdn` was called for was no mechanism at all: deriving
//! the name of a bridge, assigning an IP inside a prefix, reading `10mbit` as
//! bits per second, deciding whether an IP fits in a subnet. They are
//! deterministic functions, with no I/O and no privilege.
//!
//! Putting that behind an HTTP endpoint would cost a network hop — and a new
//! failure mode — to compute what a client can compute itself. Worse: the answer
//! MUST be identical on both sides (the bridge a client expects is the one the
//! engine creates), and two implementations that must agree are two
//! implementations that one day will not.
//!
//! That is why it is not an API: it is a shared crate, with NO DEPENDENCIES, that
//! both sides compile. `delonix-sdn` re-exports everything that lives here, so no
//! existing consumer had to change.
//!
//! `Cidr` lives here, and it is what makes this crate worth it. It was designed
//! from the start without dependencies — no `ipnet` crate, no IPv6 (disabled by a
//! security decision in this engine), arithmetic by hand — and so it crosses the
//! boundary without dragging anything along. The two functions that depended on it
//! came with it.
//!
//! WHAT STAYS OUT is anything that is not pure, and the list is the result of a
//! classification I got wrong myself the first time (I looked for I/O in the BODY
//! of each function and not in what they call):
//!
//!   `alloc_ip_in`/`alloc_ip`   delegate to `ipam::lookup`, which READS the registry
//!   `parse_net_rate`           returns `delonix-model`'s `Error`
//!
//! The first pair is real mechanism — assigning an address requires seeing which
//! are already assigned, and that is shared state, not arithmetic. That one belongs
//! in the API. `parse_net_rate` is pure, and only waits for an error type this
//! crate can return without depending on delonix-model.

pub mod policy;

/// Parses an overlay peer entry: `<node_ip>` (flat VXLAN) OR
/// `<node_ip>=<wg_pubkey>=<wg_ip>` (encrypted). Returns (node_ip, Option<(pubkey, wg_ip)>).
pub fn parse_overlay_peer(s: &str) -> (String, Option<(String, String)>) {
    // Format `node_ip=wg_pubkey=wg_ip`. The pubkey is base64 and ENDS in `=`
    // (padding) — it collides with the delimiter. Since node_ip and wg_ip are IPs (never
    // contain `=`), we delimit by the FIRST and the LAST `=`; what remains in the
    // middle is the pubkey WITH its padding intact. (Flat VXLAN peer = just `node_ip`.)
    match (s.find('='), s.rfind('=')) {
        (Some(first), Some(last)) if last > first => {
            let node = &s[..first];
            let pubkey = &s[first + 1..last];
            let wgip = &s[last + 1..];
            if !pubkey.is_empty() && !wgip.is_empty() {
                return (
                    node.to_string(),
                    Some((pubkey.to_string(), wgip.to_string())),
                );
            }
            (node.to_string(), None)
        }
        _ => (s.split('=').next().unwrap_or_default().to_string(), None),
    }
}

/// 32-bit FNV-1a hash (to derive a network's subnet/bridge from its name).
/// Public because `delonix-sdn` re-exports it internally. It is not part of the
/// surface that matters to consumers of this crate — it is the detail that
/// [`bridge_name`] depends on.
pub fn fnv32(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for byte in s.bytes() {
        h ^= byte as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// The bridge device name of a user network — **the single formula**, shared by
/// the two stores that talk about the same device (the same
/// generator-and-reader-share-the-format discipline as `fw_rule_tail`/
/// `antispoof_rule_args`).
///
/// The authority is the physical plane: `infra::network_create{,_with}` writes
/// this into the `NetDef` and it is that name the holder actually creates the
/// link with (`netadd`/`link_exists`/`resolve_net`). The declarative
/// `NetworkStore` only ever REPORTS it (`network ls`/`inspect`/`describe`), and
/// it derives it here rather than recomputing — it had its own formula
/// (`dlxn{base:02x}{hash:04x}`) and printed a device that does not exist on the
/// host, in the very column an operator reads to go and debug the device.
///
/// Note this deliberately does NOT depend on the base octet: the name alone is
/// unique in both stores, and adding the base is what made the two diverge.
/// 12 chars, comfortably inside IFNAMSIZ (15).
pub fn bridge_name(name: &str) -> String {
    format!("dlxn{:08x}", fnv32(name))
}

/// A network's address space, as a real prefix instead of the two octets of a
/// hardcoded `/16`.
///
/// **Why this type exists (ADR-0013 tier A).** Everything about a network used
/// to be derived from ONE octet: the record on disk held `210`, the bridge was
/// named from it, the gateway was `10.<n>.0.1`, and the IPAM allocated inside
/// `10.<n>.0.0/16` because that was the only shape there was. `--subnet` could
/// only ever pick which octet. That is a fine default and a poor contract: a
/// network engineer has an address plan, and «any /16 you like as long as it is
/// `10.<200-254>`» does not meet it.
///
/// Kept deliberately small — no dependency, no `ipnet` crate (this repo does not
/// grow its supply chain for arithmetic), and no IPv6: v6 is DISABLED by design
/// in this engine (v0.37.1, it was a complete bypass of the policy model) and
/// re-enabling it is a security decision, not a widening of this struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    /// Network address, host bits already cleared.
    pub base: u32,
    pub len: u8,
}

impl Cidr {
    /// Parses `a.b.c.d/len`, and ALSO the legacy two-octet form (`10.210`),
    /// which every record written before this meant as `10.210.0.0/16`.
    ///
    /// Accepting the old form here is what makes the migration a non-event:
    /// a record holding a bare octet keeps meaning exactly what it always meant,
    /// and is rewritten in the new shape on the next write. Same promotion the
    /// `base=<n>` line already does.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let (addr, len) = match s.split_once('/') {
            Some((a, l)) => (a, l.parse::<u8>().ok()?),
            // Legacy `10.210` — two octets, always a /16.
            None if s.split('.').count() == 2 => (s, 16),
            None => return None,
        };
        if len > 32 {
            return None;
        }
        let mut oct = [0u8; 4];
        let parts: Vec<&str> = addr.split('.').collect();
        if parts.is_empty() || parts.len() > 4 {
            return None;
        }
        for (i, p) in parts.iter().enumerate() {
            oct[i] = p.parse::<u8>().ok()?;
        }
        let raw = u32::from_be_bytes(oct);
        // Host bits are CLEARED rather than rejected: `10.210.5.7/16` is how
        // people write «the /16 that address is in», and refusing it would be
        // pedantry that helps nobody.
        let mask = Self::mask(len);
        Some(Self {
            base: raw & mask,
            len,
        })
    }

    fn mask(len: u8) -> u32 {
        if len == 0 {
            0
        } else {
            u32::MAX << (32 - len)
        }
    }

    /// How many addresses the prefix holds, saturating — a `/0` does not fit in
    /// a `u32` and nothing here ever wants one.
    pub fn size(&self) -> u32 {
        1u32.checked_shl(32 - self.len as u32).unwrap_or(u32::MAX)
    }

    /// The gateway: the FIRST usable address — `None` when the prefix has none.
    ///
    /// **It returns an `Option` because the honest answer is sometimes «there
    /// isn't one», and the first version of this could not say that.** It was
    /// `base + 1` unconditionally, and that is wrong twice:
    ///
    /// * on a `/32` (one address) and a `/31` (RFC 3021 point-to-point, two
    ///   addresses and no room for a gateway), `base + 1` lands OUTSIDE the
    ///   prefix or on the peer. Measured: `10.0.0.0/32` answered `10.0.0.1`;
    /// * on `255.255.255.255/32` it **overflowed** — a panic in debug, and in
    ///   release a silent wrap to `0.0.0.0`, which is the dangerous half: a
    ///   network whose gateway is the null address, configured without a word.
    ///
    /// `checked_add` and the length guard together make both impossible. A
    /// caller that needs a gateway has to handle the `None`, which is the point.
    pub fn gateway(&self) -> Option<String> {
        // A gateway needs the network address, itself, and at least one host —
        // so /31 and /32 are excluded by arithmetic, not by taste.
        if self.len >= 31 {
            return None;
        }
        self.base.checked_add(1).map(Self::fmt_u32)
    }

    /// The last address of the prefix (the broadcast, on a /24 and wider).
    pub fn last(&self) -> u32 {
        self.base | !Self::mask(self.len)
    }

    /// Is this prefix usable as a WORKLOAD network here?
    ///
    /// Separate from `parse` on purpose: parsing answers «is this a prefix»,
    /// this answers «can a network live in it», and conflating the two would
    /// make the type unusable for reading a peer's address or a route.
    ///
    /// The floor is /28 (16 addresses: network, gateway, 13 hosts, broadcast) —
    /// below that a network cannot hold enough workloads to be worth declaring,
    /// and the ceiling is /8 so one network cannot swallow a whole private
    /// range by typo.
    pub fn usable_for_network(&self) -> std::result::Result<(), String> {
        if !(8..=28).contains(&self.len) {
            return Err(format!(
                "prefix /{} is outside the usable range /8–/28 (a /29 or smaller has no room for \
                 a gateway plus hosts; wider than /8 swallows a whole private range)",
                self.len
            ));
        }
        Ok(())
    }

    pub fn contains(&self, ip: u32) -> bool {
        ip & Self::mask(self.len) == self.base
    }

    /// Do the two prefixes share any address? The question `network create` has
    /// to answer before handing out a second network that overlaps the first.
    pub fn overlaps(&self, other: &Cidr) -> bool {
        let shorter = self.len.min(other.len);
        self.base & Self::mask(shorter) == other.base & Self::mask(shorter)
    }

    pub fn to_string_cidr(&self) -> String {
        format!("{}/{}", Self::fmt_u32(self.base), self.len)
    }

    /// Formats a `u32` as a dotted address.
    ///
    /// Public because `delonix-sdn`'s `ipam` uses it to write the candidate it has
    /// just computed — it was `pub(crate)` when `Cidr` lived there, and the move to
    /// another crate turned that into a compile error, not a decision. It stays
    /// exposed with the same scope as always: a formatting detail of the type itself.
    pub fn fmt_u32(v: u32) -> String {
        let b = v.to_be_bytes();
        format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
    }

    /// Parses a dotted address into a `u32`.
    pub fn parse_addr(ip: &str) -> Option<u32> {
        let p: Vec<&str> = ip.trim().split('.').collect();
        if p.len() != 4 {
            return None;
        }
        let mut o = [0u8; 4];
        for (i, x) in p.iter().enumerate() {
            o[i] = x.parse::<u8>().ok()?;
        }
        Some(u32::from_be_bytes(o))
    }
}

/// The VM DHCP pool of a `/16`: `<a>.<b>.254.10` to `<a>.<b>.254.249` (240
/// addresses). Every VM that exists today was addressed from this exact range,
/// so a `/16` keeps it byte for byte.
const VM_DHCP_POOL_OCTET: u32 = 254;
const VM_DHCP_POOL_FIRST: u32 = 10;
const VM_DHCP_POOL_SIZE: u32 = 240;

/// The VM DHCP pool of network `prefix`, as `(first address, size)`, or `None`
/// for something that is not a network.
///
/// A `/16` keeps its historic `.254.10–.249`, in either spelling (`10.210` or
/// `10.210.0.0/16` — the CIDR one used to give `None`, and a VM on a network
/// created with `--subnet` had no predicted address).
///
/// Any other length gets `min(240, size/8)` addresses at the TOP of the prefix,
/// just below the broadcast. Before this, the server always served the `/16`
/// the gateway's first two octets named: on `172.20.9.0/24` a VM was offered
/// `172.20.254.x/16` with router `172.20.0.1` — none of it on its network —
/// and came up unreachable, outside the address registry and outside isolation
/// (measured: `IP <none>`, and its MAC never reached the bridge's neighbours).
pub fn vm_dhcp_pool(prefix: &str) -> Option<(u32, u32)> {
    let net = Cidr::parse(prefix)?;
    if net.len == 16 {
        let first = net.base + (VM_DHCP_POOL_OCTET << 8) + VM_DHCP_POOL_FIRST;
        return Some((first, VM_DHCP_POOL_SIZE));
    }
    net.usable_for_network().ok()?;
    let size = (net.size() / 8).clamp(1, VM_DHCP_POOL_SIZE);
    // `last()` is the broadcast: the pool ends one below it.
    Some((net.last() - size, size))
}

/// The address the VM DHCP hands `mac` on network `prefix` — derived from the
/// MAC, and so known on the host side before the guest has even booted.
///
/// It lives here, not in the adapter, because it is arithmetic TWO address
/// authorities must share: the DHCP server (which hands it out) and the
/// container IPAM (which must know what it may NOT hand out). Two copies would
/// diverge the day the pool changed, and the symptom would be a container on a
/// VM's IP.
pub fn vm_dhcp_lease_ip(prefix: &str, mac: &str) -> Option<String> {
    let (first, size) = vm_dhcp_pool(prefix)?;
    // The server hashes the MAC as it renders it off the wire: lowercase,
    // `:`-separated. Normalizing here (and not at each call site) is what stops
    // an upper-case MAC from a record producing a different, unused address.
    Some(Cidr::fmt_u32(first + fnv32(&mac.to_lowercase()) % size))
}

/// `true` when `ip` falls in the VM DHCP pool of network `prefix`.
///
/// It is the border between a network's two address authorities: the VM DHCP
/// owns the pool, the container IPAM everything else. Without it the IPAM
/// probed the whole network — the pool included — and a container whose id
/// landed there got a VM's address (measured: `allocate` handed out exactly the
/// `dhcp_lease_ip` of a MAC).
pub fn in_vm_dhcp_pool(prefix: &str, ip: &str) -> bool {
    let (Some((first, size)), Some(addr)) = (vm_dhcp_pool(prefix), Cidr::parse_addr(ip)) else {
        return false;
    };
    (first..first + size).contains(&addr)
}

/// **Preferred** IP (deterministic, pure) in an arbitrary `/16` (`<prefix>.A.B`),
/// derived from the id. It's just the starting point: on its own it collides by the birthday
/// paradox at ~300 containers (32 bits of the id → 16 bits of host). Real uniqueness comes from
/// the lease registry + probing in [`ipam::allocate`]; see [`alloc_ip_in`].
pub fn derive_ip_in(prefix: &str, id: &str) -> String {
    // The first 8 BYTES of the id, cut back to a character boundary. A byte
    // slice at 8 panicked on an id whose 8th byte falls inside a multi-byte
    // character (found by the `net_cidr` fuzz target, 2026-10-12) — and an id
    // here is not always a hex container id: `net netns attach <name>` passes
    // the name the operator typed. For every ASCII id the cut is the same 8
    // bytes as before, so no installed lease moves.
    let end = (0..=id.len().min(8))
        .rev()
        .find(|&i| id.is_char_boundary(i))
        .unwrap_or(0);
    let hex = &id[..end];
    let n = u32::from_str_radix(hex, 16).unwrap_or(2);
    // The /16 path stays BYTE FOR BYTE as it was, and that is deliberate: every
    // network that exists today has leases derived from this exact formula, and the
    // generalization has nothing to gain by touching the installed base. The general
    // formula below is NOT equivalent to this one (the last-octet clamp is not the
    // same as the remainder of the division over the whole space) — so the two
    // coexist instead of one pretending to be the other.
    if let Some(net) = Cidr::parse(prefix) {
        if net.len != 16 {
            return derive_ip_general(&net, n);
        }
    }
    let a = ((n >> 8) & 0xff) as u8;
    let mut b = (n & 0xff) as u8;
    if b < 2 {
        b = 2;
    }
    if b == 255 {
        b = 254;
    }
    format!("{prefix}.{a}.{b}")
}

/// **Preferred** ULA IPv6 address (deterministic, pure) for a `(network, id)`
/// pair — the IPv6 sibling of [`derive_ip_in`] (ADR-0082, dual-stack phase 2).
///
/// A Unique-Local Address under `fd00::/8` (RFC 4193): `fd` + a 40-bit global id
/// derived from the NETWORK identifier (so each network lands on its own ULA
/// prefix), subnet `0`, and a 64-bit interface id derived from the container id.
/// Like [`derive_ip_in`] it is only the PREFERRED address — real uniqueness will
/// come from a lease registry (phase 2 wires it; this is the pure hash it starts
/// from). The interface id is clamped `>= 2`, mirroring the v4 host-octet clamp,
/// so it never collides with the subnet-router anycast / gateway slot.
///
/// INERT until the dual-stack firewall (ADR-0082 phase 3) can filter v6: nothing
/// in the engine consumes this yet, by design — v6 stays off until it is governed.
pub fn derive_ula_in(network: &str, id: &str) -> String {
    let g = u64::from(fnv32(network));
    // `fd` (ULA) + the 40-bit global id spread across the first three hextets.
    let seg0 = (0xfd00 | ((g >> 24) & 0xff)) as u16;
    let seg1 = ((g >> 8) & 0xffff) as u16;
    let seg2 = ((g & 0xff) << 8) as u16;
    // Interface id from the container id (same FNV the v4 path uses), clamped.
    let iface = {
        let h = (fnv32(id) & 0xffff) as u16;
        h.max(2)
    };
    std::net::Ipv6Addr::new(seg0, seg1, seg2, 0, 0, 0, 0, iface).to_string()
}

/// Validates that `ip` is a usable unicast address in `prefix`'s `/16` subnet
/// (e.g.: prefix `10.88`): 4 octets, first two == prefix, not the gateway
/// (`prefix.0.1`), the network (`prefix.0.0`) or the broadcast (`prefix.255.255`).
pub fn valid_ip_in_subnet(prefix: &str, ip: &str) -> bool {
    // `prefix` arrives here in TWO forms and both must keep working: the legacy
    // two-octet one (`10.210`, which always meant `10.210.0.0/16`) and a real CIDR.
    // `Cidr::parse` handles both, so there is no new path here — there is the same
    // path knowing more.
    let Some(net) = Cidr::parse(prefix) else {
        return false;
    };
    let Some(addr) = Cidr::parse_addr(ip) else {
        return false;
    };
    if !net.contains(addr) {
        return false;
    }
    // Excludes the network, the gateway and the broadcast. On a `/16` this is
    // exactly what the previous version excluded by hand (`.0.0`, `.0.1`,
    // `.255.255`) — the general rule is not a change in behaviour, it is the same
    // rule said in a way that also serves a /22.
    addr != net.base && addr != net.base.wrapping_add(1) && addr != net.last()
}

/// The preferred address for a prefix of arbitrary size.
///
/// Skips the network and the gateway (the first two) and the broadcast (the last),
/// and spreads the rest across the usable space. On a prefix too small to hold
/// hosts it returns the gateway itself — `valid_ip_in_subnet` then rejects it and
/// `probe_free` finds nothing either, which is the correct answer: there is no
/// address to hand out, and it is `allocate` that says so with an error.
fn derive_ip_general(net: &Cidr, n: u32) -> String {
    let uteis = net.size().saturating_sub(3); // network + gateway + broadcast
    if uteis == 0 {
        return Cidr::fmt_u32(net.base.wrapping_add(1));
    }
    let off = n % uteis;
    Cidr::fmt_u32(net.base + 2 + off)
}

/// What an `iptables-save` file contains, counted without interpreting it.
///
/// Exists for `network import-iptables`, which ANALYZES and does not apply: it says
/// how much there is and shows a translated example, so whoever is migrating can
/// see the shape before deciding. Nothing here touches the host.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IptablesSummary {
    pub tables: usize,
    pub chains: usize,
    pub rules: usize,
    /// The first rule found, to serve as an example in the translation.
    pub sample: Option<String>,
}

/// Counts what is in an `iptables-save`.
///
/// Pure: it is reading text, and so it lives here and not in the mechanism. The
/// file's shape has been stable for decades — `*table`, `:CHAIN`, `-A rule` — and
/// it is the whole grammar this needs.
pub fn parse_iptables_save(texto: &str) -> IptablesSummary {
    let mut s = IptablesSummary::default();
    for linha in texto.lines() {
        let l = linha.trim();
        if l.starts_with('*') {
            s.tables += 1;
        } else if l.starts_with(':') {
            s.chains += 1;
        } else if l.starts_with("-A") {
            s.rules += 1;
            if s.sample.is_none() {
                s.sample = Some(l.to_string());
            }
        }
    }
    s
}

/// Whether a `matchLabels` selector selects a workload carrying `labels`.
///
/// PURE, shared by design (ADR-0032): `kind: Service` needs this to compute its
/// backend set for the DNS index, and `kind: FirewallPolicy`'s own planned
/// selector (ADR-0024, still unimplemented) is meant to reuse the exact same
/// function rather than grow a second, divergence-prone `matchLabels` reader.
/// Lives here — not in `delonix-bin`, where both Kinds' CLI code lives
/// — because `delonix-sdn::infra::build_dns_index` (which computes a
/// `Service`'s live membership) cannot depend on the bin crate.
///
/// An EMPTY `match_labels` selects NOTHING, deliberately fail-closed: a
/// `spec.selector.matchLabels: {}` that silently meant "every workload on the
/// node" would be exactly the accept-and-widen footgun this codebase's own
/// audits keep finding and removing elsewhere (`ingress ls`'s `0.0.0.0/0`
/// default, `--net-connect` bypassing the firewall, ...). A caller that wants
/// "no selection" states so by omitting the `Service` document, not by writing
/// an empty selector.
pub fn matches_labels(
    labels: &std::collections::BTreeMap<String, String>,
    match_labels: &std::collections::BTreeMap<String, String>,
) -> bool {
    !match_labels.is_empty() && match_labels.iter().all(|(k, v)| labels.get(k) == Some(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bridge name is a CONTRACT between the engine and its consumers: the
    /// device a client expects has to be the one the engine creates. These values
    /// are pinned here on purpose — changing them is changing the contract, and the
    /// test forces that to be a decision and not a refactor accident.
    #[test]
    fn bridge_name_e_determinista_e_estavel() {
        assert_eq!(bridge_name("app"), bridge_name("app"));
        assert_ne!(bridge_name("app"), bridge_name("backend"));
        // 4 + 8 hex: fits in the kernel's IFNAMSIZ (15 usable chars) with room to spare.
        let n = bridge_name("qualquer-nome-muito-comprido-mesmo");
        assert!(n.starts_with("dlxn"), "unexpected prefix: {n}");
        assert_eq!(n.len(), 12, "the name must fit in IFNAMSIZ: {n}");
    }

    #[test]
    fn peer_plano_e_peer_cifrado() {
        // Plain VXLAN: just the node's IP.
        assert_eq!(parse_overlay_peer("10.0.0.7"), ("10.0.0.7".into(), None));

        // Encrypted: the public key is base64 and ENDS in `=`, which collides with
        // the delimiter. Delimiting by the first and the last `=` is what keeps the
        // padding intact — splitting on `=` would give a truncated key and a tunnel
        // that never comes up.
        let (no, wg) =
            parse_overlay_peer("10.0.0.7=Xp5e/U8A8nQLNqhr0CbrW2YInV12wZM+z0H4qNHOoUQ==10.42.0.2");
        assert_eq!(no, "10.0.0.7");
        let (chave, ip) = wg.expect("should have a wireguard identity");
        assert_eq!(chave, "Xp5e/U8A8nQLNqhr0CbrW2YInV12wZM+z0H4qNHOoUQ=");
        assert_eq!(ip, "10.42.0.2");
    }

    #[test]
    fn fnv32_bate_certo_com_a_referencia() {
        // Known 32-bit FNV-1a vector. Without this, an "optimization" that changes
        // the dispersion renames EVERY bridge of a live installation.
        assert_eq!(fnv32(""), 0x811c_9dc5);
        assert_eq!(fnv32("a"), 0xe40c_292c);
        assert_eq!(fnv32("foobar"), 0xbf9c_f968);
    }
    /// `Cidr` crosses the boundary, and so its guarantees become a contract between
    /// the two sides — not an internal detail of a crate.
    #[test]
    fn cidr_le_as_duas_formas_que_existem_em_disco() {
        // The complete form.
        let c = Cidr::parse("10.220.0.0/16").expect("cidr");
        assert_eq!(c.len, 16);
        assert_eq!(c.to_string_cidr(), "10.220.0.0/16");

        // And the LEGACY two-octet one, which every record written before the
        // explicit /16 meant as `10.210.0.0/16`. Refusing it here would lose the
        // networks of those already running this.
        let l = Cidr::parse("10.210").expect("legacy form");
        assert_eq!((l.len, l.to_string_cidr().as_str()), (16, "10.210.0.0/16"));
    }

    #[test]
    fn cidr_limpa_os_bits_de_host() {
        // `10.220.5.9/16` is the SAME network as `10.220.0.0/16`. Storing the base
        // without clearing would make two equal networks compare as different.
        let c = Cidr::parse("10.220.5.9/16").expect("cidr");
        assert_eq!(c.to_string_cidr(), "10.220.0.0/16");
        assert_eq!(c, Cidr::parse("10.220.0.0/16").unwrap());
    }

    #[test]
    fn cidr_recusa_o_que_nao_e_endereco() {
        for mau in ["", "10.220.0.0/33", "999.1.1.1/16", "10.220.0.0/", "abc"] {
            assert!(Cidr::parse(mau).is_none(), "accepted {mau:?}");
        }
    }

    #[test]
    fn ip_derivado_cai_dentro_da_rede_e_e_estavel() {
        // Deterministic: the same id always gives the same IP — that is what lets
        // the client predict the address without asking the engine.
        let a = derive_ip_in("10.220", "deadbeef");
        assert_eq!(a, derive_ip_in("10.220", "deadbeef"));
        assert!(valid_ip_in_subnet("10.220", &a), "{a} outside 10.220/16");

        // And different ids do not collide trivially.
        assert_ne!(a, derive_ip_in("10.220", "cafebabe"));
    }

    /// ADR-0082 phase 2: the IPv6 ULA derivation is deterministic, lands in
    /// `fd00::/8`, each network has its own global prefix, different ids give
    /// different interface-ids, and the interface-id is never < 2.
    #[test]
    fn ula_derivado_e_estavel_unico_local_e_por_rede() {
        use std::net::Ipv6Addr;
        let a = derive_ula_in("backend", "deadbeef");
        // Deterministic.
        assert_eq!(a, derive_ula_in("backend", "deadbeef"));
        // It is a ULA (fd00::/8): the first octet is 0xfd.
        let parsed: Ipv6Addr = a.parse().expect("valid ULA");
        assert_eq!(parsed.octets()[0], 0xfd, "{a} is not fd00::/8");
        // Different networks → different global prefix (the first 3 hextets).
        let b = derive_ula_in("frontend", "deadbeef");
        assert_ne!(
            parsed.segments()[..3],
            b.parse::<Ipv6Addr>().unwrap().segments()[..3]
        );
        // Different ids on the SAME network → different interface-id.
        assert_ne!(a, derive_ula_in("backend", "cafebabe"));
        // Interface-id (last hextet) never < 2 (neither gateway nor anycast).
        assert!(
            derive_ula_in("backend", "0")
                .parse::<Ipv6Addr>()
                .unwrap()
                .segments()[7]
                >= 2
        );
    }

    #[test]
    fn valid_ip_in_subnet_separa_dentro_de_fora() {
        assert!(valid_ip_in_subnet("10.220", "10.220.5.9"));
        assert!(!valid_ip_in_subnet("10.220", "10.221.5.9"));
        assert!(!valid_ip_in_subnet("10.220", "not-an-ip"));
    }
    /// A real `iptables-save`, shortened. The grammar is `*table`,
    /// `:CHAIN policy [counters]`, `-A rule`.
    const SAVE: &str = r#"# Generated by iptables-save
*filter
:INPUT ACCEPT [0:0]
:FORWARD DROP [0:0]
:OUTPUT ACCEPT [0:0]
-A INPUT -i lo -j ACCEPT
-A INPUT -p tcp --dport 22 -j ACCEPT
-A FORWARD -i docker0 -o eth0 -j ACCEPT
COMMIT
*nat
:PREROUTING ACCEPT [0:0]
-A PREROUTING -p tcp --dport 80 -j DNAT --to-destination 10.0.0.5:8080
COMMIT
"#;

    #[test]
    fn conta_tabelas_cadeias_e_regras() {
        let s = parse_iptables_save(SAVE);
        assert_eq!(s.tables, 2, "*filter and *nat");
        assert_eq!(s.chains, 4, "three from filter and one from nat");
        assert_eq!(s.rules, 4);
        // The first rule serves as an example for the translation — and it has to be
        // the FIRST, not just any one: whoever reads the report compares it with the
        // file they gave it.
        assert_eq!(s.sample.as_deref(), Some("-A INPUT -i lo -j ACCEPT"));
    }

    #[test]
    fn comentarios_e_commit_nao_contam() {
        // `# Generated by` and `COMMIT` are neither table, nor chain, nor rule.
        // Counting them would inflate the report of whoever is deciding whether to migrate.
        let s = parse_iptables_save("# comment\nCOMMIT\n\n");
        assert_eq!((s.tables, s.chains, s.rules), (0, 0, 0));
        assert!(s.sample.is_none());
    }

    #[test]
    fn ficheiro_vazio_nao_rebenta() {
        assert_eq!(parse_iptables_save(""), IptablesSummary::default());
    }
    fn labels(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn an_empty_selector_selects_nothing() {
        let container = labels(&[("app", "web")]);
        assert!(!matches_labels(&container, &labels(&[])));
    }

    #[test]
    fn every_selector_key_has_to_match() {
        let container = labels(&[("app", "web"), ("tier", "frontend")]);
        assert!(matches_labels(&container, &labels(&[("app", "web")])));
        assert!(matches_labels(
            &container,
            &labels(&[("app", "web"), ("tier", "frontend")])
        ));
        // An extra key in the selector that the container does not have: fails.
        assert!(!matches_labels(
            &container,
            &labels(&[("app", "web"), ("env", "prod")])
        ));
        // Same key, different value: fails.
        assert!(!matches_labels(&container, &labels(&[("app", "worker")])));
    }

    #[test]
    fn a_container_with_no_labels_matches_no_selector() {
        assert!(!matches_labels(&labels(&[]), &labels(&[("app", "web")])));
    }

    /// Both spellings of one `/16` give the SAME DHCP address — the CIDR one
    /// gave `None`, and the VM had no predicted address.
    #[test]
    fn the_vm_dhcp_pool_is_the_same_in_both_forms_of_a_slash16() {
        let mac = "52:54:00:AB:CD:EF";
        let legacy = vm_dhcp_lease_ip("10.210", mac).unwrap();
        assert_eq!(vm_dhcp_lease_ip("10.210.0.0/16", mac), Some(legacy.clone()));
        assert!(in_vm_dhcp_pool("10.210", &legacy));
        assert!(in_vm_dhcp_pool("10.210.0.0/16", &legacy));
        // Upper and lower case are the same MAC.
        assert_eq!(
            vm_dhcp_lease_ip("10.210", &mac.to_lowercase()),
            Some(legacy)
        );
    }

    /// A network that is not a `/16` gets a pool INSIDE itself. Before, the
    /// server offered a `/24`'s VMs an address of the `/16` named by the
    /// gateway's first two octets — `172.20.254.x`, off their network.
    #[test]
    fn a_non_slash16_network_gets_a_pool_inside_itself() {
        let net = Cidr::parse("172.20.9.0/24").unwrap();
        // size/8 = 32 addresses, just below the broadcast.
        assert_eq!(vm_dhcp_pool("172.20.9.0/24"), Some((net.last() - 32, 32)));
        for i in 0..500 {
            let mac = format!("52:54:00:00:{:02x}:{:02x}", i >> 8, i & 0xff);
            let ip = vm_dhcp_lease_ip("172.20.9.0/24", &mac).unwrap();
            let a = Cidr::parse_addr(&ip).unwrap();
            assert!(net.contains(a), "{ip} outside the network");
            assert!(
                a > net.base + 1 && a < net.last(),
                "{ip} is network/gateway/broadcast"
            );
            assert!(in_vm_dhcp_pool("172.20.9.0/24", &ip));
        }
        assert!(!in_vm_dhcp_pool("172.20.9.0/24", "172.20.9.10"));
        assert!(!in_vm_dhcp_pool("172.20.9.0/24", "172.20.9.255"));
        // The smallest usable network still has one, and never the broadcast.
        let (first, size) = vm_dhcp_pool("10.9.9.0/28").unwrap();
        assert_eq!(size, 2);
        assert_eq!(first + size, Cidr::parse("10.9.9.0/28").unwrap().last());
        // Not a network: no pool.
        assert_eq!(vm_dhcp_pool("10.9.9.0/30"), None);
    }

    /// The pool's bounds, one by one: `.254.10` to `.254.249` and nothing else.
    #[test]
    fn the_dhcp_pool_bounds() {
        assert!(!in_vm_dhcp_pool("10.210", "10.210.254.9"));
        assert!(in_vm_dhcp_pool("10.210", "10.210.254.10"));
        assert!(in_vm_dhcp_pool("10.210", "10.210.254.249"));
        assert!(!in_vm_dhcp_pool("10.210", "10.210.254.250"));
        assert!(!in_vm_dhcp_pool("10.210", "10.210.253.100"));
        assert!(!in_vm_dhcp_pool("10.210", "10.211.254.100"));
    }

    /// Found by the `net_cidr` fuzz target: an id whose 8th byte falls inside
    /// a multi-byte character used to panic the byte slice. The operator names
    /// the id in `net netns attach <name>`, so this was a crash on user input.
    #[test]
    fn an_id_with_a_multibyte_character_does_not_panic() {
        for id in [
            "1z/\u{13c}x",
            "abcdefg\u{e9}",
            "\u{1f600}\u{1f600}x",
            "\u{e9}",
        ] {
            let ip = derive_ip_in("10.88", id);
            assert!(valid_ip_in_subnet("10.88", &ip), "{id:?} -> {ip}");
            let ip = derive_ip_in("172.20.4.0/22", id);
            assert!(Cidr::parse("172.20.4.0/22")
                .unwrap()
                .contains(Cidr::parse_addr(&ip).unwrap()));
        }
        // An ASCII id keeps the address it always had.
        assert_eq!(derive_ip_in("10.88", "deadbeef1234"), "10.88.190.239");
    }
}
