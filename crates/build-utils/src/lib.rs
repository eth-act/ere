#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use cargo_metadata::MetadataCommand;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Returns path to the closest workspace that contains `Cargo.lock` from `CARGO_MANIFEST_DIR`,
/// returns `None` if not found.
pub fn workspace() -> Option<PathBuf> {
    let mut dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .canonicalize()
        .ok()?;
    loop {
        if dir.join("Cargo.lock").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Returns path to the closest `Cargo.lock` from `CARGO_MANIFEST_DIR`, returns `None` if not found.
pub fn cargo_lock_path() -> Option<PathBuf> {
    workspace().map(|workspace| workspace.join("Cargo.lock"))
}

// Detect and generate a Rust source file that contains the name and version of the SDK.
pub fn detect_and_generate_name_and_sdk_version(name: &str, sdk_dep_name: &str) {
    gen_name_and_sdk_version(name, &detect_sdk_version(sdk_dep_name));

    if let Some(cargo_lock) = cargo_lock_path() {
        println!("cargo:rerun-if-changed={}", cargo_lock.display());
    }
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
}

/// Returns tag for Docker image.
///
/// Returns:
/// - Short git revision (7 digits)
/// - Crate version from Cargo.toml as fallback if git is not available
pub fn get_docker_image_tag() -> String {
    // Get short git revision
    let rev_output = std::process::Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output();

    match rev_output {
        Ok(output) if output.status.success() => {
            let rev = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !rev.is_empty() {
                return rev;
            }
        }
        _ => {}
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
