use std::{env, fs, path::Path};

use ere_util_build::{cargo_lock_path, detect_sdk_version, get_docker_image_tag, workspace};

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
        openvm_version,
        risc0_version,
        sp1_version,
        zisk_version,
    ] = [
        "airbender-sdk",
        "openvm-sdk",
        "risc0-zkvm",
        "sp1-sdk",
        "zisk-sdk",
    ]
    .map(detect_sdk_version);

    let zkvm_sdk_version_impl = format!(
        r#"impl crate::zkVMKind {{
    pub fn sdk_version(&self) -> &'static str {{
        match self {{
            Self::Airbender => "{airbender_version}",
            Self::OpenVM => "{openvm_version}",
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

    if let Some(cargo_lock) = cargo_lock_path() {
        println!("cargo:rerun-if-changed={}", cargo_lock.display());
    }
}
