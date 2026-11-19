//! The firewall summary restored after `Net` left the engine.
//!
//! Tests the PARSER, not the holder: the read itself is a read-only verb of the
//! control protocol and needs a netns up; what gets it wrong is the
//! interpretation of the `nft` lines, and that is tested with real listings.

use delonix_sdn::infra::{parse_dnat, parse_set_elements};

/// An `nft list chain` listing as the holder returns it, with the handle prefix
/// that `-a` adds.
const CHAIN: &str = r#"
table ip dlxing {
	chain pre {
		type nat hook prerouting priority dstnat; policy accept;
		iif "tap0" tcp dport 8080 dnat to 10.200.0.5:80 # handle 12
		iif "tap0" udp dport 5353 dnat to 10.200.0.9:53 # handle 13
		iif "tap0" tcp dport 5432 dnat to 10.200.0.20:5432 # handle 14
		ct state established,related accept # handle 15
	}
}
"#;

#[test]
fn extracts_the_publications_and_the_right_protocol() {
    let r = parse_dnat(CHAIN);
    assert_eq!(r.len(), 3, "should catch the three dnat rules: {r:?}");

    assert_eq!(r[0].proto, "tcp");
    assert_eq!(r[0].host_port, "8080");
    assert_eq!(r[0].to, "10.200.0.5:80");

    // UDP is the case the previous version got wrong: it read the protocol from
    // the START of the line (`starts_with("udp")`), and in an indented listing —
    // or with the `-a` prefix — the start is whitespace or `iif`, so it gave
    // `tcp` to everything. A summary that swaps UDP for TCP sends whoever is
    // debugging to look for the problem under the wrong protocol.
    assert_eq!(
        r[1].proto, "udp",
        "the protocol is read from the match, not from the start"
    );
    assert_eq!(r[1].host_port, "5353");
    assert_eq!(r[1].to, "10.200.0.9:53");

    assert_eq!(r[2].to, "10.200.0.20:5432");
}

#[test]
fn lines_without_dnat_are_ignored() {
    // The chain has rules that are not publications (`ct state … accept`), and
    // the policy. None may show up as a published port.
    assert!(parse_dnat("ct state established,related accept").is_empty());
    assert!(parse_dnat("type nat hook prerouting priority dstnat; policy accept;").is_empty());
    assert!(parse_dnat("").is_empty());
}

#[test]
fn a_truncated_rule_does_not_become_a_half_publication() {
    // With no port, or no destination, there is no rule — better to show nothing
    // than to show a port published to nowhere.
    assert!(parse_dnat("iif \"tap0\" tcp dnat to 10.200.0.5:80").is_empty());
    assert!(parse_dnat("iif \"tap0\" tcp dport 8080 dnat to ").is_empty());
}

#[test]
fn extracts_the_blocked_from_the_set() {
    let set = r#"
table ip dlxing {
	set blocked {
		type ipv4_addr
		elements = { 10.200.0.7, 10.200.0.9 }
	}
}
"#;
    assert_eq!(parse_set_elements(set), vec!["10.200.0.7", "10.200.0.9"]);
}

#[test]
fn empty_or_absent_set_gives_an_empty_list() {
    // A set with no elements is the NORMAL form of "nobody blocked" — it must not
    // be confused with a failed read.
    assert!(
        parse_set_elements("table ip dlxing {\n\tset blocked {\n\t\ttype ipv4_addr\n\t}\n}")
            .is_empty()
    );
    assert!(parse_set_elements("").is_empty());
}
