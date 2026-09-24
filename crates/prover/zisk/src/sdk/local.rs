use std::{
    env, fs,
    time::{Duration, Instant},
};

use ere_compiler_core::Elf;
use ere_prover_core::{CommonError, Input, ProverResource};
use ere_verifier_zisk::{VADCOP_FINAL_HASH_FAMILY, ZiskProgramVk, ZiskProof};
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, MutexGuard};
use proofman_fields::{Field, Goldilocks, PrimeField64};
use proofman_util::DeviceBuffer;
use zisk_common::{HashMode, ZiskPaths, io::ZiskStdin};
use zisk_prover_backend::{
    Asm, AsmOptions, BackendProverOpts, GuestProgram, ProgramId, ProverClientBuilder, ZiskProver,
};
use zisk_rom_setup::{get_elf_bin_file_path_with_hash, get_elf_bin_verkey_file_path_with_hash};
use zisk_sm_rom::CustomRom;

use crate::{
    error::Error,
    sdk::{framed_stdin, proving_key::ensure_proving_key},
};

// Use a shared prover instance to avoid `MpiCtx` get initialized twice, to support multiple
// `ZiskProver` instances creation (e.g. testing different ELFs).
static SHARED_PROVER: OnceCell<Mutex<SharedProver>> = OnceCell::new();

/// The upstream prover and its one program, which keeps ASM services and shmem until removed.
struct SharedProver {
    prover: ZiskProver<Asm>,
    program_id: Option<ProgramId>,
}

impl SharedProver {
    /// Sets up `program` in place of the current one.
    fn setup(&mut self, program: &GuestProgram) -> Result<(), Error> {
        if self.program_id.as_ref() == Some(&program.program_id) {
            return Ok(());
        }
        self.remove_program()?;
        self.prover.setup(program).run().map_err(Error::Setup)?;
        self.program_id = Some(program.program_id.clone());
        Ok(())
    }

    /// Stops the ASM services of the current program and releases its shmem.
    fn remove_program(&mut self) -> Result<(), Error> {
        if let Some(program_id) = &self.program_id {
            self.prover
                .remove_program(program_id)
                .map_err(Error::Setup)?;
        }
        self.program_id = None;
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Config {
    setup_on_init: bool,
    unlock_mapped_memory: bool,
    minimal_memory: bool,
    cpu_mops: bool,
    packed: bool,
    max_streams: Option<usize>,
    max_recursive_streams: Option<usize>,
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
        Ok(Self {
            setup_on_init: env::var_os("ERE_ZISK_SETUP_ON_INIT").is_some(),
            unlock_mapped_memory: env::var_os("ERE_ZISK_UNLOCK_MAPPED_MEMORY").is_some(),
            minimal_memory: env::var_os("ERE_ZISK_MINIMAL_MEMORY").is_some(),
            cpu_mops: env::var_os("ERE_ZISK_CPU_MOPS").is_some(),
            packed: env::var_os("ERE_ZISK_PACKED").is_some(),
            max_streams: parse_usize("ERE_ZISK_MAX_STREAMS")?,
            max_recursive_streams: parse_usize("ERE_ZISK_MAX_RECURSIVE_STREAMS")?,
            number_threads_witness: parse_usize("ERE_ZISK_NUMBER_THREADS_WITNESS")?,
            max_witness_stored: parse_usize("ERE_ZISK_MAX_WITNESS_STORED")?,
        })
    }
}

pub struct LocalProver {
    resource: ProverResource,
    config: Config,
    program: GuestProgram,
    program_vk: ZiskProgramVk,
}

impl LocalProver {
    pub fn new(elf: Elf, resource: &ProverResource) -> Result<Self, Error> {
        let config = Config::from_env()?;

        let program = GuestProgram::from_bytes("guest", elf.0);
        let program_vk = compute_program_vk(resource, &program)?;

        let local = Self {
            resource: resource.clone(),
            config,
            program,
            program_vk,
        };

        if config.setup_on_init {
            local.shared_prover()?.setup(&local.program)?;
        }

        Ok(local)
    }

    /// Replaces the program and removes the old one from the shared prover.
    pub fn setup(&mut self, elf: Elf) -> Result<(), Error> {
        let program = GuestProgram::from_bytes("guest", elf.0);
        if program.program_id == self.program.program_id {
            return Ok(());
        }

        let program_vk = compute_program_vk(&self.resource, &program)?;

        if self.config.setup_on_init {
            self.shared_prover()?.setup(&program)?;
        } else if let Some(shared_prover) = SHARED_PROVER.get() {
            shared_prover.lock().remove_program()?;
        }

        self.program = program;
        self.program_vk = program_vk;
        Ok(())
    }

    pub fn program_vk(&self) -> ZiskProgramVk {
        self.program_vk
    }

