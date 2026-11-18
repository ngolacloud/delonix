//! KRN-0001 fatia 1 — native netlink, replacing the `ip` CLI one operation at a
//! time. This first slice migrates the read-only `link_exists` query off
//! `Command::new("ip")` + text parsing onto a typed `RTM_GETLINK` over a netlink
//! socket.
//!
//! Why start here: it is read-only (cannot corrupt state), its signature is a
//! trivial `&str -> bool`, and it removes a documented class of bug — the old
//! CLI version read `ip link show`'s *stdout* because the status was unreliable,
//! and an earlier revision that trusted the exit code was ALWAYS true, reporting
//! "ingress UP" over an empty netns. A netlink `NotFound` is unambiguous by
//! construction.
//!
//! `rtnetlink` is async-first; `delonix-sdn` is otherwise synchronous. The whole
//! async surface is confined HERE: a current-thread tokio runtime, built per
//! call, bridges back to a plain blocking `fn`. A control-plane existence check
//! runs rarely, so a runtime per call is acceptable; if a hot path ever needs
//! it, cache the runtime.

use crate::error::Error;
use futures_util::stream::TryStreamExt;
use rtnetlink::new_connection;

/// Builds a one-shot current-thread tokio runtime for a single netlink op, as an
/// `Error::Command` on failure (the same error the `ip` CLI helpers return, so
/// native calls slot into the existing `?` sites unchanged).
fn netlink_runtime(what: &str) -> crate::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .map_err(|e| Error::Command {
            context: "netlink",
            message: format!("{what}: runtime: {e}"),
        })
}

/// Whether a link named `name` exists in the CURRENT network namespace, asked
/// over netlink (`RTM_GETLINK` by name) instead of `ip link show`.
///
/// Semantics match the two CLI versions it replaces: present → `true`, absent or
/// any error → `false` (so a transport failure never masquerades as "exists").
/// Unlike them, a real transport error is distinguished from a clean `NotFound`
/// and logged, instead of being folded into the same silent `false`.
pub(crate) fn link_exists(name: &str) -> bool {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            tracing::warn!("netlink link_exists({name}): no runtime ({e})");
            return false;
        }
    };
    rt.block_on(async move {
        let (connection, handle, _) = match new_connection() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("netlink link_exists({name}): open failed ({e})");
                return false;
            }
        };
        // The connection drives the socket; it has to run alongside the request.
        tokio::spawn(connection);
        let mut links = handle.link().get().match_name(name.to_string()).execute();
        match links.try_next().await {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(rtnetlink::Error::NetlinkError(err)) => {
                // The kernel's own "no such device" is a clean negative. `code`
                // is the negated errno (e.g. -ENODEV); None means a bare ACK.
                let enodev = err
                    .code
                    .map(|c| c.get() == -libc::ENODEV)
                    .unwrap_or(false);
                if !enodev {
                    tracing::warn!("netlink link_exists({name}): {err:?}");
                }
                false
            }
            Err(e) => {
                tracing::warn!("netlink link_exists({name}): {e}");
                false
            }
        }
    })
}

/// KRN-0001 fatia 2: bring a link UP via netlink (`RTM_NEWLINK` with
/// `IFF_UP`), replacing `ip link set <name> up` on the HOST-side paths (bridge,
/// host veth end). The netns-exec variants stay on the CLI for now — they need a
/// `setns` into the target netns first (fatia 3).
///
/// Resolves the name to its interface index, then flips it up. Returns the same
/// `Error::Command` the `run("ip", …)` helper did, so callers keep using `?`.
pub(crate) fn link_set_up(name: &str) -> crate::Result<()> {
    let rt = netlink_runtime("link_set_up")?;
    rt.block_on(async move {
        let (connection, handle, _) = new_connection().map_err(|e| Error::Command {
            context: "netlink",
            message: format!("link_set_up({name}): open: {e}"),
        })?;
        tokio::spawn(connection);
        // Index by name first — `set()` addresses the link by index.
        let mut links = handle.link().get().match_name(name.to_string()).execute();
        let msg = links
            .try_next()
            .await
            .map_err(|e| Error::Command {
                context: "netlink",
                message: format!("link_set_up({name}): lookup: {e}"),
            })?
            .ok_or_else(|| Error::Command {
                context: "netlink",
                message: format!("link_set_up({name}): no such link"),
            })?;
        let index = msg.header.index;
        // rtnetlink 0.24: `set` takes a built LinkMessage; `LinkUnspec` by index
        // with the UP flag is the typed equivalent of `ip link set <if> up`.
        let message = rtnetlink::LinkUnspec::new_with_index(index).up().build();
        handle
            .link()
            .set(message)
            .execute()
            .await
            .map_err(|e| Error::Command {
                context: "netlink",
                message: format!("link_set_up({name}): set up: {e}"),
            })
    })
}

