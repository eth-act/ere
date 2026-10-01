use std::{
    env,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, CostEstimation, CostProfile, Input, ProverResource, ProverResourceKind,
    PublicValues, zkVMProver,
};
use ere_verifier_openvm::{
    NUM_PUBLIC_VALUES_BYTES, OpenVMProgramVk, OpenVMProof, OpenVMVerifier, extract_public_values,
};
use once_cell::sync::OnceCell;
use openvm_circuit::arch::instructions::exe::VmExe;
use openvm_sdk::{
    CpuSdk, F, GenericSdk, StdIn,
    config::{AggregationSystemParams, AppConfig},
    fs::read_object_from_file,
    keygen::{AggProvingKey, AppProvingKey},
};
use openvm_sdk_config::{SdkVmConfig, TranspilerConfig};
use openvm_stark_sdk::config::{MAX_APP_LOG_STACKED_HEIGHT, app_params_with_100_bits_security};
use openvm_transpiler::{FromElf, openvm_platform::memory::MEM_SIZE};
use openvm_verify_stark_host::vk::VerificationBaseline;

use crate::{
    baseline::baseline,
    cost::CostEstimator,
    error::Error,
    executor::Executor,
    thread::{ProverThread, Request},
};

/// Segment memory limit, 14.5 GiB. Execution starts a new segment above it.
const DEFAULT_SEGMENT_MEMORY: usize = 29 << 29;

/// `executor` and `estimator` are lazy, because two `rvr` libraries crash the process at exit.
pub struct OpenVMProver {
    app_pk: AppProvingKey<SdkVmConfig>,
    prover_thread: ProverThread,
    setup_on_init: bool,
    elf: Elf,
    app_exe: Arc<VmExe<F>>,
    executor: OnceCell<Executor>,
    estimator: OnceCell<CostEstimator>,
    verifier: OpenVMVerifier,
}

impl OpenVMProver {
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

        let app_exe = transpile(&elf.0)?;

        let sdk = cpu_sdk(None, None)?;
        let app_pk = sdk.app_pk().clone();
        let agg_pk = AggProvingKey {
            prefix: sdk.agg_prefix_pk(),
            internal_recursive: Arc::new(
                read_object_from_file(internal_recursive_pk_path())
                    .map_err(Error::ReadInternalRecursivePkFailed)?,
            ),
        };
        let agg_prover = cpu_sdk(app_pk.clone().into(), agg_pk.into())?.agg_prover();

        let prover_thread = ProverThread::spawn(&resource, app_pk.clone(), agg_prover.clone());

        let app_exe_commit =
            prover_thread.request(|reply| Request::Commit(app_exe.clone(), reply))?;
        let verifier = OpenVMVerifier::new(OpenVMProgramVk::new(baseline(
            &app_pk,
            &agg_prover,
            app_exe_commit,
        )));

        let setup_on_init = env::var_os("ERE_OPENVM_SETUP_ON_INIT").is_some();
        if setup_on_init {
            prover_thread.request(|reply| Request::Setup(app_exe.clone(), reply))?;
        }

        Ok(Self {
            app_pk,
            prover_thread,
            setup_on_init,
            elf,
            app_exe,
            executor: OnceCell::new(),
            estimator: OnceCell::new(),
            verifier,
        })
    }

    fn executor(&self) -> Result<&Executor, Error> {
        self.executor
            .get_or_try_init(|| Executor::new(&self.app_exe))
    }

    fn estimator(&self) -> Result<&CostEstimator, Error> {
        self.estimator
            .get_or_try_init(|| CostEstimator::new(&self.elf, &self.app_exe, &self.app_pk))
    }
}

impl zkVMProver for OpenVMProver {
    type Verifier = OpenVMVerifier;
    type Error = Error;

    fn verifier(&self) -> &OpenVMVerifier {
        &self.verifier
    }

