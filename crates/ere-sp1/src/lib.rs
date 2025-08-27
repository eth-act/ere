#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::{io::Read, path::Path, time::Instant};

use serde::de::DeserializeOwned;
use sp1_sdk::{
    CpuProver, CudaProver, NetworkProver, Prover, ProverClient, SP1ProofWithPublicValues,
    SP1ProvingKey, SP1Stdin, SP1VerifyingKey,
};
use tracing::info;
use zkvm_interface::{
    Compiler, Input, InputItem, NetworkProverConfig, ProgramExecutionReport, ProgramProvingReport,
    Proof, ProverResourceType, PublicValues, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod compile;
mod compile_stock_rust;

mod error;
use error::{ExecuteError, ProveError, SP1Error, VerifyError};

#[allow(clippy::large_enum_variant)]
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

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain =
            std::env::var("ERE_GUEST_TOOLCHAIN").unwrap_or_else(|_error| "succinct".into());
        compile::compile(guest_directory, &toolchain).map_err(SP1Error::from)
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
    fn execute(
        &self,
        inputs: &Input,
    ) -> Result<(PublicValues, zkvm_interface::ProgramExecutionReport), zkVMError> {
        let mut stdin = SP1Stdin::new();
        serialize_inputs(&mut stdin, inputs);

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
        inputs: &zkvm_interface::Input,
    ) -> Result<(PublicValues, Proof, zkvm_interface::ProgramProvingReport), zkVMError> {
        info!("Generating proof…");

        let mut stdin = SP1Stdin::new();
        serialize_inputs(&mut stdin, inputs);

        let client = Self::create_client(&self.resource);
        let start = std::time::Instant::now();
        let proof_with_inputs = client.prove(&self.pk, &stdin)?;
        let proving_time = start.elapsed();

        let bytes = bincode::serialize(&proof_with_inputs)
            .map_err(|err| SP1Error::Prove(ProveError::Bincode(err)))?;

        Ok((
            proof_with_inputs.public_values.to_vec(),
            bytes,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &[u8]) -> Result<PublicValues, zkVMError> {
        info!("Verifying proof…");

        let proof: SP1ProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| SP1Error::Verify(VerifyError::Bincode(err)))?;

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

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, reader: R) -> Result<T, zkVMError> {
        bincode::deserialize_from(reader).map_err(zkVMError::other)
    }
}

fn serialize_inputs(stdin: &mut SP1Stdin, inputs: &Input) {
    for input in inputs.iter() {
        match input {
            InputItem::Object(obj) => stdin.write(obj),
            InputItem::SerializedObject(bytes) | InputItem::Bytes(bytes) => {
                stdin.write_slice(bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::compile;
    use std::{panic, sync::OnceLock};
    use test_utils::host::{
        BasicProgramIo, Io, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };

    static BASIC_PRORGAM: OnceLock<Vec<u8>> = OnceLock::new();

    fn basic_program() -> Vec<u8> {
        BASIC_PRORGAM
            .get_or_init(|| {
                RV32_IM_SUCCINCT_ZKVM_ELF
                    .compile(&testing_guest_directory("sp1", "basic"))
                    .unwrap()
            })
            .to_vec()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        let public_values = run_zkvm_execute(&zkvm, &io);
        assert_eq!(io.deserialize_outputs(&zkvm, &public_values), io.outputs());
    }

    #[test]
    fn test_execute_nightly() {
        let guest_directory = testing_guest_directory("sp1", "stock_nightly_no_std");
        let program = compile(&guest_directory, &"nightly".to_string()).unwrap();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        zkvm.execute(&Input::new()).unwrap();
    }

    #[test]
    fn test_execute_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        for inputs in [
            BasicProgramIo::empty(),
            BasicProgramIo::invalid_type(),
            BasicProgramIo::invalid_data(),
        ] {
            zkvm.execute(&inputs).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        let public_values = run_zkvm_prove(&zkvm, &io);
        assert_eq!(io.deserialize_outputs(&zkvm, &public_values), io.outputs());
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreSP1::new(program, ProverResourceType::Cpu);

        // On invalid inputs SP1 prove will panics, the issue for tracking:
        // https://github.com/eth-act/ere/issues/16.
        //
        // Note that we iterate on methods because `InputItem::Object` doesn't
        // implement `RefUnwindSafe`.
        for inputs_gen in [
            BasicProgramIo::empty,
            BasicProgramIo::invalid_type,
            BasicProgramIo::invalid_data,
        ] {
            panic::catch_unwind(|| zkvm.prove(&inputs_gen())).unwrap_err();
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

        let io = BasicProgramIo::valid();
        let public_values = run_zkvm_prove(&zkvm, &io);
        assert_eq!(io.deserialize_outputs(&zkvm, &public_values), io.outputs());
    }
}
