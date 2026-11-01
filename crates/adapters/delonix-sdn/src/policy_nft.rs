//! The nft lowering of the policy IR (ADR-0059 D6): the reference enforcement
//! point, the per-workload chain in the holder.
//!
//! [`chain_body`] renders a [`TargetPolicy`] as the rule lines of one address's
//! part of the chain, in the order the holder has always used: the ingress
//! user rules, the egress user rules, the namespace guardrail, then each
//! direction's default. The conntrack prologue is emitted once per chain by
//! `infra::fw_chain_prologue`, because state belongs to the flow and not to an
//! address.
//!
//! Every value that reaches the text comes from a typed IR field — an address,
//! a port number, a hashed set name — so nothing a record says is
//! interpolated raw. What nft here cannot hold with the IR's meaning is
//! refused by name instead of approximated.

use delonix_net_rules::policy::{Action, Direction, Peer, Policy, PortRange, Proto, Rule};
use delonix_networking::policy::TargetPolicy;

use crate::infra::{DLXALL6_SET, DLXALL_SET};

/// Which IP family a chain governs. ADR-0082 phase 3 made the holder's firewall a
/// single `inet` table that carries both, so every emitter here renders the layer-3
/// anchor (`ip`/`ip6`), the address-set element type (`ipv4_addr`/`ipv6_addr`) and
/// the ICMP protocol of the family it is told. [`Family::V4`] renders exactly the
/// text the IPv4-only lowering always did — so a v4 verdict is byte-for-byte
/// identical across the migration (the golden tests below are the guard).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    V4,
    V6,
}

impl Family {
    /// The nft layer-3 keyword that anchors an address match: `ip` for v4, `ip6`
    /// for v6.
    pub fn l3(self) -> &'static str {
        match self {
            Family::V4 => "ip",
            Family::V6 => "ip6",
        }
    }

    /// The nft set element type for an address of this family.
    pub fn addr_type(self) -> &'static str {
        match self {
            Family::V4 => "ipv4_addr",
            Family::V6 => "ipv6_addr",
        }
    }

    /// The nft `meta l4proto` value for ICMP of this family: `icmp` for v4,
    /// `icmpv6` for v6 — two distinct protocols, and the type keyword differs with
    /// them (`icmp type` vs `icmpv6 type`).
    fn icmp_proto(self) -> &'static str {
        match self {
            Family::V4 => "icmp",
            Family::V6 => "icmpv6",
        }
    }

    /// The host-prefix suffix nft lists a single address with, stripped so a
    /// one-host peer renders as a bare address (the counter reader matches the
    /// listed text): `/32` for v4, `/128` for v6.
    fn host_suffix(self) -> &'static str {
        match self {
            Family::V4 => "/32",
            Family::V6 => "/128",
        }
    }

    /// The name of the set holding EVERY governed address of this family — the
    /// namespace-isolation anchor. `dlxall` for v4, `dlxall6` for v6.
    fn all_set(self) -> &'static str {
        match self {
            Family::V4 => DLXALL_SET,
            Family::V6 => DLXALL6_SET,
        }
    }
}

/// The rule lines for the workload at `ip` (dotted IPv4). `ns_set` names the
/// nft set holding a namespace's workloads. `Err` names what the IR carries
/// and this lowering cannot represent.
pub fn chain_body(
    fam: Family,
    ip: &str,
    t: &TargetPolicy,
    ns_set: &dyn Fn(&str) -> String,
) -> Result<String, String> {
    let mut body = String::new();
    for r in t.ingress.rules.iter().filter(|r| !r.guardrail) {
        body.push_str(&user_line(fam, ip, Direction::Ingress, r)?);
    }
    for r in t.egress.rules.iter().filter(|r| !r.guardrail) {
        body.push_str(&user_line(fam, ip, Direction::Egress, r)?);
    }
    if t.egress.rules.iter().any(|r| r.guardrail) {
        return Err("an egress guardrail has no nft form here".into());
    }
    let guards: Vec<&Rule> = t.ingress.rules.iter().filter(|r| r.guardrail).collect();
    for (i, r) in guards.iter().enumerate() {
        let after_same_ns = i > 0
            && matches!(
                (&guards[i - 1].peer, &r.peer),
                (Peer::Namespace(a), Peer::OtherNamespaces(b)) if a == b
            );
        body.push_str(&guard_line(fam, ip, r, after_same_ns, ns_set)?);
    }
    body.push_str(&default_line(fam, ip, &t.ingress));
    body.push_str(&default_line(fam, ip, &t.egress));
    Ok(body)
}

