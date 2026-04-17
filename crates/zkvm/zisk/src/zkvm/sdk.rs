use crate::zkvm::{
    Error,
    sdk::{cluster::ClusterClient, local::LocalProver},
};
use ere_verifier_zisk::{PUBLIC_VALUES_SIZE, ZiskProgramVk, ZiskProof};
use ere_zkvm_interface::zkvm::{CommonError, ProverResource, ProverResourceKind, PublicValues};
use proofman_common::{
    MpiCtx, ParamsGPU, ProofCtx, ProofType, SetupCtx, SetupsVadcop, VerboseMode,
};
use proofman_fields::Goldilocks;
use proofman_starks_lib_c::free_device_buffers_c;
use std::{
    any::Any,
    env,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tempfile::tempdir;
use zisk_core::{Riscv2zisk, ZiskRom};
use zisk_rom_setup::rom_merkle_setup;
use zisk_sdk::{ElfBinaryFromFile, ZiskProofWithPublicValues};
use ziskemu::{Emu, EmuOptions};

mod cluster;
mod local;

/// Prover backend - either local or cluster.
#[allow(clippy::large_enum_variant)]
pub enum ZiskProver {
    Local(LocalProver),
    Cluster(ClusterClient),
}

pub struct ZiskSdk {
    resource: ProverResource,
    rom: ZiskRom,
    program_vk: ZiskProgramVk,
    prover: ZiskProver,
}

impl ZiskSdk {
    /// Returns SDK for the ELF.
    pub fn new(elf: Vec<u8>, resource: ProverResource) -> Result<Self, Error> {
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
            ProverResource::Network(_) => Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [
                    ProverResourceKind::Cpu,
                    ProverResourceKind::Gpu,
                    ProverResourceKind::Cluster,
                ],
            ))?,
        };

        Ok(Self {
            resource,
            rom,
            program_vk,
            prover,
        })
    }

    pub fn program_vk(&self) -> ZiskProgramVk {
        self.program_vk
    }

    /// Execute the ELF with the given `stdin`.
    pub fn execute(&self, stdin: &[u8]) -> Result<(PublicValues, u64), Error> {
        let stdin = length_prefixed_and_padded(stdin);
        let mut emu = Emu::new(&self.rom);
        emu.ctx = emu.create_emu_context(stdin, &EmuOptions::default());

        panic::catch_unwind(AssertUnwindSafe(|| emu.run_fast(&EmuOptions::default())))
            .map_err(|err| Error::EmulatorPanic(panic_msg(err)))?;

        if !emu.ctx.inst_ctx.end {
            return Err(Error::EmulatorNotTerminated);
        }

        if emu.ctx.inst_ctx.error {
            return Err(Error::EmulatorError);
        }

        let public_values = emu.get_output_8().into();
        let total_num_cycles = emu.number_of_steps();

        Ok((public_values, total_num_cycles))
    }

    pub fn prove(&self, stdin: &[u8]) -> Result<(PublicValues, ZiskProof, Duration), Error> {
        match &self.resource {
            ProverResource::Cpu if cfg!(feature = "cuda") => Err(Error::CudaFeatureEnabled)?,
            ProverResource::Gpu if cfg!(not(feature = "cuda")) => Err(Error::CudaFeatureDisabled)?,
            _ => {}
        };

        let stdin = length_prefixed_and_padded(stdin);
        let (proof, proving_time) = match &self.prover {
            ZiskProver::Local(local) => local.prove(&stdin)?,
            ZiskProver::Cluster(client) => client.prove(&stdin)?,
        };

        // Extract public values and program_vk
        let (public_values, proved_program_vk) = extract_public_values_and_program_vk(&proof)?;

        // The proved program VK should match the preprocessed
        if proved_program_vk != self.program_vk {
            return Err(ere_verifier_zisk::Error::UnexpectedProgramVk {
                expected: self.program_vk,
                got: proved_program_vk,
            })?;
        }

        let proof = if let zisk_sdk::ZiskProof::VadcopFinal(proof) = proof.proof {
            proof
        } else {
            return Err(Error::UnexpectedProofKind(match &proof.proof {
                zisk_sdk::ZiskProof::Null() => "Null",
                zisk_sdk::ZiskProof::VadcopFinalCompressed(_) => "VadcopFinalCompressed",
                zisk_sdk::ZiskProof::Plonk(_) => "Plonk",
                zisk_sdk::ZiskProof::Fflonk(_) => "Fflonk",
                _ => "Unknown",
            }));
        };

        let zisk_proof = ZiskProof {
            proof,
            public_values,
            program_vk: self.program_vk,
        };

        Ok((public_values.into(), zisk_proof, proving_time))
    }
}

fn compute_program_vk(elf: &[u8]) -> Result<ZiskProgramVk, Error> {
    let mpi_ctx = Arc::new(MpiCtx::new());
    let mut pctx = ProofCtx::create_ctx(proving_key_dir(), false, VerboseMode::Info, mpi_ctx)
        .map_err(Error::ProofCtx)?;
    let mut params = ParamsGPU::new(false);
    params.with_max_number_streams(1);
    let sctx = SetupCtx::new(&pctx.global_info, &ProofType::Basic, false, &params, &[]);
    let setups_vadcop = SetupsVadcop::new(&pctx.global_info, false, false, &params, &[]);

    let result = (|| {
        pctx.set_device_buffers(&sctx, &setups_vadcop, false, &params)
            .map_err(Error::ProofCtx)?;

        let elf = ElfBinaryFromFile {
            elf: elf.to_vec(),
            name: String::new(),
            with_hints: false,
            path: None,
        };

        let tempdir = tempdir().map_err(CommonError::tempdir)?;

        let (_, program_vk) =
            rom_merkle_setup::<Goldilocks>(&pctx, &elf, &Some(tempdir.path().to_path_buf()))
                .map_err(Error::ComputeProgramVk)?;

        Ok(program_vk)
    })();

    free_device_buffers_c(pctx.get_device_buffers_ptr());

    result.and_then(|program_vk| Ok(program_vk.try_into()?))
}

fn extract_public_values_and_program_vk(
    proof: &ZiskProofWithPublicValues,
) -> Result<([u8; PUBLIC_VALUES_SIZE], ZiskProgramVk), Error> {
    let program_vk = ZiskProgramVk::try_from(&proof.get_program_vk().vk)?;

    let mut public_values = [0; PUBLIC_VALUES_SIZE];
    proof.get_publics().read_slice(&mut public_values);
    proof.get_publics().head();

    Ok((public_values, program_vk))
}

/// Returns `data` with a LE u64 length prefix and padding to multiple of 8.
///
/// The length prefix and padding is expected by ZisK emulator/prover runtime.
fn length_prefixed_and_padded(data: &[u8]) -> Vec<u8> {
    let len = (8 + data.len()).next_multiple_of(8);
    let mut buf = Vec::with_capacity(len);
    buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
    buf.extend_from_slice(data);
    buf.resize(len, 0);
    buf
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
