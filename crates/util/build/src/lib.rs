#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

use cargo_metadata::{MetadataCommand, Source};

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

/// Detect the version of `dep_name` (direct dep of `crate_name`) and generate
/// a Rust source file with the provided `name` and resolved version.
pub fn detect_and_generate_name_and_sdk_version(name: &str, crate_name: &str, dep_name: &str) {
    gen_name_and_sdk_version(name, &detect_dep_version(crate_name, dep_name));

    if let Some(cargo_lock) = cargo_lock_path() {
        println!("cargo:rerun-if-changed={}", cargo_lock.display());
    }
}

/// Resolves the version of `dep_name` as a direct dependency of `crate_name`.
///
/// Panics if `crate_name` is not in the workspace metadata, if `dep_name` is
/// not a direct dependency of `crate_name`, or if more than one direct
/// dependency named `dep_name` exists.
pub fn detect_dep_version(crate_name: &str, dep_name: &str) -> String {
    let meta = MetadataCommand::new()
        .exec()
        .expect("cargo metadata should not fail");

    let crate_pkg = meta
        .packages
        .iter()
        .find(|pkg| pkg.name.as_str() == crate_name)
        .unwrap_or_else(|| panic!("package `{crate_name}` not found in workspace metadata"));
    let crate_node = meta
        .resolve
        .as_ref()
        .and_then(|resolve| resolve.nodes.iter().find(|node| node.id == crate_pkg.id))
        .unwrap_or_else(|| panic!("resolve node for `{crate_name}` not found"));
    let crate_deps: HashSet<_> = crate_node.deps.iter().map(|dep| &dep.pkg).collect();

    let dep_pkgs = meta
        .packages
        .iter()
        .filter(|pkg| pkg.name.as_str() == dep_name && crate_deps.contains(&pkg.id))
        .collect::<Vec<_>>();
    let dep_pkg = match dep_pkgs.as_slice() {
        [dep_pkg] => *dep_pkg,
        _ => panic!(
            "expected exactly one dependency named `{dep_name}` in package `{crate_name}`, found {}",
            dep_pkgs.len()
        ),
    };

    resolve_pkg_version(dep_pkg.source.as_ref(), &dep_pkg.version.to_string())
}

/// Resolution based on the package's `source` and `version` in `Cargo.lock`:
///
/// | source                                                  | output       | example   |
/// | ------------------------------------------------------- | ------------ | --------- |
/// | `git+...?tag={tag}#{sha}`                               | `{tag}`      | `v1.4.3`  |
/// | `git+...?...#{sha}` (non-tag deps)                      | `{sha:7}`    | `d15b86d` |
/// | anything else (crates.io, path deps)                    | `v{version}` | `v3.0.5`  |
pub fn resolve_pkg_version(source: Option<&Source>, version: &str) -> String {
    if let Some(source) = source
        && let Some(repr) = source.repr.strip_prefix("git+")
        && let Some((repr, rev)) = repr.split_once('#')
    {
        return repr
            .split_once("?tag=")
            .map(|(_, tag)| tag.to_string())
            .unwrap_or_else(|| rev[..7].to_string());
    }

    format!("v{version}")
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

#[cfg(test)]
mod tests {
    use cargo_metadata::Source;

    use crate::resolve_pkg_version;

    #[test]
    fn test_parse_sdk_version() {
        for (repr, version, expected) in [
            (
                "registry+https://github.com/rust-lang/crates.io-index".into(),
                "3.0.5",
                "v3.0.5",
            ),
            (
                "registry+https://github.com/rust-lang/crates.io-index".into(),
                "6.1.0",
                "v6.1.0",
            ),
            (
                "git+https://github.com/openvm-org/openvm.git?tag=v1.4.3#e8feb93717200e6f334b4f368dd2d0a143f69436".into(),
                "x.x.x",
                "v1.4.3",
            ),
            (
                "git+https://github.com/0xPolygonHermez/zisk.git?tag=v0.16.1#48cf7ccefb5ed62261abf6bfb007b5be8a23c547".into(),
                "x.x.x",
                "v0.16.1",
            ),
            (
                "git+https://github.com/matter-labs//zksync-airbender?rev=d15b86db8a1683b5a641cb420fe6b009e1eb5acb#d15b86db8a1683b5a641cb420fe6b009e1eb5acb".into(),
                "x.x.x",
                "d15b86d",
            ),
            (None, "0.1.0", "v0.1.0"),
        ] {
            let source = repr.map(|repr| Source {
                repr: repr.to_string(),
            });
            assert_eq!(resolve_pkg_version(source.as_ref(), version), expected);
        }
    }
}
