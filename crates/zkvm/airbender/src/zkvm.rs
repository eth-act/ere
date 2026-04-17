use crate::zkvm::sdk::AirbenderSdk;
use ere_verifier_airbender::{AirbenderProof, AirbenderVerifier};
use ere_zkvm_interface::{
    ProverResourceKind,
    compiler::Elf,
    zkvm::{
        CommonError, Input, ProgramExecutionReport, ProgramProvingReport, ProverResource,
        PublicValues, zkVM,
    },
};
use std::time::Instant;

mod error;
mod sdk;

pub use error::Error;

pub struct EreAirbender {
    sdk: AirbenderSdk,
    verifier: AirbenderVerifier,
}

impl EreAirbender {
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

impl zkVM for EreAirbender {
    type Verifier = AirbenderVerifier;
    type Error = Error;

    fn verifier(&self) -> &AirbenderVerifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream").into());
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
            return Err(CommonError::unsupported_input("no dedicated proofs stream").into());
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
    use crate::{compiler::RustRv32ima, zkvm::EreAirbender};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{
        compiler::{Compiler, Elf},
        zkvm::{Input, ProverResource, zkVM},
    };
    use std::sync::OnceLock;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            RustRv32ima
                .compile(testing_guest_directory("airbender", "basic"))
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = EreAirbender::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = EreAirbender::new(elf, ProverResource::Cpu).unwrap();

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
        let zkvm = EreAirbender::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = EreAirbender::new(elf, ProverResource::Cpu).unwrap();

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
        let zkvm = EreAirbender::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let elf = basic_elf();
        let zkvm = EreAirbender::new(elf, ProverResource::Gpu).unwrap();

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
