use std::{
    env,
    time::{Duration, Instant},
};

use ere_compiler_core::Elf;
use ere_prover_core::{Input, ProverResource};
use ere_verifier_zisk::{ZiskProgramVk, ZiskProof};
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use zisk_common::{ProofKind, io::ZiskStdin};
use zisk_prover_backend::{
    Asm, AsmOptions, BackendProverOpts, GuestProgram, ProverClientBuilder, ZiskProver,
};

use crate::{error::Error, sdk::framed_stdin};

// Use a shared prover instance to avoid `MpiCtx` get initialized twice, to support multiple
// `ZiskProver` instances creation (e.g. testing different ELFs).
static LOCAL_PROVER: OnceCell<ZiskProver<Asm>> = OnceCell::new();

struct Config {
    setup_on_init: bool,
    unlock_mapped_memory: bool,
    minimal_memory: bool,
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
        Ok(Self {
            setup_on_init: env::var_os("ERE_ZISK_SETUP_ON_INIT").is_some(),
            unlock_mapped_memory: env::var_os("ERE_ZISK_UNLOCK_MAPPED_MEMORY").is_some(),
            minimal_memory: env::var_os("ERE_ZISK_MINIMAL_MEMORY").is_some(),
            max_streams: parse_usize("ERE_ZISK_MAX_STREAMS")?,
            number_threads_witness: parse_usize("ERE_ZISK_NUMBER_THREADS_WITNESS")?,
            max_witness_stored: parse_usize("ERE_ZISK_MAX_WITNESS_STORED")?,
        })
    }
}

pub struct LocalProver {
    prover: &'static ZiskProver<Asm>,
    program: GuestProgram,
    program_vk: ZiskProgramVk,
    initialized: Mutex<bool>,
}

impl LocalProver {
    pub fn new(elf: Elf, resource: &ProverResource) -> Result<Self, Error> {
        let config = Config::from_env()?;
        let prover = LOCAL_PROVER.get_or_try_init(|| build_prover(&config, resource))?;

        let program = GuestProgram::from_bytes("guest", elf.0);
        let program_vk = prover
            .prover
            .program_vk(&program, false)
            .map_err(Error::ComputeProgramVk)?;
        let program_vk = ZiskProgramVk::try_from(program_vk.vk.as_slice())?;

        if config.setup_on_init {
            prover.setup(&program).run().map_err(Error::Setup)?;
        }

        Ok(Self {
            prover,
            program,
            program_vk,
            initialized: Mutex::new(config.setup_on_init),
        })
    }

    pub fn program_vk(&self) -> ZiskProgramVk {
        self.program_vk
    }

    pub fn prove(&self, input: &Input) -> Result<(ZiskProof, Duration), Error> {
        let mut initialized = self.initialized.lock();

        if !*initialized {
            self.prover
                .setup(&self.program)
                .run()
                .map_err(Error::Setup)?;
            *initialized = true;
        }

        let stdin = ZiskStdin::from_vec(framed_stdin(input.stdin()));

        let started = Instant::now();
        let output = self
            .prover
            .prove(&self.program, stdin)
            .wrap_proof(ProofKind::VadcopFinalMinimal)
            .run()
            .map_err(Error::Prove)?;
        let proving_time = started.elapsed();

        let proof = output
            .get_proof()
            .get_vadcop_final_proof()
            .map_err(Error::Prove)?;

        Ok((ZiskProof(proof), proving_time))
    }
}

fn build_prover(config: &Config, resource: &ProverResource) -> Result<ZiskProver<Asm>, Error> {
    let mut opts = BackendProverOpts::default();
    if cfg!(feature = "cuda") && matches!(resource, ProverResource::Gpu) {
        opts = opts.gpu();
    }
    if config.minimal_memory {
        opts = opts.minimal_memory();
    }
    if let Some(max_streams) = config.max_streams {
        opts = opts.max_streams(max_streams);
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
