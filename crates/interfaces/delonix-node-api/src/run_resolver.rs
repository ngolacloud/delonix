//! `RunResolver` — the container-create resolution as this interface needs it
//! (R3d-e, ADR-0040 A5). The interface holds this port instead of composing the
//! `delonix-oci`/`delonix-volume`/`delonix-linux` host adapters a
//! `CreateContainer` resolution needs: `delonix-node-api-bin` composes the four
//! host-structs and hands the server an implementor, so the forbidden
//! interface→adapter edge stays in the bin — the same shape as
//! [`crate::vms::VmPort`] and [`crate::store::StorePort`].

use std::path::Path;

/// The container-create resolution the node API drives through (R3d-e). Lowers a
/// `CreateContainer`'s [`delonix_compute::RunOpts`] to the engine's
/// [`delonix_compute::run::Resolved`] through the SAME `resolve_run` pipeline
/// every other entry point builds a container from, with the host adapters the
/// composition injects. The implementor opens the image store it resolves
/// against under `root` itself; errors arrive as the shared
/// [`delonix_model::Error`] the engine and this crate both speak.
pub trait RunResolver: Send + Sync {
    /// Resolve `o` into the engine's `Resolved` (image, rootfs, devices, runtime)
    /// for a container `id`/`name`/`namespace`, under the state `root`.
    fn resolve_run(
        &self,
        root: &Path,
        o: &delonix_compute::RunOpts,
        id: String,
        name: String,
        namespace: String,
    ) -> Result<delonix_compute::run::Resolved, delonix_model::Error>;
}

/// A [`RunResolver`] that composes nothing — the backing the default
/// [`crate::router`] hands the server when no engine is injected. A create never
/// reaches it in practice (the router's own tests drive refusals, which
/// `spec_to_run_opts` decides before any resolution), but the port is total, so
/// it answers `Unavailable` — the same answer [`crate::service`]'s empty `NoVms`
/// backing gives an uncomposed VM create. Production hands in the bin's
/// engine-composed resolver.
pub(crate) struct NoResolver;

impl RunResolver for NoResolver {
    fn resolve_run(
        &self,
        _root: &Path,
        _o: &delonix_compute::RunOpts,
        _id: String,
        _name: String,
        _namespace: String,
    ) -> Result<delonix_compute::run::Resolved, delonix_model::Error> {
        Err(delonix_model::Error::Unavailable(
            "no run resolver is composed (use delonix-node-api-bin)".into(),
        ))
    }
}

/// A [`RunResolver`] that composes the real host adapters at a root, for the
/// crate's own tests only — the same shape as [`crate::vms::LocalVmReads`] and
/// [`crate::store::LocalStore`]. The server never builds this; production hands
/// in the bin's engine-composed resolver. It opens the OCI image store under
/// `root` itself, exactly as `service.rs`'s create handler did before the port
/// existed.
#[cfg(test)]
pub(crate) struct LocalResolver(pub std::path::PathBuf);

#[cfg(test)]
impl RunResolver for LocalResolver {
    fn resolve_run(
        &self,
        root: &Path,
        o: &delonix_compute::RunOpts,
        id: String,
        name: String,
        namespace: String,
    ) -> Result<delonix_compute::run::Resolved, delonix_model::Error> {
        use delonix_oci::rootfs_user::UserLookupError;
        let images = delonix_oci::ImageStore::open(&self.0)?;
        let announce_pull = |_r: &str| {};
        // No progress line: run the unpacking straight through.
        let unpacking =
            |prepare: &mut dyn FnMut() -> delonix_model::Result<std::path::PathBuf>| prepare();
        let user_error = |e: UserLookupError| {
            delonix_model::Error::Invalid(match e {
                UserLookupError::EmptyUser => "spec.user: empty user".to_string(),
                UserLookupError::NoSuchUser(u) => {
                    format!("spec.user: user '{u}' does not exist in the image (/etc/passwd)")
                }
                UserLookupError::NoSuchGroup(g) => {
                    format!("spec.user: group '{g}' does not exist in the image (/etc/group)")
                }
            })
        };
        let apparmor_disabled = |p: &str| {
            delonix_linux::Error::ApparmorUnavailable(format!(
                "spec.security.apparmor_profile '{p}': this host has no AppArmor to apply it"
            ))
        };
        delonix_compute::run::resolve_run(
            o,
            id,
            name,
            namespace,
            false,
            &delonix_oci::run_images::HostImages {
                store: &images,
                announce_pull: &announce_pull,
                unpacking: &unpacking,
                user_error: &user_error,
            },
            &delonix_volume::HostVolumes {
                root: root.to_path_buf(),
            },
            &delonix_linux::cdi::HostDevices,
            &delonix_linux::run_host::HostRuntime {
                state_root: root.to_path_buf(),
                apparmor_disabled: &apparmor_disabled,
            },
        )
    }
}
