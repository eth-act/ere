use std::{env, fs, path::PathBuf};

/// To sync generated `api.rs`, run:
///
/// ```
/// cargo test --package ere-server --no-default-features --lib -- test::api_generation --exact
/// ```
#[test]
fn api_generation() {
    let tempdir = tempfile::tempdir().unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    prost_build::Config::new()
        .out_dir(tempdir.path())
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]") // enable support for JSON encoding
        .service_generator(twirp_build::service_generator())
        .compile_protos(&[dir.join("proto").join("api.proto")], &[dir.join("proto")])
        .unwrap();

    let latest = tempdir.path().join("api.rs");
    let current = dir.join("src").join("api.rs");

    // If it's in CI env, don't overwrite but only check if it's up-to-date.
    if env::var_os("GITHUB_ACTIONS").is_none() {
        fs::copy(&latest, &current).unwrap();
    }
    assert_eq!(fs::read(&latest).unwrap(), fs::read(&current).unwrap());
}
