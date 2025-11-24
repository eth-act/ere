use crate::{program::AirbenderProgram, zkvm::sdk::AirbenderSdk};
use airbender_execution_utils::ProgramProof;
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMProgramDigest,
};
use std::time::Instant;

mod error;
mod sdk;

pub use error::Error;
pub use sdk::VkHashChain;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub struct EreAirbender {
    sdk: AirbenderSdk,
}

impl EreAirbender {
    pub fn new(program: AirbenderProgram, resource: ProverResourceType) -> Result<Self, Error> {
        let gpu = matches!(resource, ProverResourceType::Gpu);
        let sdk = AirbenderSdk::new(program.bin(), gpu);
        Ok(Self { sdk })
    }
}

impl zkVM for EreAirbender {
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let start = Instant::now();
        let (public_values, cycles) = self.sdk.execute(input)?;
        let execution_duration = start.elapsed();

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles: cycles,
                execution_duration,
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        if proof_kind != ProofKind::Compressed {
            bail!(CommonError::unsupported_proof_kind(
                proof_kind,
                [ProofKind::Compressed]
            ))
        }
        let start = Instant::now();
        let (public_values, proof) = self.sdk.prove(input)?;
        let proving_time = start.elapsed();

        let proof_bytes = bincode::serde::encode_to_vec(&proof, bincode::config::legacy())
            .map_err(|err| CommonError::serialize("proof", "bincode", err))?;

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let Proof::Compressed(proof) = proof else {
            bail!(CommonError::unsupported_proof_kind(
                proof.kind(),
                [ProofKind::Compressed]
            ))
        };

        let (proof, _): (ProgramProof, _) =
            bincode::serde::decode_from_slice(proof, bincode::config::legacy())
                .map_err(|err| CommonError::deserialize("proof", "bincode", err))?;

        let public_values = self.sdk.verify(&proof)?;

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for EreAirbender {
    type ProgramDigest = VkHashChain;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(*self.sdk.vk_chain_hash())
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv32ima, program::AirbenderProgram, zkvm::EreAirbender};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{
        compiler::Compiler,
        zkvm::{ProofKind, ProverResourceType, zkVM},
    };
    use std::sync::OnceLock;

    fn basic_program() -> AirbenderProgram {
        static PROGRAM: OnceLock<AirbenderProgram> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv32ima
                    .compile(&testing_guest_directory("airbender", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu).unwrap();

        for input in [
            Vec::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().serialized_input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu).unwrap();

        for input in [
            Vec::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().serialized_input(),
        ] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
