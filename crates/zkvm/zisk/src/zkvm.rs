use crate::zkvm::sdk::{ProgramVk, ZiskSdk};
use anyhow::bail;
use ere_zkvm_interface::Elf;
use ere_zkvm_interface::zkvm::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResource, PublicValues, zkVM, zkVMProgramDigest,
};
use mpi as _; // Import symbols referenced by starks_api.cpp
use std::time::Instant;

mod error;
mod sdk;

pub use error::Error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub struct EreZisk {
    sdk: ZiskSdk,
}

impl EreZisk {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        let sdk = ZiskSdk::new(elf.0, resource)?;
        Ok(Self { sdk })
    }
}

impl zkVM for EreZisk {
    fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        if input.proofs.is_some() {
            bail!(Error::from(CommonError::unsupported_input(
                "no dedicated proofs stream"
            )))
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
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        if input.proofs.is_some() {
            bail!(Error::from(CommonError::unsupported_input(
                "no dedicated proofs stream"
            )))
        }
        if proof_kind != ProofKind::Compressed {
            bail!(Error::from(CommonError::unsupported_proof_kind(
                proof_kind,
                [ProofKind::Compressed]
            )))
        }

        let (public_values, proof, proving_time) = self.sdk.prove(input.stdin())?;

        Ok((
            public_values,
            Proof::Compressed(proof),
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let Proof::Compressed(proof) = proof else {
            bail!(Error::from(CommonError::unsupported_proof_kind(
                proof.kind(),
                [ProofKind::Compressed]
            )))
        };

        Ok(self.sdk.verify(proof)?)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for EreZisk {
    type ProgramDigest = ProgramVk;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(self.sdk.program_vk())
    }
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv64imaCustomized, zkvm::EreZisk};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{
        Elf, RemoteProverConfig,
        compiler::Compiler,
        zkvm::{Input, ProofKind, ProverResource, zkVM},
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
        let zkvm = EreZisk::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let elf = basic_elf();
        let zkvm = EreZisk::new(elf, ProverResource::Cpu).unwrap();

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
        let zkvm = EreZisk::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(not(feature = "cuda"))]
    #[test]
    fn test_prove_invalid_test_case() {
        let _guard = PROVE_LOCK.lock().unwrap();

        let elf = basic_elf();
        let zkvm = EreZisk::new(elf, ProverResource::Cpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
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
        let zkvm = EreZisk::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_invalid_test_case_gpu() {
        let _guard = PROVE_LOCK.lock().unwrap();

        let elf = basic_elf();
        let zkvm = EreZisk::new(elf, ProverResource::Gpu).unwrap();

        for input in [
            Input::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().input(),
        ] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }

        // Should be able to recover
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    #[ignore = "Requires ZisK cluster running"]
    fn test_cluster_prove() {
        let elf = basic_elf();
        let zkvm = EreZisk::new(
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
