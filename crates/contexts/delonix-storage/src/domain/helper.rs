//! What the storage context keeps of the pool-helper contract after the protocol
//! moved to the `delonix-pool-helper` adapter (ADR-0040 R2c): the remedy string a
//! «no helper» refusal names. It stays here, in the context, because the storage
//! providers (lvm, zfs) name it too — and a provider may depend on a context, not
//! on the helper adapter.

/// The remedy every «no helper» refusal names.
pub const HELPER_REMEDY: &str = "enable the helper (`systemctl enable --now \
     delonix-storage-helper.socket`) and add this user to the pool's `allowUsers`, or run the \
     engine as root with `mode: in-process` in the allowlist";
