#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    client::{JoltProof, JoltSdk},
    compiler::JoltProgram,
    error::JoltError,
};
use anyhow::bail;
use ere_zkvm_interface::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM,
};
use jolt_ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use std::{env, io::Cursor, time::Instant};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod client;
pub mod compiler;
pub mod error;

pub struct EreJolt {
    sdk: JoltSdk,
    _resource: ProverResourceType,
}

impl EreJolt {
    pub fn new(elf: JoltProgram, resource: ProverResourceType) -> Result<Self, JoltError> {
        if !matches!(resource, ProverResourceType::Cpu) {
            panic!("Network or GPU proving not yet implemented for Miden. Use CPU resource type.");
        }
        let sdk = JoltSdk::new(&elf);
        Ok(EreJolt {
            sdk,
            _resource: resource,
        })
    }
}

impl zkVM for EreJolt {
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let start = Instant::now();
        let (public_values, total_num_cycles) = self.sdk.execute(input)?;
        let execution_duration = start.elapsed();

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles,
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

        let mut proof_bytes = Vec::new();
        proof
            .serialize_compressed(&mut proof_bytes)
            .map_err(|err| CommonError::serialize("proof", "jolt", err))?;

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

        let proof = JoltProof::deserialize_compressed(&mut Cursor::new(proof))
            .map_err(|err| CommonError::deserialize("proof", "jolt", err))?;

        let public_values = self.sdk.verify(proof)?;

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
        EreJolt,
        compiler::{JoltProgram, RustRv64imacCustomized},
    };
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgramInput,
    };
    use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM};
    use std::sync::{Mutex, OnceLock};

    /// While proving, Jolt uses global static variables to store some
    /// parameters, that might cause panics if we prove concurrently, so we put
    /// a lock here for the test to work without the need to set test threads.
    static PROVE_LOCK: Mutex<()> = Mutex::new(());

    fn basic_program() -> JoltProgram {
        static PROGRAM: OnceLock<JoltProgram> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv64imacCustomized
                    .compile(&testing_guest_directory("jolt", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreJolt::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_input() {
        let program = basic_program();
        let zkvm = EreJolt::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreJolt::new(program, ProverResourceType::Cpu).unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_input() {
        let program = basic_program();
        let zkvm = EreJolt::new(program, ProverResourceType::Cpu).unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