    pub fn prove(&self, input: &Input) -> Result<(ZiskProof, Duration), Error> {
        let mut shared_prover = self.shared_prover()?;
        shared_prover.setup(&self.program)?;

        let stdin = ZiskStdin::from_vec(framed_stdin(input.stdin()));

        let started = Instant::now();
        let output = shared_prover
            .prover
            .prove(&self.program, stdin)
            .run()
            .map_err(Error::Prove)?;
        let proving_time = started.elapsed();

        let proof = output
            .get_proof()
            .get_vadcop_final_proof()
            .map_err(|err| Error::Prove(err.into()))?;

        Ok((ZiskProof(proof), proving_time))
    }

    fn shared_prover(&self) -> Result<MutexGuard<'static, SharedProver>, Error> {
        SHARED_PROVER
            .get_or_try_init(|| {
                let prover = build_prover(&self.config, &self.resource)?;
                Ok(Mutex::new(SharedProver {
                    prover,
                    program_id: None,
                }))
            })
            .map(Mutex::lock)
    }
}

fn build_prover(config: &Config, resource: &ProverResource) -> Result<ZiskProver<Asm>, Error> {
    ensure_proving_key().map_err(Error::ProvingKey)?;

    let mut opts = BackendProverOpts::default();
    if cfg!(feature = "cuda") && resource.is_gpu() {
        opts = opts.gpu();
    }
    if config.cpu_mops {
        opts = opts.cpu_mops();
    }
    if config.minimal_memory {
        opts = opts.minimal_memory();
    }
    if config.packed {
        opts = opts.packed();
    }
    if let Some(max_streams) = config.max_streams {
        opts = opts.max_streams(max_streams);
    }
    if let Some(max_recursive_streams) = config.max_recursive_streams {
        opts = opts.max_recursive_streams(max_recursive_streams);
    }
    if let Some(number_threads_witness) = config.number_threads_witness {
        opts = opts.number_threads_witness(number_threads_witness);
    }
    if let Some(max_witness_stored) = config.max_witness_stored {
        opts = opts.max_witness_stored(max_witness_stored);
    }

    let mut asm_options = AsmOptions::default();
    if config.unlock_mapped_memory {
        asm_options = asm_options.unlock_mapped_memory();
    }
    opts = opts.with_asm_options(asm_options);

    ProverClientBuilder::new()
        .asm()
        .with_prover_options(opts)
        .build()
        .map_err(Error::BuildProver)
}

/// Vendored from [`zisk_rom_setup::rom_merkle_setup`] to do program setup without creating
/// `ProofCtx` or generating assembly, which can only be created once due to mpi initialization.
/// Shares the cache files of `rom_merkle_setup` with the prover setup.
fn compute_program_vk(
    resource: &ProverResource,
    program: &GuestProgram,
) -> Result<ZiskProgramVk, Error> {
    type F = Goldilocks;

    struct Guard(bool);

    impl Drop for Guard {
        fn drop(&mut self) {
            proofman_starks_lib_c::set_gpu_mode_c(self.0);
        }
    }

    let hash_mode: HashMode = VADCOP_FINAL_HASH_FAMILY.parse().expect("infallable");
    let mut custom_rom_trace = CustomRom::build::<F>(program.elf())?;

    let buffer = custom_rom_trace.get_buffer::<F>();
    let arity = hash_mode.merkle_tree_arity();
    let n = custom_rom_trace.num_rows() as u64;
    let n_extended = hash_mode.blowup_factor() * n;
    let n_bits = n.trailing_zeros() as u64;
    let n_bits_ext = n_extended.trailing_zeros() as u64;
    let n_cols = custom_rom_trace.num_cols() as u64;
    let mut root = [F::ZERO; 4];

    let gpu = cfg!(feature = "cuda") && resource.is_gpu();
    let cache_dir = &ZiskPaths::global().cache;
    fs::create_dir_all(cache_dir)
        .map_err(|err| CommonError::create_dir("cache", cache_dir, err))?;
    let elf_bin_path = get_elf_bin_file_path_with_hash(program.hash(), cache_dir, gpu, hash_mode)
        .expect("infallable");

    proofman_starks_lib_c::set_hash_family_c(VADCOP_FINAL_HASH_FAMILY);

    let _guard = Guard(gpu);
    proofman_starks_lib_c::set_gpu_mode_c(false);

    proofman_starks_lib_c::write_custom_commit_c(
        root.as_mut_ptr() as *mut u8,
        arity,
        n_bits,
        n_bits_ext,
        n_cols,
        DeviceBuffer::default().get_ptr(),
        buffer.as_ptr() as *mut u8,
        &elf_bin_path.to_string_lossy(),
    );

    let vk = root.map(|field| field.as_canonical_u64());
    let elf_verkey_bin_path =
        get_elf_bin_verkey_file_path_with_hash(program.hash(), cache_dir, hash_mode)
            .expect("infallable");
    fs::write(&elf_verkey_bin_path, vk.map(u64::to_le_bytes).concat())
        .map_err(|err| CommonError::write_file("cache", &elf_verkey_bin_path, err))?;

    Ok(ZiskProgramVk(vk))
}
