use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let build_dir = PathBuf::from(&crate_dir).join("build");
    fs::create_dir_all(&build_dir).expect("failed to create build directory");

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_language(cbindgen::Language::C)
        .with_include_guard("ERE_VERIFIER_H")
        .with_pragma_once(true)
        .with_documentation(true)
        .generate()
        .expect("cbindgen failed to generate ere_verifier.h")
        .write_to_file(build_dir.join("ere_verifier.h"));
}