/// The rate cap on firewall logging (§10: a log without a limit lets a flood
/// exhaust disk and CPU through the kernel log). nft's `limit` statement with
/// its default burst gates the `log`, so at most this many matches a second are
/// logged; the rest are silently not logged and the verdict is unaffected.
const FW_LOG_RATE: &str = "10/second";

/// The prefix every firewall log line carries, so an operator can grep for it.
/// A trailing space keeps it readable against the packet fields nft appends.
const FW_LOG_PREFIX: &str = "dlx-fw ";

fn verdict(a: Action) -> &'static str {
    match a {
        Action::Allow => "counter accept",
        Action::Deny => "counter drop",
    }
}

fn ports(p: PortRange) -> String {
    if p.first == p.last {
        p.first.to_string()
    } else {
        format!("{}-{}", p.first, p.last)
    }
}

/// One user rule: the own-address anchor, the peer and the L4 match, then the
/// verdict — preceded, when the rule logs, by a rate-limited `log` line that
/// shares the same match and renders NO verdict (it falls through to the verdict
/// line either way, because a `limit` that stops matching ends the rule, it does
/// not decide the packet).
fn user_line(fam: Family, ip: &str, dir: Direction, r: &Rule) -> Result<String, String> {
    if !r.stateful {
        return Err("a stateless rule (the holder chain is stateful)".into());
    }
    let l3 = fam.l3();
    let (self_dir, peer_dir) = match dir {
        Direction::Ingress => ("daddr", "saddr"),
        Direction::Egress => ("saddr", "daddr"),
    };
    // The match portion up to (but not including) the verdict — shared verbatim
    // by the optional log line and the verdict line so they govern the same
    // packets. Keeps a trailing space, like the per-match pieces below.
    let mut m = format!("{l3} {self_dir} {ip} ");
    match &r.peer {
        Peer::Any => {}
        Peer::Cidr(c) => {
            let text = c.to_string_cidr();
            // A single host renders as a bare address, as the kernel lists it —
            // the counter reader matches on the listed text.
            let text = text.strip_suffix(fam.host_suffix()).unwrap_or(&text);
            m.push_str(&format!("{l3} {peer_dir} {text} "));
        }
        Peer::Namespace(_) | Peer::OtherNamespaces(_) | Peer::Selector(_) => {
            return Err("a namespace or selector peer on a user rule".into());
        }
    }
    match (r.proto, r.ports) {
        (Proto::Tcp | Proto::Udp, p) => {
            m.push_str(if r.proto == Proto::Tcp { "tcp" } else { "udp" });
            if let Some(p) = p {
                m.push_str(&format!(" dport {}", ports(p)));
            }
            m.push(' ');
        }
        (Proto::Any, Some(p)) => {
            m.push_str(&format!("meta l4proto {{ tcp, udp }} th dport {} ", ports(p)));
        }
        (Proto::Any, None) => {}
        (Proto::Icmp, icmp_ports) => {
            // ICMP has no transport ports; a port on an ICMP rule is refused at
            // the parse (`rule_of`), so reaching here with one is a bug.
            debug_assert!(icmp_ports.is_none(), "an ICMP rule reached the lowering with a port");
            // `icmp`/`icmpv6` are distinct protocols; the family picks both the
            // `meta l4proto` value and the `type` keyword.
            let icmp = fam.icmp_proto();
            m.push_str(&format!("meta l4proto {icmp} "));
            if let Some(t) = r.icmp_type {
                m.push_str(&format!("{icmp} type {t} "));
            }
        }
    }
    let mut line = String::new();
    if r.log {
        // `limit rate <R>` passes only the first R matches a second; the counter
        // AFTER it reads back how many were logged, so the cap is observable
        // without reading the kernel log. No verdict: evaluation continues to
        // the verdict line below.
        line.push_str(&format!(
            "\t\t{m}limit rate {FW_LOG_RATE} counter log prefix \"{FW_LOG_PREFIX}\"\n"
        ));
    }
    line.push_str(&format!("\t\t{m}{}\n", verdict(r.action)));
    Ok(line)
}

