use crate::prover::sdk::ZiskSdk;
use ere_prover_core::compiler::Elf;
use ere_prover_core::prover::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, ProverResource, PublicValues,
    zkVMProver,
};
use ere_verifier_zisk::{ZiskProof, ZiskVerifier};
use mpi as _; // Import symbols referenced by starks_api.cpp
use std::time::Instant;

mod error;
mod sdk;

pub use error::Error;

pub struct ZiskProver {
    sdk: ZiskSdk,
    verifier: ZiskVerifier,
}

impl ZiskProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        let sdk = ZiskSdk::new(elf.0, resource)?;
        let verifier = ZiskVerifier::new(sdk.program_vk());
        Ok(Self { sdk, verifier })
    }
}

impl zkVMProver for ZiskProver {
    type Verifier = ZiskVerifier;
    type Error = Error;

    fn verifier(&self) -> &ZiskVerifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream"))?;
        }

        let start = Instant::now();
        let (public_values, total_num_cycles) = self.sdk.execute(input.stdin())?;
        let execution_duration = start.elapsed();

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles,
                execution_duration,
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, ZiskProof, ProgramProvingReport), Error> {
        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream"))?;
        }

        let (public_values, proof, proving_time) = self.sdk.prove(input.stdin())?;

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv64imaCustomized, prover::ZiskProver};
    use ere_prover_core::{
        RemoteProverConfig,
        compiler::{Compiler, Elf},
        prover::{Input, ProverResource, zkVMProver},
    };
    use ere_util_test::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use std::sync::{Mutex, OnceLock};

    static PROVE_LOCK: Mutex<()> = Mutex::new(());

    fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            RustRv64imaCustomized
                .compile(testing_guest_directory("zisk", "basic_rust"))
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = ZiskProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = ZiskProver::new(elf, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove() {
        let _guard = PROVE_LOCK.lock().unwrap();

        let elf = basic_elf();
        let zkvm = ZiskProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove_invalid_test_case() {
        let _guard = PROVE_LOCK.lock().unwrap();

        let elf = basic_elf();
        let zkvm = ZiskProver::new(elf, ProverResource::Cpu).unwrap();

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
        let _guard = PROVE_LOCK.lock().unwrap();

        let elf = basic_elf();
        let zkvm = ZiskProver::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let _guard = PROVE_LOCK.lock().unwrap();

        let elf = basic_elf();
        let zkvm = ZiskProver::new(elf, ProverResource::Gpu).unwrap();

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
    #[ignore = "Requires ZisK cluster running"]
    fn test_cluster_prove() {
        let elf = basic_elf();
        let zkvm = ZiskProver::new(
            elf,
            ProverResource::Cluster(RemoteProverConfig {
                endpoint: "http://127.0.0.1:50051".to_string(),
                ..Default::default()
            }),
        )
        .unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }
}
