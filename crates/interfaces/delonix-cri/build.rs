// Generates the CRI v1 (Kubernetes) gRPC stubs from `proto/api.proto`
// (already stripped of gogoproto annotations) via tonic-build/prost. C2.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(true) // server + client (the client serves the tests: see `tests/grpc_status.rs`)
        .compile_protos(&["proto/api.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/api.proto");
    Ok(())
}
