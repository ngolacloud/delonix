//! The node context's pure value types (ADR-0040 A4): data and invariants with
//! no I/O. The `super::app` use cases read the host and produce these; nothing
//! here touches `/proc`, the clock, a process or a syscall.

/// IS this host — the parallel of Proxmox's `GET /nodes/{node}/status`.
///
/// Discipline (same as the node-api's `/v1/node/capacity`): a number that could
/// not be measured names itself in [`HostCapacity::unmeasured`] and stays `0`,
/// never a silent zero. A number that is LEGITIMATELY zero (a host with no
/// swap) is a real `0` and is NOT unmeasured.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostCapacity {
    /// Logical CPUs the host reports (`processor` lines in `/proc/cpuinfo`).
    pub cpus: u64,
    /// The CPU model string (first `model name` in `/proc/cpuinfo`).
    pub cpu_model: String,
    /// Total RAM in bytes (`MemTotal`).
    pub mem_total_bytes: u64,
    /// RAM in use in bytes (`MemTotal - MemAvailable`).
    pub mem_used_bytes: u64,
    /// Total swap in bytes (`SwapTotal`); a real `0` on a host without swap.
    pub swap_total_bytes: u64,
    /// Swap in use in bytes (`SwapTotal - SwapFree`).
    pub swap_used_bytes: u64,
    /// Total bytes of the filesystem holding the VM images.
    pub disk_total_bytes: u64,
    /// Bytes in use on that filesystem.
    pub disk_used_bytes: u64,
    /// Names of the counts above that could not be measured on this host.
    pub unmeasured: Vec<&'static str>,
}

/// Summary of the virtualization environment of the host where Delonix runs.
#[derive(Debug, Clone, Default)]
pub struct VirtInfo {
    /// `true` if we are inside a VM (any hypervisor).
    pub virtualized: bool,
    /// Hypervisor name: `kvm`, `qemu`, `xen`, `vmware`, `virtualbox`,
    /// `hyper-v`, `unknown` or `none` (bare-metal).
    pub hypervisor: String,
    /// `true` when the guest is KVM-accelerated (maximum performance path).
    pub is_kvm: bool,
    /// `/dev/kvm` present (KVM acceleration available — e.g. nested virtualization).
    pub kvm_accel: bool,
    /// Network interfaces served by `virtio_net` (e.g. `enp1s0`).
    pub virtio_net: Vec<String>,
    /// Disks served by `virtio_blk` (e.g. `vda`).
    pub virtio_blk: Vec<String>,
    /// Total number of devices on the virtio bus (`/sys/bus/virtio/devices`).
    pub virtio_count: usize,
}
