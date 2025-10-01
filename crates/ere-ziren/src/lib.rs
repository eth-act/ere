#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    compiler::ZirenProgram,
    error::{ExecuteError, ProveError, VerifyError, ZirenError},
};
use serde::de::DeserializeOwned;
use std::{io::Read, time::Instant};
use tracing::info;
use zkm_sdk::{
    CpuProver, Prover, ZKMProofKind, ZKMProofWithPublicValues, ZKMProvingKey, ZKMStdin,
    ZKMVerifyingKey,
};
use zkvm_interface::{
    Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof, ProverResourceType,
    PublicValues, zkVM, zkVMError,
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
    pub fn new(program: ZirenProgram, resource: ProverResourceType) -> Self {
        if matches!(
            resource,
            ProverResourceType::Gpu | ProverResourceType::Network(_)
        ) {
            panic!("Network or Gpu proving not yet implemented for ZKM. Use CPU resource type.");
        }
        let (pk, vk) = CpuProver::new().setup(&program);
        Self { program, pk, vk }
    }
}

impl zkVM for EreZiren {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let mut stdin = ZKMStdin::new();
        serialize_inputs(&mut stdin, inputs);

        let start = Instant::now();
        let (public_inputs, exec_report) = CpuProver::new()
            .execute(&self.program, &stdin)
            .map_err(|err| ZirenError::Execute(ExecuteError::Client(err.into())))?;
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
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        info!("Generating proof…");

        let mut stdin = ZKMStdin::new();
        serialize_inputs(&mut stdin, inputs);

        let start = std::time::Instant::now();
        let proof = CpuProver::new()
            .prove(&self.pk, stdin, ZKMProofKind::Compressed)
            .map_err(|err| ZirenError::Prove(ProveError::Client(err.into())))?;
        let proving_time = start.elapsed();

        let bytes = bincode::serialize(&proof)
            .map_err(|err| ZirenError::Prove(ProveError::Bincode(err)))?;

        Ok((
            proof.public_values.to_vec(),
            bytes,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &[u8]) -> Result<PublicValues, zkVMError> {
        info!("Verifying proof…");

        let proof: ZKMProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| ZirenError::Verify(VerifyError::Bincode(err)))?;

        let proof_kind = ZKMProofKind::from(&proof.proof);
        if !matches!(proof_kind, ZKMProofKind::Compressed) {
            return Err(ZirenError::Verify(VerifyError::InvalidProofKind(
                proof_kind,
            )))?;
        }

        CpuProver::new()
            .verify(&proof, &self.vk)
            .map_err(|err| ZirenError::Verify(VerifyError::Client(err.into())))?;

        Ok(proof.public_values.to_vec())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, reader: R) -> Result<T, zkVMError> {
        bincode::deserialize_from(reader).map_err(zkVMError::other)
    }
}

fn serialize_inputs(stdin: &mut ZKMStdin, inputs: &Input) {
    for input in inputs.iter() {
        match input {
            InputItem::Object(obj) => stdin.write(obj),
            InputItem::SerializedObject(bytes) | InputItem::Bytes(bytes) => {
                stdin.write_slice(bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{EreZiren, compiler::RustMips32r2Customized};
    use std::{panic, sync::OnceLock};
    use test_utils::host::{
        BasicProgramIo, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };
    use zkvm_interface::{Compiler, ProverResourceType, zkVM};

    static BASIC_PROGRAM: OnceLock<Vec<u8>> = OnceLock::new();

    fn basic_program() -> Vec<u8> {
        BASIC_PROGRAM
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
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        type F = fn() -> zkvm_interface::Input;

        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu);

        // Note that for some invalid cases the execution panics, but some not.
        for (inputs_gen, should_panic) in [
            // For empty input (insufficient input), the syscall reading input causes host to panics.
            (BasicProgramIo::empty as F, true),
            // For invalid type/data, the guest panics but handled properly by the host.
            (BasicProgramIo::invalid_type as F, false),
            (BasicProgramIo::invalid_data as F, false),
        ] {
            if should_panic {
                panic::catch_unwind(|| zkvm.execute(&inputs_gen())).unwrap_err();
            } else {
                zkvm.execute(&inputs_gen()).unwrap_err();
            }
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu);

        for inputs_gen in [
            BasicProgramIo::empty,
            BasicProgramIo::invalid_type,
            BasicProgramIo::invalid_data,
        ] {
            panic::catch_unwind(|| zkvm.prove(&inputs_gen())).unwrap_err();
        }
    }
}
