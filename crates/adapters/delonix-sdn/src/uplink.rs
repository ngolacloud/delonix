//! The ROOTFUL default outbound uplink (`--net private` with privilege).
//!
//! The rootless default (`--net private` without privilege) gives a bare
//! container its own netns with a `slirp4netns` userspace NAT (see
//! [`crate::slirp_attach`]). slirp does NOT serve rootful — with real
//! `CAP_NET_ADMIN` in the host's initial netns the right mechanism is the one
//! `docker`/`podman` use rootful: a shared Linux bridge in the host netns, a
//! `veth` pair per container (one end enslaved to the bridge, the other moved
//! into the container's netns as `eth0`), a default route via the bridge, and a
//! `nft` MASQUERADE for the bridge subnet so egress leaves through whatever host
//! NIC the route picks.
//!
//! This is deliberately NOT the holder/ingress-netns model of [`crate::infra`]
//! (that runs a bridge + single slirp INSIDE a nested netns, the rootless
//! design): here the engine is real root, so the bridge, the NAT and the
//! forwarding all live in the host's initial netns, reached directly.
//!
//! One bridge (`dlxnat0`, `10.88.0.0/16`, gateway `10.88.0.1`) is shared by every
//! rootful-default container; each gets a distinct address leased from the same
//! IPAM registry the SDN already uses. The host-side `veth` dies with the
//! container's netns (peer removal), so teardown only has to give the lease back
//! ([`reap_own_uplink`]).

use delonix_compute::Container;

use crate::{capture, link_exists, run, run_ok, Result};

/// The shared bridge for the rootful default uplink, in the HOST's initial netns.
/// Same leading `dlxnat` family of names, distinct from the ingress `delonix0`
/// (which lives inside the holder's netns) so the two never collide.
pub const UPLINK_BRIDGE: &str = "dlxnat0";
/// IPAM prefix the uplink leases container addresses from — a `/16` dedicated to
/// this path, clear of the slirp subnet (`10.0.2`), the ingress (`10.200`) and
/// custom networks (`10.201`+).
pub const UPLINK_PREFIX: &str = "10.88";
/// Prefix length of [`UPLINK_PREFIX`]'s `/16`.
pub const UPLINK_PLEN: &str = "16";
/// The bridge's own address and the containers' default gateway.
pub const UPLINK_GATEWAY: &str = "10.88.0.1";
/// The bridge subnet, as the MASQUERADE match.
pub const UPLINK_SUBNET: &str = "10.88.0.0/16";
/// The `nft` NAT table in the host netns (distinct from the ingress `dlxing`,
/// which lives inside the holder's netns).
pub const UPLINK_NAT_TABLE: &str = "delonix_nat";

/// Wires the rootful default uplink to the container whose init is `pid`, giving
/// its `eth0` the address `ip` on the shared bridge with NAT egress. Run from the
/// start's `on_started` hook (the rootful twin of [`crate::slirp_attach`]), with
/// `CAP_NET_ADMIN` in the host's initial netns.
///
/// Idempotent in its shared parts (the bridge, forwarding, the NAT rule); the
/// per-container `veth` is created fresh. On any failure after the pair exists it
/// deletes the host side, which removes the peer too — no half-wired leftover.
pub fn rootful_uplink_attach(pid: i32, ip: &str) -> Result<()> {
    if ip.is_empty() {
        return Err(crate::Error::Command {
            context: "uplink",
            message: "rootful uplink needs a bridge address but none was leased".into(),
        });
    }
    ensure_bridge()?;
    let host_if = format!("vu{pid}");
    let tmp_if = format!("vc{pid}");
    // Clear a leftover from a previous incarnation of this pid (idempotent).
    run_ok("ip", &["link", "del", &host_if]);
    // The pair; the host end onto the bridge, the other end into the container.
    run(
        "ip",
        &[
            "link", "add", &host_if, "type", "veth", "peer", "name", &tmp_if,
        ],
    )?;
    if let Err(e) = wire(pid, &host_if, &tmp_if, ip) {
        // Deleting the host side removes the peer wherever it ended up.
        run_ok("ip", &["link", "del", &host_if]);
        return Err(e);
    }
    // Forwarding and the MASQUERADE are the node's, not this container's; set
    // once, idempotently, after the first container actually needs them.
    enable_forwarding();
    ensure_nat()?;
    Ok(())
}

/// Enslaves the host side, moves the container side into `pid`'s netns and
/// addresses it there. Separated so [`rootful_uplink_attach`] can undo the pair
/// on any failure.
fn wire(pid: i32, host_if: &str, tmp_if: &str, ip: &str) -> Result<()> {
    run("ip", &["link", "set", host_if, "master", UPLINK_BRIDGE])?;
    run("ip", &["link", "set", host_if, "up"])?;
    let pid_s = pid.to_string();
    run("ip", &["link", "set", tmp_if, "netns", &pid_s])?;
    let cidr = format!("{ip}/{UPLINK_PLEN}");
    // Inside the container's netns, by pid — `eth0`, its address, its route.
    nsexec(&pid_s, &["ip", "link", "set", "lo", "up"])?;
    nsexec(&pid_s, &["ip", "link", "set", tmp_if, "name", "eth0"])?;
    nsexec(&pid_s, &["ip", "addr", "add", &cidr, "dev", "eth0"])?;
    nsexec(&pid_s, &["ip", "link", "set", "eth0", "up"])?;
    nsexec(
        &pid_s,
        &["ip", "route", "add", "default", "via", UPLINK_GATEWAY],
    )?;
    Ok(())
}

