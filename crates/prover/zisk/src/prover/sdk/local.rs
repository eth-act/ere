use crate::prover::{
    Error,
    sdk::{dot_zisk_dir, panic_msg},
};
use blake3::Hash;
use ere_prover_core::prover::CommonError;
use parking_lot::{Mutex, MutexGuard};
use proofman_common::ParamsGPU;
use std::{env, fs, panic, path::PathBuf, process::Command, thread::sleep, time::Duration};
use tempfile::tempdir;
use tracing::info;
use zisk_rom_setup::generate_assembly;
use zisk_sdk::{
    Asm, ElfBinaryFromFile, ProofOpts, ProverClientBuilder, ZiskProgramPK,
    ZiskProofWithPublicValues, ZiskProver, ZiskStdin,
};

const ELF_NAME: &str = "elf";

struct Config {
    preallocate: bool,
    unlock_mapped_memory: bool,
    minimal_memory: bool,
    shared_tables: bool,
    max_streams: Option<usize>,
    number_threads_witness: Option<usize>,
    max_witness_stored: Option<usize>,
}

impl Config {
    fn from_env() -> Result<Self, Error> {
        let parse_usize = |key| {
            env::var(key)
                .ok()
                .map(|value| {
                    value
                        .parse()
                        .map_err(|_| Error::InvalidEnvVar { key, value })
                })
                .transpose()
        };
        let preallocate = env::var_os("ERE_ZISK_PREALLOCATE").is_some();
        let unlock_mapped_memory = env::var_os("ERE_ZISK_UNLOCK_MAPPED_MEMORY").is_some();
        let minimal_memory = env::var_os("ERE_ZISK_MINIMAL_MEMORY").is_some();
        let shared_tables = env::var_os("ERE_ZISK_SHARED_TABLES").is_some();
        let max_streams = parse_usize("ERE_ZISK_MAX_STREAMS")?;
        let number_threads_witness = parse_usize("ERE_ZISK_NUMBER_THREADS_WITNESS")?;
        let max_witness_stored = parse_usize("ERE_ZISK_MAX_WITNESS_STORED")?;
        Ok(Self {
            preallocate,
            unlock_mapped_memory,
            minimal_memory,
            shared_tables,
            max_streams,
            number_threads_witness,
            max_witness_stored,
        })
    }
}

pub struct LocalProver {
    config: Config,
    elf: Vec<u8>,
    elf_hash: Hash,
    prover_and_pk: Mutex<Option<(ZiskProver<Asm>, ZiskProgramPK)>>,
}

impl LocalProver {
    pub fn new(elf: Vec<u8>) -> Result<Self, Error> {
        let config = Config::from_env()?;
        let elf_hash = blake3::hash(&elf);

        let prover_and_pk = env::var_os("ERE_ZISK_SETUP_ON_INIT")
            .map(|_| initialize(&config, &elf, elf_hash))
            .transpose()?;

        Ok(Self {
            config,
            elf,
            elf_hash,
            prover_and_pk: Mutex::new(prover_and_pk),
        })
    }

    pub fn prove(&self, stdin: &[u8]) -> Result<(ZiskProofWithPublicValues, Duration), Error> {
        let mut guard = self.prover_and_pk.lock();

        if guard.is_none() {
            *guard = Some(initialize(&self.config, &self.elf, self.elf_hash)?)
        }

        let (prover, pk) = guard.as_ref().unwrap();

        let stdin = ZiskStdin::from_vec(stdin.to_vec());
        let tempdir = tempdir().map_err(CommonError::tempdir)?;
        let mut opts = ProofOpts::default().output_dir(tempdir.path().to_path_buf());
        if self.config.minimal_memory {
            opts = opts.minimal_memory();
        }

        let result = panic::catch_unwind(|| prover.prove(pk, stdin).with_proof_options(opts).run());

        match result {
            Err(err) => {
                uninitialize(guard);
                Err(Error::ProvePanic(panic_msg(err)))
            }
            Ok(Err(err)) => {
                uninitialize(guard);
                Err(Error::Prove(err))
            }
            Ok(Ok(result)) => Ok((
                result.get_proof_with_publics().clone(),
                result.get_duration(),
            )),
        }
    }
}

fn assembly_files_exist(elf_hash: Hash) -> bool {
    ["mt", "rh", "mo"].into_iter().all(|suffix| {
        let bin = cache_dir().join(format!("{ELF_NAME}-{elf_hash}-{suffix}.bin"));
        bin.exists()
    })
}

fn initialize(
    config: &Config,
    elf: &[u8],
    elf_hash: Hash,
) -> Result<(ZiskProver<Asm>, ZiskProgramPK), Error> {
    info!("Initializing ZisK prover...");

    fs::create_dir_all(cache_dir())
        .map_err(|err| CommonError::create_dir("cache", cache_dir(), err))?;

    if !assembly_files_exist(elf_hash) {
        generate_assembly(elf, ELF_NAME, &cache_dir(), false, false)
            .map_err(|error| Error::GenerateAssembly(error.to_string()))?;
    };

    let mut params_gpu = ParamsGPU::new(config.preallocate);
    if let Some(max_streams) = config.max_streams {
        params_gpu.with_max_number_streams(max_streams);
    }
    if let Some(number_threads_witness) = config.number_threads_witness {
        params_gpu.with_number_threads_pools_witness(number_threads_witness);
    }
    if let Some(max_witness_stored) = config.max_witness_stored {
        params_gpu.with_max_witness_stored(max_witness_stored);
    }

    let prover = ProverClientBuilder::new()
        .asm()
        .gpu(Some(params_gpu))
        .unlock_mapped_memory(config.unlock_mapped_memory)
        .shared_tables(config.shared_tables)
        .build()
        .map_err(Error::InitProver)?;

    let elf_binary = ElfBinaryFromFile {
        elf: elf.to_vec(),
        name: ELF_NAME.to_string(),
        with_hints: false,
        path: None,
    };
    let (pk, _) = prover.setup(&elf_binary).map_err(Error::SetupProver)?;

    info!("ZisK prover initialized");

    Ok((prover, pk))
}

fn uninitialize(mut prover_and_pk: MutexGuard<Option<(ZiskProver<Asm>, ZiskProgramPK)>>) {
    info!("Uninitializing ZisK prover...");

    let _ = Command::new("fuser")
        .args(["-k", "-9", "23115/tcp", "23116/tcp", "23117/tcp"])
        .output();
    sleep(Duration::from_secs(1));

    drop(prover_and_pk.take());

    info!("ZisK prover uninitialized");
}

/// Returns path to `~/.zisk/cache` directory.
fn cache_dir() -> PathBuf {
    dot_zisk_dir().join("cache")
}
