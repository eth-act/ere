#[cfg(feature = "cuda")]
use std::num::NonZeroUsize;
use std::{
    collections::BTreeMap,
    env,
    fs::{self, DirBuilder},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, CostEstimation, CostProfile, Input, ProverResource, ProverResourceKind,
    PublicValues, codec::Decode, zkVMProver,
};
use ere_verifier_pico::{PicoProgramVk, PicoProof, PicoVerifier};
use once_cell::sync::OnceCell;
use pico_prover::{
    CostComponent, Phase, Prover, ProverBuilder, PureBuildOptions, PureLibrary, Stdin,
    build_pure_library,
};

use crate::error::Error;

pub struct PicoProver {
    elf: Elf,
    /// Executes without keygen, so execution does not wait for a running prove. The first
    /// execution builds it with the guest's pure-mode AOT library.
    emulator: OnceCell<ProverBuilder>,
    /// Locked, because proving keeps recursion keys across calls and takes `&mut self`.
    prover: Mutex<Prover>,
    verifier: PicoVerifier,
}

impl PicoProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        if !matches!(resource, ProverResource::Cpu | ProverResource::Gpu) {
            Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            ))?;
        }
        #[cfg(not(feature = "cuda"))]
        if matches!(resource, ProverResource::Gpu) {
            Err(Error::CudaFeatureDisabled)?;
        }
        // `pico-vm/cuda` makes every proving machine a GPU one, so such a build has no CPU prover.
        #[cfg(feature = "cuda")]
        if matches!(resource, ProverResource::Cpu) {
            Err(Error::CpuUnsupportedWithCuda)?;
        }

        let (prover, verifier) = load(&elf)?;
        Ok(Self {
            emulator: OnceCell::new(),
            elf,
            prover: Mutex::new(prover),
            verifier,
        })
    }

    fn emulator(&self) -> Result<&ProverBuilder, Error> {
        self.emulator
            .get_or_try_init(|| Ok(builder(&self.elf).pure_aot_library(pure_library(&self.elf)?)))
    }

    fn prover(&self) -> Result<MutexGuard<'_, Prover>, Error> {
        self.prover.lock().map_err(|_| Error::ProverPoisoned)
    }
}

impl zkVMProver for PicoProver {
    type Verifier = PicoVerifier;
    type Error = Error;

    fn verifier(&self) -> &PicoVerifier {
        &self.verifier
    }

    fn setup(&mut self, elf: Elf) -> Result<(), Error> {
        // A prover poisoned by a panic is rebuilt even for the same program.
        if self.elf == elf && !self.prover.is_poisoned() {
            return Ok(());
        }

        let (prover, verifier) = load(&elf)?;
        self.emulator = OnceCell::new();
        self.elf = elf;
        self.prover = Mutex::new(prover);
        self.verifier = verifier;
        Ok(())
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, Duration), Error> {
        let stdin = stdin(input)?;

        // The first execution builds the pure-mode library outside the timing.
        let emulator = self.emulator()?;
        let start = Instant::now();
        let execution = emulator.execute(stdin).map_err(Error::Execute)?;
        let execution_duration = start.elapsed();

        Ok((execution.committed_bytes.into(), execution_duration))
    }

    fn execute_estimated_cost(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, CostEstimation), Error> {
        let stdin = stdin(input)?;

        // `estimate_cost` does not return the committed bytes, so this emulates twice.
        let emulator = self.emulator()?;
        let execution = emulator.execute(stdin.clone()).map_err(Error::Execute)?;
        let estimate = emulator.estimate_cost(stdin).map_err(Error::EstimateCost)?;

        let cost = [
            ("rv64", CostComponent::Rv64),
            ("precompile", CostComponent::Precompile),
            ("system", CostComponent::System),
        ]
        .into_iter()
        .map(|(name, component)| (name.to_owned(), estimate.cells(component)))
        .collect::<BTreeMap<_, _>>();

        Ok((execution.committed_bytes.into(), CostEstimation { cost }))
    }

    fn profile(&self, _input: &Input) -> Result<(PublicValues, CostProfile), Error> {
        Err(Error::ProfileUnsupported)
    }

    fn prove(&self, input: &Input) -> Result<(PublicValues, PicoProof, Duration), Error> {
        let stdin = stdin(input)?;

        let mut prover = self.prover()?;
        let start = Instant::now();
        let proof = prover
            .prove_to(stdin, Phase::Compress)
            .map_err(Error::Prove)?;
        let proving_time = start.elapsed();

        let (envelope, committed) = proof.export_compress_v4().map_err(Error::Prove)?;
        let public_values = committed.as_slice().into();
        let proof = PicoProof::new(envelope, committed);

        Ok((public_values, proof, proving_time))
    }
}

