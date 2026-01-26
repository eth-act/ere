#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use cargo_metadata::MetadataCommand;
use std::{env, fs, path::Path};

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

/// Generate tag for Docker image.
///
/// Returns:
/// - Git tag in SemVer if current commit has a tag (e.g., `v0.1.0` -> `0.1.0`)
/// - Short git revision (7 digits) if no tag found
/// - Crate version from Cargo.toml as fallback if git is not available
pub fn get_docker_image_tag() -> String {
    // Try to get a tag pointing to the current commit
    let tag_output = std::process::Command::new("git")
        .args(["describe", "--tags", "--exact-match", "HEAD"])
        .output();

    if let Ok(output) = tag_output
        && output.status.success()
    {
        let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Remove 'v' prefix if present
        return tag.strip_prefix('v').unwrap_or(&tag).to_string();
    }

    // No tag found, try to get short revision
    let rev_output = std::process::Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output();

    if let Ok(output) = rev_output
        && output.status.success()
    {
        let rev = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !rev.is_empty() {
            return rev;
        }
    }

    // Fallback to crate version
    let meta = MetadataCommand::new()
        .exec()
        .expect("Failed to get cargo metadata");

    // `root_package` returns the crate of the `build.rs` that being ran.
    meta.root_package()
        .expect("crate to have version")
        .version
        .to_string()
}
