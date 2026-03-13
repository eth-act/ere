use crate::zkvm::{
    Error,
    sdk::{dot_zisk_dir, panic_msg},
};
use blake3::Hash;
use ere_zkvm_interface::zkvm::CommonError;
use parking_lot::{Mutex, MutexGuard};
use std::{ops::Deref, panic, path::PathBuf, time::Duration};
use tempfile::tempdir;
use tracing::info;
use zisk_rom_setup::generate_assembly;
use zisk_sdk::{
    Asm, ElfBinaryFromFile, ProofOpts, ProverClientBuilder, ZiskProgramPK,
    ZiskProofWithPublicValues, ZiskProver, ZiskStdin,
};

const ELF_NAME: &str = "elf";

pub struct LocalProver {
    elf: Vec<u8>,
    elf_hash: Hash,
    prover_and_pk: Mutex<Option<(ZiskProver<Asm>, ZiskProgramPK)>>,
}

impl LocalProver {
    pub fn new(elf: Vec<u8>) -> Result<Self, Error> {
        let elf_hash = blake3::hash(&elf);
        Ok(Self {
            elf,
            elf_hash,
            prover_and_pk: Mutex::new(None),
        })
    }

    pub fn prove(&self, stdin: &[u8]) -> Result<(ZiskProofWithPublicValues, Duration), Error> {
        let guard = self.prover_and_pk()?;
        let (prover, pk) = &*guard;
        let stdin = ZiskStdin::from_vec(stdin.to_vec());
        let tempdir = tempdir().map_err(CommonError::tempdir)?;
        let opts = ProofOpts::default().output_dir(tempdir.path().to_path_buf());

        let result = panic::catch_unwind(|| prover.prove(pk, stdin).with_proof_options(opts).run())
            .map_err(|err| {
                drop(guard);
                drop(self.prover_and_pk.lock().take());
                Error::ProvePanic(panic_msg(err))
            })?
            .map_err(Error::Prove)?;

        Ok((
            result.get_proof_with_publics().clone(),
            result.get_duration(),
        ))
    }

    fn prover_and_pk(
        &self,
    ) -> Result<impl Deref<Target = (ZiskProver<Asm>, ZiskProgramPK)>, Error> {
        let mut guard = self.prover_and_pk.lock();

        if guard.is_none() {
            if !assembly_files_exist(self.elf_hash) {
                generate_assembly(&self.elf, ELF_NAME, &cache_dir(), false, false)
                    .map_err(|error| Error::GenerateAssembly(error.to_string()))?;
            };

            info!("Initializing ZisK prover...");

            let prover = ProverClientBuilder::new()
                .asm()
                .build()
                .map_err(Error::InitProver)?;

            let elf_binary = ElfBinaryFromFile {
                elf: self.elf.clone(),
                name: ELF_NAME.to_string(),
                with_hints: false,
                path: None,
            };
            let (pk, _) = prover.setup(&elf_binary).map_err(Error::SetupProver)?;

            info!("ZisK prover initialized");

            *guard = Some((prover, pk));
        }

        Ok(MutexGuard::map(guard, |guard| guard.as_mut().unwrap()))
    }
}

fn assembly_files_exist(elf_hash: Hash) -> bool {
    ["mt", "rh", "mo"].into_iter().all(|suffix| {
        let bin = cache_dir().join(format!("{ELF_NAME}-{elf_hash}-{suffix}.bin"));
        bin.exists()
    })
}

/// Returns path to `~/.zisk/cache` directory.
fn cache_dir() -> PathBuf {
    dot_zisk_dir().join("cache")
}