/// Whitelist mode, under the whitelist compiled into `pico-prover`.
fn builder(elf: &Elf) -> ProverBuilder {
    let builder = Prover::builder(&elf.0).shipped_whitelist();
    #[cfg(feature = "cuda")]
    let builder = match env::var("ERE_PICO_GPU_EVENT_POOL_SIZE")
        .ok()
        .and_then(|value| value.parse::<NonZeroUsize>().ok())
    {
        Some(size) => builder.gpu_event_pool_size(size),
        None => builder,
    };
    builder
}

/// Builds the pure-mode AOT library of the guest into the cache once per ELF and loads it.
fn pure_library(elf: &Elf) -> Result<Arc<PureLibrary>, Error> {
    let cache_dir = pure_cache_dir()?;
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&cache_dir)
        .map_err(|err| Error::PureAotBuild(format!("{}: {err}", cache_dir.display())))?;
    ensure_private(&cache_dir)?;
    let options = PureBuildOptions {
        runtime: env::var("ERE_PICO_PURE_RUNTIME").ok(),
        offline: env::var("ERE_PICO_PURE_OFFLINE").is_ok_and(|value| value == "1"),
        cargo: None,
    };
    let library_path = build_pure_library(&elf.0, &cache_dir, &options)
        .map_err(|err| Error::PureAotBuild(err.to_string()))?;
    // SAFETY: `build_pure_library` built the library from this ELF, in a cache that only the
    // current user can access.
    let library = unsafe { PureLibrary::load(&library_path) }
        .map_err(|err| Error::PureAotBuild(err.to_string()))?;
    Ok(Arc::new(library))
}

