use crate::prover::sdk::AirbenderSdk;
use ere_compiler_core::Elf;
use ere_prover_core::{
    ProverResourceKind,
    prover::{
        CommonError, Input, ProgramExecutionReport, ProgramProvingReport, ProverResource,
        PublicValues, zkVMProver,
    },
};
use ere_verifier_airbender::{AirbenderProof, AirbenderVerifier};
use std::time::Instant;

mod error;
mod sdk;

pub use error::Error;

pub struct AirbenderProver {
    sdk: AirbenderSdk,
    verifier: AirbenderVerifier,
}

impl AirbenderProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        if !matches!(resource, ProverResource::Cpu | ProverResource::Gpu) {
            Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            ))?;
        }
        let sdk = AirbenderSdk::new(&elf, resource.is_gpu())?;
        let verifier = AirbenderVerifier::new(*sdk.program_vk());
        Ok(Self { sdk, verifier })
    }
}

impl zkVMProver for AirbenderProver {
    type Verifier = AirbenderVerifier;
    type Error = Error;

    fn verifier(&self) -> &AirbenderVerifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream"))?;
        }

        let start = Instant::now();
        let (public_values, cycles) = self.sdk.execute(input.stdin())?;
        let execution_duration = start.elapsed();

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles: cycles,
                execution_duration,
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, AirbenderProof, ProgramProvingReport), Error> {
        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream"))?;
        }
        let start = Instant::now();
        let (public_values, proof) = self.sdk.prove(input.stdin())?;
        let proving_time = start.elapsed();

        Ok((
            public_values,
            AirbenderProof(proof),
            ProgramProvingReport::new(proving_time),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::prover::AirbenderProver;
    use ere_compiler_airbender::AirbenderRustRv32ima;
    use ere_compiler_core::{Compiler, Elf};
    use ere_prover_core::prover::{Input, ProverResource, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgram,
    };
    use std::sync::OnceLock;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            AirbenderRustRv32ima
                .compile(testing_guest_directory("airbender", "basic"))
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Cpu).unwrap();

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
        let zkvm = AirbenderProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_gpu() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let elf = basic_elf();
        let zkvm = AirbenderProver::new(elf, ProverResource::Gpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }
}
