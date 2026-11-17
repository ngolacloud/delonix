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
        handle
            .address()
            .add(index, ip, plen)
            .execute()
            .await
            .map_err(|e| Error::Command {
                context: "netlink",
                message: format!("addr_add({dev}, {cidr}): {e}"),
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{addr_add, link_exists, link_set_up};

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
}
