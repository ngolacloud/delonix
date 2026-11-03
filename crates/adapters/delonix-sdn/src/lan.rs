//! Rootful realization of the `macvlan`/`ipvlan` LAN drivers.
//!
//! A `macvlan`/`ipvlan` network puts a container DIRECTLY on the `parent` NIC's
//! physical LAN: at attach, a sub-interface is created on the host's `parent`,
//! given the container's address, and MOVED into the container's network
//! namespace — no bridge, no veth, no holder/IPAM ledger. The container appears
//! on the wire with its own address (and, for `macvlan`, its own MAC).
//!
//! **Why this is rootful ONLY — and why that is the KERNEL, not a defect.**
//! Enslaving a physical NIC (`ip link add … link <parent> type macvlan`) needs
//! `CAP_NET_ADMIN` IN THE INITIAL network namespace — the namespace that owns
//! the real device. A rootless engine runs in a *nested* user+network
//! namespace: it is `uid 0` THERE, but it has no authority over the host's
//! `eno1`, and the kernel refuses the enslave with `EPERM`. No user-namespace
//! mapping grants a nested netns control of a device it does not own. So this
//! module realizes the link WHEN the engine runs with privilege in the init
//! netns (true root), and refuses — clearly, up front — when it does not.
//! Rootless `macvlan`/`ipvlan` is impossible by design, not unimplemented; the
//! honest alternative there is a `bridge` network.
//!
//! The split here mirrors the rest of the crate: [`plan`] and [`attach_argv`]
//! are pure (they compute WHAT to do and the exact `ip` invocations, running
//! nothing), which is what lets the whole sequence be asserted in a unit test
//! on a host without privilege; [`realize_attach`]/[`realize_detach`] are the
//! only functions that touch the host, and the first thing they do is check
//! [`can_realize`].

use crate::{Error, Network, Result};

/// `macvlan` sub-interface mode: `bridge` lets the sub-interfaces of one parent
/// talk to each other (Docker's default macvlan mode); `vepa`/`private`/
/// `passthru` are not exposed.
const MACVLAN_MODE: &str = "bridge";
/// `ipvlan` operating mode: `l2` keeps the parent as the broadcast domain and
/// gives each container its own IP while sharing the parent's MAC (Docker's
/// default ipvlan mode).
const IPVLAN_MODE: &str = "l2";

/// Is this engine privileged enough to realize a LAN link — i.e. does it hold
/// `CAP_NET_ADMIN` in the host's INITIAL network namespace?
///
/// Realization enslaves a physical NIC, which only the initial netns's owner
/// may do. [`delonix_node::is_rootless`] answers exactly this question
/// inverted: it is `false` only for a real `uid 0` in the initial user
/// namespace — the one case where the enslave is permitted.
pub fn can_realize() -> bool {
    !delonix_node::is_rootless()
}

/// The computed, side-effect-free plan for attaching a container to a LAN
/// network: WHAT to create, on which parent, with which address — nothing run
/// yet. [`attach_argv`] turns it into the exact `ip` invocations and
/// [`realize_attach`] runs them (only when [`can_realize`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanPlan {
    /// `macvlan` or `ipvlan`.
    pub driver: String,
    /// The host NIC the sub-interface is enslaved to (e.g. `eno1`).
    pub parent: String,
    /// The sub-interface's name inside the container — always `eth0`: on a LAN
    /// network it is the container's primary and only interface.
    pub ifname: String,
    /// The container's address with the LAN's prefix length (e.g.
    /// `192.168.1.37/24`).
    pub cidr: String,
    /// The default route the container receives — the LAN's real gateway. Empty
    /// when the network declared none (the container then gets an address but
    /// no default route, same as a `bridge` network without a gateway).
    pub gateway: String,
}

/// The `type …` tokens that create a sub-interface for a driver
/// (`type macvlan mode bridge` / `type ipvlan mode l2`), or `None` when the
/// driver is not a LAN driver.
pub fn link_type_args(driver: &str) -> Option<[&'static str; 4]> {
    match driver {
        crate::DRIVER_MACVLAN => Some(["type", "macvlan", "mode", MACVLAN_MODE]),
        crate::DRIVER_IPVLAN => Some(["type", "ipvlan", "mode", IPVLAN_MODE]),
        _ => None,
    }
}

