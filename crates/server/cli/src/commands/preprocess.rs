use anyhow::{Context, Error};
use ere_compiler_core::Elf;
use ere_prover_core::{ProverResource, zkVMProver};
use tracing::info;

use crate::construct_zkvm;

pub fn run(elf: Elf) -> Result<(), Error> {
    construct_zkvm(elf, ProverResource::Cpu)?
        .preprocess()
        .context("failed to preprocess")?;
    info!("preprocessed the ELF");
    Ok(())
}
