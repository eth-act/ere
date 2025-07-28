use crate::{
    error::{CompileError, JoltError, ProveError, VerifyError},
    utils::package_name_from_manifest,
};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use jolt::{JoltHyperKZGProof, JoltProverPreprocessing, JoltVerifierPreprocessing};
use jolt_core::host::Program;
use jolt_methods::{preprocess_prover, preprocess_verifier, prove_generic, verify_generic};
use jolt_sdk::host::DEFAULT_TARGET_DIR;
use std::{
    env::set_current_dir,
    io::Cursor,
    path::{Path, PathBuf},
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

    // Note that we use `PathBuf` as `Program` because the only way to pass elf
    // information to `jolt::host::Program` is by setting elf path, so we don't
    // bother to read it into memory here.
    type Program = PathBuf;

    fn compile(
        workspace_directory: &Path,
        guest_relative: &Path,
    ) -> Result<Self::Program, Self::Error> {
        let guest_dir = workspace_directory.join(guest_relative);

        // Change current directory for `Program::build` to build guest program.
        set_current_dir(&guest_dir).map_err(|source| CompileError::SetCurrentDirFailed {
            source,
            path: guest_dir.to_path_buf(),
        })?;

        let package_name = package_name_from_manifest(Path::new("Cargo.toml"))?;

        // Note that if this fails, it will panic, hence we need to catch it.
        let elf_path = std::panic::catch_unwind(|| {
            let mut program = Program::new(&package_name);
            program.set_std(true);
            program.build(DEFAULT_TARGET_DIR);
            program.elf.unwrap()
        })
        .map_err(|_| CompileError::BuildFailed)?;

        Ok(elf_path)
    }
}

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct EreJoltProof {
    proof: JoltHyperKZGProof,
    public_outputs: Vec<u8>,
}

pub struct EreJolt {
    program: jolt::host::Program,
    prover_preprocessing: JoltProverPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript>,
    verifier_preprocessing: JoltVerifierPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript>,
    _resource: ProverResourceType,
}

impl EreJolt {
    pub fn new(elf_path: PathBuf, _resource: ProverResourceType) -> Self {
        // Set a dummy package name because we don't need to compile anymore.
        // And once we set the `program.elf`, methods other than `Program::build`
        // will work since they only depend on the path to elf.
        let mut program = Program::new("");
        program.elf = Some(elf_path);
        let prover_preprocessing = preprocess_prover(&program);
        let verifier_preprocessing = preprocess_verifier(&program);
        EreJolt {
            program,
            prover_preprocessing,
            verifier_preprocessing,
            _resource,
        }
    }
}
impl zkVM for EreJolt {
    fn execute(
        &self,
        _inputs: &Input,
    ) -> Result<zkvm_interface::ProgramExecutionReport, zkVMError> {
        // TODO: Check how to pass private input to jolt
        let summary = self.program.clone().trace_analyze::<jolt::F>(&[]);
        let trace_len = summary.trace_len();

        Ok(ProgramExecutionReport::new(trace_len as u64))
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), zkVMError> {
        let now = std::time::Instant::now();
        let proof = prove_generic(&self.program, self.prover_preprocessing.clone(), inputs);
        let elapsed = now.elapsed();

        let mut proof_bytes = Vec::new();
        proof
            .serialize_compressed(&mut proof_bytes)
            .map_err(|err| JoltError::Prove(ProveError::Serialization(err)))?;

        Ok((proof_bytes, ProgramProvingReport::new(elapsed)))
    }

    fn verify(&self, proof_bytes: &[u8]) -> Result<(), zkVMError> {
        let proof = EreJoltProof::deserialize_compressed(&mut Cursor::new(proof_bytes))
            .map_err(|err| JoltError::Verify(VerifyError::Serialization(err)))?;

        verify_generic(proof, self.verifier_preprocessing.clone()).map_err(JoltError::Verify)?;

        Ok(())
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
    use std::{
        fs,
        path::{Path, PathBuf},
    };
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
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/jolt")
    }

    #[test]
    fn test_compile_trait() {
        let test_guest_path = get_compile_test_guest_program_path();
        let elf_path = JOLT_TARGET::compile(&test_guest_path, Path::new("")).unwrap();
        assert!(
            fs::metadata(elf_path).unwrap().len() != 0,
            "elf has not been compiled"
        );
    }

    #[test]
    fn test_execute() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program = JOLT_TARGET::compile(&test_guest_path, Path::new("")).unwrap();
        let mut inputs = Input::new();
        inputs.write(1_u32);

        let zkvm = EreJolt::new(program, ProverResourceType::Cpu);
        zkvm.execute(&inputs).unwrap();
    }

    #[test]
    fn test_prove_verify() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program = JOLT_TARGET::compile(&test_guest_path, Path::new("")).unwrap();
        let mut inputs = Input::new();
        inputs.write(1_u32);

        let zkvm = EreJolt::new(program, ProverResourceType::Cpu);
        let (proof, _) = zkvm.prove(&inputs).unwrap();
        zkvm.verify(&proof).unwrap();
    }
}
