use std::{env, fs, path::Path};

// Detect and generate a Rust source file that contains the name and version of the SDK.
pub fn detect_and_generate_name_and_sdk_version(name: &str, sdk_dep_name: &str) {
    let meta = cargo_metadata::MetadataCommand::new()
        .exec()
        .expect("Failed to get cargo metadata");

    let version = meta
        .packages
        .iter()
        .find(|pkg| pkg.name.eq_ignore_ascii_case(sdk_dep_name))
        .map(|pkg| pkg.version.to_string())
        .unwrap_or_else(|| {
            panic!("Dependency {sdk_dep_name} not found in Cargo.toml");
        });

    gen_name_and_sdk_version(name, &version);
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
