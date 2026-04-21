use std::{path::PathBuf, sync::Arc, time::Instant};

use ere_compiler_core::Elf;
use ere_prover_core::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, ProverResource,
    ProverResourceKind, PublicValues, zkVMProver, zkVMVerifier,
};
use ere_verifier_openvm::{OpenVMProgramVk, OpenVMProof, OpenVMVerifier};
use openvm_circuit::arch::instructions::exe::VmExe;
use openvm_sdk::{
    CpuSdk, F, StdIn,
    commit::AppExecutionCommit,
    config::SdkVmConfig,
    fs::read_object_from_file,
    keygen::{AggProvingKey, AppProvingKey},
};

use crate::error::Error;

pub struct OpenVMProver {
    app_exe: Arc<VmExe<F>>,
    app_pk: AppProvingKey<SdkVmConfig>,
    agg_pk: AggProvingKey,
    app_commit: AppExecutionCommit,
    resource: ProverResource,
    verifier: OpenVMVerifier,
}

impl OpenVMProver {
    pub fn new(elf: Elf, resource: ProverResource) -> Result<Self, Error> {
        if !matches!(resource, ProverResource::Cpu | ProverResource::Gpu) {
            Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
            ))?;
        }

        let sdk = CpuSdk::standard();

        let app_exe = sdk.convert_to_exe(elf.0).map_err(Error::Transpile)?;

        let (app_pk, _) = sdk.app_keygen();

        let agg_pk = read_object_from_file::<AggProvingKey, _>(agg_pk_path())
            .map_err(Error::ReadAggKeyFailed)?;

        let _ = sdk.set_agg_pk(agg_pk.clone());

        let app_commit = sdk
            .prover(app_exe.clone())
            .map_err(Error::ProverInit)?
            .app_commit();

        let verifier = OpenVMVerifier::new(OpenVMProgramVk(app_commit));

        Ok(Self {
            app_exe,
            app_pk,
            agg_pk,
            app_commit,
            resource,
            verifier,
        })
    }

    fn cpu_sdk(&self) -> Result<CpuSdk, Error> {
        let sdk = CpuSdk::standard();
        let _ = sdk.set_app_pk(self.app_pk.clone());
        let _ = sdk.set_agg_pk(self.agg_pk.clone());
        Ok(sdk)
    }

    #[cfg(feature = "cuda")]
    fn gpu_sdk(&self) -> Result<openvm_sdk::GpuSdk, Error> {
        let sdk = openvm_sdk::GpuSdk::standard();
        let _ = sdk.set_app_pk(self.app_pk.clone());
        let _ = sdk.set_agg_pk(self.agg_pk.clone());
        Ok(sdk)
    }
}

impl zkVMProver for OpenVMProver {
    type Verifier = OpenVMVerifier;
    type Error = Error;

    fn verifier(&self) -> &OpenVMVerifier {
        &self.verifier
    }

    fn execute(&self, input: &Input) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream"))?;
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input.stdin());

        let start = Instant::now();
        let public_values = self
            .cpu_sdk()?
            .execute(self.app_exe.clone(), stdin)
            .map_err(Error::Execute)?;
        let execution_duration = start.elapsed();

        Ok((
            public_values.into(),
            ProgramExecutionReport {
                execution_duration,
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, OpenVMProof, ProgramProvingReport), Error> {
        if input.proofs.is_some() {
            return Err(CommonError::unsupported_input("no dedicated proofs stream"))?;
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input.stdin());

        let start = Instant::now();
        let (proof, app_commit) = match self.resource {
            ProverResource::Cpu => self.cpu_sdk()?.prove(self.app_exe.clone(), stdin),
            #[cfg(feature = "cuda")]
            ProverResource::Gpu => self.gpu_sdk()?.prove(self.app_exe.clone(), stdin),
            #[cfg(not(feature = "cuda"))]
            ProverResource::Gpu => return Err(Error::CudaFeatureDisabled),
            _ => {
                return Err(CommonError::unsupported_prover_resource_kind(
                    self.resource.kind(),
                    [ProverResourceKind::Cpu, ProverResourceKind::Gpu],
                ))?;
            }
        }
        .map_err(Error::Prove)?;
        let proving_time = start.elapsed();

        if app_commit != self.app_commit {
            return Err(Error::UnexpectedAppCommit {
                preprocessed: self.app_commit.into(),
                proved: app_commit.into(),
            });
        }

        let proof = OpenVMProof(proof);

        // FIXME: Remove this if the `sdk.prove()` above checks exit code.
        let public_values = self.verifier.verify(&proof)?;

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }
}

fn agg_pk_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
        .join(".openvm/agg_stark.pk")
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use ere_compiler_core::{Compiler, Elf};
    use ere_compiler_openvm::OpenVMRustRv32imaCustomized;
    use ere_prover_core::{Input, ProverResource, zkVMProver};
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
                .compile(testing_guest_directory("openvm", "basic"))
                .unwrap()
        })
        .clone()
    }

    #[test]
    fn test_execute() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
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

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
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
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[cfg(feature = "cuda")]
    #[test]
    fn test_prove_gpu() {
        let elf = basic_elf();
        let zkvm = OpenVMProver::new(elf, ProverResource::Gpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
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
        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }
}
