//! The run specification checked on its own, before any side effect.
//!
//! Every check here reads only the [`RunOpts`] — no store, no host, no network —
//! so a combination of flags that cannot mean anything is refused before an
//! image is pulled, an address is leased or a process exists.

use crate::RunOpts;
use delonix_model::{Error, Result};

/// The container a `--net container:<name>` value names — Docker/Podman parity:
/// the new container joins the network namespace of an already-running one
/// (same IP, same published ports). `None` for every other `--net` value. PURE.
///
/// `container:` is a THIRD built-in mode alongside `host`/`none`: it names no
/// SDN network, so [`custom_net_name`] must exclude it (otherwise `container:db`
/// would be taken for a network literally named `container:db` and fail at
/// `check_network`).
pub fn net_container_target(net: &str) -> Option<&str> {
    net.strip_prefix("container:").filter(|n| !n.is_empty())
}

/// The custom network a `--net` value names: `host`, `none` and
/// `container:<name>` are the three built-in modes, anything else is a network.
/// PURE.
pub fn custom_net_name(net: &str) -> Option<String> {
    (net != "host" && net != "none" && net_container_target(net).is_none()).then(|| net.to_string())
}

/// Refuses the flag combinations that cannot mean anything. PURE.
///
/// `--net-bps` without a custom network was checked only AFTER the workload had
/// been created: in the foreground the process ran to completion and the command
/// then failed, leaving the record behind; with `-d` the supervisor returned
/// first and the flag was accepted and silently ignored. Both measured.
pub fn check_run_opts(o: &RunOpts) -> Result<()> {
    // `--net container:<name>`: the netns (its address, its routes, its ports)
    // belongs to the TARGET, so every flag that would configure a network of
    // OUR own is incompatible — refused here, before anything is created, with a
    // message that names the target rather than the generic "requires --net
    // <network>" the checks below would give. Docker/Podman refuse the same set.
    if let Some(target) = net_container_target(&o.net) {
        if !o.ports.is_empty() {
            return Err(Error::Invalid(format!(
                "-p/--publish is not compatible with --net container:{target} — the network \
                 namespace belongs to '{target}'; publish the port when you run '{target}'"
            )));
        }
        if o.expose.is_some() {
            return Err(Error::Invalid(format!(
                "--expose is not compatible with --net container:{target} — the netns and its \
                 routes belong to '{target}'"
            )));
        }
        if o.ip.is_some() {
            return Err(Error::Invalid(format!(
                "--ip is not compatible with --net container:{target} — the address is '{target}'s"
            )));
        }
        if o.net_bps.is_some() || o.net_burst.is_some() {
            return Err(Error::Invalid(format!(
                "--net-bps/--net-burst are not compatible with --net container:{target} — shaping \
                 is a property of '{target}'s netns"
            )));
        }
        return Ok(());
    }
    if o.net_burst.is_some() && o.net_bps.is_none() {
        return Err(Error::Invalid(
            "--net-burst only makes sense together with --net-bps".into(),
        ));
    }
    let custom = custom_net_name(&o.net);
    if o.net_bps.is_some() && custom.is_none() {
        return Err(Error::Invalid(
            "--net-bps only applies with `--net <network>` (shaping is on the ingress veth)".into(),
        ));
    }
    // `--ip` with `--net host/none` has no SDN address to fix, and accepting it
    // there would silently do nothing.
    if o.ip.is_some() && custom.is_none() {
        return Err(Error::Invalid(
            "--ip requires --net <network> — a fixed address only makes sense on the SDN, not with --net host/none"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(net: &str) -> RunOpts {
        RunOpts {
            net: net.into(),
            ..Default::default()
        }
    }

    #[test]
    fn host_and_none_are_not_custom_networks() {
        assert_eq!(custom_net_name("host"), None);
        assert_eq!(custom_net_name("none"), None);
        assert_eq!(custom_net_name("pnet"), Some("pnet".to_string()));
    }

    #[test]
    fn container_join_is_a_third_built_in_mode_not_a_network() {
        // `container:<name>` names a container to share a netns with, never a
        // custom SDN network: `custom_net_name` must not route it to `attach`.
        assert_eq!(net_container_target("container:db"), Some("db"));
        assert_eq!(net_container_target("container:team/db"), Some("team/db"));
        assert_eq!(custom_net_name("container:db"), None);
        // Degenerate / unrelated values are not a join.
        assert_eq!(net_container_target("container:"), None);
        assert_eq!(net_container_target("host"), None);
        assert_eq!(net_container_target("pnet"), None);
        // A real network whose name merely contains the word is untouched.
        assert_eq!(
            custom_net_name("my-container-net"),
            Some("my-container-net".to_string())
        );
    }

    #[test]
    fn container_join_refuses_its_own_network_flags() {
        let join = || opts("container:db");
        // -p / --publish
        let mut o = join();
        o.ports = vec!["8080:80".into()];
        assert!(check_run_opts(&o).is_err());
        // --expose
        let mut o = join();
        o.expose = Some(80);
        assert!(check_run_opts(&o).is_err());
        // --ip
        let mut o = join();
        o.ip = Some("10.0.0.9".into());
        assert!(check_run_opts(&o).is_err());
        // --net-bps / --net-burst
        let mut o = join();
        o.net_bps = Some("1mbit".into());
        assert!(check_run_opts(&o).is_err());
        // Nothing of ours asked for: accepted.
        assert!(check_run_opts(&join()).is_ok());
    }

    #[test]
    fn net_bps_needs_a_custom_network() {
        let mut o = opts("none");
        o.net_bps = Some("1mbit".into());
        assert!(check_run_opts(&o).is_err());
        o.net = "host".into();
        assert!(check_run_opts(&o).is_err());
        o.net = "pnet".into();
        assert!(check_run_opts(&o).is_ok());
    }

    #[test]
    fn net_burst_needs_net_bps() {
        let mut o = opts("pnet");
        o.net_burst = Some("32kb".into());
        assert!(check_run_opts(&o).is_err());
        o.net_bps = Some("1mbit".into());
        assert!(check_run_opts(&o).is_ok());
    }

    #[test]
    fn a_fixed_ip_needs_a_custom_network() {
        let mut o = opts("host");
        o.ip = Some("10.200.0.9".into());
        assert!(check_run_opts(&o).is_err());
        o.net = "pnet".into();
        assert!(check_run_opts(&o).is_ok());
        assert!(check_run_opts(&opts("host")).is_ok());
    }
}
