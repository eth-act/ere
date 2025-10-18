#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{client::AirbenderSdk, compiler::AirbenderProgram, error::AirbenderError};
use airbender_execution_utils::ProgramProof;
use ere_zkvm_interface::{
    ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, ProverResourceType,
    PublicValues, zkVM, zkVMError,
};
use std::time::Instant;

mod client;
pub mod compiler;
pub mod error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub struct EreAirbender {
    sdk: AirbenderSdk,
}

impl EreAirbender {
    pub fn new(bin: AirbenderProgram, resource: ProverResourceType) -> Self {
        let gpu = matches!(resource, ProverResourceType::Gpu);
        let sdk = AirbenderSdk::new(&bin, gpu);
        Self { sdk }
    }
}

impl zkVM for EreAirbender {
    fn execute(&self, input: &[u8]) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
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
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        if proof_kind != ProofKind::Compressed {
            panic!("Only Compressed proof kind is supported.");
        }

        let start = Instant::now();
        let (public_values, proof) = self.sdk.prove(input)?;
        let proving_time = start.elapsed();

        let proof_bytes = bincode::serde::encode_to_vec(&proof, bincode::config::legacy())
            .map_err(AirbenderError::BincodeEncode)?;

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        let Proof::Compressed(proof) = proof else {
            return Err(zkVMError::other("Only Compressed proof kind is supported."));
        };

        let (proof, _): (ProgramProof, _) =
            bincode::serde::decode_from_slice(proof, bincode::config::legacy())
                .map_err(AirbenderError::BincodeDecode)?;

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

#[cfg(test)]
mod tests {
    use crate::{
        EreAirbender,
        compiler::{AirbenderProgram, RustRv32ima},
    };
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgramInput,
    };
    use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM};
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
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu);

        let test_case = BasicProgramInput::valid().into_output_sha256();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_input() {
        let program = basic_program();
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu);

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu);

        let test_case = BasicProgramInput::valid().into_output_sha256();
        run_zkvm_execute(&zkvm, &test_case);
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_input() {
        let program = basic_program();
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu);

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
