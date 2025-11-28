use crate::{
    program::PicoProgram,
    zkvm::sdk::{BaseVerifyingKey, MetaProof, ProverClient},
};
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMProgramDigest,
};
use pico_p3_field::PrimeField32;
use pico_vm::emulator::stdin::EmulatorStdinBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{env, panic, time::Instant};

mod error;
mod sdk;

pub use error::Error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

#[derive(Serialize, Deserialize)]
pub struct PicoProofWithPublicValues {
    proof: MetaProof,
    public_values: Vec<u8>,
}

pub struct ErePico {
    program: PicoProgram,
}

impl ErePico {
    pub fn new(program: PicoProgram, resource: ProverResourceType) -> Result<Self, Error> {
        if !matches!(resource, ProverResourceType::Cpu) {
            panic!("Network or GPU proving not yet implemented for Pico. Use CPU resource type.");
        }
        Ok(ErePico { program })
    }

    pub fn client(&self) -> ProverClient {
        ProverClient::new(self.program.elf())
    }
}

impl zkVM for ErePico {
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let mut stdin = EmulatorStdinBuilder::default();
        stdin.write_slice(input);

        let ((total_num_cycles, region_cycles, public_values), execution_duration) =
            panic::catch_unwind(|| {
                let client = self.client();
                let start = Instant::now();
                let result = client.execute(stdin);
                (result, start.elapsed())
            })
            .map_err(|err| Error::ExecutePanic(panic_msg(err)))?;

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles,
                region_cycles,
                execution_duration,
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

        let mut stdin = EmulatorStdinBuilder::default();
        stdin.write_slice(input);

        let ((public_values, proof), proving_time) = panic::catch_unwind(|| {
            let client = self.client();
            let start = Instant::now();
            let result = client.prove(stdin)?;
            Ok((result, start.elapsed()))
        })
        .map_err(|err| Error::ProvePanic(panic_msg(err)))?
        .map_err(Error::Prove)?;

        let proof_bytes = bincode::serde::encode_to_vec(
            &PicoProofWithPublicValues {
                proof,
                public_values: public_values.clone(),
            },
            bincode::config::legacy(),
        )
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

        let client = self.client();

        let (proof, _): (PicoProofWithPublicValues, _) =
            bincode::serde::decode_from_slice(proof, bincode::config::legacy())
                .map_err(|err| CommonError::deserialize("proof", "bincode", err))?;

        client.verify(&proof.proof).map_err(Error::Verify)?;

        let claimed = <[u8; 32]>::from(Sha256::digest(&proof.public_values));
        let proved = extract_public_values_sha256_digest(&proof.proof)?;
        if claimed != proved {
            bail!(Error::UnexpectedPublicValuesDigest { claimed, proved });
        }

        Ok(proof.public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for ErePico {
    type ProgramDigest = BaseVerifyingKey;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(self.client().vk().clone())
    }
}

/// Extract public values sha256 digest from base proof of compressed proof.
/// The sha256 digest will be placed at the first 32 field elements of the
/// public values of the only base proof.
fn extract_public_values_sha256_digest(proof: &MetaProof) -> Result<[u8; 32], Error> {
    if proof.proofs().len() != 1 {
        return Err(Error::InvalidBaseProofLength(proof.proofs().len()));
    }

    if proof.proofs()[0].public_values.len() < 32 {
        return Err(Error::InvalidPublicValuesLength(
            proof.proofs()[0].public_values.len(),
        ));
    }

    Ok(proof.proofs()[0].public_values[..32]
        .iter()
        .map(|value| u8::try_from(value.as_canonical_u32()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::InvalidPublicValues)?
        .try_into()
        .unwrap())
}

fn panic_msg(err: Box<dyn std::any::Any + Send + 'static>) -> String {
    None.or_else(|| err.downcast_ref::<String>().cloned())
        .or_else(|| err.downcast_ref::<&'static str>().map(ToString::to_string))
        .unwrap_or_else(|| "unknown panic msg".to_string())
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv32imaCustomized, program::PicoProgram, zkvm::ErePico};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgramInput,
    };
    use ere_zkvm_interface::{
        compiler::Compiler,
        zkvm::{ProofKind, ProverResourceType, zkVM},
    };
    use std::sync::OnceLock;

    fn basic_program() -> PicoProgram {
        static PROGRAM: OnceLock<PicoProgram> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv32imaCustomized
                    .compile(&testing_guest_directory("pico", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_input() {
        let program = basic_program();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_input() {
        let program = basic_program();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
