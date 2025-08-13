use crate::JoltError;
use crate::error::CompileError;
use std::{fs, path::Path};
use toml::Value;

/// Reads the `[package] name` out of a Cargo.toml.
///
/// * `manifest_path` – absolute or relative path to a Cargo.toml.
/// * Returns → `String` with the package name (`fib`, `my_guest`, …).
pub(crate) fn package_name_from_manifest(manifest_path: &Path) -> Result<String, JoltError> {
    let manifest =
        fs::read_to_string(manifest_path).map_err(|source| CompileError::PackageNameNotFound {
            source: source.into(),
            path: manifest_path.to_path_buf(),
        })?;
    let value: Value =
        manifest
            .parse::<Value>()
            .map_err(|source| CompileError::PackageNameNotFound {
                source: source.into(),
                path: manifest_path.to_path_buf(),
            })?;

    Ok(value
        .get("package")
        .and_then(|pkg| pkg.get("name"))
        .and_then(Value::as_str)
        .map(|s| s.to_owned())
        .ok_or_else(|| CompileError::PackageNameNotFound {
            source: "[package.name] not found".into(),
            path: manifest_path.to_path_buf(),
        })?)
}
