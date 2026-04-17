use crate::construct_zkvm;
use anyhow::{Context, Error};
use ere_compiler_core::Elf;
use ere_prover_core::{ProverResource, codec::Encode, zkVMProver};
use std::fs;
use tracing::info;

pub fn run(elf: Elf, program_vk_path: &str) -> Result<(), Error> {
    let zkvm = construct_zkvm(elf, ProverResource::Cpu)?;
    let program_vk = zkvm
        .program_vk()
        .encode_to_vec()
        .context("failed to encode program_vk")?;

    fs::write(program_vk_path, &program_vk)
        .with_context(|| format!("failed to write program_vk to {program_vk_path}"))?;

    let program_vk_size = program_vk.len();
    info!("wrote {program_vk_size} bytes of encoded program_vk to {program_vk_path}");

    Ok(())
}
