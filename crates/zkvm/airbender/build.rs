use ere_build_utils::{cargo_lock_path, detect_and_generate_name_and_sdk_version};

fn main() {
    detect_and_generate_name_and_sdk_version("airbender", "execution_utils");

    if let Ok(cargo_lock) = cargo_lock_path(3) {
        println!("cargo:rerun-if-changed={}", cargo_lock.display());
    }
}
