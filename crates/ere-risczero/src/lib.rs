use std::time::Instant;

use compile::compile_risczero_program;
use risc0_zkvm::{ExecutorEnv, ProverOpts, Receipt, default_executor, default_prover};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, ProverResourceType,
    zkVM, zkVMError,
};

mod compile;
pub use compile::Risc0Program;

mod error;
use error::RiscZeroError;

#[allow(non_camel_case_types)]
pub struct RV32_IM_RISCZERO_ZKVM_ELF;

impl Compiler for RV32_IM_RISCZERO_ZKVM_ELF {
    type Error = RiscZeroError;

    type Program = Risc0Program;

    fn compile(path_to_program: &std::path::Path) -> Result<Self::Program, Self::Error> {
        compile_risczero_program(path_to_program).map_err(RiscZeroError::from)
    }
}

impl EreRisc0 {
    pub fn new(
        program: <RV32_IM_RISCZERO_ZKVM_ELF as Compiler>::Program,
        resource_type: ProverResourceType,
    ) -> Self {
        match resource_type {
            ProverResourceType::Cpu => {
                #[cfg(any(feature = "cuda", feature = "metal"))]
                panic!("CPU mode requires both 'cuda' and 'metal' features to be disabled");
            }
            ProverResourceType::Gpu => {
                #[cfg(not(any(feature = "cuda", feature = "metal")))]
                panic!("GPU selected but neither 'cuda' nor 'metal' feature is enabled");
            }
            ProverResourceType::Network(_) => {
                panic!(
                    "Network proving not yet implemented for RISC Zero. Use CPU or GPU resource type."
                );
            }
        }

        Self {
            program,
            resource_type,
        }
    }
}

pub struct EreRisc0 {
    program: <RV32_IM_RISCZERO_ZKVM_ELF as Compiler>::Program,
    #[allow(dead_code)]
    resource_type: ProverResourceType,
}

impl zkVM for EreRisc0 {
    fn execute(&self, inputs: &Input) -> Result<ProgramExecutionReport, zkVMError> {
        let executor = default_executor();
        let mut env = ExecutorEnv::builder();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => {
                    env.write(serialize).unwrap();
                }
                InputItem::Bytes(items) => {
                    env.write_frame(items);
                }
            }
        }
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
        let prover = default_prover();
        let mut env = ExecutorEnv::builder();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => {
                    env.write(serialize).unwrap();
                }
                InputItem::Bytes(items) => {
                    env.write_frame(&items);
                }
            }
        }
        let env = env.build().map_err(|err| zkVMError::Other(err.into()))?;

        let now = std::time::Instant::now();
        let prove_info = prover
            .prove_with_opts(env, &self.program.elf, &ProverOpts::succinct())
            .map_err(|err| zkVMError::Other(err.into()))?;
        let proving_time = now.elapsed();

        let encoded =
            borsh::to_vec(&prove_info.receipt).map_err(|err| zkVMError::Other(Box::new(err)))?;
        Ok((encoded, ProgramProvingReport::new(proving_time)))
    }

    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError> {
        let decoded: Receipt =
            borsh::from_slice(&proof).map_err(|err| zkVMError::Other(Box::new(err)))?;

        decoded
            .verify(self.program.image_id)
            .map_err(|err| zkVMError::Other(Box::new(err)))
    }
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
            .join("risczero")
            .join("compile")
            .join("project_structure_build")
            .canonicalize()
            .expect("Failed to find or canonicalize test Risc0 methods crate")
    }

    fn get_compiled_test_r0_elf_for_prove() -> Result<Risc0Program, RiscZeroError> {
        let test_guest_path = get_prove_test_guest_program_path();
        RV32_IM_RISCZERO_ZKVM_ELF::compile(&test_guest_path)
    }

    #[test]
    fn test_prove_r0_dummy_input() {
        let program = get_compiled_test_r0_elf_for_prove().unwrap();

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu);

        let proof_bytes = match zkvm.prove(&input_builder) {
            Ok((prove_result, _)) => prove_result,
            Err(err) => {
                panic!("Proving error in test: {:?}", err);
            }
        };

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        let verify_results = zkvm.verify(&proof_bytes).is_ok();
        assert!(verify_results);

        // TODO: Check public inputs
    }

    #[test]
    // TODO: Note: SP1 will panic here
    // #[should_panic]
    fn test_prove_r0_fails_on_bad_input_causing_execution_failure() {
        let elf_bytes = get_compiled_test_r0_elf_for_prove().unwrap();

        let empty_input = Input::new();

        let zkvm = EreRisc0::new(elf_bytes, ProverResourceType::Cpu);
        let prove_result = zkvm.prove(&empty_input);
        assert!(prove_result.is_err());
    }
}

#[cfg(test)]
mod execute_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_compiled_test_r0_elf() -> Result<Risc0Program, RiscZeroError> {
        let test_guest_path = get_execute_test_guest_program_path();
        RV32_IM_RISCZERO_ZKVM_ELF::compile(&test_guest_path)
    }

    fn get_execute_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("risczero")
            .join("compile")
            .join("project_structure_build")
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

        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu);

        let result = zkvm.execute(&input_builder);

        if let Err(e) = &result {
            panic!("Execution error: {:?}", e);
        }
    }

    #[test]
    fn test_execute_r0_no_input_for_guest_expecting_input() {
        let program = get_compiled_test_r0_elf().unwrap();

        let empty_input = Input::new();

        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu);
        let result = zkvm.execute(&empty_input);

        assert!(
            result.is_err(),
            "execute should fail if guest expects input but none is provided."
        );
    }
}
