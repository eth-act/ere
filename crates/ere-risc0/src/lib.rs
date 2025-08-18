#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{compile::compile_risc0_program, error::Risc0Error};
use risc0_zkvm::{ExecutorEnv, ExecutorEnvBuilder, Receipt, default_executor};
use std::{path::Path, time::Instant};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, ProverResourceType,
    zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod compile;
mod error;
mod prove;

pub use compile::Risc0Program;

#[allow(non_camel_case_types)]
pub struct RV32_IM_RISC0_ZKVM_ELF;

impl Compiler for RV32_IM_RISC0_ZKVM_ELF {
    type Error = Risc0Error;

    type Program = Risc0Program;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        compile_risc0_program(guest_directory).map_err(Risc0Error::from)
    }
}

pub struct EreRisc0 {
    program: <RV32_IM_RISC0_ZKVM_ELF as Compiler>::Program,
    resource: ProverResourceType,
}

impl EreRisc0 {
    pub fn new(
        program: <RV32_IM_RISC0_ZKVM_ELF as Compiler>::Program,
        resource: ProverResourceType,
    ) -> Result<Self, zkVMError> {
        match resource {
            ProverResourceType::Cpu => {}
            ProverResourceType::Gpu => {
                // If not using Metal, we use the bento stack which requires
                // Docker to spin up the proving services that use Cuda.
                if !cfg!(feature = "metal") {
                    prove::bento::build_bento_images()
                        .map_err(|err| zkVMError::Other(Box::new(err)))?;
                    prove::bento::docker_compose_bento_up()
                        .map_err(|err| zkVMError::Other(Box::new(err)))?;
                }
            }
            ProverResourceType::Network(_) => {
                panic!(
                    "Network proving not yet implemented for RISC Zero. Use CPU or GPU resource type."
                );
            }
        }

        Ok(Self { program, resource })
    }
}

impl zkVM for EreRisc0 {
    fn execute(&self, inputs: &Input) -> Result<ProgramExecutionReport, zkVMError> {
        let executor = default_executor();
        let mut env = ExecutorEnv::builder();
        serialize_inputs(&mut env, inputs).map_err(|err| zkVMError::Other(err.into()))?;
        let env = env.build().map_err(|err| zkVMError::Other(err.into()))?;

        let start = Instant::now();
        let session_info = executor
            .execute(env, &self.program.elf)
            .map_err(|err| zkVMError::Other(err.into()))?;
        Ok(ProgramExecutionReport {
            total_num_cycles: session_info.cycles() as u64,
            execution_duration: start.elapsed(),
            ..Default::default()
        })
    }

    fn prove(&self, inputs: &Input) -> Result<(Vec<u8>, ProgramProvingReport), zkVMError> {
        let (receipt, proving_time) = match self.resource {
            ProverResourceType::Cpu => prove::default::prove(&self.program, inputs)?,
            ProverResourceType::Gpu => {
                if cfg!(feature = "metal") {
                    // The default prover selects the prover depending on the
                    // feature flag, if non enabled, it executes the pre-installed
                    // binary to generate the proof; if `metal` is enabled, it
                    // uses the local built binary.
                    prove::default::prove(&self.program, inputs)?
                } else {
                    prove::bento::prove(&self.program, inputs)?
                }
            }
            ProverResourceType::Network(_) => {
                panic!(
                    "Network proving not yet implemented for RISC Zero. Use CPU or GPU resource type."
                );
            }
        };

        let encoded = borsh::to_vec(&receipt).map_err(|err| zkVMError::Other(Box::new(err)))?;
        Ok((encoded, ProgramProvingReport::new(proving_time)))
    }

    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError> {
        let decoded: Receipt =
            borsh::from_slice(proof).map_err(|err| zkVMError::Other(Box::new(err)))?;

        decoded
            .verify(self.program.image_id)
            .map_err(|err| zkVMError::Other(Box::new(err)))
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl Drop for EreRisc0 {
    fn drop(&mut self) {
        if matches!(self.resource, ProverResourceType::Gpu) && !cfg!(feature = "metal") {
            prove::bento::docker_compose_bento_down().unwrap_or_else(|err| {
                tracing::error!("Failed to shutdown bento docker compose sevices\n{err}")
            })
        }
    }
}

fn serialize_inputs(env: &mut ExecutorEnvBuilder, inputs: &Input) -> Result<(), anyhow::Error> {
    for input in inputs.iter() {
        match input {
            // Corresponding to `env.read::<T>()`.
            InputItem::Object(obj) => env.write(obj)?,
            // Corresponding to `env.read::<T>()`.
            //
            // Note that we call `write_slice` to append the bytes to the inputs
            // directly, to avoid double serailization.
            InputItem::SerializedObject(bytes) => env.write_slice(bytes),
            // Corresponding to `env.read_frame()`.
            //
            // Note that `write_frame` is different from `write_slice`, it
            // prepends the `bytes.len().to_le_bytes()`.
            InputItem::Bytes(bytes) => env.write_frame(bytes),
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;
    use test_utils::host::{
        BasicProgramInputGen, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };

    static BASIC_PRORGAM: OnceLock<Risc0Program> = OnceLock::new();

    fn basic_program() -> Risc0Program {
        BASIC_PRORGAM
            .get_or_init(|| {
                RV32_IM_RISC0_ZKVM_ELF
                    .compile(&testing_guest_directory("risc0", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        let inputs = BasicProgramInputGen::valid();
        run_zkvm_execute(&zkvm, &inputs);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        for inputs in [
            BasicProgramInputGen::empty(),
            BasicProgramInputGen::invalid_string(),
            BasicProgramInputGen::invalid_type(),
        ] {
            zkvm.execute(&inputs).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        let inputs = BasicProgramInputGen::valid();
        run_zkvm_prove(&zkvm, &inputs);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        for inputs in [
            BasicProgramInputGen::empty(),
            BasicProgramInputGen::invalid_string(),
            BasicProgramInputGen::invalid_type(),
        ] {
            zkvm.prove(&inputs).unwrap_err();
        }
    }
}
