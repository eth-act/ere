use std::{env, fs, path::PathBuf};

/// To sync generated `api.rs`, run:
///
/// ```
/// cargo test --package ere-cluster-client-zisk --lib -- test::api_generation --exact
/// ```
#[test]
fn api_generation() {
    let tempdir = tempfile::tempdir().unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .out_dir(tempdir.path())
        .compile_protos(
            &[dir.join("proto").join("zisk_distributed_api.proto")],
            &[dir.join("proto")],
        )
        .unwrap();

    let latest = tempdir.path().join("zisk.distributed.api.v1.rs");
    let current = dir.join("src").join("api.rs");

    // If it's in CI env, don't overwrite but only check if it's up-to-date.
    if env::var_os("GITHUB_ACTIONS").is_none() {
        fs::copy(&latest, &current).unwrap();
    }
    assert_eq!(fs::read(&latest).unwrap(), fs::read(&current).unwrap());
}
