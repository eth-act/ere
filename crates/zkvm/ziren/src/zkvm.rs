use crate::program::ZirenProgram;
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMProgramDigest,
};
use std::{panic, time::Instant};
use tracing::info;
use zkm_sdk::{
    CpuProver, Prover, ZKMProofKind, ZKMProofWithPublicValues, ZKMProvingKey, ZKMStdin,
    ZKMVerifyingKey,
};

mod error;

pub use error::Error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub struct EreZiren {
    program: ZirenProgram,
    pk: ZKMProvingKey,
    vk: ZKMVerifyingKey,
}

impl EreZiren {
    pub fn new(program: ZirenProgram, resource: ProverResourceType) -> Result<Self, Error> {
        if matches!(
            resource,
            ProverResourceType::Gpu | ProverResourceType::Network(_)
        ) {
            panic!("Network or Gpu proving not yet implemented for ZKM. Use CPU resource type.");
        }
        let (pk, vk) = CpuProver::new().setup(program.elf());
        Ok(Self { program, pk, vk })
    }
}

impl zkVM for EreZiren {
    fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let stdin = input_to_stdin(input)?;

        let start = Instant::now();
        let (public_inputs, exec_report) = CpuProver::new()
            .execute(self.program.elf(), &stdin)
            .map_err(Error::Execute)?;
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
        input: &Input,
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        info!("Generating proof…");

        let stdin = input_to_stdin(input)?;

        let inner_proof_kind = match proof_kind {
            ProofKind::Compressed => ZKMProofKind::Compressed,
            ProofKind::Groth16 => ZKMProofKind::Groth16,
        };

        let start = std::time::Instant::now();
        let proof =
            panic::catch_unwind(|| CpuProver::new().prove(&self.pk, stdin, inner_proof_kind))
                .map_err(|err| Error::ProvePanic(panic_msg(err)))?
                .map_err(Error::Prove)?;
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
            bail!(Error::InvalidProofKind(proof_kind, inner_proof_kind));
        }

        CpuProver::new()
            .verify(&proof, &self.vk)
            .map_err(Error::Verify)?;

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

fn input_to_stdin(input: &Input) -> Result<ZKMStdin, Error> {
    let mut stdin = ZKMStdin::new();
    stdin.write_slice(input.stdin());
    if let Some(proofs) = input.proofs() {
        for (proof, vk) in proofs.map_err(Error::DeserializeInputProofs)? {
            stdin.write_proof(proof, vk);
        }
    }
    Ok(stdin)
}

fn panic_msg(err: Box<dyn std::any::Any + Send + 'static>) -> String {
    None.or_else(|| err.downcast_ref::<String>().cloned())
        .or_else(|| err.downcast_ref::<&'static str>().map(ToString::to_string))
        .unwrap_or_else(|| "unknown panic msg".to_string())
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustMips32r2Customized, program::ZirenProgram, zkvm::EreZiren};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{
        compiler::Compiler,
        zkvm::{Input, ProofKind, ProverResourceType, zkVM},
    };
    use std::sync::OnceLock;

    fn basic_program() -> ZirenProgram {
        static PROGRAM: OnceLock<ZirenProgram> = OnceLock::new();
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

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