/// The transient host-side name of the sub-interface, before it is moved into
/// the container's netns and renamed to `eth0`. Unique per container netns and
/// within `IFNAMSIZ` (15): the netns name is already `sanitize`d to <= 12, so
/// `lan<netns>` fits, but it is truncated defensively.
fn host_ifname(netns: &str) -> String {
    let mut n = format!("lan{netns}");
    n.truncate(15);
    n
}

/// Builds the attach plan from a LAN network record and a container id.
///
/// It does NOT check that the parent exists on the host — that is
/// [`crate::NetworkStore::create_lan`]'s job at create time, and the re-check
/// belongs in [`realize_attach`], where the host is actually touched. Fails
/// when the network is not a LAN driver, has no parent, or carries a subnet
/// that is not a CIDR with room for a host address.
///
/// The address is derived deterministically from `id` within the LAN's subnet
/// ([`crate::alloc_ip_cidr`]): a `macvlan`/`ipvlan` container sits on a physical
/// LAN whose addresses are NOT in the engine's IPAM ledger (there is no `/16`
/// the engine owns to lease from), so the address is computed, not leased — the
/// same shape the capability catalog records for an address derived rather than
/// reserved.
pub fn plan(net: &Network, id: &str) -> Result<LanPlan> {
    if !net.is_lan_driver() {
        return Err(Error::UnknownDriver(format!(
            "network '{}' is driver '{}', not a macvlan/ipvlan LAN",
            net.name, net.driver
        )));
    }
    let parent = net.parent.clone().ok_or_else(|| {
        Error::NetworkRecordCorrupted(format!(
            "macvlan/ipvlan network '{}' has no parent NIC",
            net.name
        ))
    })?;
    let plen = net
        .subnet
        .split_once('/')
        .and_then(|(_, p)| p.parse::<u32>().ok())
        .filter(|p| *p < 31)
        .ok_or_else(|| Error::InvalidLanSubnet(format!("invalid subnet '{}'", net.subnet)))?;
    let ip = crate::alloc_ip_cidr(&net.subnet, id).ok_or_else(|| {
        Error::InvalidLanSubnet(format!(
            "subnet '{}' has no room for a container address",
            net.subnet
        ))
    })?;
    Ok(LanPlan {
        driver: net.driver.clone(),
        parent,
        ifname: "eth0".to_string(),
        cidr: format!("{ip}/{plen}"),
        gateway: net.gateway.clone(),
    })
}

/// The exact `ip` invocations that realize `plan`, in order — each inner vector
/// is the argument list to `ip` (the program is prepended by the runner, as
/// everywhere else in this crate). Pure: building the sequence runs nothing,
/// which is what lets it be asserted in a unit test without privilege.
///
/// `netns` is the NAMED network namespace (under `/run/netns`) the container
/// will join — the same `ip netns`-based mechanism the bridge driver's
/// `do_attach` uses, so the container-join half downstream is identical. The
/// sub-interface is created on the host under a transient name, moved into the
/// netns, and only there renamed to `eth0`: creating it `eth0` on the host
/// would race a second concurrent attach for that name.
pub fn attach_argv(netns: &str, plan: &LanPlan) -> Vec<Vec<String>> {
    let lt = link_type_args(&plan.driver).expect("a LanPlan always carries a LAN driver");
    let hostif = host_ifname(netns);
    let s = |args: &[&str]| {
        args.iter()
            .map(|a| (*a).to_string())
            .collect::<Vec<String>>()
    };
    let mut steps = vec![
        s(&["netns", "add", netns]),
        s(&[
            "link",
            "add",
            hostif.as_str(),
            "link",
            plan.parent.as_str(),
            lt[0],
            lt[1],
            lt[2],
            lt[3],
        ]),
        s(&["link", "set", hostif.as_str(), "netns", netns]),
        s(&[
            "netns",
            "exec",
            netns,
            "ip",
            "link",
            "set",
            hostif.as_str(),
            "name",
            plan.ifname.as_str(),
        ]),
        s(&[
            "netns",
            "exec",
            netns,
            "ip",
            "addr",
            "add",
            plan.cidr.as_str(),
            "dev",
            plan.ifname.as_str(),
        ]),
        s(&[
            "netns",
            "exec",
            netns,
            "ip",
            "link",
            "set",
            plan.ifname.as_str(),
            "up",
        ]),
        s(&["netns", "exec", netns, "ip", "link", "set", "lo", "up"]),
    ];
    if !plan.gateway.is_empty() {
        steps.push(s(&[
            "netns",
            "exec",
            netns,
            "ip",
            "route",
            "add",
            "default",
            "via",
            plan.gateway.as_str(),
        ]));
    }
    steps
}

