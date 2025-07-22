#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::{path::Path, time::Instant};

use sp1_sdk::{
    CpuProver, CudaProver, NetworkProver, Prover, ProverClient, SP1ProofWithPublicValues,
    SP1ProvingKey, SP1Stdin, SP1VerifyingKey,
};
use tracing::info;
use zkvm_interface::{
    Compiler, Input, InputItem, NetworkProverConfig, ProgramExecutionReport, ProgramProvingReport,
    ProverResourceType, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod compile;

mod error;
use error::{ExecuteError, ProveError, SP1Error, VerifyError};

enum ProverType {
    Cpu(CpuProver),
    Gpu(CudaProver),
    Network(NetworkProver),
}

impl ProverType {
    fn setup(
        &self,
        program: &<RV32_IM_SUCCINCT_ZKVM_ELF as Compiler>::Program,
    ) -> (SP1ProvingKey, SP1VerifyingKey) {
        match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.setup(program),
            ProverType::Gpu(cuda_prover) => cuda_prover.setup(program),
            ProverType::Network(network_prover) => network_prover.setup(program),
        }
    }

    fn execute(
        &self,
        program: &<RV32_IM_SUCCINCT_ZKVM_ELF as Compiler>::Program,
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
    ) -> Result<SP1ProofWithPublicValues, SP1Error> {
        match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.prove(pk, input).compressed().run(),
            ProverType::Gpu(cuda_prover) => cuda_prover.prove(pk, input).compressed().run(),
            ProverType::Network(network_prover) => {
                network_prover.prove(pk, input).compressed().run()
            }
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

#[allow(non_camel_case_types)]
pub struct RV32_IM_SUCCINCT_ZKVM_ELF;
pub struct EreSP1 {
    program: <RV32_IM_SUCCINCT_ZKVM_ELF as Compiler>::Program,
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

impl Compiler for RV32_IM_SUCCINCT_ZKVM_ELF {
    type Error = SP1Error;

    type Program = Vec<u8>;

    fn compile(
        workspace_directory: &Path,
        guest_relative: &Path,
    ) -> Result<Self::Program, Self::Error> {
        compile::compile(workspace_directory, guest_relative).map_err(SP1Error::from)
    }
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

    pub fn new(
        program: <RV32_IM_SUCCINCT_ZKVM_ELF as Compiler>::Program,
        resource: ProverResourceType,
    ) -> Self {
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
    fn execute(&self, inputs: &Input) -> Result<zkvm_interface::ProgramExecutionReport, zkVMError> {
        let mut stdin = SP1Stdin::new();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::Bytes(items) => stdin.write_slice(items),
            }
        }

        let client = Self::create_client(&self.resource);
        let start = Instant::now();
        let (_, exec_report) = client.execute(&self.program, &stdin)?;
        Ok(ProgramExecutionReport {
            total_num_cycles: exec_report.total_instruction_count(),
            region_cycles: exec_report.cycle_tracker.into_iter().collect(),
            execution_duration: start.elapsed(),
        })
    }

    fn prove(
        &self,
        inputs: &zkvm_interface::Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), zkVMError> {
        info!("Generating proof…");

        let mut stdin = SP1Stdin::new();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::Bytes(items) => stdin.write_slice(items),
            };
        }

        let client = Self::create_client(&self.resource);
        let start = std::time::Instant::now();
        let proof_with_inputs = client.prove(&self.pk, &stdin)?;
        let proving_time = start.elapsed();

        let bytes = bincode::serialize(&proof_with_inputs)
            .map_err(|err| SP1Error::Prove(ProveError::Bincode(err)))?;

        Ok((bytes, ProgramProvingReport::new(proving_time)))
    }

    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError> {
        info!("Verifying proof…");

        let proof: SP1ProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| SP1Error::Verify(VerifyError::Bincode(err)))?;

        let client = Self::create_client(&self.resource);
        client.verify(&proof, &self.vk).map_err(zkVMError::from)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

#[cfg(test)]
mod execute_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_compiled_test_sp1_elf() -> Result<Vec<u8>, SP1Error> {
        let test_guest_path = get_execute_test_guest_program_path();
        RV32_IM_SUCCINCT_ZKVM_ELF::compile(&test_guest_path, Path::new(""))
    }

    fn get_execute_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("sp1")
            .join("execute")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/sp1")
    }

    #[test]
    fn test_execute_sp1_dummy_input() {
        let elf_bytes = get_compiled_test_sp1_elf()
            .expect("Failed to compile test SP1 guest for execution test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreSP1::new(elf_bytes, ProverResourceType::Cpu);

        let result = zkvm.execute(&input_builder);

        if let Err(e) = &result {
            panic!("Execution error: {:?}", e);
        }
    }

    #[test]
    fn test_execute_sp1_no_input_for_guest_expecting_input() {
        let elf_bytes = get_compiled_test_sp1_elf()
            .expect("Failed to compile test SP1 guest for execution test");

        let empty_input = Input::new();

        let zkvm = EreSP1::new(elf_bytes, ProverResourceType::Cpu);
        let result = zkvm.execute(&empty_input);

        assert!(
            result.is_err(),
            "execute should fail if guest expects input but none is provided."
        );
    }
}

