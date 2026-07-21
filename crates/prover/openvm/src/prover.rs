use ere_compiler_core::Elf;
use ere_prover_core::{
    Input, ProgramExecutionReport, ProgramProvingReport, ProverResource, PublicValues, zkVMProver,
};
use ere_verifier_openvm::{OpenVMProof, OpenVMVerifier, extract_public_values, sdk_vm_config};

use crate::{error::Error, executor::Executor, sdk::OpenVMSdk};

pub struct OpenVMProver {
    sdk: OpenVMSdk,
    executor: Executor,
}

impl OpenVMProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        // Execution stays in process for every resource, so the executor is
        // built from the transpiled guest before the backend is chosen. It
        // needs no proving key, which is what lets cluster mode skip keygen
        // entirely.
        let executor = Executor::new(sdk_vm_config(), &crate::sdk::app_exe(&elf)?)?;
        let sdk = OpenVMSdk::new(elf, resource)?;
        Ok(Self { sdk, executor })
    }
}

impl zkVMProver for OpenVMProver {
    type Verifier = OpenVMVerifier;
    type Error = Error;

    fn verifier(&self) -> &OpenVMVerifier {
        self.sdk.verifier()
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        if input.proofs.is_some() {
            Err(ere_prover_core::CommonError::unsupported_input(
                "no dedicated proofs stream",
            ))?
        }

        self.executor.execute(crate::sdk::stdin(input))
    }

    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, OpenVMProof, ProgramProvingReport), Error> {
        let (proof, proving_time) = self.sdk.prove(input)?;
        let public_values = extract_public_values(&proof.0.user_pvs_proof.public_values)?;

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_openvm::OpenVMRustRv32imaCustomized;
    use ere_prover_core::{Input, ProverResource, RemoteProverConfig, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        program::basic::BasicProgram,
    };

    use crate::prover::OpenVMProver;

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            OpenVMRustRv32imaCustomized
                .compile(testing_guest_directory("openvm", "basic"), &[])
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

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
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

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
    #[ignore = "Requires an Axiom Edge cluster running"]
    fn test_cluster_prove() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(
            elf,
            ProverResource::Cluster(RemoteProverConfig {
                endpoint: "http://127.0.0.1:3000".to_string(),
                ..Default::default()
            }),
        )
        .unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_gpu() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Gpu).unwrap();

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
}
