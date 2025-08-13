use build_utils::{detect_sdk_version, detect_self_crate_version};
use std::{env, fs, path::Path};

fn main() {
    generate_crate_version();
    generate_zkvm_sdk_version_impl();
    println!("cargo:rerun-if-changed=Cargo.lock");
}

fn generate_crate_version() {
    let crate_version = format!(
        "const CRATE_VERSION: &str = \"{}\";",
        detect_self_crate_version()
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("crate_version.rs");
    fs::write(dst, crate_version).unwrap();
}

fn generate_zkvm_sdk_version_impl() {
    let [
        jolt_version,
        nexus_version,
        openvm_version,
        pico_version,
        risc0_version,
        sp1_version,
    ] = [
        "jolt-sdk",
        "nexus-sdk",
        "openvm-sdk",
        "pico-sdk",
        "risc0-zkvm",
        "sp1-sdk",
    ]
    .map(detect_sdk_version);

    // FIXME: ZisK doesn't depend on SDK yet, so we hardcode the version here,
    //        same as the one in `scripts/sdk_installers/install_zisk_sdk.sh`.
    //        Once ZisK's SDK is ready, we should update this to detect the SDK
    //        version.
    //        The issue for tracking https://github.com/eth-act/ere/issues/73.
    let zisk_version = "0.9.0";

    let zkvm_sdk_version_impl = format!(
        r#"impl crate::ErezkVM {{
    pub fn sdk_version(&self) -> &'static str {{
        match self {{
            Self::Jolt => "{jolt_version}",
            Self::Nexus => "{nexus_version}",
            Self::OpenVM => "{openvm_version}",
            Self::Pico => "{pico_version}",
            Self::Risc0 => "{risc0_version}",
            Self::SP1 => "{sp1_version}",
            Self::Zisk => "{zisk_version}",
        }}
    }}
}}"#,
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("zkvm_sdk_version_impl.rs");
    fs::write(dst, zkvm_sdk_version_impl).unwrap();
}
