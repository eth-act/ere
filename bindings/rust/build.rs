fn main() {
    let dir = std::env::var("ERE_VERIFIER_LIB_DIR")
        .expect("ERE_VERIFIER_LIB_DIR must name the directory of libere_verifier_c.a");
    println!("cargo:rerun-if-env-changed=ERE_VERIFIER_LIB_DIR");
    println!("cargo:rerun-if-changed={dir}/libere_verifier_c.a");
    println!("cargo:rustc-link-search=native={dir}");
    println!("cargo:rustc-link-lib=static=ere_verifier_c");
}
