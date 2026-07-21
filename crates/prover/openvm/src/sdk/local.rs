use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use ere_compiler_core::Elf;
use ere_prover_core::{CommonError, Input, ProverResource, ProverResourceKind};
use ere_verifier_openvm::{OpenVMProgramVk, OpenVMProof, OpenVMVerifier};
use openvm_circuit::arch::{VmBuilder, VmExecutionConfig, instructions::exe::VmExe};
use openvm_sdk::{
    CpuSdk, F, GenericSdk, SC,
    config::AggregationSystemParams,
    fs::read_object_from_file,
    keygen::{AggProvingKey, AppProvingKey},
};
use openvm_sdk_config::SdkVmConfig;
use openvm_stark_sdk::openvm_stark_backend::StarkEngine;

use crate::{
    error::Error,
    sdk::{app_config, app_exe, stdin},
};

/// In-process proving on this machine's CPU or GPU.
pub struct LocalProver {
    app_exe: Arc<VmExe<F>>,
    app_pk: AppProvingKey<SdkVmConfig>,
    agg_pk: AggProvingKey,
    resource: ProverResource,
    verifier: OpenVMVerifier,
}

impl LocalProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        let app_exe = app_exe(&elf)?;

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
        let verifier = OpenVMVerifier::new(OpenVMProgramVk::new(baseline));

        Ok(Self {
            app_exe,
            app_pk,
            agg_pk,
            resource,
            verifier,
        })
    }

    pub fn verifier(&self) -> OpenVMVerifier {
        self.verifier.clone()
    }

    pub fn prove(&self, input: &Input) -> Result<(OpenVMProof, Duration), Error> {
        if cfg!(not(feature = "cuda")) && self.resource == ProverResource::Gpu {
            return Err(Error::CudaFeatureDisabled);
        }

        let start = Instant::now();
        let (proof, _) = match self.resource {
            ProverResource::Cpu => self
                .cpu_sdk()?
                .prove(self.app_exe.clone(), stdin(input), &[]),
            #[cfg(feature = "cuda")]
            ProverResource::Gpu => self
                .gpu_sdk()?
                .prove(self.app_exe.clone(), stdin(input), &[]),
            #[cfg(not(feature = "cuda"))]
            ProverResource::Gpu => return Err(Error::CudaFeatureDisabled),
            _ => Err(CommonError::unsupported_prover_resource_kind(
                self.resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            ))?,
        }
        .map_err(Error::Prove)?;

        Ok((OpenVMProof::new(proof), start.elapsed()))
    }

    fn cpu_sdk(&self) -> Result<CpuSdk, Error> {
        sdk(self.app_pk.clone().into(), self.agg_pk.clone().into())
    }

    #[cfg(feature = "cuda")]
    fn gpu_sdk(&self) -> Result<openvm_sdk::GpuSdk, Error> {
        sdk(self.app_pk.clone().into(), self.agg_pk.clone().into())
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

fn internal_recursive_pk_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
        .join(".openvm")
        .join("internal_recursive.pk")
}
