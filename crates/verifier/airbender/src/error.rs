use airbender_host::HostError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a proof or program VK.
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[from] bincode::error::DecodeError),

    /// Error returned by the `airbender-host` SDK.
    #[error(transparent)]
    Sdk(#[from] HostError),
}
