#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to decode program vk: {0}")]
    DecodeProgramVk(String),
    #[error("failed to decode proof: {0}")]
    DecodeProof(String),
    #[error("verification failed: {0}")]
    Verification(String),
}

impl Error {
    pub(crate) fn decode_program_vk(err: impl ToString) -> Self {
        Self::DecodeProgramVk(err.to_string())
    }

    pub(crate) fn decode_proof(err: impl ToString) -> Self {
        Self::DecodeProof(err.to_string())
    }

    pub(crate) fn verification(err: impl ToString) -> Self {
        Self::Verification(err.to_string())
    }
}
