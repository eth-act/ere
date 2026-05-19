use std::{
    env,
    path::Path,
    process::{self, Command},
};

fn main() {
    println!("cargo:rerun-if-env-changed=PATH");

    if env::var("CARGO_FEATURE_CUDA").is_ok() && !nvcc_exists() {
        eprintln!("`cuda` feature requires `nvcc` at /usr/local/cuda/bin/nvcc or on PATH.");
        process::exit(1);
    }
}

fn nvcc_exists() -> bool {
    Path::new("/usr/local/cuda/bin/nvcc").exists()
        || Command::new("nvcc")
            .arg("--version")
            .status()
            .is_ok_and(|status| status.success())
}
