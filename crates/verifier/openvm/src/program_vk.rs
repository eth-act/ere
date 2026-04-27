use core::convert::Infallible;

use ere_verifier_core::codec::{Decode, Encode};
use openvm_sdk::commit::{AppExecutionCommit, CommitBytes};
use serde::{Deserialize, Serialize};

use crate::Error;

const PROGRAM_VK_SIZE: usize = 64;

/// Verifying key that identifies a specific compiled guest program.
///
/// Produced during setup and consumed by [`OpenVMVerifier`] together with an
/// [`OpenVMProof`] to authenticate that the proof was generated from the
/// same program. Wraps a 64-byte `AppExecutionCommit`
/// (`app_exe_commit || app_vm_commit`); encoded as the concatenation of the
/// two 32-byte commits.
///
/// [`OpenVMVerifier`]: crate::OpenVMVerifier
/// [`OpenVMProof`]: crate::OpenVMProof
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpenVMProgramVk(pub AppExecutionCommit);

impl Encode for OpenVMProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        Ok([self.0.app_exe_commit, self.0.app_vm_commit]
            .iter()
            .flat_map(|commit| commit.as_slice())
            .copied()
            .collect())
    }
}

impl Decode for OpenVMProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() != PROGRAM_VK_SIZE {
            return Err(Error::InvalidProgramVkLength {
                expected: PROGRAM_VK_SIZE,
                got: slice.len(),
            });
        }
        let [app_exe_commit, app_vm_commit] =
            [&slice[..32], &slice[32..]].map(|slice| CommitBytes::new(slice.try_into().unwrap()));
        Ok(Self(AppExecutionCommit {
            app_exe_commit,
            app_vm_commit,
        }))
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(OpenVMProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(OpenVMProgramVk);