/// Realizes a LAN attach: creates the sub-interface on the parent NIC, moves it
/// into the container's `netns`, addresses it and sets its default route.
///
/// Refuses UP FRONT, with a clear `permission denied`, when the engine is
/// rootless — the kernel would refuse the enslave anyway, and a diagnosis that
/// names the real cause (CAP_NET_ADMIN in the init netns) is worth more than an
/// opaque `EPERM` from deep inside `ip link add`. Re-checks the parent exists
/// (it did at create; a NIC can be unplugged since), then runs the sequence,
/// clearing any leftovers of a failed previous attempt first.
pub fn realize_attach(netns: &str, plan: &LanPlan) -> Result<()> {
    if !can_realize() {
        return Err(delonix_model::Error::PermissionDenied(format!(
            "macvlan/ipvlan needs CAP_NET_ADMIN in the host's INITIAL network namespace to \
             enslave the parent NIC '{}'; this engine is rootless (a nested user/network \
             namespace), where the kernel forbids it — use a `bridge` network, or run the \
             node as real root",
            plan.parent
        ))
        .into());
    }
    if !crate::link_exists(&plan.parent) {
        return Err(Error::ParentNicMissing(format!(
            "parent NIC '{}' does not exist on the host",
            plan.parent
        )));
    }
    let hostif = host_ifname(netns);
    // Best-effort cleanup of a previous failed attempt (idempotent).
    crate::run_ok("ip", &["netns", "del", netns]);
    crate::run_ok("ip", &["link", "del", &hostif]);
    for argv in attach_argv(netns, plan) {
        let args: Vec<&str> = argv.iter().map(String::as_str).collect();
        crate::run("ip", &args)?;
    }
    Ok(())
}

