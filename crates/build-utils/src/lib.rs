#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use cargo_metadata::MetadataCommand;
use std::{env, fs, path::Path};

pub mod docker;

// Detect and generate a Rust source file that contains the name and version of the SDK.
pub fn detect_and_generate_name_and_sdk_version(name: &str, sdk_dep_name: &str) {
    gen_name_and_sdk_version(name, &detect_sdk_version(sdk_dep_name));
}

// Detect version of the SDK.
pub fn detect_sdk_version(sdk_dep_name: &str) -> String {
    let meta = MetadataCommand::new()
        .exec()
        .expect("Failed to get cargo metadata");

    meta.packages
        .iter()
        .find(|pkg| pkg.name.eq_ignore_ascii_case(sdk_dep_name))
        .map(|pkg| pkg.version.to_string())
        .unwrap_or_else(|| {
            panic!("Dependency {sdk_dep_name} not found in Cargo.toml");
        })
}

// Generate a Rust source file that contains the provided name and version of the SDK.
pub fn gen_name_and_sdk_version(name: &str, version: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest = Path::new(&out_dir).join("name_and_sdk_version.rs");
    fs::write(
        &dest,
        format!("const NAME: &str = \"{name}\";\nconst SDK_VERSION: &str = \"{version}\";"),
    )
    .unwrap();
    println!("cargo:rerun-if-changed=Cargo.lock");
}

/// Detects version of the crate of the `build.rs` that being ran.
pub fn detect_self_crate_version() -> String {
    let meta = MetadataCommand::new()
        .exec()
        .expect("Failed to get cargo metadata");

    // `root_package` returns the crate of the `build.rs` that being ran.
    let version = meta.root_package().unwrap().version.to_string();

    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .expect("Failed to get git revision");
    let rev = String::from_utf8_lossy(&output.stdout).trim().to_string();

    format!("{version}-{rev}")
}
