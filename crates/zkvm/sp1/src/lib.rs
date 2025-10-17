#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    compiler::SP1Program,
    error::{ExecuteError, ProveError, SP1Error, VerifyError},
};
use ere_zkvm_interface::{
    NetworkProverConfig, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};
use sp1_sdk::{
    CpuProver, CudaProver, NetworkProver, Prover, ProverClient, SP1ProofMode,
    SP1ProofWithPublicValues, SP1ProvingKey, SP1Stdin, SP1VerifyingKey,
};
use std::time::Instant;
use tracing::info;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod compiler;
pub mod error;

#[allow(clippy::large_enum_variant)]
enum ProverType {
    Cpu(CpuProver),
    Gpu(CudaProver),
    Network(NetworkProver),
}

impl ProverType {
    fn setup(&self, program: &SP1Program) -> (SP1ProvingKey, SP1VerifyingKey) {
        match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.setup(program),
            ProverType::Gpu(cuda_prover) => cuda_prover.setup(program),
            ProverType::Network(network_prover) => network_prover.setup(program),
        }
    }

    fn execute(
        &self,
        program: &SP1Program,
        input: &SP1Stdin,
    ) -> Result<(sp1_sdk::SP1PublicValues, sp1_sdk::ExecutionReport), SP1Error> {
        let cpu_executor_builder = match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.execute(program, input),
            ProverType::Gpu(cuda_prover) => cuda_prover.execute(program, input),
            ProverType::Network(network_prover) => network_prover.execute(program, input),
        };

        cpu_executor_builder
            .run()
            .map_err(|e| SP1Error::Execute(ExecuteError::Client(e.into())))
    }

    fn prove(
        &self,
        pk: &SP1ProvingKey,
        input: &SP1Stdin,
        mode: SP1ProofMode,
    ) -> Result<SP1ProofWithPublicValues, SP1Error> {
        match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.prove(pk, input).mode(mode).run(),
            ProverType::Gpu(cuda_prover) => cuda_prover.prove(pk, input).mode(mode).run(),
            ProverType::Network(network_prover) => network_prover.prove(pk, input).mode(mode).run(),
        }
        .map_err(|e| SP1Error::Prove(ProveError::Client(e.into())))
    }

    fn verify(
        &self,
        proof: &SP1ProofWithPublicValues,
        vk: &SP1VerifyingKey,
    ) -> Result<(), SP1Error> {
        match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.verify(proof, vk),
            ProverType::Gpu(cuda_prover) => cuda_prover.verify(proof, vk),
            ProverType::Network(network_prover) => network_prover.verify(proof, vk),
        }
        .map_err(|e| SP1Error::Verify(VerifyError::Client(e.into())))
    }
}

pub struct EreSP1 {
    program: SP1Program,
    /// Proving key
    pk: SP1ProvingKey,
    /// Verification key
    vk: SP1VerifyingKey,
    /// Prover resource configuration for creating clients
    resource: ProverResourceType,
    // FIXME: The current version of SP1 (v5.0.5) has a problem where if proving the program crashes in the
    // Moongate container, it leaves an internal mutex poisoned, which prevents further proving attempts.
    // This is a workaround to avoid the poisoned mutex issue by creating a new client for each prove call.
    // We still use the `setup(...)` method to create the proving and verification keys only once, such that when
    // later calling `prove(...)` in a fresh client, we can reuse the keys and avoiding extra work.
    //
    // Eventually, this should be fixed in the SP1 SDK and we can create the `client` in the `new(...)` method.
    // For more context see: https://github.com/eth-act/zkevm-benchmark-workload/issues/54
}

impl EreSP1 {
    fn create_network_prover(config: &NetworkProverConfig) -> NetworkProver {
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

    fn create_client(resource: &ProverResourceType) -> ProverType {
        match resource {
            ProverResourceType::Cpu => ProverType::Cpu(ProverClient::builder().cpu().build()),
            ProverResourceType::Gpu => ProverType::Gpu(ProverClient::builder().cuda().build()),
            ProverResourceType::Network(config) => {
                ProverType::Network(Self::create_network_prover(config))
            }
        }
    }

    pub fn new(program: SP1Program, resource: ProverResourceType) -> Self {
        let (pk, vk) = Self::create_client(&resource).setup(&program);

        Self {
            program,
            pk,
            vk,
            resource,
        }
    }
}

impl zkVM for EreSP1 {
    fn execute(&self, input: &[u8]) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let mut stdin = SP1Stdin::new();
        stdin.write_slice(input);

