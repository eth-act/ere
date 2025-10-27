#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{compiler::NexusProgram, error::NexusError};
use anyhow::bail;
use ere_zkvm_interface::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM,
};
use nexus_core::nvm::{self, ElfFile};
use nexus_sdk::{
    KnownExitCodes, Prover, Verifiable, Viewable,
    stwo::seq::{Proof as NexusProof, Stwo},
};
use nexus_vm::trace::Trace;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::info;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod compiler;
pub mod error;

#[derive(Serialize, Deserialize)]
pub struct NexusProofBundle {
    proof: NexusProof,
    public_values: Vec<u8>,
}

pub struct EreNexus {
    elf: NexusProgram,
}

impl EreNexus {
    pub fn new(elf: NexusProgram, resource: ProverResourceType) -> Result<Self, NexusError> {
        if !matches!(resource, ProverResourceType::Cpu) {
            panic!("Network or GPU proving not yet implemented for Nexus. Use CPU resource type.");
        }
        Ok(Self { elf })
    }
}

impl zkVM for EreNexus {
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let elf = ElfFile::from_bytes(&self.elf).map_err(NexusError::ParseElf)?;

        // Nexus sdk does not provide a trace, so we need to use core `nvm`
        // Encoding is copied directly from `prove_with_input`
        let mut private_encoded = if input.is_empty() {
            Vec::new()
        } else {
            postcard::to_stdvec_cobs(&input)
                .map_err(|err| CommonError::serialize("input", "postcard", err))?
        };

        if !private_encoded.is_empty() {
            let private_padded_len = (private_encoded.len() + 3) & !3;
            assert!(private_padded_len >= private_encoded.len());
            private_encoded.resize(private_padded_len, 0x00);
        }

        let start = Instant::now();
        let (view, trace) = nvm::k_trace(elf, &[], &[], private_encoded.as_slice(), 1)
            .map_err(NexusError::Execute)?;
        let execution_duration = start.elapsed();

        let public_values = view
            .public_output()
            .map_err(|err| CommonError::deserialize("public_values", "postcard", err))?;

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles: trace.get_num_steps() as u64,
                execution_duration,
                ..Default::default()
            },
        ))
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

        let elf = ElfFile::from_bytes(&self.elf).map_err(NexusError::ParseElf)?;

        let prover = Stwo::new(&elf).map_err(NexusError::Prove)?;

        let start = Instant::now();
        let (view, proof) = prover
            .prove_with_input(&input, &())
            .map_err(NexusError::Prove)?;
        let proving_time = start.elapsed();

        let public_values = view
            .public_output()
            .map_err(|err| CommonError::deserialize("public_values", "postcard", err))?;

        let proof_bundle = NexusProofBundle {
            proof,
            public_values,
        };

        let proof_bytes = bincode::serde::encode_to_vec(&proof_bundle, bincode::config::legacy())
            .map_err(|err| CommonError::serialize("proof", "bincode", err))?;

        Ok((
            proof_bundle.public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let Proof::Compressed(proof) = proof else {
            bail!(CommonError::unsupported_proof_kind(
                proof.kind(),
                [ProofKind::Compressed]
            ))
        };

        info!("Verifying proof...");

        let (proof_bundle, _): (NexusProofBundle, _) =
            bincode::serde::decode_from_slice(proof, bincode::config::legacy())
                .map_err(|err| CommonError::deserialize("proof", "bincode", err))?;

        proof_bundle
            .proof
            .verify_expected_from_program_bytes::<(), Vec<u8>>(
                &(),
                KnownExitCodes::ExitSuccess as u32,
                &proof_bundle.public_values,
                &self.elf,
                &[],
            )
            .map_err(NexusError::Verify)?;

        info!("Verify Succeeded!");

        Ok(proof_bundle.public_values)
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
    use crate::{EreNexus, NexusProgram, compiler::RustRv32i};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgramInput,
    };
    use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM};
    use std::sync::OnceLock;

    fn basic_program() -> NexusProgram {
        static PROGRAM: OnceLock<NexusProgram> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv32i
                    .compile(&testing_guest_directory("nexus", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_input() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgramInput::valid();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_input() {
        let program = basic_program();
        let zkvm = EreNexus::new(program, ProverResourceType::Cpu).unwrap();

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
