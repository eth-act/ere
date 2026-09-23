use ere_verifier_core::{PublicValues, zkVMVerifier};
use lambda_vm_prover::{GoldilocksCubicProofOptions, verify_with_options};

use crate::{Error, LambdaVMProgramVk, LambdaVMProof};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

/// FRI blowup factor that proofs are generated and verified with.
///
/// A proof does not carry its proof options, so the prover and the verifier must agree on them.
///
/// According to https://github.com/yetanotherco/lambda_vm/blob/ffc4ac19e755d93ed631ace71f17577478d8d21b/bin/cli/src/main.rs#L214-L216.
pub const BLOWUP_FACTOR: u8 = 2;

/// Verifier bound to a specific compiled guest program.
#[derive(Clone, Debug)]
pub struct LambdaVMVerifier {
    program_vk: LambdaVMProgramVk,
}

impl LambdaVMVerifier {
    /// Creates a new verifier bound to `program_vk`.
    pub fn new(program_vk: LambdaVMProgramVk) -> Self {
        Self { program_vk }
    }
}

impl zkVMVerifier for LambdaVMVerifier {
    type ProgramVk = LambdaVMProgramVk;
    type Proof = LambdaVMProof;
    type Error = Error;

    fn program_vk(&self) -> &LambdaVMProgramVk {
        &self.program_vk
    }

    fn verify(&self, proof: &LambdaVMProof) -> Result<PublicValues, Error> {
        let options = GoldilocksCubicProofOptions::with_blowup(BLOWUP_FACTOR)
            .map_err(|err| Error::ProofOptions(err.to_string()))?;

        if !verify_with_options(&proof.0, &self.program_vk.0, &options, None, None)? {
            return Err(Error::InvalidProof);
        }

        Ok(proof.0.public_output.as_slice().into())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}
