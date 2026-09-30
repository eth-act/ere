use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, CostEstimation, Input, ProverResource, ProverResourceKind, PublicValues,
    zkVMProver, zkVMVerifier,
};
use ere_verifier_lambdavm::{BLOWUP_FACTOR, LambdaVMProgramVk, LambdaVMProof, LambdaVMVerifier};
use lambda_vm_prover::{GoldilocksCubicProofOptions, MaxRowsConfig, prove_with_options_and_inputs};

use crate::{cost::CostEstimator, error::Error, executor::Executor};

pub struct LambdaVMProver {
    executor: Executor,
    estimator: CostEstimator,
    verifier: LambdaVMVerifier,
}

impl LambdaVMProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        if !matches!(resource, ProverResource::Cpu) {
            Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [ProverResourceKind::Cpu],
            ))?;
        }

        let program = load(&elf)?;

        let executor = Executor::new(&program);
        let estimator = CostEstimator::new(&elf, &program);

        let verifier = LambdaVMVerifier::new(LambdaVMProgramVk::new(elf.0));

        Ok(Self {
            executor,
            estimator,
            verifier,
        })
    }
}

impl zkVMProver for LambdaVMProver {
    type Verifier = LambdaVMVerifier;
    type Error = Error;

    fn verifier(&self) -> &LambdaVMVerifier {
        &self.verifier
    }

    fn setup(&mut self, elf: Elf) -> Result<(), Error> {
        if self.verifier.program_vk().0 == elf.0 {
            return Ok(());
        }

        let program = load(&elf)?;
        self.executor = Executor::new(&program);
        self.estimator = CostEstimator::new(&elf, &program);
        self.verifier = LambdaVMVerifier::new(LambdaVMProgramVk::new(elf.0));
        Ok(())
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, Duration), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        self.executor.execute(input.stdin())
    }

    fn execute_estimated_cost(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, CostEstimation), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        self.estimator.estimate(input.stdin())
    }

    fn prove(&self, input: &Input) -> Result<(PublicValues, LambdaVMProof, Duration), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let options = GoldilocksCubicProofOptions::with_blowup(BLOWUP_FACTOR)
            .map_err(|err| Error::InvalidProofOptions(err.to_string()))?;

        let start = Instant::now();
        let proof = prove_with_options_and_inputs(
            &self.verifier.program_vk().0,
            input.stdin(),
            &options,
            &MaxRowsConfig::default(),
        )
        .map_err(Error::Prove)?;
        let proving_time = start.elapsed();

        let public_values = proof.public_output.as_slice().into();
        let proof = LambdaVMProof::new(proof);

        Ok((public_values, proof, proving_time))
    }
}

fn load(elf: &Elf) -> Result<Arc<lambda_vm_executor::elf::Elf>, Error> {
    lambda_vm_executor::elf::Elf::load(&elf.0)
        .map(Arc::new)
        .map_err(Error::DecodeElf)
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_lambdavm::{LambdaVMRustRv64ima, LambdaVMRustRv64imaCustomized};
    use ere_prover_core::{Input, ProverResource, codec::Encode, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{
            TestCase, run_zkvm_execute, run_zkvm_execute_estimated_cost, run_zkvm_prove,
            testing_guest_directory,
        },
        program::basic::BasicProgram,
    };

    use crate::prover::LambdaVMProver;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            LambdaVMRustRv64imaCustomized
                .compile(testing_guest_directory("lambdavm", "basic"), &[])
                .unwrap()
        })
        .clone()
    }

    fn stock_nightly_no_std_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            LambdaVMRustRv64ima
                .compile(
                    testing_guest_directory("lambdavm", "stock_nightly_no_std"),
                    &[],
                )
                .unwrap()
        })
        .clone()
    }

    /// Switches from the basic program to `stock_nightly_no_std` and back, then runs both again.
    ///
    /// The other zkVMs switch to `zkvm_interface`, which LambdaVM does not support yet.
    fn run_switchable(zkvm: &mut LambdaVMProver, prove: bool) {
        let basic_vk = zkvm.program_vk().encode_to_vec().unwrap();
        zkvm.setup(stock_nightly_no_std_elf()).unwrap();
        let stock_nightly_no_std_vk = zkvm.program_vk().encode_to_vec().unwrap();

        zkvm.setup(basic_elf()).unwrap();
        assert_eq!(zkvm.program_vk().encode_to_vec().unwrap(), basic_vk);
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        if prove {
            run_zkvm_prove(&*zkvm, &test_case);
        } else {
            run_zkvm_execute(&*zkvm, &test_case);
        }

        zkvm.setup(stock_nightly_no_std_elf()).unwrap();
        assert_eq!(
            zkvm.program_vk().encode_to_vec().unwrap(),
            stock_nightly_no_std_vk
        );
        if prove {
            let (_, proof, _) = zkvm.prove(&Input::new()).unwrap();
            zkvm.verify(&proof).unwrap();
        } else {
            zkvm.execute(&Input::new()).unwrap();
        }
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = LambdaVMProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = LambdaVMProver::new(elf, ProverResource::Cpu).unwrap();

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
        let zkvm = LambdaVMProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute_estimated_cost(&zkvm, &test_case);
    }

    #[test]
    fn test_prove() {
        let elf = basic_elf();
        let zkvm = LambdaVMProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = LambdaVMProver::new(elf, ProverResource::Cpu).unwrap();

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
    fn test_execute_switchable() {
        let mut zkvm = LambdaVMProver::new(basic_elf(), ProverResource::Cpu).unwrap();
        run_switchable(&mut zkvm, false);
    }

    #[test]
    fn test_prove_switchable() {
        let mut zkvm = LambdaVMProver::new(basic_elf(), ProverResource::Cpu).unwrap();
        run_switchable(&mut zkvm, true);
    }

    // TODO: Add `test_execute_zkvm_interface` when LambdaVM exports the `zkvm_*`
    // accelerator symbols.
}
