#![cfg_attr(not(test), warn(unused_crate_dependencies))]

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

use serde::de::DeserializeOwned;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use tracing::info;
use zkm_build::{BuildArgs, execute_build_program};
use zkm_sdk::{ProverClient, ZKMProofWithPublicValues, ZKMProvingKey, ZKMStdin, ZKMVerifyingKey};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};

// mod compile;

mod error;
use crate::error::CompileError;
use error::{ExecuteError, ProveError, VerifyError, ZKMError};

#[allow(non_camel_case_types)]
pub struct RV32_IM_ZKM_ZKVM_ELF;
pub struct EreZKM {
    program: <RV32_IM_ZKM_ZKVM_ELF as Compiler>::Program,
    /// Proving key
    pk: ZKMProvingKey,
    /// Verification key
    vk: ZKMVerifyingKey,
    /// Proof and Verification orchestrator
    client: ProverClient,
}

impl Compiler for RV32_IM_ZKM_ZKVM_ELF {
    type Error = ZKMError;

    type Program = Vec<u8>;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let target_elf_paths =
            execute_build_program(&BuildArgs::default(), Some(guest_directory.to_path_buf()))
                .map_err(|e| ZKMError::CompileError(CompileError::Client(Box::from(e))))?;
        if target_elf_paths.is_empty() {
            return Err(ZKMError::CompileError(CompileError::Client(Box::from(
                "No ELF files were built.",
            ))));
        }

        let elf_path = &target_elf_paths[0].1;
        println!("3");

        println!("3. elf_path: {:?}", elf_path);
        let bytes = std::fs::read(elf_path)
            .map_err(|e| ZKMError::CompileError(CompileError::Client(Box::from(e))))?;
        println!("4");

        Ok(bytes.to_vec())
    }
}

impl EreZKM {
    pub fn new(
        program: <RV32_IM_ZKM_ZKVM_ELF as Compiler>::Program,
        resource: ProverResourceType,
    ) -> Self {
        let client = match resource {
            ProverResourceType::Cpu => ProverClient::cpu(),
            _ => unimplemented!(
                "Network or Gpu proving not yet implemented for ZKM. Use CPU resource type."
            ),
        };
        let (pk, vk) = client.setup(&program);

        Self {
            program,
            client,
            pk,
            vk,
        }
    }
}

impl zkVM for EreZKM {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let mut stdin = ZKMStdin::new();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::SerializedObject(bytes) | InputItem::Bytes(bytes) => {
                    stdin.write_slice(bytes)
                }
            }
        }

        let start = Instant::now();
        let (public_inputs, exec_report) = self
            .client
            .execute(&self.program, stdin)
            .run()
            .map_err(|err| ZKMError::Execute(ExecuteError::Client(Box::from(err))))?;
        Ok((
            public_inputs.to_vec(),
            ProgramExecutionReport {
                total_num_cycles: exec_report.total_instruction_count(),
                region_cycles: exec_report.cycle_tracker.into_iter().collect(),
                execution_duration: start.elapsed(),
            },
        ))
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        info!("Generating proof…");

        let mut stdin = ZKMStdin::new();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::SerializedObject(bytes) | InputItem::Bytes(bytes) => {
                    stdin.write_slice(bytes)
                }
            }
        }

        let start = std::time::Instant::now();
        let proof_with_inputs = self
            .client
            .prove(&self.pk, stdin)
            .run()
            .map_err(|err| ZKMError::Execute(ExecuteError::Client(Box::from(err))))?;
        let proving_time = start.elapsed();

        let bytes = bincode::serialize(&proof_with_inputs)
            .map_err(|err| ZKMError::Prove(ProveError::Bincode(err)))?;

        Ok((
            proof_with_inputs.public_values.to_vec(),
            bytes,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &[u8]) -> Result<PublicValues, zkVMError> {
        info!("Verifying proof…");

        let proof: ZKMProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| ZKMError::Verify(VerifyError::Bincode(err)))?;

        self.client
            .verify(&proof, &self.vk)
            .map_err(|e| ZKMError::Verify(VerifyError::Client(Box::new(e))))
            .map_err(zkVMError::from)?;

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

#[cfg(test)]
#[allow(non_snake_case)]
mod execute_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_compiled_test_ZKM_elf() -> Result<Vec<u8>, ZKMError> {
        let test_guest_path = get_execute_test_guest_program_path();
        println!("Test guest path: {:?}", test_guest_path);
        RV32_IM_ZKM_ZKVM_ELF.compile(&test_guest_path)
    }

    fn get_execute_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("zkm")
            .join("compile")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/ZKM")
    }

    #[test]
    fn test_execute_ZKM_dummy_input() {
        let elf_bytes = get_compiled_test_ZKM_elf()
            .expect("Failed to compile test ZKM guest for execution test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        input_builder.write(n);

        let zkvm = EreZKM::new(elf_bytes, ProverResourceType::Cpu);

        let result = zkvm.execute(&input_builder);

        if let Err(e) = &result {
            panic!("Execution error: {:?}", e);
        }
    }

    #[test]
    fn test_execute_ZKM_no_input_for_guest_expecting_input() {
        let elf_bytes = get_compiled_test_ZKM_elf()
            .expect("Failed to compile test ZKM guest for execution test");

        let empty_input = Input::new();

        let zkvm = EreZKM::new(elf_bytes, ProverResourceType::Cpu);
        let result = zkvm.execute(&empty_input);

        assert!(
            result.is_err(),
            "execute should fail if guest expects input but none is provided."
        );
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod prove_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_prove_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("zkm")
            .join("compile")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/ZKM")
    }

    fn get_compiled_test_ZKM_elf_for_prove() -> Result<Vec<u8>, ZKMError> {
        let test_guest_path = get_prove_test_guest_program_path();
        RV32_IM_ZKM_ZKVM_ELF.compile(&test_guest_path)
    }

    #[test]
    fn test_prove_ZKM_dummy_input() {
        let elf_bytes = get_compiled_test_ZKM_elf_for_prove()
            .expect("Failed to compile test ZKM guest for proving test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreZKM::new(elf_bytes, ProverResourceType::Cpu);

        let proof_bytes = match zkvm.prove(&input_builder) {
            Ok((public_inputs, prove_result, _)) => prove_result,
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
    #[should_panic]
    fn test_prove_ZKM_fails_on_bad_input_causing_execution_failure() {
        let elf_bytes = get_compiled_test_ZKM_elf_for_prove()
            .expect("Failed to compile test ZKM guest for proving test");

        let empty_input = Input::new();

        let zkvm = EreZKM::new(elf_bytes, ProverResourceType::Cpu);
        let prove_result = zkvm.prove(&empty_input);
        assert!(prove_result.is_err())
    }
}
