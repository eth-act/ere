#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    error::{CompileError, JoltError, ProveError, VerifyError},
    utils::package_name_from_manifest,
};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use jolt::{JoltHyperKZGProof, JoltProverPreprocessing, JoltVerifierPreprocessing};
use jolt_core::host::Program;
use jolt_methods::{preprocess_prover, preprocess_verifier, prove_generic, verify_generic};
use jolt_sdk::host::DEFAULT_TARGET_DIR;
use serde::de::DeserializeOwned;
use std::{
    env::set_current_dir,
    fs,
    io::{Cursor, Read},
    path::Path,
};
use tempfile::TempDir;
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProverResourceType,
    PublicValues, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));
mod error;
mod jolt_methods;
mod utils;

#[allow(non_camel_case_types)]
pub struct JOLT_TARGET;

impl Compiler for JOLT_TARGET {
    type Error = JoltError;

    type Program = Vec<u8>;

    fn compile(&self, guest_dir: &Path) -> Result<Self::Program, Self::Error> {
        // Change current directory for `Program::build` to build guest program.
        set_current_dir(guest_dir).map_err(|source| CompileError::SetCurrentDirFailed {
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

        let elf = fs::read(&elf_path).map_err(|source| CompileError::ReadElfFailed {
            source,
            path: elf_path.to_path_buf(),
        })?;

        Ok(elf)
    }
}

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct EreJoltProof {
    proof: JoltHyperKZGProof,
    public_outputs: Vec<u8>,
}

pub struct EreJolt {
    elf: Vec<u8>,
    prover_preprocessing: JoltProverPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript>,
    verifier_preprocessing: JoltVerifierPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript>,
    _resource: ProverResourceType,
}

impl EreJolt {
    pub fn new(elf: Vec<u8>, _resource: ProverResourceType) -> Result<Self, zkVMError> {
        let (_tempdir, program) = program(&elf)?;
        let prover_preprocessing = preprocess_prover(&program);
        let verifier_preprocessing = preprocess_verifier(&program);
        Ok(EreJolt {
            elf,
            prover_preprocessing,
            verifier_preprocessing,
            _resource,
        })
    }
}

impl zkVM for EreJolt {
    fn execute(
        &self,
        _inputs: &Input,
    ) -> Result<(PublicValues, zkvm_interface::ProgramExecutionReport), zkVMError> {
        let (_tempdir, program) = program(&self.elf)?;

        // TODO: Check how to pass private input to jolt, issue for tracking:
        //       https://github.com/a16z/jolt/issues/371.
        let summary = program.clone().trace_analyze::<jolt::F>(&[]);
        let trace_len = summary.trace_len();

        // TODO: Public values
        let public_values = Vec::new();

        Ok((public_values, ProgramExecutionReport::new(trace_len as u64)))
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, zkvm_interface::ProgramProvingReport), zkVMError> {
        let (_tempdir, program) = program(&self.elf)?;

        let now = std::time::Instant::now();
        let proof = prove_generic(&program, self.prover_preprocessing.clone(), inputs);
        let elapsed = now.elapsed();

        let mut proof_bytes = Vec::new();
        proof
            .serialize_compressed(&mut proof_bytes)
            .map_err(|err| JoltError::Prove(ProveError::Serialization(err)))?;

        // TODO: Public values
        let public_values = Vec::new();

        Ok((
            public_values,
            proof_bytes,
            ProgramProvingReport::new(elapsed),
        ))
    }

    fn verify(&self, proof_bytes: &[u8]) -> Result<PublicValues, zkVMError> {
        let proof = EreJoltProof::deserialize_compressed(&mut Cursor::new(proof_bytes))
            .map_err(|err| JoltError::Verify(VerifyError::Serialization(err)))?;

        verify_generic(proof, self.verifier_preprocessing.clone()).map_err(JoltError::Verify)?;

        // TODO: Public values
        let public_values = Vec::new();

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, _reader: R) -> Result<T, zkVMError> {
        todo!()
    }
}

/// Create `jolt::host::Program` by storing the compiled `elf` to a temporary
/// file, and set the elf path for `program`, so methods like `decode`, `trace`
/// and `trace_analyze` that depend on elf path will work.
pub fn program(elf: &[u8]) -> Result<(TempDir, jolt::host::Program), zkVMError> {
    let tempdir = TempDir::new().map_err(|err| zkVMError::Other(err.into()))?;
    let elf_path = tempdir.path().join("guest.elf");
    fs::write(&elf_path, elf).map_err(|err| zkVMError::Other(err.into()))?;
    // Set a dummy package name because we don't need to compile anymore.
    let mut program = Program::new("");
    program.elf = Some(elf_path);
    Ok((tempdir, program))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;
    use test_utils::host::testing_guest_directory;

    static BASIC_PRORGAM: OnceLock<Vec<u8>> = OnceLock::new();

    fn basic_program() -> Vec<u8> {
        BASIC_PRORGAM
            .get_or_init(|| {
                JOLT_TARGET
                    .compile(&testing_guest_directory("jolt", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_compiler_impl() {
        let elf_bytes = basic_program();
        assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
    }
}
