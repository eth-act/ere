use crate::zkvm::{
    Error,
    sdk::{cluster::ClusterClient, local::LocalProver},
};
use bytemuck::cast_slice;
use ere_zkvm_interface::zkvm::{CommonError, ProverResource, ProverResourceKind, PublicValues};
use proofman_common::{
    MpiCtx, ParamsGPU, ProofCtx, ProofType, SetupCtx, SetupsVadcop, VerboseMode,
};
use proofman_fields::Goldilocks;
use proofman_util::VadcopFinalProof;
use proofman_verifier::verify_vadcop_final;
use std::{
    any::Any,
    env,
    mem::ManuallyDrop,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tempfile::tempdir;
use zisk_core::{Riscv2zisk, ZiskRom};
use zisk_rom_setup::rom_merkle_setup;
use zisk_sdk::{ElfBinaryFromFile, ZISK_PUBLICS, ZiskProofWithPublicValues};
use ziskemu::{Emu, EmuOptions};

mod cluster;
mod local;

/// Merkle root of ROM trace.
pub type ProgramVk = [u8; 32];

/// Verifying key of the aggregation proof.
pub const VADCOP_FINAL_VK: [u64; 4] = [
    944087685208638250,
    1018683872844500993,
    1859314700573321599,
    17405033883420002132,
];

/// Prover backend - either local or cluster.
#[allow(clippy::large_enum_variant)]
pub enum ZiskProver {
    Local(LocalProver),
    Cluster(ClusterClient),
}

pub struct ZiskSdk {
    rom: ZiskRom,
    program_vk: ProgramVk,
    prover: ZiskProver,
}

impl ZiskSdk {
    /// Returns SDK for the ELF.
    pub fn new(elf: Vec<u8>, resource: ProverResource) -> Result<Self, Error> {
        match &resource {
            ProverResource::Cpu if cfg!(feature = "cuda") => Err(Error::CudaFeatureEnabled)?,
            ProverResource::Gpu if cfg!(not(feature = "cuda")) => Err(Error::CudaFeatureDisabled)?,
            ProverResource::Network(_) => Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [
                    ProverResourceKind::Cpu,
                    ProverResourceKind::Gpu,
                    ProverResourceKind::Cluster,
                ],
            ))?,
            _ => {}
        };

        // Convert ELF to ZisK ROM
        let rom = Riscv2zisk::new(&elf)
            .run()
            .map_err(|e| Error::Riscv2zisk(e.to_string()))?;

        // Compute program VK
        let program_vk = compute_program_vk(&elf)?;

        // Initialize prover
        let prover = match &resource {
            ProverResource::Cpu | ProverResource::Gpu => ZiskProver::Local(LocalProver::new(elf)?),
            ProverResource::Cluster(config) => ZiskProver::Cluster(ClusterClient::new(config)?),
            _ => unreachable!(),
        };

        Ok(Self {
            rom,
            program_vk,
            prover,
        })
    }

    pub fn program_vk(&self) -> ProgramVk {
        self.program_vk
    }

    /// Execute the ELF with the given `stdin`.
    pub fn execute(&self, stdin: &[u8]) -> Result<(PublicValues, u64), Error> {
        let mut emu = Emu::new(&self.rom);
        emu.ctx = emu.create_emu_context(stdin.to_vec(), &EmuOptions::default());

        panic::catch_unwind(AssertUnwindSafe(|| emu.run_fast(&EmuOptions::default())))
            .map_err(|err| Error::EmulatorPanic(panic_msg(err)))?;

        if !emu.ctx.inst_ctx.end {
            return Err(Error::EmulatorNotTerminated);
        }

        if emu.ctx.inst_ctx.error {
            return Err(Error::EmulatorError);
        }

        let public_values = emu.get_output_8();
        let total_num_cycles = emu.number_of_steps();

        Ok((public_values, total_num_cycles))
    }

    /// Prove the ELF with the given stdin.
    ///
    /// Returns the public values, proof, and proving time.
    pub fn prove(&self, stdin: &[u8]) -> Result<(PublicValues, Vec<u8>, Duration), Error> {
        let (proof, proving_time) = match &self.prover {
            ZiskProver::Local(local) => local.prove(stdin)?,
            ZiskProver::Cluster(client) => client.prove(stdin)?,
        };

        // Extract public values and program_vk
        let (public_values, proved_program_vk) = extract_public_values_and_progam_vk(&proof)?;

        // The proved program VK should match the preprocessed
        if proved_program_vk != self.program_vk {
            return Err(Error::UnexpectedProgramVk {
                preprocessed: self.program_vk,
                proved: proved_program_vk,
            });
        }

        Ok((
            public_values,
            bincode::serde::encode_to_vec(&proof, bincode::config::legacy())
                .map_err(|err| CommonError::serialize("proof", "bincode", err))?,
            proving_time,
        ))
    }

    /// Verify the proof of the ELF, and returns public values.
    pub fn verify(&self, proof: &[u8]) -> Result<PublicValues, Error> {
        let proof: ZiskProofWithPublicValues =
            bincode::serde::decode_from_slice(proof, bincode::config::legacy())
                .map_err(|err| CommonError::deserialize("proof", "bincode", err))?
                .0;

        let vadcop_final_proof = vadcop_final_proof_aligned(&proof)?;
        if !verify_vadcop_final(&vadcop_final_proof, cast_slice(&VADCOP_FINAL_VK)) {
            return Err(Error::InvalidProof);
        }

        // Extract public values and program_vk
        let (public_values, proved_program_vk) = extract_public_values_and_progam_vk(&proof)?;

        // The proved program VK should match the preprocessed
        if proved_program_vk != self.program_vk {
            return Err(Error::UnexpectedProgramVk {
                preprocessed: self.program_vk,
                proved: proved_program_vk,
            });
        }

        Ok(public_values)
    }
}