/// One namespace guardrail line. The cut of other namespaces only needs the
/// `!= @<own set>` when the same namespace was not admitted just before it.
fn guard_line(
    fam: Family,
    ip: &str,
    r: &Rule,
    after_same_ns: bool,
    ns_set: &dyn Fn(&str) -> String,
) -> Result<String, String> {
    let l3 = fam.l3();
    let all = fam.all_set();
    match (&r.peer, r.action) {
        (Peer::Namespace(ns), Action::Allow) => Ok(format!(
            "\t\t{l3} daddr {ip} {l3} saddr @{} counter accept\n",
            ns_set(ns)
        )),
        (Peer::OtherNamespaces(_), Action::Deny) if after_same_ns => Ok(format!(
            "\t\t{l3} daddr {ip} {l3} saddr @{all} ct state new counter drop\n"
        )),
        (Peer::OtherNamespaces(ns), Action::Deny) => Ok(format!(
            "\t\t{l3} daddr {ip} {l3} saddr @{all} {l3} saddr != @{} ct state new counter drop\n",
            ns_set(ns)
        )),
        _ => Err("a guardrail other than the namespace isolation".into()),
    }
}

fn default_line(fam: Family, ip: &str, p: &Policy) -> String {
    let l3 = fam.l3();
    match (p.direction, p.default) {
        (_, Action::Allow) => String::new(),
        (Direction::Ingress, Action::Deny) => format!("\t\t{l3} daddr {ip} counter drop\n"),
        (Direction::Egress, Action::Deny) => format!("\t\t{l3} saddr {ip} counter drop\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use delonix_net_rules::policy::Packet;
    use delonix_net_rules::Cidr;
    use delonix_networking::policy::{from_container_fw, golden};

    const IP: &str = "10.200.0.5";

    fn ns_set(ns: &str) -> String {
        crate::infra::dlxns_set(&crate::infra::namespace_isolation_key(ns))
    }

    /// The verdict the rendered nft text gives a packet, read from the text
    /// itself: every line is parsed, every match is evaluated, and a token
    /// this evaluator does not know fails the test instead of being skipped.
    fn nft_verdict(body: &str, dir: Direction, pkt: &Packet) -> Action {
        let own = Cidr::parse_addr(IP).unwrap();
        let (src, dst) = match dir {
            Direction::Ingress => (pkt.peer, own),
            Direction::Egress => (own, pkt.peer),
        };
        let in_set = |set: &str| match &pkt.peer_namespace {
            None => false,
            Some(_) if set == DLXALL_SET => true,
            Some(ns) => ns_set(ns) == set,
        };
        let addr_matches = |value: &str, addr: u32, negate: bool| -> bool {
            let hit = if let Some(set) = value.strip_prefix('@') {
                // Sets hold workload addresses; only the peer can be in one.
                addr == pkt.peer && in_set(set)
            } else if value.contains('/') {
                Cidr::parse(value).expect("cidr").contains(addr)
            } else {
                Cidr::parse_addr(value).expect("addr") == addr
            };
            hit != negate
        };
        let port_matches = |spec: &str| -> bool {
            let Some(port) = pkt.dport else { return false };
            match spec.split_once('-') {
                Some((a, b)) => (a.parse().unwrap()..=b.parse().unwrap()).contains(&port),
                None => spec.parse::<u16>().unwrap() == port,
            }
        };
        for line in body.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            let mut i = 0;
            let mut matched = true;
            let mut verdict = None;
            while i < t.len() {
                match t[i] {
                    "ip" => {
                        let addr = match t[i + 1] {
                            "saddr" => src,
                            "daddr" => dst,
                            other => panic!("unknown ip selector {other} in {line}"),
                        };
                        let (negate, value, step) = if t[i + 2] == "!=" {
                            (true, t[i + 3], 4)
                        } else {
                            (false, t[i + 2], 3)
                        };
                        matched &= addr_matches(value, addr, negate);
                        i += step;
                    }
                    "tcp" | "udp" => {
                        let want = if t[i] == "tcp" {
                            Proto::Tcp
                        } else {
                            Proto::Udp
                        };
                        matched &= pkt.proto == want;
                        if t.get(i + 1) == Some(&"dport") {
                            matched &= port_matches(t[i + 2]);
                            i += 3;
                        } else {
                            i += 1;
                        }
                    }
                    "meta" => {
                        assert_eq!(
                            &t[i..i + 9],
                            [
                                "meta",
                                "l4proto",
                                "{",
                                "tcp,",
                                "udp",
                                "}",
                                "th",
                                "dport",
                                t[i + 8]
                            ],
                            "{line}"
                        );
                        matched &=
                            matches!(pkt.proto, Proto::Tcp | Proto::Udp) && port_matches(t[i + 8]);
                        i += 9;
                    }
                    "ct" => {
                        assert_eq!(t[i + 1], "state", "{line}");
                        matched &= match t[i + 2] {
                            "new" => pkt.new,
                            "established,related" => !pkt.new,
                            "invalid" => false,
                            other => panic!("unknown ct state {other} in {line}"),
                        };
                        i += 3;
                    }
                    "counter" => i += 1,
                    "accept" => {
                        verdict = Some(Action::Allow);
                        i += 1;
                    }
                    "drop" => {
                        verdict = Some(Action::Deny);
                        i += 1;
                    }
                    other => panic!("unknown token {other} in {line}"),
                }
            }
            if matched {
                return verdict.unwrap_or_else(|| panic!("no verdict in {line}"));
            }
        }
        // Falling off the chain returns to the dispatch, which lets it pass.
        Action::Allow
    }

    /// Every golden cell, through the TEXT the lowering renders: the prologue
    /// the holder emits once per chain, then the body. The same verdict as
    /// the reference evaluator, or the lowering is wrong (ADR-0059 D6).
    #[test]
    fn the_rendered_chain_gives_every_golden_verdict() {
        for case in golden::cases() {
            let t = from_container_fw(&case.record).unwrap();
            let body = format!(
                "{}{}",
                crate::infra::fw_chain_prologue(&case.record),
                chain_body(Family::V4, IP, &t, &ns_set).unwrap()
            );
            assert_eq!(
                nft_verdict(&body, case.direction, &case.packet),
                case.want,
                "{}:\n{body}",
                case.name
            );
        }
    }

    /// The rendered chain of every golden record is a script nft accepts.
    /// Needs `nft` and an unprivileged user namespace (`unshare -rn`). The
    /// hosted CI runner blocks the latter, so there it returns without checking
    /// anything; it runs on a developer machine and in the lab, and the verdict
    /// test above does not depend on it.
    #[test]
    fn nft_accepts_every_rendered_chain() {
        let probe = std::process::Command::new("unshare")
            .args(["-rn", "nft", "--version"])
            .output();
        if !matches!(probe, Ok(ref o) if o.status.success()) {
            return;
        }
        for case in golden::cases() {
            let t = from_container_fw(&case.record).unwrap();
            let body = format!(
                "{}{}",
                crate::infra::fw_chain_prologue(&case.record),
                chain_body(Family::V4, IP, &t, &ns_set).unwrap()
            );
            let script = format!(
                "table ip t {{\n\tset {DLXALL_SET} {{ type ipv4_addr; }}\n\tset {} {{ type ipv4_addr; }}\n\tchain c {{\n{body}\t}}\n}}\n",
                ns_set("team-a")
            );
            let mut child = std::process::Command::new("unshare")
                .args(["-rn", "nft", "--check", "-f", "-"])
                .stdin(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            use std::io::Write;
            child
                .stdin
                .take()
                .unwrap()
                .write_all(script.as_bytes())
                .unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(
                out.status.success(),
                "{}: nft refused\n{script}\n{}",
                case.name,
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }

    /// The generator this module replaced, kept verbatim as the oracle for the
    /// byte-for-byte check below (its `nft_safe` skip dropped: a record the IR
    /// refuses never reaches the comparison).
    fn legacy_body(ip: &str, fw: &delonix_model::records::ContainerFw) -> String {
        use crate::infra::{dlxns_set, fw_rule_tail, namespace_isolation_key};
        let mut body = String::new();
        if !fw.enabled {
            return body;
        }
        for r in &fw.rules {
            let self_dir = if r.dir == "out" { "saddr" } else { "daddr" };
            let tail = fw_rule_tail(r).unwrap();
            body.push_str(&format!("\t\tip {self_dir} {ip} {tail}\n"));
        }
        let nsset = dlxns_set(&namespace_isolation_key(&fw.namespace));
        let has_explicit_in = fw.policy_in == "deny" || fw.rules.iter().any(|r| r.dir == "in");
        if has_explicit_in {
            body.push_str(&format!(
                "\t\tip daddr {ip} ip saddr @{DLXALL_SET} ip saddr != @{nsset} ct state new counter drop\n"
            ));
        } else {
            body.push_str(&format!(
                "\t\tip daddr {ip} ip saddr @{nsset} counter accept\n"
            ));
            body.push_str(&format!(
                "\t\tip daddr {ip} ip saddr @{DLXALL_SET} ct state new counter drop\n"
            ));
        }
        if fw.policy_in == "deny" {
            body.push_str(&format!("\t\tip daddr {ip} counter drop\n"));
        }
        if fw.policy_out == "deny" {
            body.push_str(&format!("\t\tip saddr {ip} counter drop\n"));
        }
        body
    }

    /// The lowering emits the lines the holder emitted before it, for every
    /// golden record and a few shapes the table does not cover (egress rules
    /// before the guardrail, a peer prefix, a /32 peer, a range, a disabled
    /// record). A chain that changes text changes what `ingress ls` can read
    /// back, even with the same verdicts.
    ///
    /// One difference, and it is the only one: the old generator interleaved
    /// inbound and outbound rules in record order, the IR keeps them per
    /// direction. The order WITHIN a direction is the same, and that is the
    /// order that decides: an inbound line anchors on `ip daddr <workload>`, an
    /// outbound one on `ip saddr <workload>`, and a forwarded packet never has
    /// the workload's address at both ends, so no packet matches one of each.
    #[test]
    fn the_lowering_emits_the_text_the_holder_always_emitted() {
        let mut records: Vec<_> = golden::cases().into_iter().map(|c| c.record).collect();
        records.push(golden::fw(
            "deny",
            "deny",
            &[
                ("out", "udp", "53", "10.0.0.0/8", "allow"),
                ("in", "tcp", "8000-8010", "172.16.31.103/32", "allow"),
                ("in", "any", "9999", "", "deny"),
                ("out", "tcp", "", "*", "allow"),
                ("in", "", "", "0.0.0.0/0", "allow"),
            ],
        ));
        records.push(golden::fw("", "", &[("out", "tcp", "443", "", "allow")]));
        let mut off = golden::fw("deny", "", &[("in", "tcp", "22", "", "allow")]);
        off.enabled = false;
        records.push(off);
        for fw in records {
            let t = from_container_fw(&fw).unwrap();
            let (new, old) = (chain_body(Family::V4, IP, &t, &ns_set).unwrap(), legacy_body(IP, &fw));
            for anchor in [format!("\t\tip daddr {IP} "), format!("\t\tip saddr {IP} ")] {
                let side = |b: &str| -> Vec<String> {
                    b.lines()
                        .map(|l| format!("{l}\n"))
                        .filter(|l| l.starts_with(&anchor))
                        .collect()
                };
                assert_eq!(side(&new), side(&old), "{anchor:?} in {fw:?}");
            }
            assert_eq!(new.len(), old.len(), "{fw:?}");
        }
    }

    /// An ICMP rule lowers to `meta l4proto icmp`, with the specific type when
    /// one is set — the match the holder chain used to refuse.
    #[test]
    fn an_icmp_rule_lowers_to_the_protocol_and_the_optional_type() {
        let mut t = from_container_fw(&golden::fw("deny", "", &[])).unwrap();
        t.ingress.rules.insert(
            0,
            Rule {
                icmp_type: Some(8),
                ..Rule::new(Action::Allow, Proto::Icmp, Peer::Any)
            },
        );
        let body = chain_body(Family::V4, IP, &t, &ns_set).unwrap();
        assert!(
            body.contains(&format!(
                "ip daddr {IP} meta l4proto icmp icmp type 8 counter accept"
            )),
            "{body}"
        );
        // No type: only the protocol is matched, never a bare `icmp type`.
        t.ingress.rules[0].icmp_type = None;
        let body = chain_body(Family::V4, IP, &t, &ns_set).unwrap();
        assert!(
            body.contains(&format!("ip daddr {IP} meta l4proto icmp counter accept")),
            "{body}"
        );
        assert!(!body.contains("icmp type"), "{body}");
    }

    /// A logged rule renders a rate-limited `log` line that shares the verdict
    /// line's match and carries no verdict of its own, so logging cannot change
    /// what the rule does, and cannot flood the kernel log.
    #[test]
    fn a_logged_rule_renders_a_rate_limited_log_line_before_the_verdict() {
        let mut t = from_container_fw(&golden::fw("deny", "", &[])).unwrap();
        t.ingress.rules.insert(
            0,
            Rule {
                log: true,
                ports: Some(PortRange::one(22)),
                ..Rule::new(Action::Allow, Proto::Tcp, Peer::Any)
            },
        );
        let body = chain_body(Family::V4, IP, &t, &ns_set).unwrap();
        let log = body
            .lines()
            .find(|l| l.contains("log prefix"))
            .unwrap_or_else(|| panic!("a log line in {body}"));
        assert!(
            log.contains("limit rate"),
            "the log must be rate-limited: {log}"
        );
        assert!(log.contains("tcp dport 22"), "the log shares the match: {log}");
        assert!(
            !log.contains("accept") && !log.contains("drop"),
            "the log line carries no verdict: {log}"
        );
        assert!(
            body.contains(&format!("ip daddr {IP} tcp dport 22 counter accept")),
            "the verdict is a separate line: {body}"
        );
    }

    /// nft accepts an ICMP-type chain and a logged chain. Needs `nft` and an
    /// unprivileged user namespace; where the runner blocks that, it returns
    /// without checking (the render tests above do not depend on it).
    #[test]
    fn nft_accepts_an_icmp_and_a_logged_chain() {
        let probe = std::process::Command::new("unshare")
            .args(["-rn", "nft", "--version"])
            .output();
        if !matches!(probe, Ok(ref o) if o.status.success()) {
            return;
        }
        let mut t = from_container_fw(&golden::fw("deny", "deny", &[])).unwrap();
        t.ingress.rules.insert(
            0,
            Rule {
                icmp_type: Some(8),
                log: true,
                ..Rule::new(Action::Allow, Proto::Icmp, Peer::Any)
            },
        );
        let body = chain_body(Family::V4, IP, &t, &ns_set).unwrap();
        let script = format!(
            "table ip t {{\n\tset {DLXALL_SET} {{ type ipv4_addr; }}\n\tset {} {{ type ipv4_addr; }}\n\tchain c {{\n{body}\t}}\n}}\n",
            ns_set("team-a")
        );
        let mut child = std::process::Command::new("unshare")
            .args(["-rn", "nft", "--check", "-f", "-"])
            .stdin(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "nft refused\n{script}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// ADR-0082 phase 3: the emitters are parametrised by family. The SAME policy
    /// renders its layer-3 anchor as `ip` for v4 and `ip6` for v6 — own address,
    /// peer, namespace guard and default alike — while the verdict/match structure
    /// is otherwise untouched. The v4 rendering is byte-for-byte what the IPv4-only
    /// lowering produced (the golden tests above are the standing guard for that).
    #[test]
    fn the_family_parametrises_the_layer3_anchor() {
        // `policy_in = deny` adds the namespace guard and the default drop, so this
        // one record exercises `user_line`, `guard_line` and `default_line`.
        let fw = golden::fw("deny", "deny", &[("in", "tcp", "22", "", "allow")]);
        let t = from_container_fw(&fw).unwrap();

        let v4 = chain_body(Family::V4, "10.200.0.5", &t, &ns_set).unwrap();
        assert!(
            v4.contains("ip daddr 10.200.0.5 tcp dport 22 counter accept"),
            "{v4}"
        );
        assert!(
            v4.contains(&format!("ip daddr 10.200.0.5 ip saddr @{DLXALL_SET}")),
            "the v4 guard anchors on the v4 all-set: {v4}"
        );
        assert!(v4.contains("ip daddr 10.200.0.5 counter drop"), "{v4}");
        assert!(!v4.contains("ip6"), "v4 renders no ip6 anchor: {v4}");

        let v6 = chain_body(Family::V6, "fd00:1::5", &t, &ns_set).unwrap();
        assert!(
            v6.contains("ip6 daddr fd00:1::5 tcp dport 22 counter accept"),
            "{v6}"
        );
        assert!(
            v6.contains(&format!("ip6 daddr fd00:1::5 ip6 saddr @{DLXALL6_SET}")),
            "the v6 guard anchors on the v6 all-set: {v6}"
        );
        assert!(v6.contains("ip6 daddr fd00:1::5 counter drop"), "{v6}");

        // The only difference between the two renderings is the L3 keyword and the
        // family-specific address/set names — nothing structural.
        assert_eq!(
            v4.replace("ip ", "ip6 ")
                .replace("10.200.0.5", "fd00:1::5")
                .replace(DLXALL_SET, DLXALL6_SET),
            v6
        );

        // The pure family helpers that drive it all.
        assert_eq!((Family::V4.l3(), Family::V4.addr_type()), ("ip", "ipv4_addr"));
        assert_eq!((Family::V6.l3(), Family::V6.addr_type()), ("ip6", "ipv6_addr"));
        assert_eq!(
            (Family::V4.host_suffix(), Family::V6.host_suffix()),
            ("/32", "/128")
        );
    }

    /// An ICMP rule lowers to the family's protocol: `icmp` for v4, `icmpv6` for
    /// v6 — two distinct protocols, and the `type` keyword follows.
    #[test]
    fn an_icmp_rule_lowers_to_the_family_protocol() {
        let mut t = from_container_fw(&golden::fw("deny", "", &[])).unwrap();
        t.ingress.rules.insert(
            0,
            Rule {
                icmp_type: Some(8),
                ..Rule::new(Action::Allow, Proto::Icmp, Peer::Any)
            },
        );
        let v4 = chain_body(Family::V4, "10.200.0.5", &t, &ns_set).unwrap();
        assert!(
            v4.contains("meta l4proto icmp icmp type 8 counter accept"),
            "{v4}"
        );
        let v6 = chain_body(Family::V6, "fd00:1::5", &t, &ns_set).unwrap();
        assert!(
            v6.contains("meta l4proto icmpv6 icmpv6 type 8 counter accept"),
            "{v6}"
        );
    }

    #[test]
    fn what_the_holder_chain_cannot_hold_is_refused_by_name() {
        let mut t =
            from_container_fw(&golden::fw("", "", &[("in", "tcp", "22", "", "deny")])).unwrap();
        t.ingress.rules[0].stateful = false;
        assert!(chain_body(Family::V4, IP, &t, &ns_set)
            .unwrap_err()
            .contains("stateless"));
        t.ingress.rules[0].stateful = true;
        t.ingress.rules[0].peer = Peer::Selector(vec![("app".into(), "web".into())]);
        assert!(chain_body(Family::V4, IP, &t, &ns_set)
            .unwrap_err()
            .contains("selector"));
    }
}
