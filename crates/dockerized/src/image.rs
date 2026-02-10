use crate::{DOCKER_IMAGE_TAG, util::env::image_registry, zkVMKind};

/// Returns tag of images in format of `{version}{suffix}`.
pub fn image_tag(zkvm_kind: zkVMKind, gpu: bool) -> String {
    let suffix = match (zkvm_kind, gpu) {
        // Only the following zkVMs requires CUDA setup in the base image
        // when GPU support is required.
        (
            zkVMKind::Airbender
            | zkVMKind::OpenVM
            | zkVMKind::Risc0
            | zkVMKind::SP1
            | zkVMKind::Zisk,
            true,
        ) => "-cuda",
        _ => "",
    };
    format!("{DOCKER_IMAGE_TAG}{suffix}")
}

/// Returns `ere-base:{image_tag}`
pub fn base_image(zkvm_kind: zkVMKind, gpu: bool) -> String {
    let image_tag = image_tag(zkvm_kind, gpu);
    with_image_registry(format!("ere-base:{image_tag}"))
}

/// Returns `ere-base-{zkvm_kind}:{image_tag}`
pub fn base_zkvm_image(zkvm_kind: zkVMKind, gpu: bool) -> String {
    let image_tag = image_tag(zkvm_kind, gpu);
    with_image_registry(format!("ere-base-{zkvm_kind}:{image_tag}"))
}

/// Returns `ere-server-{zkvm_kind}:{image_tag}`
pub fn server_zkvm_image(zkvm_kind: zkVMKind, gpu: bool) -> String {
    let image_tag = image_tag(zkvm_kind, gpu);
    with_image_registry(format!("ere-server-{zkvm_kind}:{image_tag}"))
}

/// Returns `ere-compiler-{zkvm_kind}:{image_tag}`
pub fn compiler_zkvm_image(zkvm_kind: zkVMKind) -> String {
    let image_tag = image_tag(zkvm_kind, false);
    with_image_registry(format!("ere-compiler-{zkvm_kind}:{image_tag}"))
}

fn with_image_registry(image: String) -> String {
    image_registry()
        .map(|registry| format!("{}/{image}", registry.trim_end_matches('/')))
        .unwrap_or_else(|| image)
}
