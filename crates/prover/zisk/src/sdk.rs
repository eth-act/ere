use core::{any::Any, panic::AssertUnwindSafe, time::Duration};
use std::{env, panic, path::PathBuf, sync::Arc};

use ere_cluster_client_zisk::ZiskClusterClient;
use ere_prover_core::{CommonError, Input, ProverResource, ProverResourceKind, PublicValues};
use ere_util_tokio::block_on;
use ere_verifier_zisk::{ZiskProgramVk, ZiskProof};
use proofman_common::{
    MpiCtx, ParamsGPU, ProofCtx, ProofType, SetupCtx, SetupsVadcop, VerboseMode,
};
use proofman_fields::Goldilocks;
use proofman_starks_lib_c::free_device_buffers_c;
use tempfile::tempdir;
use zisk_core::{Riscv2zisk, ZiskRom};
use zisk_rom_setup::rom_merkle_setup;
use zisk_sdk::ElfBinaryFromFile;
use ziskemu::{Emu, EmuOptions};

use crate::{error::Error, sdk::local::LocalProver};

mod local;

/// Prover backend - either local or cluster.
#[allow(clippy::large_enum_variant)]
pub enum ZiskProver {
    Local(LocalProver),
    Cluster(ZiskClusterClient),
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

        // Compute ProgramVk
        let program_vk = compute_program_vk(&elf)?;

        // Initialize prover
        let prover = match &resource {
            ProverResource::Cpu | ProverResource::Gpu => ZiskProver::Local(LocalProver::new(elf)?),
            ProverResource::Cluster(config) => ZiskProver::Cluster(ZiskClusterClient::new(config)?),
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
        let stdin = framed_stdin(stdin);
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

    pub fn prove(&self, input: &Input) -> Result<(PublicValues, ZiskProof, Duration), Error> {
        match &self.resource {
            ProverResource::Cpu if cfg!(feature = "cuda") => Err(Error::CudaFeatureEnabled)?,
            ProverResource::Gpu if cfg!(not(feature = "cuda")) => Err(Error::CudaFeatureDisabled)?,
            _ => {}
        };

        let (proof, proving_time) = match &self.prover {
            ZiskProver::Local(local) => local.prove(&framed_stdin(input.stdin()))?,
            ZiskProver::Cluster(client) => block_on(client.prove(input))?,
        };

        // The proved ProgramVk should match the preprocessed
        if self.program_vk != proof.program_vk {
            return Err(ere_verifier_zisk::Error::UnexpectedProgramVk {
                expected: self.program_vk,
                got: proof.program_vk,
            })?;
        }

        Ok((proof.public_values.into(), proof, proving_time))
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

/// Returns `data` with a LE u64 length prefix and padding to multiple of 8.
///
/// The length prefix and padding is expected by ZisK emulator/prover runtime.
fn framed_stdin(data: &[u8]) -> Vec<u8> {
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

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use ere_verifier_zisk::ZiskProgramVk;
    use tempfile::tempdir;

    use crate::{
        prover::tests::basic_elf,
        sdk::{compute_program_vk, dot_zisk_dir},
    };

    #[test]
    fn compute_program_vk_matches_cargo_zisk_rom_setup() {
        let elf = basic_elf();

        let program_vk = {
            let tempdir = tempdir().unwrap();
            let elf_path = tempdir.path().join("guest.elf");
            fs::write(&elf_path, &elf.0).unwrap();

            let cargo_zisk = dot_zisk_dir().join("bin").join("cargo-zisk");
            let status = Command::new(&cargo_zisk)
                .arg("rom-setup")
                .arg("-e")
                .arg(&elf_path)
                .arg("-o")
                .arg(tempdir.path())
                .status()
                .unwrap();
            assert!(status.success());

            let verkey_paths = fs::read_dir(tempdir.path())
                .unwrap()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".verkey.bin"))
                })
                .collect::<Vec<_>>();
            assert_eq!(verkey_paths.len(), 1);

            ZiskProgramVk::try_from(fs::read(&verkey_paths[0]).unwrap().as_slice()).unwrap()
        };

        assert_eq!(compute_program_vk(&elf).unwrap(), program_vk);
    }
}
