use std::{
    any::Any,
    env,
    panic::{self, AssertUnwindSafe},
    time::Duration,
};

use ere_cluster_client_zisk::ZiskClusterClient;
use ere_compiler_core::Elf;
use ere_prover_core::{CommonError, Input, ProverResource, ProverResourceKind, PublicValues};
use ere_util_tokio::block_on;
use ere_verifier_zisk::{ZiskProgramVk, ZiskProof, ensure_program_vk_matches};
use tokio::time::Instant;
use zisk_core::{Riscv2zisk, ZiskRom};
use ziskemu::{Emu, EmuOptions};

use crate::{error::Error, sdk::local::LocalProver};

mod local;

#[allow(clippy::large_enum_variant)]
enum Backend {
    Local(LocalProver),
    Cluster {
        client: ZiskClusterClient,
        prove_timeout: Option<Duration>,
    },
}

pub struct ZiskSdk {
    resource: ProverResource,
    backend: Backend,
    rom: ZiskRom,
}

impl ZiskSdk {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        // Convert ELF to ZisK ROM
        let rom = Riscv2zisk::new(&elf)
            .run()
            .map_err(|err| Error::Riscv2zisk(err.to_string()))?;

        // Initialize prover
        let backend = match &resource {
            ProverResource::Cpu | ProverResource::Gpu => {
                Backend::Local(LocalProver::new(elf, &resource)?)
            }
            ProverResource::Cluster(config) => {
                let client = block_on(ZiskClusterClient::new(config, elf))?;
                let prove_timeout = env::var("ERE_ZISK_CLUSTER_PROVE_TIMEOUT_SECS")
                    .ok()
                    .and_then(|val| val.parse::<u64>().ok().map(Duration::from_secs));
                Backend::Cluster {
                    client,
                    prove_timeout,
                }
            }
            ProverResource::Network(_) => {
                return Err(CommonError::unsupported_prover_resource_kind(
                    resource.kind(),
                    [
                        ProverResourceKind::Cpu,
                        ProverResourceKind::Gpu,
                        ProverResourceKind::Cluster,
                    ],
                )
                .into());
            }
        };

        Ok(Self {
            resource,
            backend,
            rom,
        })
    }

    pub fn program_vk(&self) -> ZiskProgramVk {
        match &self.backend {
            Backend::Local(local) => local.program_vk(),
            Backend::Cluster { client, .. } => client.program_vk(),
        }
    }

    /// Execute the ELF with the given `stdin`.
    pub fn execute(&self, input: &Input) -> Result<(PublicValues, u64), Error> {
        let stdin = framed_stdin(input.stdin());
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
        if cfg!(not(feature = "cuda")) && self.resource == ProverResource::Gpu {
            return Err(Error::CudaFeatureDisabled);
        }

        let (proof, proving_time) = match &self.backend {
            Backend::Local(local) => local.prove(input)?,
            Backend::Cluster {
                client,
                prove_timeout,
            } => block_on(async {
                let deadline = prove_timeout.map(|timeout| Instant::now() + timeout);
                client.prove(input, deadline).await.map_err(Error::Cluster)
            })?,
        };

        let (program_vk, public_values) = proof.program_vk_and_public_values()?;

        ensure_program_vk_matches(self.program_vk(), program_vk)?;

        Ok((public_values, proof, proving_time))
    }
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

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use ere_prover_core::zkVMProver;
    use ere_verifier_zisk::ZiskProgramVk;
    use tempfile::tempdir;

    use crate::prover::tests::{basic_elf, basic_elf_zkvm};

    #[test]
    fn program_vk_matches_cargo_zisk_program_setup() {
        let program_vk = {
            let tempdir = tempdir().unwrap();
            let elf_path = tempdir.path().join("guest.elf");
            fs::write(&elf_path, &basic_elf().0).unwrap();

            let status = Command::new("cargo-zisk")
                .arg("program-setup")
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

        assert_eq!(*basic_elf_zkvm().program_vk(), program_vk);
    }
}
