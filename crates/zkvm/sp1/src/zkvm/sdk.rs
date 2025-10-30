use crate::zkvm::Error;
use ere_zkvm_interface::zkvm::{NetworkProverConfig, ProverResourceType};
use sp1_sdk::{
    CpuProver, CudaProver, NetworkProver, Prover as _, ProverClient, SP1ProofMode,
    SP1ProofWithPublicValues, SP1ProvingKey, SP1Stdin, SP1VerifyingKey,
};

#[allow(clippy::large_enum_variant)]
pub enum Prover {
    Cpu(CpuProver),
    Gpu(CudaProver),
    Network(NetworkProver),
}

impl Default for Prover {
    fn default() -> Self {
        Self::new(&ProverResourceType::Cpu)
    }
}

impl Prover {
    pub fn new(resource: &ProverResourceType) -> Self {
        match resource {
            ProverResourceType::Cpu => Self::Cpu(ProverClient::builder().cpu().build()),
            ProverResourceType::Gpu => Self::Gpu(ProverClient::builder().cuda().build()),
            ProverResourceType::Network(config) => Self::Network(build_network_prover(config)),
        }
    }

    pub fn setup(&self, elf: &[u8]) -> (SP1ProvingKey, SP1VerifyingKey) {
        match self {
            Self::Cpu(cpu_prover) => cpu_prover.setup(elf),
            Self::Gpu(cuda_prover) => cuda_prover.setup(elf),
            Self::Network(network_prover) => network_prover.setup(elf),
        }
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

fn build_network_prover(config: &NetworkProverConfig) -> NetworkProver {
    let mut builder = ProverClient::builder().network();
    // Check if we have a private key in the config or environment
    if let Some(api_key) = &config.api_key {
        builder = builder.private_key(api_key);
    } else if let Ok(private_key) = std::env::var("NETWORK_PRIVATE_KEY") {
        builder = builder.private_key(&private_key);
    } else {
        panic!(
            "Network proving requires a private key. Set NETWORK_PRIVATE_KEY environment variable or provide api_key in NetworkProverConfig"
        );
    }
    // Set the RPC URL if provided
    if !config.endpoint.is_empty() {
        builder = builder.rpc_url(&config.endpoint);
    } else if let Ok(rpc_url) = std::env::var("NETWORK_RPC_URL") {
        builder = builder.rpc_url(&rpc_url);
    }
    // Otherwise SP1 SDK will use its default RPC URL
    builder.build()
}