/// Tears a LAN attach down: deletes the container's named netns (which destroys
/// the sub-interface inside it) and any host-side leftover. Best-effort and
/// idempotent — a no-op on a host that never realized the link.
pub fn realize_detach(netns: &str) -> Result<()> {
    let hostif = host_ifname(netns);
    crate::run_ok("ip", &["netns", "del", netns]);
    crate::run_ok("ip", &["link", "del", &hostif]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn macvlan_net() -> Network {
        Network::lan("lanA", "macvlan", "eno1", "192.168.1.0/24", "192.168.1.1")
    }

    #[test]
    fn plan_builds_eth0_with_the_lan_prefix_and_a_derived_address() {
        let p = plan(&macvlan_net(), "deadbeef").expect("a valid macvlan plan");
        assert_eq!(p.driver, "macvlan");
        assert_eq!(p.parent, "eno1");
        assert_eq!(p.ifname, "eth0");
        assert_eq!(p.gateway, "192.168.1.1");
        // address is inside the /24 and carries the LAN's prefix length.
        assert!(
            p.cidr.ends_with("/24"),
            "cidr must carry the plen: {}",
            p.cidr
        );
        let ip = p.cidr.split('/').next().unwrap();
        assert!(
            ip.starts_with("192.168.1."),
            "address must be in the subnet: {ip}"
        );
    }

    #[test]
    fn plan_is_deterministic_in_the_id() {
        let a = plan(&macvlan_net(), "feedface").unwrap();
        let b = plan(&macvlan_net(), "feedface").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn plan_refuses_a_non_lan_network() {
        let bridge = Network::default_bridge();
        let err = plan(&bridge, "deadbeef").expect_err("bridge is not a LAN driver");
        assert!(matches!(err, Error::UnknownDriver(_)), "got {err:?}");
    }

    #[test]
    fn plan_refuses_a_record_without_a_parent() {
        let mut net = macvlan_net();
        net.parent = None;
        let err = plan(&net, "deadbeef").expect_err("a LAN record needs a parent");
        assert!(
            matches!(err, Error::NetworkRecordCorrupted(_)),
            "got {err:?}"
        );
    }

    #[test]
    fn plan_refuses_a_subnet_that_is_not_a_cidr() {
        let net = Network::lan("lanB", "ipvlan", "eno1", "not-a-cidr", "");
        let err = plan(&net, "deadbeef").expect_err("a bad subnet is refused");
        assert!(matches!(err, Error::InvalidLanSubnet(_)), "got {err:?}");
    }

    #[test]
    fn link_type_args_are_the_docker_default_modes() {
        assert_eq!(
            link_type_args("macvlan"),
            Some(["type", "macvlan", "mode", "bridge"])
        );
        assert_eq!(
            link_type_args("ipvlan"),
            Some(["type", "ipvlan", "mode", "l2"])
        );
        assert_eq!(link_type_args("bridge"), None);
    }

    #[test]
    fn attach_argv_is_the_full_macvlan_sequence_in_order() {
        let p = LanPlan {
            driver: "macvlan".into(),
            parent: "eno1".into(),
            ifname: "eth0".into(),
            cidr: "192.168.1.5/24".into(),
            gateway: "192.168.1.1".into(),
        };
        let steps = attach_argv("dlxc", &p);
        let joined: Vec<String> = steps.iter().map(|s| s.join(" ")).collect();
        assert_eq!(
            joined,
            vec![
                "netns add dlxc",
                "link add landlxc link eno1 type macvlan mode bridge",
                "link set landlxc netns dlxc",
                "netns exec dlxc ip link set landlxc name eth0",
                "netns exec dlxc ip addr add 192.168.1.5/24 dev eth0",
                "netns exec dlxc ip link set eth0 up",
                "netns exec dlxc ip link set lo up",
                "netns exec dlxc ip route add default via 192.168.1.1",
            ]
        );
    }

    #[test]
    fn attach_argv_omits_the_default_route_when_no_gateway() {
        let p = LanPlan {
            driver: "ipvlan".into(),
            parent: "eno1".into(),
            ifname: "eth0".into(),
            cidr: "10.0.0.5/24".into(),
            gateway: String::new(),
        };
        let steps = attach_argv("dlxc", &p);
        // ipvlan l2, and NO `route add default` step.
        assert!(steps
            .iter()
            .any(|s| s.join(" ") == "link add landlxc link eno1 type ipvlan mode l2"));
        assert!(
            !steps.iter().any(|s| s.contains(&"default".to_string())),
            "a network with no gateway must not add a default route"
        );
    }

    #[test]
    fn host_ifname_stays_within_ifnamsiz() {
        // netns names are sanitized to <= 12; even at 12 the host name fits 15.
        assert!(host_ifname("abcdefghijkl").len() <= 15);
        assert_eq!(host_ifname("dlxc"), "landlxc");
    }

    /// The privilege gate, PROVEN on whatever host runs the battery.
    ///
    /// Rootless (this engine's normal mode): realization is refused up front
    /// with a `permission denied` that names the kernel constraint — the one
    /// behaviour that CAN be exercised without root. Rootful: the gate passes
    /// and the refusal, if any, comes from the (bogus) parent, NOT from
    /// privilege. Either way nothing on the host is touched, because the parent
    /// does not exist.
    #[test]
    fn realization_is_gated_on_cap_net_admin_in_the_init_netns() {
        let plan = LanPlan {
            driver: "macvlan".into(),
            parent: "dlx-no-such-nic-00".into(),
            ifname: "eth0".into(),
            cidr: "192.168.1.5/24".into(),
            gateway: "192.168.1.1".into(),
        };
        let err = realize_attach("dlxtest0", &plan).expect_err("must refuse: no such parent");
        if delonix_node::is_rootless() {
            let msg = err.to_string();
            assert!(
                msg.contains("CAP_NET_ADMIN") && msg.contains("permission denied"),
                "rootless must refuse on privilege, got: {msg}"
            );
        } else {
            assert!(
                matches!(err, Error::ParentNicMissing(_)),
                "rootful must pass the gate and fail on the parent, got: {err:?}"
            );
        }
    }

    #[test]
    fn can_realize_is_the_inverse_of_rootless() {
        assert_eq!(can_realize(), !delonix_node::is_rootless());
    }
}
