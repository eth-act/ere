use std::{path::PathBuf, sync::Arc, time::Instant};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, ProverResource,
    ProverResourceKind, PublicValues, zkVMProver,
};
use ere_verifier_openvm::{
    NUM_PUBLIC_VALUES_BYTES, OpenVMProgramVk, OpenVMProof, OpenVMVerifier, extract_public_values,
};
use openvm_circuit::arch::{VmBuilder, VmExecutionConfig, instructions::exe::VmExe};
use openvm_sdk::{
    CpuSdk, F, GenericSdk, SC, StdIn,
    config::{AggregationSystemParams, AppConfig},
    fs::read_object_from_file,
    keygen::{AggProvingKey, AppProvingKey},
};
use openvm_sdk_config::{SdkVmConfig, TranspilerConfig};
use openvm_stark_sdk::{
    config::{MAX_APP_LOG_STACKED_HEIGHT, app_params_with_100_bits_security},
    openvm_stark_backend::StarkEngine,
};
use openvm_transpiler::{FromElf, openvm_platform::memory::MEM_SIZE};

use crate::{error::Error, executor::Executor};

pub struct OpenVMProver {
    app_exe: Arc<VmExe<F>>,
    app_pk: AppProvingKey<SdkVmConfig>,
    agg_pk: AggProvingKey,
    resource: ProverResource,
    executor: Executor,
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

        let app_exe = Arc::new(
            VmExe::from_elf(
                openvm_transpiler::elf::Elf::decode(&elf.0, MEM_SIZE.try_into().unwrap())
                    .map_err(Error::DecodeElf)?,
                app_config().app_vm_config.transpiler(),
            )
            .map_err(Error::Transpile)?,
        );

        let sdk = cpu_sdk(None, None)?;
        let app_pk = sdk.app_pk().clone();
        let agg_pk = AggProvingKey {
            prefix: sdk.agg_prefix_pk(),
            internal_recursive: Arc::new(
                read_object_from_file(internal_recursive_pk_path())
                    .map_err(Error::ReadInternalRecursivePkFailed)?,
            ),
        };

        let baseline = cpu_sdk(app_pk.clone().into(), agg_pk.clone().into())?
            .prover(app_exe.clone())
            .map_err(Error::ProverInit)?
            .generate_baseline();

        let executor = Executor::new(sdk_vm_config(), &app_exe)?;

        let verifier = OpenVMVerifier::new(OpenVMProgramVk::new(baseline.clone()));

        Ok(Self {
            app_exe,
            app_pk,
            agg_pk,
            resource,
            executor,
            verifier,
        })
    }

    fn cpu_sdk(&self) -> Result<CpuSdk, Error> {
        sdk(self.app_pk.clone().into(), self.agg_pk.clone().into())
    }

    #[cfg(feature = "cuda")]
    fn gpu_sdk(&self) -> Result<openvm_sdk::GpuSdk, Error> {
        sdk(self.app_pk.clone().into(), self.agg_pk.clone().into())
    }
}

impl zkVMProver for OpenVMProver {
    type Verifier = OpenVMVerifier;
    type Error = Error;

    fn verifier(&self) -> &OpenVMVerifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input.stdin());

        self.executor.execute(stdin)
    }

    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, OpenVMProof, ProgramProvingReport), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input.stdin());

        let start = Instant::now();
        let (proof, _) = match self.resource {
            ProverResource::Cpu => self.cpu_sdk()?.prove(self.app_exe.clone(), stdin, &[]),
            #[cfg(feature = "cuda")]
            ProverResource::Gpu => self.gpu_sdk()?.prove(self.app_exe.clone(), stdin, &[]),
            #[cfg(not(feature = "cuda"))]
            ProverResource::Gpu => return Err(Error::CudaFeatureDisabled),
            _ => Err(CommonError::unsupported_prover_resource_kind(
                self.resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            ))?,
        }
        .map_err(Error::Prove)?;
        let proving_time = start.elapsed();

        let public_values = extract_public_values(&proof.user_pvs_proof.public_values)?;
        let proof = OpenVMProof::new(proof);

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }
}

fn cpu_sdk(
    app_pk: Option<AppProvingKey<SdkVmConfig>>,
    agg_pk: Option<AggProvingKey>,
) -> Result<CpuSdk, Error> {
    sdk(app_pk, agg_pk)
}

fn sdk<E, VB>(
    app_pk: Option<AppProvingKey<SdkVmConfig>>,
    agg_pk: Option<AggProvingKey>,
) -> Result<GenericSdk<E, VB>, Error>
where
    E: StarkEngine<SC = SC>,
    VB: Default + VmBuilder<E, VmConfig = SdkVmConfig>,
    VB::VmConfig: VmExecutionConfig<F>,
{
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

fn sdk_vm_config() -> SdkVmConfig {
    let mut config = SdkVmConfig::standard();
    config.system.config = config
        .system
        .config
        .with_public_values_bytes(NUM_PUBLIC_VALUES_BYTES);
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
mod tests {
    use std::sync::OnceLock;

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_openvm::OpenVMRustRv64imaCustomized;
    use ere_prover_core::{Input, ProverResource, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::{basic::BasicProgram, zkvm_interface},
    };

    use crate::prover::OpenVMProver;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            OpenVMRustRv64imaCustomized
                .compile(testing_guest_directory("openvm", "basic"), &[])
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_gpu() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Gpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_zkvm_interface() {
        let elf = OpenVMRustRv64imaCustomized
            .compile(testing_guest_directory("openvm", "zkvm_interface"), &[])
            .unwrap();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

        for test_case in zkvm_interface::test_cases() {
            run_zkvm_execute(&zkvm, &test_case);
        }
    }
}
