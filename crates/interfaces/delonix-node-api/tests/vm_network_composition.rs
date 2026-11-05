//! The composition installs the node's VM network before it serves (plan 68
//! item N3). Since R3d-e the interface's `compose()` no longer does this: the
//! provider names the `delonix-sdn` adapter, so `delonix-node-api-bin` installs
//! it into the shared `vm_registry` before serving, the same registrations the
//! CLI's `main` makes. This test stands in for that composition, installing the
//! provider itself exactly as the bin does — its own test binary, so nothing
//! else in the process registers a network first.
//!
//! Nothing here creates a VM or touches the dataplane: registering the port
//! is a struct in a `OnceLock`, and the test only asks the registry whether
//! it is there — the question the Cloud Hypervisor backend asks before it
//! attaches a guest to a network.

mod common;

/// A SHORT socket path (`sun_path` is 108 bytes), removed on every exit.
fn short_sock() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir_in("/tmp").expect("a directory under /tmp");
    let sock = dir.path().join("x.sock").to_str().unwrap().to_owned();
    (dir, sock)
}

#[test]
fn the_composition_registers_the_vm_network_a_networked_create_needs() {
    // Before the composition runs, nothing in this process registered one: the
    // assertion below can only pass because of what the composition did.
    assert!(
        delonix_compute::vm_registry::network().is_err(),
        "a VM network was registered before the composition ran; this test proves nothing"
    );
    let (_sock_dir, sock) = short_sock();
    let s = sock.clone();
    std::thread::spawn(move || {
        // The composition the bin performs before serving (R3d-e): install the
        // SDN VM-network provider into the shared registry, then serve.
        delonix_compute::vm_registry::set_network(Box::new(
            delonix_sdn::vm_network::HostVmNetwork {
                state_root: delonix_node_api::node::state_root(),
            },
        ));
        let _ = delonix_node_api::serve_blocking(
            &format!("unix://{s}"),
            common::backing(),
            common::backing_store(),
            common::backing_resolver(),
            common::backing_net(),
            common::backing_images(),
        );
    });
    for _ in 0..200 {
        if std::path::Path::new(&sock).exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        std::path::Path::new(&sock).exists(),
        "the server never bound"
    );
    if let Err(e) = delonix_compute::vm_registry::network() {
        panic!("the composition serves without a VM network: {e}");
    }
}