/// KRN-0001 fatia 3: add an address to a HOST-side link via netlink
/// (`RTM_NEWADDR`), replacing `ip addr add <ip>/<plen> dev <name>`. `cidr` is the
/// `ip/plen` form the call-sites already build. The netns-exec variants (adding
/// an address INSIDE the container) stay on the CLI — they need `setns` first.
pub(crate) fn addr_add(dev: &str, cidr: &str) -> crate::Result<()> {
    addr_add_flags(dev, cidr, false)
}

/// As [`addr_add`], but `nodad` sets `IFA_F_NODAD` — the `ip -6 addr add … nodad`
/// of the v6 paths. The container's v6 ULA is deterministic and anti-spoof is
/// keyed on it, so Duplicate Address Detection only adds latency.
pub(crate) fn addr_add_flags(dev: &str, cidr: &str, nodad: bool) -> crate::Result<()> {
    let (ip_str, plen_str) = cidr.split_once('/').ok_or_else(|| Error::Command {
        context: "netlink",
        message: format!("addr_add({dev}, {cidr}): not an ip/plen"),
    })?;
    let ip: std::net::IpAddr = ip_str.parse().map_err(|e| Error::Command {
        context: "netlink",
        message: format!("addr_add({dev}): bad ip {ip_str}: {e}"),
    })?;
    let plen: u8 = plen_str.parse().map_err(|e| Error::Command {
        context: "netlink",
        message: format!("addr_add({dev}): bad prefix {plen_str}: {e}"),
    })?;
    let rt = netlink_runtime("addr_add")?;
    let dev = dev.to_string();
    rt.block_on(async move {
        let (connection, handle, _) = new_connection().map_err(|e| Error::Command {
            context: "netlink",
            message: format!("addr_add({dev}): open: {e}"),
        })?;
        tokio::spawn(connection);
        let mut links = handle.link().get().match_name(dev.clone()).execute();
        let msg = links
            .try_next()
            .await
            .map_err(|e| Error::Command {
                context: "netlink",
                message: format!("addr_add({dev}): lookup: {e}"),
            })?
            .ok_or_else(|| Error::Command {
                context: "netlink",
                message: format!("addr_add({dev}): no such link"),
            })?;
        let index = msg.header.index;
        let mut req = handle.address().add(index, ip, plen);
        if nodad {
            req.message_mut().header.flags |=
                rtnetlink::packet_route::address::AddressHeaderFlags::Nodad;
        }
        req.execute().await.map_err(|e| Error::Command {
            context: "netlink",
            message: format!("addr_add({dev}, {cidr}): {e}"),
        })
    })
}

/// KRN-0001 fatia 4: add a DEFAULT route via `gateway` over netlink
/// (`RTM_NEWROUTE`), replacing `ip route add default via <gw>`. v4/v6 is chosen
/// from the gateway form. Meant to run inside [`with_netns`] for the container's
/// own table.
pub(crate) fn route_add_default(gateway: &str) -> crate::Result<()> {
    use std::net::IpAddr;
    let gw: IpAddr = gateway.parse().map_err(|e| Error::Command {
        context: "netlink",
        message: format!("route_add_default({gateway}): bad gateway: {e}"),
    })?;
    let rt = netlink_runtime("route_add_default")?;
    let gateway = gateway.to_string();
    rt.block_on(async move {
        let (connection, handle, _) = new_connection().map_err(|e| Error::Command {
            context: "netlink",
            message: format!("route_add_default({gateway}): open: {e}"),
        })?;
        tokio::spawn(connection);
        let route = match gw {
            IpAddr::V4(v4) => rtnetlink::RouteMessageBuilder::<std::net::Ipv4Addr>::new()
                .gateway(v4)
                .build(),
            IpAddr::V6(v6) => rtnetlink::RouteMessageBuilder::<std::net::Ipv6Addr>::new()
                .gateway(v6)
                .build(),
        };
        handle
            .route()
            .add(route)
            .execute()
            .await
            .map_err(|e| Error::Command {
                context: "netlink",
                message: format!("route_add_default({gateway}): {e}"),
            })
    })
}