    fn setup(&mut self, elf: Elf) -> Result<(), Error> {
        if self.elf == elf {
            return Ok(());
        }

        let app_exe = transpile(&elf.0)?;
        let verifier = OpenVMVerifier::new(OpenVMProgramVk::new(VerificationBaseline {
            app_exe_commit: self
                .prover_thread
                .request(|reply| Request::Commit(app_exe.clone(), reply))?,
            ..self.program_vk().0.clone()
        }));
        // Unloads the rvr libraries of the old program before the setup loads new ones.
        self.executor = OnceCell::new();
        self.estimator = OnceCell::new();
        if self.setup_on_init {
            self.prover_thread
                .request(|reply| Request::Setup(app_exe.clone(), reply))?;
        } else {
            self.prover_thread.request(Request::Reset)?;
        }
        self.elf = elf;
        self.app_exe = app_exe;
        self.verifier = verifier;
        Ok(())
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, Duration), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input.stdin());

        self.executor()?.execute(stdin)
    }

    fn execute_estimated_cost(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, CostEstimation), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input.stdin());

        self.estimator()?.estimate(stdin)
    }

    fn profile(&self, input: &Input) -> Result<(PublicValues, CostProfile), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input.stdin());

        self.estimator()?.profile(stdin)
    }

    fn prove(&self, input: &Input) -> Result<(PublicValues, OpenVMProof, Duration), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input.stdin());

        let start = Instant::now();
        let proof = self
            .prover_thread
            .request(|reply| Request::Prove(self.app_exe.clone(), stdin, reply))?;
        let proving_time = start.elapsed();

        let public_values = extract_public_values(&proof.user_pvs_proof.public_values)?;
        let proof = OpenVMProof::new(proof);

        Ok((public_values, proof, proving_time))
    }
}

pub(crate) fn transpile(elf: &[u8]) -> Result<Arc<VmExe<F>>, Error> {
    Ok(Arc::new(
        VmExe::from_elf(
            openvm_transpiler::elf::Elf::decode(elf, MEM_SIZE.try_into().unwrap())
                .map_err(Error::DecodeElf)?,
            app_config().app_vm_config.transpiler(),
        )
        .map_err(Error::Transpile)?,
    ))
}

pub(crate) fn cpu_sdk(
    app_pk: Option<AppProvingKey<SdkVmConfig>>,
    agg_pk: Option<AggProvingKey>,
) -> Result<CpuSdk, Error> {
    let mut builder = GenericSdk::builder();
    builder = if let Some(app_pk) = app_pk {
        builder.app_pk(app_pk)
    } else {
        builder.app_config(app_config())
    };
    builder = if let Some(agg_pk) = agg_pk {
        builder.agg_pk(agg_pk)
    } else {
        builder.agg_params(AggregationSystemParams::default())
    };
    builder
        .build_without_transpiler()
        .map_err(Error::ProverInit)
}

pub(crate) fn sdk_vm_config() -> SdkVmConfig {
    let mut config = SdkVmConfig::standard();
    config.system.config = config
        .system
        .config
        .with_public_values_bytes(NUM_PUBLIC_VALUES_BYTES);
    config.system.config.segmentation_max_memory = env::var("ERE_OPENVM_SEGMENT_MEMORY")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_SEGMENT_MEMORY);
    config.optimize()
}

fn app_config() -> AppConfig<SdkVmConfig> {
    let system_params = app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT);
    AppConfig::new(sdk_vm_config(), system_params)
}

