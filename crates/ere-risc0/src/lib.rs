use build_utils::docker;
use compile::compile_risc0_program;
use risc0_zkvm::Receipt;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, ProverResourceType,
    zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod compile;
pub use compile::Risc0Program;

mod error;
use error::Risc0Error;

#[allow(non_camel_case_types)]
pub struct RV32_IM_RISC0_ZKVM_ELF;

impl Compiler for RV32_IM_RISC0_ZKVM_ELF {
    type Error = Risc0Error;

    type Program = Risc0Program;

    fn compile(
        workspace_directory: &Path,
        guest_relative: &Path,
    ) -> Result<Self::Program, Self::Error> {
        compile_risc0_program(workspace_directory, guest_relative).map_err(Risc0Error::from)
    }
}

impl EreRisc0 {
    pub fn new(
        program: <RV32_IM_RISC0_ZKVM_ELF as Compiler>::Program,
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
    program: <RV32_IM_RISC0_ZKVM_ELF as Compiler>::Program,
    #[allow(dead_code)]
    resource_type: ProverResourceType,
}

impl zkVM for EreRisc0 {
    fn execute(&self, inputs: &Input) -> Result<ProgramExecutionReport, zkVMError> {
        // Build the Docker image
        let tag = "ere-risc0-cli:latest";
        docker::build_image(&PathBuf::from("docker/risc0/Dockerfile"), tag)
            .map_err(|e| zkVMError::Other(Box::new(e)))?;

        // Create temporary directory for file exchange
        let temp_dir = TempDir::new().map_err(|e| zkVMError::Other(Box::new(e)))?;
        let elf_path = temp_dir.path().join("guest.elf");
        let input_path = temp_dir.path().join("input");
        let report_path = temp_dir.path().join("report");

        // Write ELF file to temp directory
        fs::write(&elf_path, &self.program.elf).map_err(|e| zkVMError::Other(Box::new(e)))?;
        // Write input bytes to temp directory
        fs::write(&input_path, &serialize_input(inputs)?)
            .map_err(|e| zkVMError::Other(Box::new(e)))?;

        // Run Docker command for execution
        let status = docker::DockerRunCommand::new(tag)
            .remove_after_run()
            .with_volume(temp_dir.path().to_string_lossy().to_string(), "/workspace")
            .with_command([
                "execute",
                "/workspace/guest.elf",
                "/workspace/input",
                "/workspace/report",
            ])
            .run()
            .map_err(|e| zkVMError::Other(Box::new(e)))?;

        if !status.success() {
            return Err(zkVMError::Other("Docker execution command failed".into()));
        }

        // Read the execution report from the output file
        let report: ProgramExecutionReport = bincode::deserialize(
            &fs::read(report_path).map_err(|e| zkVMError::Other(Box::new(e)))?,
        )
        .map_err(|e| zkVMError::Other(Box::new(e)))?;

        Ok(report)
    }

    fn prove(&self, inputs: &Input) -> Result<(Vec<u8>, ProgramProvingReport), zkVMError> {
        // Build the Docker image
        let tag = "ere-risc0-cli:latest";
        docker::build_image(&PathBuf::from("docker/risc0/Dockerfile"), tag)
            .map_err(|e| zkVMError::Other(Box::new(e)))?;

        // Create temporary directory for file exchange
        let temp_dir = TempDir::new().map_err(|e| zkVMError::Other(Box::new(e)))?;
        let elf_path = temp_dir.path().join("guest.elf");
        let input_path = temp_dir.path().join("input");
        let proof_path = temp_dir.path().join("proof");
        let report_path = temp_dir.path().join("report");

        // Write ELF file to temp directory
        fs::write(&elf_path, &self.program.elf).map_err(|e| zkVMError::Other(Box::new(e)))?;
        // Write input bytes to temp directory
        fs::write(&input_path, &serialize_input(inputs)?)
            .map_err(|e| zkVMError::Other(Box::new(e)))?;

        // Run Docker command for proving
        let status = docker::DockerRunCommand::new(tag)
            .remove_after_run()
            .with_volume(temp_dir.path().to_string_lossy().to_string(), "/workspace")
            .with_command([
                "prove",
                "/workspace/guest.elf",
                "/workspace/input",
                "/workspace/proof",
                "/workspace/report",
            ])
            .run()
            .map_err(|e| zkVMError::Other(Box::new(e)))?;

        if !status.success() {
            return Err(zkVMError::Other("Docker proving command failed".into()));
        }

        // Read the proof from the output file
        let proof = fs::read(proof_path).map_err(|e| zkVMError::Other(Box::new(e)))?;
        let report = bincode::deserialize(
            &fs::read(report_path).map_err(|e| zkVMError::Other(Box::new(e)))?,
        )
        .map_err(|e| zkVMError::Other(Box::new(e)))?;

        Ok((proof, report))
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

// Serialize input bytes in the same way as the `ExecutorEnvBuilder`.
fn serialize_input(inputs: &Input) -> Result<Vec<u8>, zkVMError> {
    let mut input_bytes = Vec::new();
    for input in inputs.iter() {
        match input {
            InputItem::Object(serialize) => {
                let vec = risc0_zkvm::serde::to_vec(serialize)
                    .map_err(|e| zkVMError::Other(Box::new(e)))?;
                input_bytes.extend_from_slice(bytemuck::cast_slice(&vec));
            }
            InputItem::Bytes(items) => {
                input_bytes.extend_from_slice(&(items.len() as u32).to_le_bytes());
                input_bytes.extend_from_slice(items);
            }
        }
    }
    Ok(input_bytes)
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
        RV32_IM_RISC0_ZKVM_ELF::compile(&test_guest_path, Path::new(""))
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

    fn get_compiled_test_r0_elf() -> Result<Risc0Program, Risc0Error> {
        let test_guest_path = get_execute_test_guest_program_path();
        RV32_IM_RISC0_ZKVM_ELF::compile(&test_guest_path, Path::new(""))
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

        let zkvm = EreRisc0::new(program, ProverResourceType::Cpu);

        zkvm.execute(&input_builder)
            .unwrap_or_else(|err| panic!("Execution error: {err:?}"));
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
