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
mod prove_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_prove_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("risc0")
            .join("compile")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test Risc0 methods crate")
    }

    fn get_compiled_test_r0_elf_for_prove() -> Result<Risc0Program, Risc0Error> {
        let test_guest_path = get_prove_test_guest_program_path();
        RV32_IM_RISC0_ZKVM_ELF.compile(&test_guest_path)
    }

    #[test]
    fn test_prove_r0_dummy_input() {
        let program = get_compiled_test_r0_elf_for_prove().unwrap();

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        let (proof_bytes, _) = zkvm
            .prove(&input_builder)
            .unwrap_or_else(|err| panic!("Proving error in test: {err:?}"));

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        let verify_results = zkvm.verify(&proof_bytes).is_ok();
        assert!(verify_results);

        // TODO: Check public inputs
    }

    #[test]
    fn test_prove_r0_fails_on_bad_input_causing_execution_failure() {
        let elf_bytes = get_compiled_test_r0_elf_for_prove().unwrap();

        let empty_input = Input::new();

        let zkvm = EreRisc0::new(elf_bytes, ProverResourceType::Cpu).unwrap();
        let prove_result = zkvm.prove(&empty_input);
        assert!(prove_result.is_err());
    }

    #[test]
    #[ignore = "Requires GPU to run"]
    fn test_prove_r0_dummy_input_bento() {
        let program = get_compiled_test_r0_elf_for_prove().unwrap();

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreRisc0::new(program, ProverResourceType::Gpu).unwrap();

        let (proof_bytes, _) = zkvm
            .prove(&input_builder)
            .unwrap_or_else(|err| panic!("Proving error in test: {err:?}"));

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        let verify_results = zkvm.verify(&proof_bytes).is_ok();
        assert!(verify_results);
    }
}

#[cfg(test)]
mod execute_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_compiled_test_r0_elf() -> Result<Risc0Program, Risc0Error> {
        let test_guest_path = get_execute_test_guest_program_path();
        RV32_IM_RISC0_ZKVM_ELF.compile(&test_guest_path)
    }

    fn get_execute_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("risc0")
            .join("compile")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test Risc0 methods crate")
    }

    #[test]
    fn test_execute_r0_dummy_input() {
        let program = get_compiled_test_r0_elf().unwrap();

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();

        zkvm.execute(&input_builder)
            .unwrap_or_else(|err| panic!("Execution error: {err:?}"));
    }

    #[test]
    fn test_execute_r0_no_input_for_guest_expecting_input() {
        let program = get_compiled_test_r0_elf().unwrap();

        let empty_input = Input::new();

        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu).unwrap();
        let result = zkvm.execute(&empty_input);

        assert!(
            result.is_err(),
            "execute should fail if guest expects input but none is provided."
        );
    }
}
