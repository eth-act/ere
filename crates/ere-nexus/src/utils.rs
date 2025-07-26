use std::fs;
use toml::Table;

pub fn get_cargo_package_name(crate_path: &std::path::Path) -> Option<String> {
    let cargo_contents = fs::read_to_string(crate_path.join("Cargo.toml")).ok()?;
    let cargo_toml: Table = toml::from_str(&cargo_contents).ok()?;

    cargo_toml
        .get("package")?
        .get("name")?
        .as_str()
        .map(|s| s.to_string())
}
