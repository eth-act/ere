use std::path::Path;

use error::JoltError;
use jolt_core::host::Program;
use jolt_methods::{preprocess_prover, preprocess_verifier, prove_generic, verify_generic};
use jolt_sdk::host::DEFAULT_TARGET_DIR;
use utils::{
    deserialize_public_input_with_proof, package_name_from_manifest,
    serialize_public_input_with_proof,
};
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, ProverResourceType, zkVM,
    zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));
mod error;
mod jolt_methods;
mod utils;

#[allow(non_camel_case_types)]
pub struct JOLT_TARGET;

impl Compiler for JOLT_TARGET {
    type Error = JoltError;

    type Program = Program;

    fn compile(
        workspace_directory: &Path,
        guest_relative: &Path,
    ) -> Result<Self::Program, Self::Error> {
        let manifest_path = workspace_directory
            .join(guest_relative)
            .to_path_buf()
            .join("Cargo.toml");
        let package_name = package_name_from_manifest(&manifest_path).unwrap();
        let mut program = Program::new(&package_name);
        program.set_std(true);
        program.set_manifest_path(manifest_path);

        // TODO: Note that if this fails, it will panic which is why it doesn't return a Result.
        program.build(DEFAULT_TARGET_DIR);

        Ok(program)
    }
}

pub struct EreJolt {
    program: <JOLT_TARGET as Compiler>::Program,
}

impl EreJolt {
    pub fn new(
        program: <JOLT_TARGET as Compiler>::Program,
        _resource_type: ProverResourceType,
    ) -> Self {
        EreJolt { program }
    }
}
impl zkVM for EreJolt {
    fn execute(
        &self,
        _inputs: &Input,
    ) -> Result<zkvm_interface::ProgramExecutionReport, zkVMError> {
        // TODO: check ProgramSummary
        // TODO: FIXME
        // let summary = self
        //     .program
        //     .clone()
        //     .trace_analyze::<jolt::F>(inputs.bytes());
        // let trace_len = summary.trace_len();
        let trace_len = 0;

        Ok(ProgramExecutionReport::new(trace_len as u64))
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), zkVMError> {
        // TODO: make this stateful and do in setup since its expensive and should be done once per program;
        let preprocessed_key = preprocess_prover(&self.program);

        let now = std::time::Instant::now();
        let (output_bytes, proof) = prove_generic(&self.program, preprocessed_key, inputs);
        let elapsed = now.elapsed();

        let proof_with_public_inputs =
            serialize_public_input_with_proof(&output_bytes, &proof).unwrap();

        Ok((proof_with_public_inputs, ProgramProvingReport::new(elapsed)))
    }

    fn verify(&self, proof_with_public_inputs: &[u8]) -> Result<(), zkVMError> {
        let preprocessed_verifier = preprocess_verifier(&self.program);
        let (public_inputs, proof) =
            deserialize_public_input_with_proof(proof_with_public_inputs).unwrap();

        let mut outputs = Input::new();
        assert!(public_inputs.is_empty());
        outputs.write(public_inputs);

        // TODO: I don't think we should require the inputs when verifying
        let inputs = Input::new();

        let valid = verify_generic(proof, inputs, outputs, preprocessed_verifier);
        if valid {
            Ok(())
        } else {
            Err(zkVMError::from(JoltError::ProofVerificationFailed))
        }
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
    use crate::{EreJolt, JOLT_TARGET};
    use std::path::{Path, PathBuf};
    use zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};

    // TODO: for now, we just get one test file
    // TODO: but this should get the whole directory and compile each test
    fn get_compile_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("jolt")
            .join("compile")
            .join("basic")
            .join("guest")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/jolt")
    }

    #[test]
    fn test_compile_trait() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program = JOLT_TARGET::compile(&test_guest_path, Path::new("")).unwrap();
        assert!(program.elf.is_some(), "elf has not been compiled");
    }

    #[test]
    fn test_execute() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program = JOLT_TARGET::compile(&test_guest_path, Path::new("")).unwrap();
        let mut inputs = Input::new();
        inputs.write(1 as u32);

        let zkvm = EreJolt::new(program, ProverResourceType::Cpu);
        let _execution = zkvm.execute(&inputs).unwrap();
    }
    // #[test]
    // fn test_prove_verify() {
    //     let test_guest_path = get_compile_test_guest_program_path();
    //     let program = JOLT_TARGET::compile(&test_guest_path).unwrap();

    //     // TODO: I don't think we should require the inputs when verifying
    //     let inputs = Input::new();

    //     let (proof, _) = EreJolt::prove(&program, &inputs).unwrap();
    //     EreJolt::verify(&program, &proof).unwrap();
    // }
}
