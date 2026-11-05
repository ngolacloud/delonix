//! The engine-wide resource summary DTO (ADR-0040 R3f). A pure, `Serialize`-only
//! data record — "how much is this node using right now" (counts, cgroup memory,
//! network bytes, disk usage per area). It lives in the foundation so the engine
//! can produce it and every surface can name it without a new adapter edge: the
//! `delonix-engine` composition fills it (`Engine::dash_summary`), `delonix-mgmt`
//! serves it as `GET /v1/dash` and reads its fields for the Prometheus gauges,
//! and `delonix-mcp` serializes it for `runtime.info`/`metrics.query`.
//!
//! It was `delonix-mgmt`'s own `dashstats::DashSummary` until R3f moved the
//! collector into the engine; the field set and their JSON keys are unchanged, so
//! `GET /v1/dash` stays byte-for-byte what it was.

/// A point-in-time snapshot of engine-wide resource usage.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DashSummary {
    pub containers_running: u64,
    pub containers_total: u64,
    pub vms_running: u64,
    pub vms_total: u64,
    pub networks_total: u64,
    pub volumes_total: u64,
    pub images_total: u64,
    pub secrets_total: u64,
    /// `memory.current` of the whole Delonix cgroup slice, in bytes.
    pub memory_bytes_used: u64,
    /// `memory.max` of the whole Delonix cgroup slice, in bytes (0 = unlimited/unknown — cgroup v2 reports the literal string `"max"`, which doesn't parse as a number).
    pub memory_bytes_limit: u64,
    /// Sum of received bytes across ALL of every running container's interfaces (not just the primary one — a `--net-connect`ed container's `eth1` counts too), cumulative since each interface's creation. `None` when the caller opted out of the netns reads — a real "0 bytes" is `Some(0)`, not `None`.
    pub network_rx_bytes: Option<u64>,
    pub network_tx_bytes: Option<u64>,
    /// How many of `containers_running` did NOT contribute to the sum above
    /// (always `0` when `network_rx_bytes`/`tx_bytes` are `None`, since then
    /// nothing was attempted at all). A container on `--net host`/`--net none`
    /// has no netns to inspect, so it is surfaced here rather than folded into
    /// the sum as a silent "+0" that would make a partial measurement look
    /// complete.
    pub network_unmeasured_containers: u64,
    /// `None` when the caller opted out of the directory walk — the walk is a
    /// full recursive `du` of several store areas and can take over a minute on
    /// a host with many/large containers, never safe inline on a scrape budget.
    pub storage_bytes_images: Option<u64>,
    pub storage_bytes_volumes: Option<u64>,
    pub storage_bytes_vm_images: Option<u64>,
    pub storage_bytes_containers: Option<u64>,
}