/// Runs `argv` inside the network namespace of `pid` (by pid, like the holder's
/// `join_argv`): `nsenter -t <pid> -n -- <argv>`.
fn nsexec(pid_s: &str, argv: &[&str]) -> Result<()> {
    let mut args = vec!["-t", pid_s, "-n", "--"];
    args.extend_from_slice(argv);
    run("nsenter", &args)
}

/// Creates the shared bridge if it is missing, addresses it as the gateway and
/// brings it up. Idempotent.
fn ensure_bridge() -> Result<()> {
    if !link_exists(UPLINK_BRIDGE) {
        run("ip", &["link", "add", UPLINK_BRIDGE, "type", "bridge"])?;
    }
    // Idempotent: re-adding the address on a bridge that already has it is a
    // harmless `EEXIST` we ignore.
    run_ok(
        "ip",
        &[
            "addr",
            "add",
            &format!("{UPLINK_GATEWAY}/{UPLINK_PLEN}"),
            "dev",
            UPLINK_BRIDGE,
        ],
    );
    run("ip", &["link", "set", UPLINK_BRIDGE, "up"])?;
    Ok(())
}

/// IPv4 forwarding in the host's initial netns — the bridge routes between the
/// container veths and the uplink NIC. Best-effort: a node that forbids the write
/// (an unexpected sysctl lockdown) still gets a diagnosable failure downstream.
fn enable_forwarding() {
    let _ = std::fs::write("/proc/sys/net/ipv4/ip_forward", "1");
}

/// The NAT table/chain/rule for the bridge subnet, in the host netns. Idempotent:
/// the table and base chain are `add`ed (a no-op when present) and the masquerade
/// rule is added only when the chain does not already carry the subnet.
fn ensure_nat() -> Result<()> {
    run_ok("nft", &["add", "table", "ip", UPLINK_NAT_TABLE]);
    run_ok(
        "nft",
        &[
            "add",
            "chain",
            "ip",
            UPLINK_NAT_TABLE,
            "postrouting",
            "{ type nat hook postrouting priority 100; }",
        ],
    );
    let listed = capture(
        "nft",
        &["list", "chain", "ip", UPLINK_NAT_TABLE, "postrouting"],
    )
    .unwrap_or_default();
    if !listed.contains(UPLINK_SUBNET) {
        run(
            "nft",
            &[
                "add",
                "rule",
                "ip",
                UPLINK_NAT_TABLE,
                "postrouting",
                "ip",
                "saddr",
                UPLINK_SUBNET,
                "oifname",
                "!=",
                UPLINK_BRIDGE,
                "masquerade",
            ],
        )?;
    }
    Ok(())
}

/// Returns a rootful-default-uplink container's address lease. The host-side
/// `veth` is already gone (it dies with the container's netns), so there is
/// nothing else on the wire to undo — only the IPAM lease.
///
/// A no-op for every other kind of container: rootless (slirp handles it), a
/// custom network or pod member (the teardown in [`crate::run_network`] frees
/// those), or `--net host`/`none`.
pub fn reap_own_uplink(c: &Container) {
    if !is_rootful_uplink(c) {
        return;
    }
    if let Some(ip) = &c.ip {
        crate::ipam::release(&crate::ipam::key_for_ip(ip), &c.id);
    }
}

/// Whether `c` is a rootful default-uplink container — the same conditions the
/// handler sets the launch flag under, read back off the record.
pub fn is_rootful_uplink(c: &Container) -> bool {
    c.network.is_none()
        && c.pod.is_none()
        && c.net_mode.as_deref() == Some("private")
        && !delonix_node::is_rootless()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(id: &str) -> Container {
        Container::new(
            id.into(),
            "n".into(),
            "img".into(),
            vec!["sh".into()],
            "64M".into(),
        )
    }

    /// `is_rootful_uplink` keys on the record exactly as the launch flag is set:
    /// `private`, no network, no pod. (The rootless/rootful split is the host's,
    /// asserted live, not here.)
    #[test]
    fn only_private_without_network_or_pod_is_an_uplink_container() {
        let mut c = base("a000000000000001");
        c.net_mode = Some("private".into());
        // The record shape is right; the rootless gate is environmental.
        assert_eq!(
            is_rootful_uplink(&c),
            !delonix_node::is_rootless(),
            "a bare `private` record is an uplink iff privileged"
        );

        // A custom network is never the default uplink.
        let mut net = base("a000000000000002");
        net.net_mode = Some("private".into());
        net.network = Some("mynet".into());
        assert!(!is_rootful_uplink(&net));

        // `host`/`none` are never the default uplink.
        let mut host = base("a000000000000003");
        host.net_mode = Some("host".into());
        assert!(!is_rootful_uplink(&host));
    }
}
