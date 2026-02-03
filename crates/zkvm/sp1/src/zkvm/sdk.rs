use crate::zkvm::{Error, panic_msg};
use ere_zkvm_interface::{
    CommonError, RemoteProverConfig,
    zkvm::{ProverResource, ProverResourceKind},
};
use sp1_sdk::{
    CpuProver, NetworkProver, Prover as _, ProverClient, SP1ProofMode, SP1ProofWithPublicValues,
    SP1ProvingKey, SP1Stdin, SP1VerifyingKey,
};
use std::{
    env,
    ops::Deref,
    panic::{self, AssertUnwindSafe},
    process::Command,
};
use tracing::error;

// https://github.com/succinctlabs/sp1/blob/v5.2.4/crates/cuda/src/lib.rs#L207C34-L207C78.
const SP1_CUDA_IMAGE: &str = "public.ecr.aws/succinct-labs/sp1-gpu:8fd1ef7";

#[allow(clippy::large_enum_variant)]
pub enum Prover {
    Cpu(CpuProver),
    Gpu(CudaProver),
    Network(NetworkProver),
}

impl Default for Prover {
    fn default() -> Self {
        Self::new(&ProverResource::Cpu).unwrap()
    }
}

impl Prover {
    pub fn new(resource: &ProverResource) -> Result<Self, Error> {
        Ok(match resource {
            ProverResource::Cpu => Self::Cpu(ProverClient::builder().cpu().build()),
            ProverResource::Gpu => Self::Gpu(CudaProver::new()?),
            ProverResource::Network(config) => Self::Network(build_network_prover(config)?),
            ProverResource::Cluster(_) => Err(CommonError::unsupported_prover_resource_kind(
                ProverResourceKind::Cluster,
                [
                    ProverResourceKind::Cpu,
                    ProverResourceKind::Gpu,
                    ProverResourceKind::Network,
                ],
            ))?,
        })
    }

    pub fn setup(&self, elf: &[u8]) -> Result<(SP1ProvingKey, SP1VerifyingKey), Error> {
        panic::catch_unwind(AssertUnwindSafe(|| match self {
            Self::Cpu(cpu_prover) => cpu_prover.setup(elf),
            Self::Gpu(cuda_prover) => cuda_prover.setup(elf),
            Self::Network(network_prover) => network_prover.setup(elf),
        }))
        .map_err(|err| Error::SetupElfFailed(panic_msg(err)))
    }

    pub fn execute(
        &self,
        elf: &[u8],
        input: &SP1Stdin,
    ) -> Result<(sp1_sdk::SP1PublicValues, sp1_sdk::ExecutionReport), Error> {
        match self {
            Self::Cpu(cpu_prover) => cpu_prover.execute(elf, input).run(),
            Self::Gpu(cuda_prover) => cuda_prover.execute(elf, input).run(),
            Self::Network(network_prover) => network_prover.execute(elf, input).run(),
        }
        .map_err(Error::Execute)
    }

    pub fn prove(
        &self,
        pk: &SP1ProvingKey,
        input: &SP1Stdin,
        mode: SP1ProofMode,
    ) -> Result<SP1ProofWithPublicValues, Error> {
        match self {
            Self::Cpu(cpu_prover) => cpu_prover.prove(pk, input).mode(mode).run(),
            Self::Gpu(cuda_prover) => cuda_prover.prove(pk, input).mode(mode).run(),
            Self::Network(network_prover) => network_prover.prove(pk, input).mode(mode).run(),
        }
        .map_err(Error::Prove)
    }

    pub fn verify(
        &self,
        proof: &SP1ProofWithPublicValues,
        vk: &SP1VerifyingKey,
    ) -> Result<(), Error> {
        match self {
            Self::Cpu(cpu_prover) => cpu_prover.verify(proof, vk),
            Self::Gpu(cuda_prover) => cuda_prover.verify(proof, vk),
            Self::Network(network_prover) => network_prover.verify(proof, vk),
        }
        .map_err(Error::Verify)
    }
}

pub struct CudaProver {
    container_name: String,
    prover: sp1_sdk::CudaProver,
}

impl Deref for CudaProver {
    type Target = sp1_sdk::CudaProver;

    fn deref(&self) -> &Self::Target {
        &self.prover
    }
}

impl Drop for CudaProver {
    fn drop(&mut self) {
        let mut cmd = Command::new("docker");
        cmd.args(["container", "rm", "--force", self.container_name.as_ref()]);
        if let Err(err) = cmd
            .output()
            .map_err(|err| CommonError::command(&cmd, err))
            .and_then(|output| {
                (!output.status.success()).then_some(()).ok_or_else(|| {
                    CommonError::command_exit_non_zero(&cmd, output.status, Some(&output))
                })
            })
        {
            error!(
                "Failed to remove docker container {}: {err}",
                self.container_name
            );
        }
    }
}

impl CudaProver {
    fn new() -> Result<Self, Error> {
        // Ported from https://github.com/succinctlabs/sp1/blob/v5.2.4/crates/cuda/src/lib.rs#L199.

        let container_name = "sp1-gpu".to_string();
        let image_name = env::var("SP1_GPU_IMAGE").unwrap_or_else(|_| SP1_CUDA_IMAGE.to_string());
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "none".to_string());
        let gpus = env::var("ERE_GPU_DEVICES").unwrap_or_else(|_| "all".to_string());

        let mut cmd = Command::new("docker");
        cmd.args([
            "run",
            "--rm",
            "--env",
            &format!("RUST_LOG={rust_log}"),
            "--publish",
            "3000:3000",
            "--gpus",
            &gpus,
            "--name",
            &container_name,
            &image_name,
        ]);

        let host = if let Ok(network) = env::var("ERE_DOCKER_NETWORK") {
            cmd.args(["--network", network.as_str()]);
            container_name.as_str()
        } else {
            "127.0.0.1"
        };
        let endpoint = format!("http://{host}:3000/twirp/");
        cmd.spawn().map_err(|err| CommonError::command(&cmd, err))?;

        let prover =
            panic::catch_unwind(|| ProverClient::builder().cuda().server(&endpoint).build())
                .map_err(|err| Error::InitCudaProverFailed(panic_msg(err)))?;

        Ok(Self {
            container_name,
            prover,
        })
    }
}

fn build_network_prover(config: &RemoteProverConfig) -> Result<NetworkProver, Error> {
    let mut builder = ProverClient::builder().network();
    // Check if we have a private key in the config or environment
    if let Some(api_key) = &config.api_key {
        builder = builder.private_key(api_key);
    } else if let Ok(private_key) = env::var("NETWORK_PRIVATE_KEY") {
        builder = builder.private_key(&private_key);
    } else {
        return Err(Error::MissingApiKey);
    }
    // Set the RPC URL if provided
    if !config.endpoint.is_empty() {
        builder = builder.rpc_url(&config.endpoint);
    } else if let Ok(rpc_url) = env::var("NETWORK_RPC_URL") {
        builder = builder.rpc_url(&rpc_url);
    }
    // Otherwise SP1 SDK will use its default RPC URL
    Ok(builder.build())
}