fn compute_program_vk(elf: &[u8]) -> Result<ProgramVk, Error> {
    let mpi_ctx = Arc::new(MpiCtx::new());
    let mut pctx = ProofCtx::create_ctx(proving_key_dir(), false, VerboseMode::Info, mpi_ctx)
        .map_err(Error::ProofCtx)?;
    let mut params = ParamsGPU::new(false);
    params.with_max_number_streams(1);
    let sctx = SetupCtx::new(&pctx.global_info, &ProofType::Basic, false, &params, &[]);
    let setups_vadcop = SetupsVadcop::new(&pctx.global_info, false, false, &params, &[]);
    pctx.set_device_buffers(&sctx, &setups_vadcop, false, &params)
        .map_err(Error::ProofCtx)?;

    let elf = ElfBinaryFromFile {
        elf: elf.to_vec(),
        name: String::new(),
        with_hints: false,
    };

    let tempdir = tempdir().map_err(CommonError::tempdir)?;

    let (_, program_vk) =
        rom_merkle_setup::<Goldilocks>(&pctx, &elf, &Some(tempdir.path().to_path_buf()))
            .map_err(Error::ComputeProgramVk)?;

    program_vk_from_slice(&program_vk)
}

fn extract_public_values_and_progam_vk(
    proof: &ZiskProofWithPublicValues,
) -> Result<(PublicValues, ProgramVk), Error> {
    let program_vk = program_vk_from_slice(&proof.get_program_vk().vk)?;

    let mut public_values = vec![0; ZISK_PUBLICS * 4];
    proof.get_publics().read_slice(&mut public_values);
    proof.get_publics().head();

    Ok((public_values, program_vk))
}

fn program_vk_from_slice(program_vk: &[u8]) -> Result<ProgramVk, Error> {
    (program_vk.len() == 32)
        .then(|| program_vk.try_into().unwrap())
        .ok_or_else(|| Error::InvalidProgramVkLength(program_vk.len()))
}

fn vadcop_final_proof_aligned(
    proof: &ZiskProofWithPublicValues,
) -> Result<VadcopFinalProof, Error> {
    let mut vadcop_final_proof = proof
        .get_vadcop_final_proof()
        .map_err(Error::InvalidProofFormat)?;
    vadcop_final_proof.proof = align_to_u64(vadcop_final_proof.proof)?;
    vadcop_final_proof.public_values = align_to_u64(vadcop_final_proof.public_values)?;
    Ok(vadcop_final_proof)
}

/// Returns u64-aligned bytes.
///
/// Returns an error if `data.len()` is not a multiple of 8.
fn align_to_u64(data: Vec<u8>) -> Result<Vec<u8>, Error> {
    if !data.len().is_multiple_of(8) {
        return Err(Error::InvalidProofSize(data.len()));
    }
    Ok(if data.as_ptr().cast::<u64>().is_aligned() {
        data
    } else {
        let mut aligned = ManuallyDrop::new(vec![0u64; data.len() / 8]);
        bytemuck::cast_slice_mut(&mut aligned).copy_from_slice(&data);
        let ptr = aligned.as_mut_ptr().cast::<u8>();
        let len = aligned.len() * size_of::<u64>();
        let cap = aligned.capacity() * size_of::<u64>();
        // SAFETY: `ptr` came from a `Vec<u64>` allocation.
        unsafe { Vec::from_raw_parts(ptr, len, cap) }
    })
}

fn panic_msg(err: Box<dyn Any + Send + 'static>) -> String {
    None.or_else(|| err.downcast_ref::<String>().cloned())
        .or_else(|| err.downcast_ref::<&'static str>().map(ToString::to_string))
        .unwrap_or_else(|| "unknown panic msg".to_string())
}

/// Returns path to `~/.zisk` directory.
fn dot_zisk_dir() -> PathBuf {
    PathBuf::from(env::var("HOME").expect("env `$HOME` should be set")).join(".zisk")
}

/// Returns path to `~/.zisk/provingKey` directory.
fn proving_key_dir() -> PathBuf {
    dot_zisk_dir().join("provingKey")
}