/// KRN-0001 fatia 4: run `f` with THIS thread moved into the network namespace
/// at `netns_path` (`/run/netns/<name>`), then let the thread exit — the same
/// throwaway-thread `setns` pattern as `cni::set_netns_sysctls`. A netlink socket
/// opened inside `f` binds to the target netns, so `link_set_up`/`addr_add` act
/// on the CONTAINER's interfaces (`eth0`, `lo`) instead of shelling out to
/// `ip netns exec`. Only this thread switches netns; the rest of the process is
/// untouched, which is why it must run on a dedicated thread.
pub(crate) fn with_netns<F, T>(netns_path: &str, f: F) -> crate::Result<T>
where
    F: FnOnce() -> crate::Result<T> + Send + 'static,
    T: Send + 'static,
{
    use std::os::fd::AsRawFd as _;
    let path = netns_path.to_string();
    std::thread::spawn(move || -> crate::Result<T> {
        let file = std::fs::File::open(&path).map_err(|e| Error::Command {
            context: "netns-open",
            message: format!("{path}: {e}"),
        })?;
        // SAFETY: setns on a valid netns fd; CLONE_NEWNET changes only THIS
        // thread, which exits right after `f` returns.
        if unsafe { libc::setns(file.as_raw_fd(), libc::CLONE_NEWNET) } != 0 {
            return Err(Error::Command {
                context: "netns-setns",
                message: format!("{path}: {}", std::io::Error::last_os_error()),
            });
        }
        f()
    })
    .join()
    .map_err(|_| Error::Command {
        context: "netns",
        message: "the netns thread panicked".into(),
    })?
}

/// KRN-0001 fatia 5 (the ROUTE family): dump the routes that carry routing
/// protocol `proto` in the netns at `netns_path`, as their destination prefixes.
/// This is the typed `RTM_GETROUTE` equivalent of
/// `ip netns exec <ns> ip route show proto <proto>` + `route_plan::parse_route_dests`
/// — the READ half of the query→diff→mutate that keeps a workload's table in
/// agreement with the declared network routes.
///
/// Only v4 is dumped: `Cidr` is v4 by design (ADR-0013 tier A) and the routes
/// this engine installs are v4, exactly as the text version was (`-4`/`/32`).
/// Routes without a destination attribute (the default route) are skipped — they
/// never carry our private protocol and have no prefix to compare.
pub(crate) fn route_list_proto(netns_path: &str, proto: u8) -> crate::Result<Vec<crate::Cidr>> {
    use rtnetlink::packet_route::route::{RouteAddress, RouteAttribute, RouteProtocol};
    use std::net::Ipv4Addr;
    with_netns(netns_path, move || {
        let rt = netlink_runtime("route_list_proto")?;
        rt.block_on(async move {
            let (connection, handle, _) = new_connection().map_err(|e| Error::Command {
                context: "netlink",
                message: format!("route_list_proto: open: {e}"),
            })?;
            tokio::spawn(connection);
            // An empty-destination request becomes an `NLM_F_DUMP` of every v4
            // route; we filter by protocol in-process, the same way the old CLI
            // asked the kernel to filter with `proto <proto>`.
            let dump = rtnetlink::RouteMessageBuilder::<Ipv4Addr>::new().build();
            let mut routes = handle.route().get(dump).execute();
            let mut out = Vec::new();
            while let Some(msg) = routes.try_next().await.map_err(|e| Error::Command {
                context: "netlink",
                message: format!("route_list_proto: dump: {e}"),
            })? {
                if msg.header.protocol != RouteProtocol::Other(proto) {
                    continue;
                }
                let Some(v4) = msg.attributes.iter().find_map(|a| match a {
                    RouteAttribute::Destination(RouteAddress::Inet(v4)) => Some(*v4),
                    _ => None,
                }) else {
                    continue;
                };
                let len = msg.header.destination_prefix_length;
                // Host bits cleared, as `Cidr::parse` would — so equality with a
                // planned `WorkloadRoute.dest` (also a masked `Cidr`) holds.
                let mask = if len == 0 { 0 } else { u32::MAX << (32 - u32::from(len)) };
                out.push(crate::Cidr {
                    base: u32::from(v4) & mask,
                    len,
                });
            }
            Ok(out)
        })
    })
}

