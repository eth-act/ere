//! Provenance of the ere revision under test.
//!
//! Everything is read from the revision's own `Cargo.toml` and `Cargo.lock`
//! (via git), not from this build, so any revision with published images can
//! be tested and recorded accurately.

use std::process::Command;

use anyhow::{Context, bail};
use cargo_metadata::Source;
use ere_dockerized::zkVMKind;
use ere_util_build::resolve_pkg_version;
use serde::Deserialize;

/// Root of the ere checkout this binary is built from.
const ERE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../..");

/// Crate and dependency that pin each zkVM SDK; must match `crates/catalog/build.rs`.
const SDK_DEPS: [(zkVMKind, &str, &str); 3] = [
    (zkVMKind::OpenVM, "ere-platform-openvm", "openvm"),
    (zkVMKind::SP1, "ere-verifier-sp1", "sp1-verifier"),
    (zkVMKind::Zisk, "ere-verifier-zisk", "zisk-verifier"),
];

#[derive(Debug)]
pub struct Revision {
    /// Full commit hash.
    pub commit: String,
    /// Workspace version at the revision, e.g. `0.18.1`.
    pub ere_version: String,
    lockfile: Lockfile,
}

impl Revision {
    /// Resolves `rev` (commit, tag or branch) in the ere checkout.
    pub fn resolve(rev: &str) -> anyhow::Result<Self> {
        let commit = git(&["rev-parse", &format!("{rev}^{{commit}}")])
            .with_context(|| format!("unknown ere revision `{rev}` (try `git fetch`)"))?;

        let manifest: toml::Table =
            toml::from_str(&git(&["show", &format!("{commit}:Cargo.toml")])?)
                .context("failed to parse Cargo.toml")?;
        let ere_version = manifest
            .get("workspace")
            .and_then(|workspace| workspace.get("package"))
            .and_then(|package| package.get("version"))
            .and_then(|version| version.as_str())
            .context("Cargo.toml has no workspace.package.version")?
            .to_string();

        let lockfile = toml::from_str(&git(&["show", &format!("{commit}:Cargo.lock")])?)
            .context("failed to parse Cargo.lock")?;

        Ok(Self {
            commit,
            ere_version,
            lockfile,
        })
    }

    /// Tag of the images CI published for this revision (first 7 hex digits).
    pub fn image_tag(&self) -> &str {
        &self.commit[..7]
    }

    /// zkVM SDK version pinned by this revision, resolved like `zkVMKind::sdk_version`.
    pub fn sdk_version(&self, zkvm_kind: zkVMKind) -> anyhow::Result<String> {
        let (_, crate_name, dep_name) = SDK_DEPS
            .into_iter()
            .find(|(kind, ..)| *kind == zkvm_kind)
            .expect("every zkVM kind has an SDK dependency");
        let pkg = self.lockfile.dependency(crate_name, dep_name)?;
        let source = pkg.source.clone().map(|repr| Source { repr });
        Ok(resolve_pkg_version(source.as_ref(), &pkg.version))
    }
}

#[derive(Debug, Deserialize)]
struct Lockfile {
    package: Vec<LockPackage>,
}

#[derive(Debug, Deserialize)]
struct LockPackage {
    name: String,
    version: String,
    source: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
}

impl Lockfile {
    /// Returns the package `dep_name` that `crate_name` depends on.
    ///
    /// A dependency entry reads `name`, `name version`, or
    /// `name version (source)` when the name alone is ambiguous.
    fn dependency(&self, crate_name: &str, dep_name: &str) -> anyhow::Result<&LockPackage> {
        let krate = self
            .package
            .iter()
            .find(|pkg| pkg.name == crate_name)
            .with_context(|| format!("`{crate_name}` not found in Cargo.lock"))?;
        let entry = krate
            .dependencies
            .iter()
            .find(|entry| entry.split(' ').next() == Some(dep_name))
            .with_context(|| format!("`{crate_name}` does not depend on `{dep_name}`"))?;

        let mut parts = entry.splitn(3, ' ').skip(1);
        let version = parts.next();
        let source = parts
            .next()
            .map(|source| source.trim_start_matches('(').trim_end_matches(')'));

        let candidates = Vec::from_iter(self.package.iter().filter(|pkg| {
            pkg.name == dep_name
                && version.is_none_or(|version| pkg.version == version)
                && source.is_none_or(|source| {
                    pkg.source
                        .as_deref()
                        .is_some_and(|repr| repr.starts_with(source))
                })
        }));
        match candidates.as_slice() {
            [pkg] => Ok(pkg),
            _ => bail!(
                "expected one `{dep_name}` for `{crate_name}` in Cargo.lock, found {}",
                candidates.len()
            ),
        }
    }
}

fn git(args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(ERE_ROOT)
        .args(args)
        .output()
        .context("failed to run git")?;
    if !output.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use ere_dockerized::zkVMKind;

    use crate::provenance::Revision;

    /// The revision this binary is built from must resolve to the same SDK
    /// versions that `ere-catalog` computes for the build.
    #[test]
    fn head_matches_catalog() {
        let head = Revision::resolve("HEAD").unwrap();
        assert_eq!(head.image_tag().len(), 7);
        for zkvm_kind in zkVMKind::iter() {
            assert_eq!(
                head.sdk_version(zkvm_kind).unwrap(),
                zkvm_kind.sdk_version()
            );
        }
    }
}
