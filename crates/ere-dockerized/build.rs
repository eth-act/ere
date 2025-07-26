use build_utils::detect_sdk_versions;
use std::{env, fs, path::Path};

fn main() {
    let [
        jolt_version,
        nexus_version,
        openvm_version,
        pico_version,
        risc0_version,
        sp1_version,
        zisk_version,
    ] = detect_sdk_versions([
        "jolt-sdk",
        "nexus-sdk",
        "openvm-sdk",
        "pico-sdk",
        "risc0-zkvm",
        "sp1-sdk",
        "lib-c",
    ])
    .collect::<Vec<_>>()
    .try_into()
    .unwrap();

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
    println!("cargo:rerun-if-changed=Cargo.lock");
}
