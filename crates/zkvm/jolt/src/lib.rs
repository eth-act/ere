#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    compiler::JoltProgram,
    error::JoltError,
    jolt_methods::{preprocess_prover, preprocess_verifier, prove_generic, verify_generic},
};
use anyhow::bail;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ere_zkvm_interface::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM,
};
use jolt::{JoltHyperKZGProof, JoltProverPreprocessing, JoltVerifierPreprocessing};
use std::{env, fs, io::Cursor};
use tempfile::TempDir;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod compiler;
pub mod error;
mod jolt_methods;

#[derive(CanonicalSerialize, CanonicalDeserialize)]
pub struct EreJoltProof {
    proof: JoltHyperKZGProof,
    public_outputs: Vec<u8>,
}

pub struct EreJolt {
    elf: JoltProgram,
    prover_preprocessing: JoltProverPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript>,
    verifier_preprocessing: JoltVerifierPreprocessing<4, jolt::F, jolt::PCS, jolt::ProofTranscript>,
    _resource: ProverResourceType,
}

impl EreJolt {
    pub fn new(elf: JoltProgram, resource: ProverResourceType) -> Result<Self, JoltError> {
        if !matches!(resource, ProverResourceType::Cpu) {
            panic!("Network or GPU proving not yet implemented for Miden. Use CPU resource type.");
        }
        let (_tempdir, program) = program(&elf)?;
        let prover_preprocessing = preprocess_prover(&program);
        let verifier_preprocessing = preprocess_verifier(&program);
        Ok(EreJolt {
            elf,
            prover_preprocessing,
            verifier_preprocessing,
            _resource: resource,
        })
    }
}

impl zkVM for EreJolt {
    fn execute(&self, _input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
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
        input: &[u8],
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        if proof_kind != ProofKind::Compressed {
            bail!(CommonError::unsupported_proof_kind(
                proof_kind,
                [ProofKind::Compressed]
            ))
        }

        let (_tempdir, program) = program(&self.elf)?;

        let now = std::time::Instant::now();
        let proof = prove_generic(&program, self.prover_preprocessing.clone(), input);
        let elapsed = now.elapsed();

        let mut proof_bytes = Vec::new();
        proof
            .serialize_compressed(&mut proof_bytes)
            .map_err(|err| CommonError::serialize("proof", "jolt", err))?;

        // TODO: Public values
        let public_values = Vec::new();

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(elapsed),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let Proof::Compressed(proof) = proof else {
            bail!(CommonError::unsupported_proof_kind(
                proof.kind(),
                [ProofKind::Compressed]
            ))
        };

        let proof = EreJoltProof::deserialize_compressed(&mut Cursor::new(proof))
            .map_err(|err| CommonError::deserialize("proof", "jolt", err))?;

        verify_generic(proof, self.verifier_preprocessing.clone())?;

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
}

/// Create `jolt::host::Program` by storing the compiled `elf` to a temporary
/// file, and set the elf path for `program`, so methods like `decode`, `trace`
/// and `trace_analyze` that depend on elf path will work.
pub fn program(elf: &[u8]) -> Result<(TempDir, jolt::host::Program), JoltError> {
    let tempdir = TempDir::new().map_err(CommonError::tempdir)?;
    let elf_path = tempdir.path().join("guest.elf");
    fs::write(&elf_path, elf).map_err(|err| CommonError::write_file("elf", &elf_path, err))?;
    // Set a dummy package name because we don't need to compile anymore.
    let mut program = jolt::host::Program::new("");
    program.elf = Some(elf_path);
    Ok((tempdir, program))
}
