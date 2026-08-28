use std::{
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, CostEstimation, Input, ProverResource, PublicValues, zkVMProver,
};
use ere_util_tokio::block_on;
use ere_verifier_sp1::{SP1ProgramVk, SP1Proof, SP1Verifier};
use sp1_sdk::{HashableKey, SP1Stdin};
use tracing::info;

use crate::{cost::SP1CostEstimator, error::Error, executor::SP1Executor, sdk::SP1Sdk};

pub struct SP1Prover {
    executor: SP1Executor,
    estimator: OnceLock<SP1CostEstimator>,
    elf: Arc<[u8]>,
    sdk: SP1Sdk,
    verifier: SP1Verifier,
}

impl SP1Prover {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        let elf: Arc<[u8]> = Arc::from(elf.0);
        let executor = SP1Executor::new(&elf)?;
        let sdk = block_on(SP1Sdk::new(Arc::clone(&elf), &resource))?;
        let program_vk = SP1ProgramVk(sdk.vk().hash_koalabear());
        let verifier = SP1Verifier::new(program_vk);
        Ok(Self {
            executor,
            estimator: OnceLock::new(),
            elf,
            sdk,
            verifier,
        })
    }

    fn estimator(&self) -> &SP1CostEstimator {
        self.estimator
            .get_or_init(|| SP1CostEstimator::new(self.executor.program(), &self.elf))
    }
}

impl zkVMProver for SP1Prover {
    type Verifier = SP1Verifier;
    type Error = Error;

    fn verifier(&self) -> &SP1Verifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, Duration), Error> {
        self.executor.execute(input_to_stdin(input)?)
    }

    fn execute_estimated_cost(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, CostEstimation), Error> {
        // The gas estimator reads stdin only, so it cannot price a proofs stream.
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        self.estimator().estimate(input.stdin())
    }

    fn prove(&self, input: &Input) -> Result<(PublicValues, SP1Proof, Duration), Error> {
        info!("Generating proof...");

        let stdin = input_to_stdin(input)?;

        let start = Instant::now();
        let proof = block_on(self.sdk.prove(stdin))?;
        let proving_time = start.elapsed();

        let public_values = proof.public_values.as_slice().into();

        Ok((public_values, SP1Proof(proof), proving_time))
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
        host::{
            TestCase, run_zkvm_execute, run_zkvm_execute_estimated_cost, run_zkvm_prove,
            testing_guest_directory,
        },
        program::{basic::BasicProgram, zkvm_interface},
    };

    use crate::prover::SP1Prover;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            SP1RustRv64imaCustomized
                .compile(testing_guest_directory("sp1", "basic"), &[])
                .unwrap()
        })
        .clone()
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
    fn test_execute_estimated_cost() {
        let elf = basic_elf();
        let zkvm = SP1Prover::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute_estimated_cost(&zkvm, &test_case);
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
    #[tokio::test(flavor = "multi_thread")]
    async fn test_prove_gpu() {
        let elf = basic_elf();
        let zkvm = SP1Prover::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[tokio::test(flavor = "multi_thread")]
    async fn test_prove_invalid_test_case_gpu() {
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

    #[test]
    fn test_execute_zkvm_interface() {
        let elf = SP1RustRv64imaCustomized
            .compile(testing_guest_directory("sp1", "zkvm_interface"), &[])
            .unwrap();
        let zkvm = SP1Prover::new(elf, ProverResource::Cpu).unwrap();

        for test_case in zkvm_interface::test_cases() {
            run_zkvm_execute(&zkvm, &test_case);
        }
    }
}
