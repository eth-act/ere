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

        let program =
            Arc::new(lambda_vm_executor::elf::Elf::load(&elf.0).map_err(Error::DecodeElf)?);

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

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_lambdavm::LambdaVMRustRv64imaCustomized;
    use ere_prover_core::{CommonError, Input, ProverResource, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{
            TestCase, run_zkvm_execute, run_zkvm_execute_estimated_cost, run_zkvm_prove,
            testing_guest_directory,
        },
        program::basic::BasicProgram,
    };

    use crate::{error::Error, prover::LambdaVMProver};

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            LambdaVMRustRv64imaCustomized
                .compile(testing_guest_directory("lambdavm", "basic"), &[])
                .unwrap()
        })
        .clone()
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
    fn test_unsupported_prover_resource() {
        let elf = basic_elf();
        let err = LambdaVMProver::new(elf, ProverResource::Gpu).err().unwrap();
        assert!(matches!(
            err,
            Error::CommonError(CommonError::UnsupportedProverResourceKind { .. })
        ));
    }

    // TODO: Add `test_execute_zkvm_interface` when LambdaVM exports the `zkvm_*`
    // accelerator symbols.
}
