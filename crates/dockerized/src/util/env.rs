use std::env;

pub const ERE_IMAGE_REGISTRY: &str = "ERE_IMAGE_REGISTRY";
pub const ERE_FORCE_REBUILD_DOCKER_IMAGE: &str = "ERE_FORCE_REBUILD_DOCKER_IMAGE";
pub const ERE_GPU_DEVICES: &str = "ERE_GPU_DEVICES";
pub const ERE_DOCKER_NETWORK: &str = "ERE_DOCKER_NETWORK";

/// Returns image registry from env variable `ERE_IMAGE_REGISTRY`.
///
/// If env varialbe is valid, it will be prepended to all images. For example
/// if `ERE_IMAGE_REGISTRY=ghcr.io/eth-act/ere`, the [`base_image`] will return
/// `ghcr.io/eth-act/ere/ere-base:{image_tag}`.
///
/// [`base_image`]: crate::image::base_image
pub fn image_registry() -> Option<String> {
    env::var(ERE_IMAGE_REGISTRY).ok()
}

/// Returns whether env variable `ERE_FORCE_REBUILD_DOCKER_IMAGE` is set or not.
pub fn force_rebuild_docker_image() -> bool {
    env::var_os(ERE_FORCE_REBUILD_DOCKER_IMAGE).is_some()
}

/// Returns env variable `ERE_GPU_DEVICES`.
pub fn gpu_devices() -> Option<String> {
    env::var(ERE_GPU_DEVICES).ok()
}

/// Returns env variable `ERE_DOCKER_NETWORK`.
pub fn docker_network() -> Option<String> {
    env::var(ERE_DOCKER_NETWORK).ok()
}