#[cfg(test)]
mod prove_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_prove_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("sp1")
            .join("prove")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/sp1")
    }

    fn get_compiled_test_sp1_elf_for_prove() -> Result<Vec<u8>, SP1Error> {
        let test_guest_path = get_prove_test_guest_program_path();
        RV32_IM_SUCCINCT_ZKVM_ELF::compile(&test_guest_path, Path::new(""))
    }

    #[test]
    fn test_prove_sp1_dummy_input() {
        let elf_bytes = get_compiled_test_sp1_elf_for_prove()
            .expect("Failed to compile test SP1 guest for proving test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreSP1::new(elf_bytes, ProverResourceType::Cpu);

        let proof_bytes = match zkvm.prove(&input_builder) {
            Ok((prove_result, _)) => prove_result,
            Err(err) => {
                panic!("Proving error in test: {:?}", err);
            }
        };

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        let verify_results = zkvm.verify(&proof_bytes).is_ok();
        assert!(verify_results);

        // TODO: Check public inputs
    }

    #[test]
    #[should_panic]
    fn test_prove_sp1_fails_on_bad_input_causing_execution_failure() {
        let elf_bytes = get_compiled_test_sp1_elf_for_prove()
            .expect("Failed to compile test SP1 guest for proving test");

        let empty_input = Input::new();

        let zkvm = EreSP1::new(elf_bytes, ProverResourceType::Cpu);
        let prove_result = zkvm.prove(&empty_input);
        assert!(prove_result.is_err())
    }

    #[test]
    #[ignore = "Requires NETWORK_PRIVATE_KEY environment variable to be set"]
    fn test_prove_sp1_network() {
        // Check if we have the required environment variable
        if std::env::var("NETWORK_PRIVATE_KEY").is_err() {
            eprintln!("Skipping network test: NETWORK_PRIVATE_KEY not set");
            return;
        }

        let elf_bytes = get_compiled_test_sp1_elf_for_prove()
            .expect("Failed to compile test SP1 guest for proving test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        // Create a network prover configuration
        let network_config = NetworkProverConfig {
            endpoint: std::env::var("NETWORK_RPC_URL").unwrap_or_default(),
            api_key: std::env::var("NETWORK_PRIVATE_KEY").ok(),
        };

        let zkvm = EreSP1::new(elf_bytes, ProverResourceType::Network(network_config));

        // Execute first to ensure the program works
        let exec_result = zkvm.execute(&input_builder);
        assert!(exec_result.is_ok(), "Execution should succeed");

        // Now prove using the network
        let proof_bytes = match zkvm.prove(&input_builder) {
            Ok((prove_result, report)) => {
                println!("Network proving completed in {:?}", report.proving_time);
                prove_result
            }
            Err(err) => {
                panic!("Network proving error: {:?}", err);
            }
        };

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        // Verify the proof
        let verify_result = zkvm.verify(&proof_bytes);
        assert!(verify_result.is_ok(), "Verification should succeed");
    }
}
