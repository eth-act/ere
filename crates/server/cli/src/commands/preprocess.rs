use anyhow::{Context, Error};
use ere_compiler_core::Elf;
use ere_prover_core::{ProverResource, zkVMProver};
use tracing::info;

use crate::construct_zkvm;

pub fn run(elf: Elf) -> Result<(), Error> {
    construct_zkvm(elf, resource())?
        .preprocess()
        .context("failed to preprocess")?;
    info!("preprocessed the ELF");
    Ok(())
}

/// Returns the CPU, except for a Pico `cuda` build, which has no CPU prover.
fn resource() -> ProverResource {
    if cfg!(all(feature = "pico", feature = "cuda")) {
        ProverResource::Gpu
    } else {
        ProverResource::Cpu
    }
}