        let client = Self::create_client(&self.resource);
        let start = Instant::now();
        let (public_values, exec_report) = client.execute(&self.program, &stdin)?;

        Ok((
            public_values.to_vec(),
            ProgramExecutionReport {
                total_num_cycles: exec_report.total_instruction_count(),
                region_cycles: exec_report.cycle_tracker.into_iter().collect(),
                execution_duration: start.elapsed(),
            },
        ))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        info!("Generating proof…");

        let mut stdin = SP1Stdin::new();
        stdin.write_slice(input);

        let mode = match proof_kind {
            ProofKind::Compressed => SP1ProofMode::Compressed,
            ProofKind::Groth16 => SP1ProofMode::Groth16,
        };

        let client = Self::create_client(&self.resource);
        let start = std::time::Instant::now();
        let proof = client.prove(&self.pk, &stdin, mode)?;
        let proving_time = start.elapsed();

        let public_values = proof.public_values.to_vec();
        let proof = Proof::new(
            proof_kind,
            bincode::serde::encode_to_vec(&proof, bincode::config::legacy())
                .map_err(|err| SP1Error::Prove(ProveError::Bincode(err)))?,
        );

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        info!("Verifying proof…");

        let proof_kind = proof.kind();

        let (proof, _): (SP1ProofWithPublicValues, _) =
            bincode::serde::decode_from_slice(proof.as_bytes(), bincode::config::legacy())
                .map_err(|err| SP1Error::Verify(VerifyError::Bincode(err)))?;
        let inner_proof_kind = SP1ProofMode::from(&proof.proof);

        if !matches!(
            (proof_kind, inner_proof_kind),
            (ProofKind::Compressed, SP1ProofMode::Compressed)
                | (ProofKind::Groth16, SP1ProofMode::Groth16)
        ) {
            return Err(SP1Error::Verify(VerifyError::InvalidProofKind(
                proof_kind,
                inner_proof_kind,
            )))?;
        }

        let client = Self::create_client(&self.resource);
        client.verify(&proof, &self.vk).map_err(zkVMError::from)?;

        let public_values_bytes = proof.public_values.as_slice().to_vec();

        Ok(public_values_bytes)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

#[cfg(test)]
mod tests {
    use crate::{EreSP1, compiler::RustRv32imaCustomized};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgramInput,
    };
    use ere_zkvm_interface::{Compiler, NetworkProverConfig, ProofKind, ProverResourceType, zkVM};
    use std::{panic, sync::OnceLock};

    static BASIC_PROGRAM: OnceLock<Vec<u8>> = OnceLock::new();

    fn basic_program() -> Vec<u8> {
        BASIC_PROGRAM
            .get_or_init(|| {
                RustRv32imaCustomized
                    .compile(&testing_guest_directory("sp1", "basic"))
                    .unwrap()
            })
            .to_vec()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        let test_case = BasicProgramInput::valid();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_input() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        let test_case = BasicProgramInput::valid();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_input() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        // When guest panics SP1 prove will also panics.
        // Issue for tracking: https://github.com/eth-act/ere/issues/172.
        for input in [Vec::new(), BasicProgramInput::invalid().serialized_input()] {
            panic::catch_unwind(|| zkvm.prove(&input, ProofKind::default())).unwrap_err();
        }
    }

    #[test]
    #[ignore = "Requires NETWORK_PRIVATE_KEY environment variable to be set"]
    fn test_prove_sp1_network() {
        // Check if we have the required environment variable
        if std::env::var("NETWORK_PRIVATE_KEY").is_err() {
            eprintln!("Skipping network test: NETWORK_PRIVATE_KEY not set");
            return;
        }

        // Create a network prover configuration
        let network_config = NetworkProverConfig {
            endpoint: std::env::var("NETWORK_RPC_URL").unwrap_or_default(),
            api_key: std::env::var("NETWORK_PRIVATE_KEY").ok(),
        };
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Network(network_config));

        let test_case = BasicProgramInput::valid();
        run_zkvm_prove(&zkvm, &test_case);
    }
}
