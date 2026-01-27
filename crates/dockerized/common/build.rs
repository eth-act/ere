use ere_build_utils::{cargo_lock_path, detect_sdk_version, get_docker_image_tag, workspace};
use std::{env, fs, path::Path};

fn main() {
    generate_docker_image_tag();
    generate_zkvm_sdk_version_impl();
}

fn generate_docker_image_tag() {
    let docker_image_tag = format!(
        "/// Docker image tag.\npub const DOCKER_IMAGE_TAG: &str = \"{}\";",
        get_docker_image_tag()
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("docker_image_tag.rs");
    fs::write(dst, docker_image_tag).unwrap();

    if let Some(dot_git) =
        workspace().and_then(|workspace| workspace.join(".git").canonicalize().ok())
    {
        for dir in ["HEAD", "refs", "packed-refs"] {
            if dot_git.join(dir).exists() {
                println!("cargo:rerun-if-changed={}", dot_git.join(dir).display());
            }
        }
    }
}

fn generate_zkvm_sdk_version_impl() {
    let [
        airbender_version,
        jolt_version,
        miden_version,
        nexus_version,
        openvm_version,
        pico_version,
        risc0_version,
        sp1_version,
        ziren_version,
        zisk_version,
    ] = [
        "execution_utils",
        "jolt-sdk",
        "miden-core",
        "nexus-sdk",
        "openvm-sdk",
        "pico-vm",
        "risc0-zkvm",
        "sp1-sdk",
        "zkm-sdk",
        "ziskos",
    ]
    .map(detect_sdk_version);

    let zkvm_sdk_version_impl = format!(
        r#"impl crate::zkVMKind {{
    pub fn sdk_version(&self) -> &'static str {{
        match self {{
            Self::Airbender => "{airbender_version}",
            Self::Jolt => "{jolt_version}",
            Self::Miden => "{miden_version}",
            Self::Nexus => "{nexus_version}",
            Self::OpenVM => "{openvm_version}",
            Self::Pico => "{pico_version}",
            Self::Risc0 => "{risc0_version}",
            Self::SP1 => "{sp1_version}",
            Self::Ziren => "{ziren_version}",
            Self::Zisk => "{zisk_version}",
        }}
    }}
}}"#,
    );

    let out_dir = env::var("OUT_DIR").unwrap();
    let dst = Path::new(&out_dir).join("zkvm_sdk_version_impl.rs");
    fs::write(dst, zkvm_sdk_version_impl).unwrap();

    if let Some(cargo_lock) = cargo_lock_path() {
        println!("cargo:rerun-if-changed={}", cargo_lock.display());
    }
}
