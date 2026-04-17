use crate::prover::Error;
use ere_prover_core::{
    CommonError, RemoteProverConfig,
    prover::{ProverResource, ProverResourceKind},
};
use sp1_cuda::CudaProvingKey;
use sp1_hypercube::air::{PublicValues, SP1_PROOF_NUM_PV_ELTS};
use sp1_p3_field::PrimeField32;
use sp1_recursion_executor::{RECURSIVE_PROOF_NUM_PV_ELTS, RecursionPublicValues};
use sp1_sdk::{
    CpuProver, CudaProver, Elf, ExecutionReport, NetworkProver, ProveRequest, Prover as SP1Prover,
    ProverClient, ProvingKey as SP1ProvingKeyTrait, SP1Proof, SP1ProofMode,
    SP1ProofWithPublicValues, SP1ProvingKey as CpuProvingKey, SP1PublicValues, SP1Stdin,
    SP1VerifyingKey, StatusCode,
};
use std::{borrow::Borrow, env, sync::Arc};

pub enum SP1Sdk {
    Cpu {
        prover: CpuProver,
        pk: CpuProvingKey,
    },
    Gpu {
        prover: CudaProver,
        pk: CudaProvingKey,
    },
    Network {
        prover: Box<NetworkProver>,
        pk: CpuProvingKey,
    },
}

impl SP1Sdk {
    pub async fn new(elf: Vec<u8>, resource: &ProverResource) -> Result<Self, Error> {
        let elf = Elf::Dynamic(Arc::from(elf));
        Ok(match resource {
            ProverResource::Cpu => {
                let prover = ProverClient::builder().cpu().build().await;
                let pk = prover.setup(elf).await.map_err(Error::setup)?;
                Self::Cpu { prover, pk }
            }
            ProverResource::Gpu => {
                let prover = ProverClient::builder().cuda().build().await;
                let pk = prover.setup(elf).await.map_err(Error::setup)?;
                Self::Gpu { prover, pk }
            }
            ProverResource::Network(config) => {
                let prover = build_network_prover(config).await?;
                let pk = prover.setup(elf).await.map_err(Error::setup)?;
                Self::Network {
                    prover: Box::new(prover),
                    pk,
                }
            }
            _ => Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [
                    ProverResourceKind::Cpu,
                    ProverResourceKind::Gpu,
                    ProverResourceKind::Network,
                ],
            ))?,
        })
    }

    pub fn vk(&self) -> &SP1VerifyingKey {
        match self {
            Self::Cpu { pk, .. } => pk.verifying_key(),
            Self::Gpu { pk, .. } => pk.verifying_key(),
            Self::Network { pk, .. } => pk.verifying_key(),
        }
    }

    pub async fn execute(
        &self,
        input: SP1Stdin,
    ) -> Result<(SP1PublicValues, ExecutionReport), Error> {
        let (public_values, exec_report) = match self {
            Self::Cpu { prover, pk } => prover.execute(pk.elf().clone(), input).await,
            Self::Gpu { prover, pk } => prover.execute(pk.elf().clone(), input).await,
            Self::Network { prover, pk } => prover.execute(pk.elf().clone(), input).await,
        }
        .map_err(|e| Error::Execute(e.into()))?;

        let exit_code = exec_report.exit_code as u32;
        if exit_code != StatusCode::SUCCESS.as_u32() {
            return Err(Error::ExecutionFailed(exit_code));
        }

        Ok((public_values, exec_report))
    }

    pub async fn prove(
        &self,
        input: SP1Stdin,
        mode: SP1ProofMode,
    ) -> Result<SP1ProofWithPublicValues, Error> {
        let proof = match self {
            Self::Cpu { prover, pk } => {
                let req = prover.prove(pk, input).mode(mode);
                req.await.map_err(Error::prove)
            }
            Self::Gpu { prover, pk } => {
                let req = prover.prove(pk, input).mode(mode);
                req.await.map_err(Error::prove)
            }
            Self::Network { prover, pk } => {
                let req = prover.prove(pk, input).mode(mode);
                req.await.map_err(Error::prove)
            }
        }?;

        let exit_code = extract_exit_code(&proof)?;
        if exit_code != StatusCode::SUCCESS.as_u32() {
            return Err(Error::ExecutionFailed(exit_code));
        }

        Ok(proof)
    }
}

async fn build_network_prover(config: &RemoteProverConfig) -> Result<NetworkProver, Error> {
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
    Ok(builder.build().await)
}

/// Extracts the exit code from an public values of proof.
///
/// The `exit_code` field is extracted from the public values struct of proof,
/// mirroring the approach used in `verify_proof` of `sp1_sdk`.
fn extract_exit_code(proof: &SP1ProofWithPublicValues) -> Result<u32, Error> {
    match &proof.proof {
        SP1Proof::Core(shard_proofs) => shard_proofs.last().and_then(|proof| {
            (proof.public_values.len() == SP1_PROOF_NUM_PV_ELTS).then(|| {
                let pv: &PublicValues<[_; 4], [_; 3], [_; 4], _> =
                    proof.public_values.as_slice().borrow();
                pv.exit_code.as_canonical_u32()
            })
        }),
        SP1Proof::Compressed(proof) => {
            (proof.proof.public_values.len() == RECURSIVE_PROOF_NUM_PV_ELTS).then(|| {
                let pv: &RecursionPublicValues<_> = proof.proof.public_values.as_slice().borrow();
                pv.exit_code.as_canonical_u32()
            })
        }
        SP1Proof::Plonk(proof) => proof
            .public_inputs
            .get(2)
            .and_then(|value| value.parse::<u32>().ok()),
        SP1Proof::Groth16(proof) => proof
            .public_inputs
            .get(2)
            .and_then(|value| value.parse::<u32>().ok()),
    }
    .ok_or(Error::ExitCodeExtractionFailed)
}
