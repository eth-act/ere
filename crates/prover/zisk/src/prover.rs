use std::time::{Duration, Instant};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, CostEstimation, Input, ProverResource, PublicValues, zkVMProver,
};
use ere_verifier_zisk::{ZiskProof, ZiskVerifier};

use crate::{error::Error, sdk::ZiskSdk};

pub struct ZiskProver {
    sdk: ZiskSdk,
    verifier: ZiskVerifier,
}

impl ZiskProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        let sdk = ZiskSdk::new(elf, resource)?;
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

    fn setup(&mut self, elf: Elf) -> Result<(), Error> {
        self.sdk.setup(elf)?;
        self.verifier = ZiskVerifier::new(self.sdk.program_vk());
        Ok(())
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, Duration), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let start = Instant::now();
        let public_values = self.sdk.execute(input)?;
        let execution_duration = start.elapsed();

        Ok((public_values, execution_duration))
    }

    fn execute_estimated_cost(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, CostEstimation), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        self.sdk.execute_estimated_cost(input)
    }

    fn prove(&self, input: &Input) -> Result<(PublicValues, ZiskProof, Duration), Error> {
        if input.proofs.is_some() {
            Err(CommonError::unsupported_input("no dedicated proofs stream"))?
        }

        let (public_values, proof, proving_time) = self.sdk.prove(input)?;

        Ok((public_values, proof, proving_time))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_zisk::ZiskRustRv64imaCustomized;
    use ere_prover_core::{Input, ProverResource, RemoteProverConfig, codec::Encode, zkVMProver};
    use ere_util_test::{
        codec::BincodeLegacy,
        host::{
            TestCase, run_zkvm_execute, run_zkvm_execute_estimated_cost, run_zkvm_prove,
            testing_guest_directory,
        },
        program::{
            basic::BasicProgram,
            zkvm_interface::{self, Accelerator},
        },
    };

    use crate::prover::ZiskProver;

    pub(crate) fn basic_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            ZiskRustRv64imaCustomized
                .compile(testing_guest_directory("zisk", "basic_rust"), &[])
                .unwrap()
        })
        .clone()
    }

    fn zkvm_interface_elf() -> Elf {
        static ELF: OnceLock<Elf> = OnceLock::new();
        ELF.get_or_init(|| {
            ZiskRustRv64imaCustomized
                .compile(testing_guest_directory("zisk", "zkvm_interface"), &[])
                .unwrap()
        })
        .clone()
    }

    /// Switches from the basic program to `zkvm_interface` and back, then runs both again.
    fn run_switchable(zkvm: &mut ZiskProver, prove: bool) {
        let basic_vk = zkvm.program_vk().encode_to_vec().unwrap();
        zkvm.setup(zkvm_interface_elf()).unwrap();
        let zkvm_interface_vk = zkvm.program_vk().encode_to_vec().unwrap();

        zkvm.setup(basic_elf()).unwrap();
        assert_eq!(zkvm.program_vk().encode_to_vec().unwrap(), basic_vk);
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        if prove {
            run_zkvm_prove(&*zkvm, &test_case);
        } else {
            run_zkvm_execute(&*zkvm, &test_case);
        }

        zkvm.setup(zkvm_interface_elf()).unwrap();
        assert_eq!(
            zkvm.program_vk().encode_to_vec().unwrap(),
            zkvm_interface_vk
        );
        let test_case = zkvm_interface::test_cases()
            .into_iter()
            .find(|test_case| test_case.0[0].accelerator == Accelerator::Sha256)
            .unwrap();
        if prove {
            run_zkvm_prove(&*zkvm, &test_case);
        } else {
            run_zkvm_execute(&*zkvm, &test_case);
        }

        zkvm.setup(basic_elf()).unwrap();
    }

    pub(crate) fn basic_elf_zkvm() -> MutexGuard<'static, ZiskProver> {
        static ZKVM: OnceLock<Mutex<ZiskProver>> = OnceLock::new();
        ZKVM.get_or_init(|| {
            let resource = if cfg!(feature = "cuda") {
                ProverResource::Gpu
            } else {
                ProverResource::Cpu
            };
            Mutex::new(ZiskProver::new(basic_elf(), resource).unwrap())
        })
        .lock()
        .unwrap()
    }

    #[test]
    fn test_execute() {
        let zkvm = &*basic_elf_zkvm();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let zkvm = &*basic_elf_zkvm();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_execute_estimated_cost() {
        let zkvm = &*basic_elf_zkvm();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute_estimated_cost(zkvm, &test_case);
    }

    #[test]
    fn test_prove() {
        let zkvm = &*basic_elf_zkvm();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let zkvm = &*basic_elf_zkvm();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            assert!(zkvm.prove(&input).is_err());
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(zkvm, &test_case);
    }

    #[test]
    fn test_execute_switchable() {
        run_switchable(&mut basic_elf_zkvm(), false);
    }

    #[test]
    fn test_prove_switchable() {
        run_switchable(&mut basic_elf_zkvm(), true);
    }

    #[test]
    #[ignore = "Requires ZisK cluster running"]
    fn test_cluster_prove() {
        let elf = basic_elf();
        let zkvm = ZiskProver::new(
            elf,
            ProverResource::Cluster(RemoteProverConfig {
                endpoint: "http://127.0.0.1:7000".to_string(),
                ..Default::default()
            }),
        )
        .unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_zkvm_interface() {
        let elf = ZiskRustRv64imaCustomized
            .compile(testing_guest_directory("zisk", "zkvm_interface"), &[])
            .unwrap();
        let zkvm = ZiskProver::new(elf, ProverResource::Cpu).unwrap();

        for test_case in zkvm_interface::test_cases() {
            run_zkvm_execute(&zkvm, &test_case);
        }
    }
}
