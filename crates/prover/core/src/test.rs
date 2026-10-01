use std::{env, fs, path::PathBuf};

/// To sync generated `cost/pprof.rs`, run:
///
/// ```
/// cargo test --package ere-prover-core --lib -- test::pprof_generation --exact
/// ```
#[test]
fn pprof_generation() {
    let tempdir = tempfile::tempdir().unwrap();
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    prost_build::Config::new()
        .out_dir(tempdir.path())
        .compile_protos(
            &[dir.join("proto").join("profile.proto")],
            &[dir.join("proto")],
        )
        .unwrap();

    let latest = tempdir.path().join("perftools.profiles.rs");
    let current = dir.join("src").join("cost").join("pprof.rs");

    // If it's in CI env, don't overwrite but only check if it's up-to-date.
    if env::var_os("GITHUB_ACTIONS").is_none() {
        fs::copy(&latest, &current).unwrap();
    }
    assert_eq!(fs::read(&latest).unwrap(), fs::read(&current).unwrap());
}
