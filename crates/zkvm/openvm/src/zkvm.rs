use crate::program::OpenVMProgram;
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMProgramDigest,
};
use openvm_circuit::arch::instructions::exe::VmExe;
use openvm_continuations::verifier::internal::types::VmStarkProof;
use openvm_sdk::{
    CpuSdk, F, SC, StdIn,
    codec::{Decode, Encode},
    commit::AppExecutionCommit,
    config::{AppConfig, DEFAULT_APP_LOG_BLOWUP, DEFAULT_LEAF_LOG_BLOWUP, SdkVmConfig},
    fs::read_object_from_file,
    keygen::{AggProvingKey, AggVerifyingKey, AppProvingKey},
};
use openvm_stark_sdk::{config::FriParameters, openvm_stark_backend::p3_field::PrimeField32};
use openvm_transpiler::{elf::Elf, openvm_platform::memory::MEM_SIZE};
use std::{env, path::PathBuf, sync::Arc, time::Instant};

mod error;

pub use error::Error;

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub struct EreOpenVM {
    app_config: AppConfig<SdkVmConfig>,
    app_exe: Arc<VmExe<F>>,
    app_pk: AppProvingKey<SdkVmConfig>,
    agg_pk: AggProvingKey,
    agg_vk: AggVerifyingKey,
    app_commit: AppExecutionCommit,
    resource: ProverResourceType,
}

impl EreOpenVM {
    pub fn new(program: OpenVMProgram, resource: ProverResourceType) -> Result<Self, Error> {
        match resource {
            #[cfg(not(feature = "cuda"))]
            ProverResourceType::Gpu => {
                panic!("Feature `cuda` is disabled. Enable `cuda` to use GPU resource type")
            }
            ProverResourceType::Network(_) => {
                panic!(
                    "Network proving not yet implemented for OpenVM. Use CPU or GPU resource type."
                );
            }
            _ => {}
        }

        let app_config = if let Some(value) = program.app_config() {
            toml::from_str(value).map_err(Error::InvalidAppConfig)?
        } else {
            // The default `AppConfig` copied from https://github.com/openvm-org/openvm/blob/v1.4.1/crates/cli/src/default.rs#L35.
            AppConfig {
                app_fri_params: FriParameters::standard_with_100_bits_conjectured_security(
                    DEFAULT_APP_LOG_BLOWUP,
                )
                .into(),
                // By default it supports RISCV32IM with IO but no precompiles.
                app_vm_config: SdkVmConfig::builder()
                    .system(Default::default())
                    .rv32i(Default::default())
                    .rv32m(Default::default())
                    .io(Default::default())
                    .build(),
                leaf_fri_params: FriParameters::standard_with_100_bits_conjectured_security(
                    DEFAULT_LEAF_LOG_BLOWUP,
                )
                .into(),
                compiler_options: Default::default(),
            }
        };

        let sdk = CpuSdk::new(app_config.clone()).map_err(Error::SdkInit)?;

        let elf = Elf::decode(program.elf(), MEM_SIZE as u32).map_err(Error::ElfDecode)?;

        let app_exe = sdk.convert_to_exe(elf).map_err(Error::Transpile)?;

        let (app_pk, _) = sdk.app_keygen();

        let agg_pk = read_object_from_file::<AggProvingKey, _>(agg_pk_path())
            .map_err(Error::ReadAggKeyFailed)?;
        let agg_vk = agg_pk.get_agg_vk();

        let _ = sdk.set_agg_pk(agg_pk.clone());

        let app_commit = sdk
            .prover(app_exe.clone())
            .map_err(Error::ProverInit)?
            .app_commit();

        Ok(Self {
            app_config,
            app_exe,
            app_pk,
            agg_pk,
            agg_vk,
            app_commit,
            resource,
        })
    }

    fn cpu_sdk(&self) -> Result<CpuSdk, Error> {
        let sdk =
            CpuSdk::new_without_transpiler(self.app_config.clone()).map_err(Error::SdkInit)?;
        let _ = sdk.set_app_pk(self.app_pk.clone());
        let _ = sdk.set_agg_pk(self.agg_pk.clone());
        Ok(sdk)
    }

    #[cfg(feature = "cuda")]
    fn gpu_sdk(&self) -> Result<openvm_sdk::GpuSdk, Error> {
        let sdk = openvm_sdk::GpuSdk::new_without_transpiler(self.app_config.clone())
            .map_err(Error::SdkInit)?;
        let _ = sdk.set_app_pk(self.app_pk.clone());
        let _ = sdk.set_agg_pk(self.agg_pk.clone());
        Ok(sdk)
    }
}

