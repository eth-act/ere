#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![allow(clippy::uninlined_format_args)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use nexus_sdk::compile::cargo::CargoPackager;
use nexus_sdk::compile::{Compile, Compiler as NexusCompiler};
use nexus_sdk::stwo::seq::{Proof, Stwo};
use nexus_sdk::{Local, Prover, Verifiable};
use tracing::info;
use zkvm_interface::{
    Compiler, Input, ProgramExecutionReport, ProgramProvingReport, ProverResourceType, zkVM,
    zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod error;
pub(crate) mod utils;

use crate::error::ProveError;
use crate::utils::get_cargo_package_name;
use error::{CompileError, NexusError, VerifyError};

#[allow(non_camel_case_types)]
pub struct NEXUS_TARGET;

impl Compiler for NEXUS_TARGET {
    type Error = NexusError;

    type Program = PathBuf;

    fn compile(&self, guest_path: &Path) -> Result<Self::Program, Self::Error> {
        // 1. Check guest path
        if !guest_path.exists() {
            return Err(NexusError::PathNotFound(guest_path.to_path_buf()));
        }
        std::env::set_current_dir(guest_path).map_err(|e| CompileError::Client(e.into()))?;

        let package_name = get_cargo_package_name(guest_path)
            .ok_or(CompileError::Client(Box::from(format!(
                "Failed to get guest package name, where guest path: {:?}",
                guest_path
            ))))
            .map_err(|e| CompileError::Client(e.into()))?;
        let mut prover_compiler = NexusCompiler::<CargoPackager>::new(&package_name);
        let elf_path = prover_compiler
            .build()
            .map_err(|e| CompileError::Client(e.into()))?;

        Ok(elf_path)
    }
}

pub struct EreNexus {
    program: <NEXUS_TARGET as Compiler>::Program,
}

impl EreNexus {
    pub fn new(
        program: <NEXUS_TARGET as Compiler>::Program,
        _resource_type: ProverResourceType,
    ) -> Self {
        Self { program }
    }
}
impl zkVM for EreNexus {
    fn execute(
        &self,
        _inputs: &Input,
    ) -> Result<zkvm_interface::ProgramExecutionReport, zkVMError> {
        // TODO: Serialize inputs by `postcard` and make sure there is no double serailization.
        // Issue for tracking: https://github.com/eth-act/ere/issues/63.

        // TODO: Execute and get cycle count

        Ok(ProgramExecutionReport::default())
    }

    fn prove(
        &self,
        _inputs: &Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), zkVMError> {
        let prover: Stwo<Local> = Stwo::new_from_file(&self.program.to_string_lossy().to_string())
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

        Ok((bytes, ProgramProvingReport::new(elapsed)))
    }

    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError> {
        info!("Verifying proof...");

        let proof: Proof = bincode::deserialize(proof)
            .map_err(|err| NexusError::Verify(VerifyError::Bincode(err)))?;

        let prover: Stwo<Local> = Stwo::new_from_file(&self.program.to_string_lossy().to_string())
            .map_err(|e| NexusError::Prove(ProveError::Client(e.into())))
            .map_err(zkVMError::from)?;
        let elf = prover.elf.clone(); // save elf for use with verification
        #[rustfmt::skip]
        proof
        .verify_expected::<(), ()>(
            &(),  // no public input
            nexus_sdk::KnownExitCodes::ExitSuccess as u32,
            &(),  // no public output
            &elf, // expected elf (program binary)
            &[],  // no associated data,
        )
        .map_err(|e| NexusError::Verify(VerifyError::Client(e.into())))
        .map_err(zkVMError::from)?;

        info!("Verify Succeeded!");
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
    use zkvm_interface::Compiler;

    use crate::NEXUS_TARGET;

    use super::*;
    use std::path::PathBuf;

    fn get_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("nexus")
            .join("guest")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/nexus")
    }

    #[test]
    fn test_compile() -> anyhow::Result<()> {
        let test_guest_path = get_test_guest_program_path();
        let elf_path = NEXUS_TARGET.compile(&test_guest_path)?;
        let prover: Stwo<Local> = Stwo::new_from_file(&elf_path.to_string_lossy().to_string())?;
        let elf = prover.elf.clone();
        assert!(
            !elf.instructions.is_empty(),
            "ELF bytes should not be empty."
        );
        Ok(())
    }

    #[test]
    fn test_execute() {
        let test_guest_path = get_test_guest_program_path();
        let elf = NEXUS_TARGET
            .compile(&test_guest_path)
            .expect("compilation failed");
        let mut input = Input::new();
        input.write(10u64);

        let zkvm = EreNexus::new(elf, ProverResourceType::Cpu);
        zkvm.execute(&input).unwrap();
    }

    #[test]
    fn test_prove_verify() -> anyhow::Result<()> {
        let test_guest_path = get_test_guest_program_path();
        let elf = NEXUS_TARGET.compile(&test_guest_path)?;
        let mut input = Input::new();
        input.write(10u64);

        let zkvm = EreNexus::new(elf, ProverResourceType::Cpu);
        let (proof, _) = zkvm.prove(&input).unwrap();
        zkvm.verify(&proof).expect("proof should verify");
        Ok(())
    }
}
