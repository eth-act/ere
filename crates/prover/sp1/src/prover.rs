use std::time::Instant;

use ere_compiler_core::Elf;
use ere_prover_core::{
    Input, ProgramExecutionReport, ProgramProvingReport, ProverResource, PublicValues, block_on,
    zkVMProver,
};
use ere_verifier_sp1::{SP1ProgramVk, SP1Proof, SP1Verifier};
use sp1_sdk::{SP1ProofMode, SP1Stdin};
use tracing::info;

use crate::prover::sdk::SP1Sdk;

mod error;
mod sdk;

pub use error::Error;

pub struct SP1Prover {
    sdk: SP1Sdk,
    verifier: SP1Verifier,
}

impl SP1Prover {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        let sdk = block_on(SP1Sdk::new(elf.0, &resource))?;
        let program_vk = SP1ProgramVk(sdk.vk().clone());
        let verifier = SP1Verifier::new(program_vk);
        Ok(Self { sdk, verifier })
    }
}

impl zkVMProver for SP1Prover {
    type Verifier = SP1Verifier;
    type Error = Error;

    fn verifier(&self) -> &SP1Verifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        let stdin = input_to_stdin(input)?;

        let start = Instant::now();
        let (public_values, exec_report) = block_on(self.sdk.execute(stdin))?;
        let execution_duration = start.elapsed();

        Ok((
            public_values.as_slice().into(),
            ProgramExecutionReport {
                total_num_cycles: exec_report.total_instruction_count(),
                region_cycles: exec_report.cycle_tracker.into_iter().collect(),
                execution_duration,
            },
        ))
    }

    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, SP1Proof, ProgramProvingReport), Error> {
        info!("Generating proof...");

        let stdin = input_to_stdin(input)?;

        let start = Instant::now();
        let proof = block_on(self.sdk.prove(stdin, SP1ProofMode::Compressed))?;
        let proving_time = start.elapsed();

        let public_values = proof.public_values.as_slice().into();

        Ok((
            public_values,
            SP1Proof(proof),
            ProgramProvingReport::new(proving_time),
        ))
    }
}

fn input_to_stdin(input: &Input) -> Result<SP1Stdin, Error> {
    let mut stdin = SP1Stdin::new();
    stdin.write_slice(input.stdin());
    if let Some(proofs) = input.proofs() {
        for (proof, vk) in proofs.map_err(Error::DeserializeInputProofs)? {
            stdin.write_proof(proof, vk);
        }
    }
    Ok(stdin)
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_sp1::SP1RustRv64imaCustomized;
    use ere_prover_core::{Input, ProverResource, RemoteProverConfig, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgram,
    };

    use crate::prover::SP1Prover;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            SP1RustRv64imaCustomized
                .compile(testing_guest_directory("sp1", "basic"))
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute_rust_rv64ima() {
        use ere_compiler_core::Compiler;
        use ere_compiler_sp1::SP1RustRv64ima;
        use ere_prover_core::{Input, ProverResource, zkVMProver};
        use ere_util_test::host::testing_guest_directory;

        let guest_directory = testing_guest_directory("sp1", "stock_nightly_no_std");
        let elf = SP1RustRv64ima.compile(guest_directory).unwrap();
        let zkvm = crate::prover::SP1Prover::new(elf, ProverResource::Cpu).unwrap();
        zkvm.execute(&Input::new()).unwrap();
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = SP1Prover::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = SP1Prover::new(elf, ProverResource::Cpu).unwrap();

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
        let zkvm = SP1Prover::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = SP1Prover::new(elf, ProverResource::Cpu).unwrap();

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
        let zkvm = SP1Prover::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let elf = basic_elf();
        let zkvm = SP1Prover::new(elf, ProverResource::Gpu).unwrap();

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
    #[ignore = "Requires NETWORK_PRIVATE_KEY environment variable to be set"]
    fn test_prove_sp1_network() {
        // Check if we have the required environment variable
        if std::env::var("NETWORK_PRIVATE_KEY").is_err() {
            eprintln!("Skipping network test: NETWORK_PRIVATE_KEY not set");
            return;
        }

        // Create a remote prover configuration
        let config = RemoteProverConfig {
            endpoint: std::env::var("NETWORK_RPC_URL").unwrap_or_default(),
            api_key: std::env::var("NETWORK_PRIVATE_KEY").ok(),
        };
        let elf = basic_elf();
        let zkvm = SP1Prover::new(elf, ProverResource::Network(config)).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }
}
