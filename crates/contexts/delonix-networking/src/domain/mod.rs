//! The networking context's pure model and rules (ADR-0040 A4): the network
//! roles and their provider ports (segment, DNS, NAT, IPAM, gateway), the policy
//! lowering, provider resolution, the ownership marks and the step ledger. All
//! pure — the actual dataplane I/O lives in the `delonix-sdn` adapter, which
//! consumes these plans; nothing here reads a file, a socket or `/proc`.

pub mod dns;
pub mod error;
pub mod gateway;
pub mod ipam;
pub mod ledger;
pub mod nat;
pub mod ownership;
pub mod policy;
pub mod resolve;
pub mod segment;
