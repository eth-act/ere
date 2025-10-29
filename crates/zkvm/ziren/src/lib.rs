#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{compiler::ZirenProgram, error::ZirenError};
use anyhow::bail;
use ere_zkvm_interface::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMProgramDigest,
};
use std::{panic, time::Instant};
use tracing::info;
use zkm_sdk::{
    CpuProver, Prover, ZKMProofKind, ZKMProofWithPublicValues, ZKMProvingKey, ZKMStdin,
    ZKMVerifyingKey,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod compiler;
pub mod error;

pub struct EreZiren {
    program: ZirenProgram,
    pk: ZKMProvingKey,
    vk: ZKMVerifyingKey,
}

impl EreZiren {
    pub fn new(program: ZirenProgram, resource: ProverResourceType) -> Result<Self, ZirenError> {
        if matches!(
            resource,
            ProverResourceType::Gpu | ProverResourceType::Network(_)
        ) {
            panic!("Network or Gpu proving not yet implemented for ZKM. Use CPU resource type.");
        }
        let (pk, vk) = CpuProver::new().setup(&program);
        Ok(Self { program, pk, vk })
    }
}

impl zkVM for EreZiren {
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let mut stdin = ZKMStdin::new();
        stdin.write_slice(input);

        let start = Instant::now();
        let (public_inputs, exec_report) = CpuProver::new()
            .execute(&self.program, &stdin)
            .map_err(ZirenError::Execute)?;
        let execution_duration = start.elapsed();

        Ok((
            public_inputs.to_vec(),
            ProgramExecutionReport {
                total_num_cycles: exec_report.total_instruction_count(),
                region_cycles: exec_report.cycle_tracker.into_iter().collect(),
                execution_duration,
            },
        ))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        info!("Generating proof…");

        let mut stdin = ZKMStdin::new();
        stdin.write_slice(input);

        let inner_proof_kind = match proof_kind {
            ProofKind::Compressed => ZKMProofKind::Compressed,
            ProofKind::Groth16 => ZKMProofKind::Groth16,
        };

        let start = std::time::Instant::now();
        let proof =
            panic::catch_unwind(|| CpuProver::new().prove(&self.pk, stdin, inner_proof_kind))
                .map_err(|err| ZirenError::ProvePanic(panic_msg(err)))?
                .map_err(ZirenError::Prove)?;
        let proving_time = start.elapsed();

        let public_values = proof.public_values.to_vec();
        let proof = Proof::new(
            proof_kind,
            bincode::serde::encode_to_vec(&proof, bincode::config::legacy())
                .map_err(|err| CommonError::serialize("proof", "bincode", err))?,
        );

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        info!("Verifying proof…");

        let proof_kind = proof.kind();

        let (proof, _): (ZKMProofWithPublicValues, _) =
            bincode::serde::decode_from_slice(proof.as_bytes(), bincode::config::legacy())
                .map_err(|err| CommonError::deserialize("proof", "bincode", err))?;
        let inner_proof_kind = ZKMProofKind::from(&proof.proof);

        if !matches!(
            (proof_kind, inner_proof_kind),
            (ProofKind::Compressed, ZKMProofKind::Compressed)
                | (ProofKind::Groth16, ZKMProofKind::Groth16)
        ) {
            bail!(ZirenError::InvalidProofKind(proof_kind, inner_proof_kind));
        }

        CpuProver::new()
            .verify(&proof, &self.vk)
            .map_err(ZirenError::Verify)?;

        Ok(proof.public_values.to_vec())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for EreZiren {
    type ProgramDigest = ZKMVerifyingKey;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(self.vk.clone())
    }
}

fn panic_msg(err: Box<dyn std::any::Any + Send + 'static>) -> String {
    None.or_else(|| err.downcast_ref::<String>().cloned())
        .or_else(|| err.downcast_ref::<&'static str>().map(ToString::to_string))
        .unwrap_or_else(|| "unknown panic msg".to_string())
}

#[cfg(test)]
mod tests {
    use crate::{EreZiren, compiler::RustMips32r2Customized};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgramInput,
    };
    use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM};
    use std::sync::OnceLock;

    fn basic_program() -> Vec<u8> {
        static PROGRAM: OnceLock<Vec<u8>> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustMips32r2Customized
                    .compile(&testing_guest_directory("ziren", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_input() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_input() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
