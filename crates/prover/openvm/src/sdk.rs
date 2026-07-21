use std::{env, sync::Arc, time::Duration};

use ere_cluster_client_openvm::OpenVMClusterClient;
use ere_compiler_core::Elf;
use ere_prover_core::{CommonError, Input, ProverResource, ProverResourceKind};
use ere_util_tokio::block_on;
use ere_verifier_openvm::{OpenVMProof, OpenVMVerifier, sdk_vm_config};
use openvm_sdk::{F, StdIn, config::AppConfig};
use openvm_sdk_config::{SdkVmConfig, TranspilerConfig};
use openvm_stark_sdk::config::{MAX_APP_LOG_STACKED_HEIGHT, app_params_with_100_bits_security};
use openvm_transpiler::{FromElf, openvm_platform::memory::MEM_SIZE};

use crate::{error::Error, sdk::local::LocalProver};

mod local;

/// Default cluster prove timeout, overridable per deployment.
const DEFAULT_OPENVM_CLUSTER_PROVE_TIMEOUT_SECS: u64 = 600;

/// Where proving happens for a given [`ProverResource`].
#[allow(clippy::large_enum_variant)]
enum Backend {
    Local(LocalProver),
    Cluster {
        client: OpenVMClusterClient,
        prove_timeout: Duration,
    },
}

/// Owns the backend and the artifacts shared by both, so [`crate::OpenVMProver`]
/// stays a thin trait face.
pub struct OpenVMSdk {
    backend: Backend,
    verifier: OpenVMVerifier,
}

impl OpenVMSdk {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        match &resource {
            ProverResource::Cpu | ProverResource::Gpu => {
                let local = LocalProver::new(elf, resource)?;
                let verifier = local.verifier();
                Ok(Self {
                    backend: Backend::Local(local),
                    verifier,
                })
            }
            // The cluster owns the proving keys, so nothing is generated
            // locally: the verifier comes back from the deployment that will
            // produce the proofs.
            ProverResource::Cluster(config) => {
                let client = block_on(OpenVMClusterClient::new(config, elf))?;
                let prove_timeout = Duration::from_secs(
                    env::var("ERE_OPENVM_CLUSTER_PROVE_TIMEOUT_SECS")
                        .ok()
                        .and_then(|val| val.parse::<u64>().ok())
                        .unwrap_or(DEFAULT_OPENVM_CLUSTER_PROVE_TIMEOUT_SECS),
                );
                let verifier = client.verifier().clone();
                Ok(Self {
                    backend: Backend::Cluster {
                        client,
                        prove_timeout,
                    },
                    verifier,
                })
            }
            ProverResource::Network(_) => Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [
                    ProverResourceKind::Cpu,
                    ProverResourceKind::Gpu,
                    ProverResourceKind::Cluster,
                ],
            )
            .into()),
        }
    }

    pub fn verifier(&self) -> &OpenVMVerifier {
        &self.verifier
    }

    pub fn prove(&self, input: &Input) -> Result<(OpenVMProof, Duration), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        match &self.backend {
            Backend::Local(local) => local.prove(input),
            Backend::Cluster {
                client,
                prove_timeout,
            } => block_on(async {
                let deadline = tokio::time::Instant::now() + *prove_timeout;
                client.prove(input, deadline).await.map_err(Error::Cluster)
            }),
        }
    }
}

/// Transpile `elf` into the executable the VM runs.
pub(crate) fn app_exe(
    elf: &Elf,
) -> Result<Arc<openvm_circuit::arch::instructions::exe::VmExe<F>>, Error> {
    let decoded = openvm_transpiler::elf::Elf::decode(&elf.0, MEM_SIZE.try_into().unwrap())
        .map_err(Error::DecodeElf)?;
    let exe = openvm_circuit::arch::instructions::exe::VmExe::from_elf(
        decoded,
        app_config().app_vm_config.transpiler(),
    )
    .map_err(Error::Transpile)?;
    Ok(Arc::new(exe))
}

/// Guest input in the shape the VM expects.
pub(crate) fn stdin(input: &Input) -> StdIn {
    let mut stdin = StdIn::default();
    stdin.write_bytes(input.stdin());
    stdin
}

pub(crate) fn app_config() -> AppConfig<SdkVmConfig> {
    let system_params = app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT);
    AppConfig::new(sdk_vm_config(), system_params)
}