/// KRN-0001 fatia 5: remove the route to `dest` that carries protocol `proto`
/// from the netns at `netns_path` — the typed `RTM_DELROUTE` replacing
/// `ip netns exec <ns> ip route del <dest> proto <proto>`. The protocol is set in
/// the message so the kernel removes ONLY a route this engine installed, never a
/// connected or default route that happens to share the prefix.
pub(crate) fn route_del_proto(netns_path: &str, dest: crate::Cidr, proto: u8) -> crate::Result<()> {
    use rtnetlink::packet_route::route::RouteProtocol;
    use std::net::Ipv4Addr;
    with_netns(netns_path, move || {
        let rt = netlink_runtime("route_del_proto")?;
        rt.block_on(async move {
            let (connection, handle, _) = new_connection().map_err(|e| Error::Command {
                context: "netlink",
                message: format!("route_del_proto: open: {e}"),
            })?;
            tokio::spawn(connection);
            let route = rtnetlink::RouteMessageBuilder::<Ipv4Addr>::new()
                .destination_prefix(Ipv4Addr::from(dest.base), dest.len)
                .protocol(RouteProtocol::Other(proto))
                .build();
            handle
                .route()
                .del(route)
                .execute()
                .await
                .map_err(|e| Error::Command {
                    context: "netlink",
                    message: format!("route_del_proto({}/{}): {e}", dest.base, dest.len),
                })
        })
    })
}

