use openvm_circuit::arch::VmVerificationError;
use thiserror::Error;

use crate::vendor::CommitBytes;

#[derive(Debug, Error)]
pub enum Error {
    /// Failed to deserialize a proof or program VK.
    #[error("Failed to deserialize: {0}")]
    Deserialize(#[from] bincode::error::DecodeError),

    /// VK byte slice was not the expected 64 bytes.
    #[error("Invalid ProgramVk length, expected: {expected}, got: {got}")]
    InvalidProgramVkLength { expected: usize, got: usize },

    /// VM-level verification failure.
    #[error("VM verification failed: {0}")]
    VmVerification(#[from] VmVerificationError),

    /// Claimed app exe commit did not match the expected one.
    #[error("Invalid app exe commit: expected {expected:?}, actual {actual:?}")]
    InvalidAppExeCommit {
        expected: CommitBytes,
        actual: CommitBytes,
    },

    /// Claimed app vm commit did not match the expected one.
    #[error("Invalid app vm commit: expected {expected:?}, actual {actual:?}")]
    InvalidAppVmCommit {
        expected: CommitBytes,
        actual: CommitBytes,
    },

    /// A field element could not be downcast to `u8`.
    #[error("Invalid public value")]
    InvalidPublicValue,
}
