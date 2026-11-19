//! IP allocation invariants via **property-based testing** (Sprint 5 — Damas:
//! the invariants the types do not capture are proved by `proptest` over
//! thousands of generated inputs, rather than a handful of hand-picked examples).

use proptest::prelude::*;

/// A hexadecimal `id` like the ones Delonix generates (1..=16 nibbles).
fn hex_id() -> impl Strategy<Value = String> {
    "[0-9a-f]{1,16}"
}

proptest! {
    /// DETERMINISM: the same `(prefix, id)` always gives the same IP. A
    /// container engine depends on this (reconciliation, stable DNS).
    #[test]
    fn alloc_ip_in_is_deterministic(prefix in "10\\.[0-9]{1,2}", id in hex_id()) {
        let a = delonix_sdn::alloc_ip_in(&prefix, &id);
        let b = delonix_sdn::alloc_ip_in(&prefix, &id);
        prop_assert_eq!(a, b);
    }

    /// BOUNDS: the last octet is never `.0` (network), `.1` (gateway) nor `.255`
    /// (broadcast) — it is always in `[2, 254]`. Avoids clashes with network infra.
    #[test]
    fn alloc_ip_in_last_octet_is_a_valid_host(prefix in "10\\.[0-9]{1,2}", id in hex_id()) {
        let ip = delonix_sdn::alloc_ip_in(&prefix, &id);
        let last: u32 = ip.rsplit('.').next().unwrap().parse().unwrap();
        prop_assert!((2..=254).contains(&last), "invalid octet in {ip}");
        prop_assert!(ip.starts_with(&format!("{prefix}.")), "{ip} outside prefix {prefix}");
    }

    /// SUBNET MEMBERSHIP: `alloc_ip_cidr` always returns an address INSIDE the
    /// subnet, and never the network, the `.1` or the broadcast.
    #[test]
    fn alloc_ip_cidr_stays_in_subnet(
        b in 0u8..=255, c in 0u8..=255, plen in 8u32..=30, id in hex_id(),
    ) {
        let subnet = format!("10.{b}.{c}.0/{plen}");
        let ip = delonix_sdn::alloc_ip_cidr(&subnet, &id)
            .expect("subnet /8../30 with hosts must allocate");

        // Rebuild network/broadcast from the subnet and confirm membership.
        let to_u32 = |s: &str| -> u32 {
            let o: Vec<u32> = s.split('.').map(|p| p.parse().unwrap()).collect();
            (o[0] << 24) | (o[1] << 16) | (o[2] << 8) | o[3]
        };
        let base = to_u32(&format!("10.{b}.{c}.0")) & (u32::MAX << (32 - plen));
        let size = 1u32 << (32 - plen);
        let net = base;
        let broadcast = base + size - 1;
        let val = to_u32(&ip);

        prop_assert!(val > net && val < broadcast, "{ip} outside ]{net}, {broadcast}[");
        prop_assert!(val != net + 1, "{ip} is the gateway (.1)");
    }
}
