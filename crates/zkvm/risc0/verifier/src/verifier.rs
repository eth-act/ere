use crate::{Error, Risc0ProgramVk, Risc0Proof};
use ere_verifier_core::{PublicValues, zkVMVerifier};
use risc0_zkvm::InnerReceipt;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// Verifier bound to a specific compiled guest program.
///
/// Implements [`zkVMVerifier`]. Holds the pre-computed [`Risc0ProgramVk`]
/// needed to authenticate proofs.
pub struct Risc0Verifier {
    program_vk: Risc0ProgramVk,
}

impl Risc0Verifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: Risc0ProgramVk) -> Self {
        Self { program_vk }
    }
}

impl zkVMVerifier for Risc0Verifier {
    type ProgramVk = Risc0ProgramVk;
    type Proof = Risc0Proof;
    type Error = Error;

    fn program_vk(&self) -> &Risc0ProgramVk {
        &self.program_vk
    }

    fn verify(&self, proof: &Risc0Proof) -> Result<PublicValues, Self::Error> {
        let receipt = &proof.0;

        if !matches!(receipt.inner, InnerReceipt::Succinct(_)) {
            let got = match &receipt.inner {
                InnerReceipt::Composite(_) => "Composite",
                InnerReceipt::Succinct(_) => "Succinct",
                InnerReceipt::Groth16(_) => "Groth16",
                InnerReceipt::Fake(_) => "Fake",
                _ => "Unknown",
            };
            return Err(Error::UnexpectedProofKind(got.to_string()));
        }

        receipt.verify(self.program_vk.0)?;

        Ok(receipt.journal.bytes.as_slice().into())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}