impl zkVM for EreOpenVM {
    fn execute(&self, input: &[u8]) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let mut stdin = StdIn::default();
        stdin.write_bytes(input);

        let start = Instant::now();
        let public_values = self
            .cpu_sdk()?
            .execute(self.app_exe.clone(), stdin)
            .map_err(Error::Execute)?;

        Ok((
            public_values,
            ProgramExecutionReport {
                execution_duration: start.elapsed(),
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        if proof_kind != ProofKind::Compressed {
            bail!(CommonError::unsupported_proof_kind(
                proof_kind,
                [ProofKind::Compressed]
            ))
        }

        let mut stdin = StdIn::default();
        stdin.write_bytes(input);

        let now = std::time::Instant::now();
        let (proof, app_commit) = match self.resource {
            ProverResourceType::Cpu => self.cpu_sdk()?.prove(self.app_exe.clone(), stdin),
            #[cfg(feature = "cuda")]
            ProverResourceType::Gpu => self.gpu_sdk()?.prove(self.app_exe.clone(), stdin),
            #[cfg(not(feature = "cuda"))]
            ProverResourceType::Gpu => {
                panic!("Feature `cuda` is disabled. Enable `cuda` to use GPU resource type")
            }
            ProverResourceType::Network(_) => {
                panic!(
                    "Network proving not yet implemented for OpenVM. Use CPU or GPU resource type."
                );
            }
        }
        .map_err(Error::Prove)?;
        let elapsed = now.elapsed();

        if app_commit != self.app_commit {
            bail!(Error::UnexpectedAppCommit {
                preprocessed: self.app_commit.into(),
                proved: app_commit.into(),
            });
        }

        let public_values = extract_public_values(&proof.user_public_values)?;
        let proof_bytes = proof
            .encode_to_vec()
            .map_err(|err| CommonError::serialize("proof", "openvm_sdk", err))?;

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(elapsed),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let Proof::Compressed(proof) = proof else {
            bail!(CommonError::unsupported_proof_kind(
                proof.kind(),
                [ProofKind::Compressed]
            ))
        };

        let proof = VmStarkProof::<SC>::decode(&mut proof.as_slice())
            .map_err(|err| CommonError::deserialize("proof", "openvm_sdk", err))?;

        CpuSdk::verify_proof(&self.agg_vk, self.app_commit, &proof).map_err(Error::Verify)?;

        let public_values = extract_public_values(&proof.user_public_values)?;

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for EreOpenVM {
    type ProgramDigest = AppExecutionCommit;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(self.app_commit)
    }
}

/// Extract public values in bytes from field elements.
///
/// The public values revealed in guest program will be flatten into `Vec<u8>`
/// then converted to field elements `Vec<F>`, so here we try to downcast it.
fn extract_public_values(user_public_values: &[F]) -> Result<Vec<u8>, Error> {
    user_public_values
        .iter()
        .map(|v| u8::try_from(v.as_canonical_u32()).ok())
        .collect::<Option<_>>()
        .ok_or(Error::InvalidPublicValue)
}

fn agg_pk_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
        .join(".openvm/agg_stark.pk")
}

#[cfg(test)]
mod tests {
    use crate::{compiler::RustRv32imaCustomized, program::OpenVMProgram, zkvm::EreOpenVM};
    use ere_test_utils::{
        host::{TestCase, run_zkvm_execute, run_zkvm_prove, testing_guest_directory},
        io::serde::bincode::BincodeLegacy,
        program::basic::BasicProgram,
    };
    use ere_zkvm_interface::{
        compiler::Compiler,
        zkvm::{ProofKind, ProverResourceType, zkVM},
    };
    use std::sync::OnceLock;

    fn basic_program() -> OpenVMProgram {
        static PROGRAM: OnceLock<OpenVMProgram> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                RustRv32imaCustomized
                    .compile(&testing_guest_directory("openvm", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_execute(&zkvm, &test_case);
    }

    #[test]
    fn test_execute_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        for input in [
            Vec::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().serialized_input(),
        ] {
            zkvm.execute(&input).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        let test_case = BasicProgram::<BincodeLegacy>::valid_test_case().into_output_sha256();
        run_zkvm_prove(&zkvm, &test_case);
    }

    #[test]
    fn test_prove_invalid_test_case() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        for input in [
            Vec::new(),
            BasicProgram::<BincodeLegacy>::invalid_test_case().serialized_input(),
        ] {
            zkvm.prove(&input, ProofKind::default()).unwrap_err();
        }
    }
}