fn internal_recursive_pk_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
        .join(".openvm")
        .join("internal_recursive.pk")
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_openvm::OpenVMRustRv64imaCustomized;
    use ere_prover_core::{Input, ProverResource, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{
            TestCase, profile_root_cost, run_zkvm_execute, run_zkvm_execute_estimated_cost,
            run_zkvm_profile, run_zkvm_profile_without_function_symbols,
            run_zkvm_profile_zkvm_interface, run_zkvm_prove, run_zkvm_switchable,
            testing_guest_directory,
        },
        program::{
            basic::BasicProgram,
            zkvm_interface::{self, Accelerator},
        },
    };
    use openvm_transpiler::openvm_platform::memory::STACK_TOP;

    use crate::prover::OpenVMProver;

    pub(crate) fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            OpenVMRustRv64imaCustomized
                .compile(testing_guest_directory("openvm", "basic"), &[])
                .unwrap()
        })
        .clone()
    }

    fn zkvm_interface_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            OpenVMRustRv64imaCustomized
                .compile(testing_guest_directory("openvm", "zkvm_interface"), &[])
                .unwrap()
        })
        .clone()
    }

    fn zkvm(gpu: bool) -> MutexGuard<'static, OpenVMProver> {
        static ZKVM: [OnceLock<Mutex<OpenVMProver>>; 2] = [OnceLock::new(), OnceLock::new()];
        ZKVM[gpu as usize]
            .get_or_init(|| {
                let resource = if gpu {
                    ProverResource::Gpu
                } else {
                    ProverResource::Cpu
                };
                Mutex::new(OpenVMProver::new(basic_elf(), resource).unwrap())
            })
            .lock()
            .unwrap()
    }

    fn with_basic_elf<T: zkVMProver>(mut zkvm: MutexGuard<T>) -> MutexGuard<T> {
        zkvm.setup(basic_elf()).unwrap();
        zkvm
    }

    fn with_zkvm_interface<T: zkVMProver>(mut zkvm: MutexGuard<T>) -> MutexGuard<T> {
        zkvm.setup(zkvm_interface_elf()).unwrap();
        zkvm
    }

    #[test]
    fn test_execute() {
        let zkvm = with_basic_elf(zkvm(false));
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&*zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let zkvm = with_basic_elf(zkvm(false));
        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_execute_estimated_cost() {
        let zkvm = with_basic_elf(zkvm(false));
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute_estimated_cost(&*zkvm, &test_case);
    }

    #[test]
    fn test_profile() {
        let zkvm = with_basic_elf(zkvm(false));
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        let profile = run_zkvm_profile(&*zkvm, &test_case);
        assert!(profile_root_cost(&profile, "[segment base]") > 0);
        assert!(profile.peak_stack_bytes < STACK_TOP);
    }

    #[test]
    fn test_profile_invalid_test_case() {
        let zkvm = with_basic_elf(zkvm(false));
        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.profile(&input).unwrap_err();
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_profile(&*zkvm, &test_case);
    }

    #[test]
    fn test_profile_zkvm_interface() {
        let zkvm = with_zkvm_interface(zkvm(false));
        run_zkvm_profile_zkvm_interface(&*zkvm);
        // Modexp runs over several segments, which the profile cuts where the estimate does.
        run_zkvm_profile(&*zkvm, &zkvm_interface::test_case(Accelerator::Modexp));
    }

    #[test]
    fn test_profile_elf_without_function_symbols() {
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_profile_without_function_symbols(&mut *zkvm(false), basic_elf(), &test_case);
    }

    #[test]
    fn test_prove() {
        let zkvm = with_basic_elf(zkvm(false));
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&*zkvm, &test_case);
    }

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
        let zkvm = with_zkvm_interface(zkvm(false));
        for test_case in zkvm_interface::test_cases() {
            run_zkvm_execute(&*zkvm, &test_case);
        }
    }

    #[test]
    fn test_execute_switchable() {
        run_zkvm_switchable(&mut *zkvm(false), basic_elf(), zkvm_interface_elf(), false);
    }

    #[test]
    fn test_prove_switchable() {
        run_zkvm_switchable(&mut *zkvm(false), basic_elf(), zkvm_interface_elf(), true);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_switchable_gpu() {
        run_zkvm_switchable(&mut *zkvm(true), basic_elf(), zkvm_interface_elf(), true);
    }
}
