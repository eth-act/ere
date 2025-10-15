#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    compiler::NexusProgram,
    error::{NexusError, ProveError, VerifyError},
};
use ere_zkvm_interface::{
    Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, ProverResourceType,
    PublicValues, zkVM, zkVMError,
};
use nexus_sdk::{Local, Prover, Verifiable, stwo::seq::Stwo};
use serde::de::DeserializeOwned;
use std::{io::Read, time::Instant};
use tracing::info;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod compiler;
pub mod error;

pub struct EreNexus {
    program: NexusProgram,
}

impl EreNexus {
    pub fn new(program: NexusProgram, _resource_type: ProverResourceType) -> Self {
        Self { program }
    }
}

impl zkVM for EreNexus {
    fn execute(
        &self,
        _inputs: &Input,
    ) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        // TODO: Serialize inputs by `postcard` and make sure there is no double serailization.
        // Issue for tracking: https://github.com/eth-act/ere/issues/63.

        // TODO: Execute and get cycle count

        // TODO: Public values
        let public_values = Vec::new();

        Ok((public_values, ProgramExecutionReport::default()))
    }

    fn prove(
        &self,
        _inputs: &Input,
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        if proof_kind != ProofKind::Compressed {
            panic!("Only Compressed proof kind is supported.");
        }

        let prover: Stwo<Local> = Stwo::new_from_bytes(&self.program)
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))
            .map_err(zkVMError::from)?;

        // TODO: Serialize inputs by `postcard` and make sure there is no double serailization.
        // Issue for tracking: https://github.com/eth-act/ere/issues/63.

        let now = Instant::now();
        let (_view, proof) = prover
            .prove_with_input(&(), &())
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))
            .map_err(zkVMError::from)?;
        let elapsed = now.elapsed();

        let bytes = bincode::serialize(&proof)
            .map_err(|err| NexusError::Prove(ProveError::Bincode(err)))?;

        // TODO: Public values
        let public_values = Vec::new();

        Ok((
            public_values,
            Proof::Compressed(bytes),
            ProgramProvingReport::new(elapsed),
        ))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        let Proof::Compressed(proof) = proof else {
            return Err(zkVMError::other("Only Compressed proof kind is supported."));
        };

        info!("Verifying proof...");

        let proof: nexus_sdk::stwo::seq::Proof = bincode::deserialize(proof)
            .map_err(|err| NexusError::Verify(VerifyError::Bincode(err)))?;

        let prover: Stwo<Local> = Stwo::new_from_bytes(&self.program)
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))
            .map_err(zkVMError::from)?;
        let elf = prover.elf.clone(); // save elf for use with verification
        proof
            .verify_expected::<(), ()>(
                &(), // no public input
                nexus_sdk::KnownExitCodes::ExitSuccess as u32,
                &(),  // no public output
                &elf, // expected elf (program binary)
                &[],  // no associated data,
            )
            .map_err(|e| NexusError::Verify(VerifyError::Client(e.into())))
            .map_err(zkVMError::from)?;

        info!("Verify Succeeded!");

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
        // Issue for tracking: https://github.com/eth-act/ere/issues/63.
        todo!()
    }
}
