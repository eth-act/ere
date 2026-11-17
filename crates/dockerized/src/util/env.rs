use std::env;

use ere_prover_core::CommonError;

use crate::util::docker::docker_host_memory;

pub const ERE_IMAGE_REGISTRY: &str = "ERE_IMAGE_REGISTRY";
pub const ERE_IMAGE_TAG: &str = "ERE_IMAGE_TAG";
pub const ERE_FORCE_REBUILD_DOCKER_IMAGE: &str = "ERE_FORCE_REBUILD_DOCKER_IMAGE";
pub const ERE_GPU_DEVICES: &str = "ERE_GPU_DEVICES";
pub const ERE_DOCKER_NETWORK: &str = "ERE_DOCKER_NETWORK";
pub const ERE_DOCKER_MEMORY: &str = "ERE_DOCKER_MEMORY";
pub const ERE_DOCKER_MEMORY_SWAP: &str = "ERE_DOCKER_MEMORY_SWAP";
pub const ERE_OPENVM_CACHE_VOLUME: &str = "ERE_OPENVM_CACHE_VOLUME";
pub const ERE_PICO_CACHE_VOLUME: &str = "ERE_PICO_CACHE_VOLUME";
pub const ERE_PICO_PURE_CACHE_VOLUME: &str = "ERE_PICO_PURE_CACHE_VOLUME";
pub const ERE_ZISK_CACHE_VOLUME: &str = "ERE_ZISK_CACHE_VOLUME";
pub const ERE_ZISK_PROVING_KEY_VOLUME: &str = "ERE_ZISK_PROVING_KEY_VOLUME";

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

/// Returns image tag from env variable `ERE_IMAGE_TAG`, or [`DOCKER_IMAGE_TAG`] when it is unset.
///
/// For example with `ERE_IMAGE_TAG=0.18.1`, the [`base_image`] with GPU returns
/// `ere-base:0.18.1-cuda`.
///
/// [`DOCKER_IMAGE_TAG`]: crate::DOCKER_IMAGE_TAG
/// [`base_image`]: crate::image::base_image
pub fn image_tag() -> String {
    env::var(ERE_IMAGE_TAG)
        .ok()
        .unwrap_or_else(|| crate::DOCKER_IMAGE_TAG.to_string())
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

/// Returns env variable `ERE_DOCKER_MEMORY`, or 90% of the total memory of the Docker host in bytes
/// when it is unset, which keeps 10% for the host.
///
/// The value limits the RAM of each container, so a container that uses too much memory swaps or is
/// killed instead of the processes of the host.
pub fn docker_memory() -> Result<String, CommonError> {
    match env::var(ERE_DOCKER_MEMORY) {
        Ok(memory) => Ok(memory),
        Err(_) => Ok((docker_host_memory()? / 10 * 9).to_string()),
    }
}

/// Returns env variable `ERE_DOCKER_MEMORY_SWAP`, or `-1` when it is unset.
///
/// The value is the limit of RAM and swap together of each container, and `-1` lets a container use
/// all free swap of the host.
pub fn docker_memory_swap() -> String {
    env::var(ERE_DOCKER_MEMORY_SWAP).unwrap_or_else(|_| "-1".to_string())
}