/// KRN-0001 fatia 5: create-or-replace the route «`dest` via `gateway` dev `dev`»
/// carrying protocol `proto` in the netns at `netns_path` — the typed
/// `RTM_NEWROUTE` with `NLM_F_CREATE | NLM_F_REPLACE` replacing
/// `ip netns exec <ns> ip route replace <dest> via <gw> dev <dev> proto <proto>`.
/// `replace()` keeps the idempotency the CLI `replace` gave: a second pass over
/// an unchanged plan converges without error.
///
/// The output interface is addressed by index, so the device name is resolved
/// INSIDE the netns first (`eth1` in the container, not on the host).
pub(crate) fn route_replace(
    netns_path: &str,
    dest: crate::Cidr,
    gateway: &str,
    dev: &str,
    proto: u8,
) -> crate::Result<()> {
    use rtnetlink::packet_route::route::RouteProtocol;
    use std::net::Ipv4Addr;
    let gw: Ipv4Addr = gateway.parse().map_err(|e| Error::Command {
        context: "netlink",
        message: format!("route_replace: bad gateway {gateway}: {e}"),
    })?;
    // Owned copies: the closure crosses a thread boundary (`with_netns`) and so
    // must be `'static` — it cannot borrow the caller's `&str`s.
    let gateway = gateway.to_string();
    let dev = dev.to_string();
    with_netns(netns_path, move || {
        let rt = netlink_runtime("route_replace")?;
        rt.block_on(async move {
            let (connection, handle, _) = new_connection().map_err(|e| Error::Command {
                context: "netlink",
                message: format!("route_replace: open: {e}"),
            })?;
            tokio::spawn(connection);
            // Resolve the device name to its index in THIS (container) netns.
            let mut links = handle.link().get().match_name(dev.clone()).execute();
            let msg = links
                .try_next()
                .await
                .map_err(|e| Error::Command {
                    context: "netlink",
                    message: format!("route_replace({dev}): lookup: {e}"),
                })?
                .ok_or_else(|| Error::Command {
                    context: "netlink",
                    message: format!("route_replace({dev}): no such link"),
                })?;
            let index = msg.header.index;
            let route = rtnetlink::RouteMessageBuilder::<Ipv4Addr>::new()
                .destination_prefix(Ipv4Addr::from(dest.base), dest.len)
                .gateway(gw)
                .output_interface(index)
                .protocol(RouteProtocol::Other(proto))
                .build();
            handle
                .route()
                .add(route)
                .replace()
                .execute()
                .await
                .map_err(|e| Error::Command {
                    context: "netlink",
                    message: format!(
                        "route_replace({}/{} via {gateway} dev {dev}): {e}",
                        dest.base, dest.len
                    ),
                })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{addr_add, link_exists, link_set_up, route_del_proto, route_list_proto, route_replace};
    use crate::Cidr;

    /// KRN-0001 fatia 1: the native `RTM_GETLINK` query answers like `ip link
    /// show` did — loopback always exists in any netns, an invented name never
    /// does. This is the regression guard: a transport error or the old
    /// stdout-sniffing bug would make the negative case wrongly return `true`.
    #[test]
    fn loopback_exists_and_an_invented_name_does_not() {
        assert!(link_exists("lo"), "loopback must exist in this netns");
        assert!(
            !link_exists("dlx-noexist-zzz0"),
            "an invented link name must resolve to a clean NotFound (false)"
        );
    }

    /// KRN-0001 fatia 2: `set up` on a MISSING link must error cleanly (the
    /// lookup fails before any permission check), never panic or hang. The
    /// success path needs CAP_NET_ADMIN — a plain test process lacks it, so it
    /// is proven by the network e2e (bridge/veth up in the holder netns), not
    /// here.
    #[test]
    fn set_up_on_a_missing_link_errors() {
        assert!(
            link_set_up("dlx-noexist-zzz0").is_err(),
            "`set up` on a missing link must be an error"
        );
    }

    /// KRN-0001 fatia 3: a malformed CIDR and a missing link both error before
    /// any privileged syscall (so they run without CAP_NET_ADMIN). The success
    /// path is proven by the network e2e (the bridge gets its gateway address).
    #[test]
    fn addr_add_rejects_a_bad_cidr_and_a_missing_link() {
        assert!(addr_add("lo", "not-a-cidr").is_err(), "non ip/plen must error");
        assert!(
            addr_add("dlx-noexist-zzz0", "10.0.0.1/24").is_err(),
            "a missing link must error"
        );
    }

    /// KRN-0001 fatia 5: a netns path that does not exist errors before any
    /// netlink traffic — `with_netns` cannot `setns` into a file that is not
    /// there. This guards the read half: a transport/setns failure must surface
    /// as an error (so the caller can default to "nothing read" and log), never
    /// as a silent empty table that would make the diff delete real routes. The
    /// success path needs a live netns and CAP_NET_ADMIN and is proven by the
    /// network e2e.
    #[test]
    fn route_list_proto_on_a_missing_netns_errors() {
        assert!(
            route_list_proto("/run/netns/dlx-noexist-zzz0", 118).is_err(),
            "a missing netns must error, not report an empty route table"
        );
    }

    /// KRN-0001 fatia 5: the mutate half rejects bad inputs before any privileged
    /// syscall. A non-existent netns fails the `setns`; a non-parsable gateway
    /// fails even earlier, before the netns thread. Both run without
    /// CAP_NET_ADMIN; the convergence path is proven by the network e2e.
    #[test]
    fn route_del_and_replace_reject_bad_inputs() {
        let dest = Cidr::parse("10.82.0.0/16").unwrap();
        assert!(
            route_del_proto("/run/netns/dlx-noexist-zzz0", dest, 118).is_err(),
            "del in a missing netns must error"
        );
        assert!(
            route_replace("/run/netns/whatever", dest, "not-an-ip", "eth1", 118).is_err(),
            "a non-parsable gateway must error before touching any netns"
        );
        assert!(
            route_replace("/run/netns/dlx-noexist-zzz0", dest, "10.81.0.1", "eth1", 118).is_err(),
            "replace in a missing netns must error"
        );
    }
}