/// `ERE_PICO_PURE_CACHE`, else `pico-pure` under `$XDG_CACHE_HOME` or `$HOME/.cache`.
fn pure_cache_dir() -> Result<PathBuf, Error> {
    let absolute = |key| {
        env::var_os(key)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    absolute("ERE_PICO_PURE_CACHE")
        .or_else(|| {
            absolute("XDG_CACHE_HOME")
                .or_else(|| absolute("HOME").map(|home| home.join(".cache")))
                .map(|dir| dir.join("pico-pure"))
        })
        .ok_or_else(|| Error::PureAotBuild("neither XDG_CACHE_HOME nor HOME is set".into()))
}

/// Rejects `dir` unless the current user owns it and no other user can access it, so the modes of
/// the libraries inside do not matter. The parent directories must not be writable by other users
/// either.
fn ensure_private(dir: &Path) -> Result<(), Error> {
    let metadata = fs::metadata(dir)
        .map_err(|err| Error::PureAotBuild(format!("{}: {err}", dir.display())))?;
    // SAFETY: `geteuid` has no preconditions.
    let euid = unsafe { libc::geteuid() };
    if metadata.uid() != euid || metadata.mode() & 0o077 != 0 {
        Err(Error::PureAotBuild(format!(
            "{} must be owned by the current user and accessible only by it (mode 700)",
            dir.display()
        )))?
    }
    Ok(())
}

fn load(elf: &Elf) -> Result<(Prover, PicoVerifier), Error> {
    let prover = builder(elf).build().map_err(Error::ProverInit)?;
    let bundle = prover.vk_bundle().map_err(Error::ProverInit)?;
    let verifier = PicoVerifier::new(PicoProgramVk::decode_from_slice(&bundle)?)?;
    Ok((prover, verifier))
}

fn stdin(input: &Input) -> Result<Stdin, Error> {
    if input.proofs.is_some() {
        Err(CommonError::unsupported_input("no dedicated proofs stream"))?
    }

    let mut stdin = Stdin::new();
    stdin.write_bytes(input.stdin());
    Ok(stdin)
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_pico::PicoRustRv64imaCustomized;
    use ere_prover_core::{Input, ProverResource, codec::Encode, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{
            TestCase, run_zkvm_execute, run_zkvm_execute_estimated_cost, run_zkvm_prove,
            run_zkvm_switchable, testing_guest_directory,
        },
        program::{basic::BasicProgram, zkvm_interface},
    };
    use ere_verifier_pico::PicoProof;
    use pico_prover::ProveError;

    use crate::{
        error::Error,
        prover::{PicoProver, stdin},
    };

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            PicoRustRv64imaCustomized
                .compile(testing_guest_directory("pico", "basic"), &[])
                .unwrap()
        })
        .clone()
    }

    fn zkvm_interface_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            PicoRustRv64imaCustomized
                .compile(testing_guest_directory("pico", "zkvm_interface"), &[])
                .unwrap()
        })
        .clone()
    }

    /// One prover per resource, shared by the tests, since each prove takes tens of GiB.
    fn zkvm(gpu: bool) -> MutexGuard<'static, PicoProver> {
        static ZKVM: [OnceLock<Mutex<PicoProver>>; 2] = [OnceLock::new(), OnceLock::new()];
        ZKVM[gpu as usize]
            .get_or_init(|| {
                let resource = if gpu {
                    ProverResource::Gpu
                } else {
                    ProverResource::Cpu
                };
                Mutex::new(PicoProver::new(basic_elf(), resource).unwrap())
            })
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    fn with_basic_elf(mut zkvm: MutexGuard<PicoProver>) -> MutexGuard<PicoProver> {
        zkvm.setup(basic_elf()).unwrap();
        zkvm
    }

    fn with_zkvm_interface(mut zkvm: MutexGuard<PicoProver>) -> MutexGuard<PicoProver> {
        zkvm.setup(zkvm_interface_elf()).unwrap();
        zkvm
    }

    /// The prover a build has: CPU without `cuda`, GPU with it.
    fn default_zkvm() -> MutexGuard<'static, PicoProver> {
        zkvm(cfg!(feature = "cuda"))
    }

    #[test]
    fn test_execute() {
        let zkvm = with_basic_elf(default_zkvm());
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&*zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let zkvm = with_basic_elf(default_zkvm());
        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_execute_estimated_cost() {
        let zkvm = with_basic_elf(default_zkvm());
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute_estimated_cost(&*zkvm, &test_case);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove() {
        let zkvm = with_basic_elf(zkvm(false));
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove_invalid_test_case() {
        let zkvm = with_basic_elf(zkvm(false));
        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_gpu() {
        let zkvm = with_basic_elf(zkvm(true));
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let zkvm = with_basic_elf(zkvm(true));
        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

    #[test]
    fn test_execute_zkvm_interface() {
        let zkvm = with_zkvm_interface(default_zkvm());
        for test_case in zkvm_interface::test_cases() {
            run_zkvm_execute(&*zkvm, &test_case);
        }
    }

    #[test]
    fn test_execute_switchable() {
        run_zkvm_switchable(
            &mut *default_zkvm(),
            basic_elf(),
            zkvm_interface_elf(),
            false,
        );
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove_switchable() {
        run_zkvm_switchable(&mut *zkvm(false), basic_elf(), zkvm_interface_elf(), true);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_switchable_gpu() {
        run_zkvm_switchable(&mut *zkvm(true), basic_elf(), zkvm_interface_elf(), true);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_gpu_without_cuda_feature() {
        let err = PicoProver::new(basic_elf(), ProverResource::Gpu)
            .err()
            .unwrap();
        assert!(matches!(err, crate::Error::CudaFeatureDisabled));
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_cpu_with_cuda_feature() {
        let err = PicoProver::new(basic_elf(), ProverResource::Cpu)
            .err()
            .unwrap();
        assert!(matches!(err, crate::Error::CpuUnsupportedWithCuda));
    }

    /// The compress proof: at most 300 KiB, verified only as a compress proof, its output bound.
    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove_compress_only() {
        compress_only(&with_basic_elf(zkvm(false)));
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_compress_only_gpu() {
        compress_only(&with_basic_elf(zkvm(true)));
    }

    fn compress_only(zkvm: &PicoProver) {
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        let (_, proof, _) = zkvm.prove(&test_case.input()).unwrap();
        let encoded = proof.encode_to_vec().unwrap();
        assert!(
            encoded.len() <= 300 * 1024,
            "the proof is over 300 KiB: {}",
            encoded.len()
        );
        zkvm.verify(&proof).unwrap();

        // Another output under the same proof.
        let mut committed = proof.committed.clone();
        committed[0] ^= 1;
        zkvm.verify(&PicoProof::new(proof.envelope.clone(), committed))
            .unwrap_err();

        // The root proof the compress proof re-proves, as a v3 envelope: refused.
        let mut prover = zkvm.prover().unwrap();
        let root = prover.prove(stdin(&test_case.input()).unwrap()).unwrap();
        drop(prover);
        let (root_envelope, committed) = root.export_v3().unwrap();
        zkvm.verify(&PicoProof::new(root_envelope, committed))
            .unwrap_err();
    }

    /// Each case proves and verifies, or Pico refuses a chunk that selects its catch-all whitelist
    /// shape, or on the GPU the case runs out of device memory.
    #[test]
    #[ignore = "needs about 300 GB of host memory"]
    fn test_prove_zkvm_interface() {
        let zkvm = with_zkvm_interface(default_zkvm());
        for test_case in zkvm_interface::test_cases() {
            match zkvm.prove(&test_case.input()) {
                Ok((_, proof, _)) => {
                    zkvm.verify(&proof).unwrap();
                }
                Err(Error::Prove(ProveError::CatchAllShape(_))) => {}
                #[cfg(feature = "cuda")]
                Err(Error::Prove(ProveError::GpuOutOfMemory(_))) => {}
                Err(err) => panic!("{err}"),
            }
        }
    }
}
