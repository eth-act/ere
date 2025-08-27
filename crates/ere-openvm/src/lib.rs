#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::error::{CommonError, CompileError, ExecuteError, OpenVMError, ProveError, VerifyError};
use openvm_build::GuestOptions;
use openvm_circuit::arch::instructions::exe::VmExe;
use openvm_continuations::verifier::internal::types::VmStarkProof;
use openvm_sdk::{
    F, SC, Sdk, StdIn,
    codec::{Decode, Encode},
    commit::AppExecutionCommit,
    config::{AppConfig, DEFAULT_APP_LOG_BLOWUP, DEFAULT_LEAF_LOG_BLOWUP, SdkVmConfig},
    keygen::AggVerifyingKey,
};
use openvm_stark_sdk::config::FriParameters;
use openvm_transpiler::{elf::Elf, openvm_platform::memory::MEM_SIZE};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fs, io::Read, path::Path, sync::Arc, time::Instant};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));
mod error;

#[allow(non_camel_case_types)]
pub struct OPENVM_TARGET;

#[derive(Clone, Serialize, Deserialize)]
pub struct OpenVMProgram {
    elf: Vec<u8>,
    app_config: AppConfig<SdkVmConfig>,
}

impl Compiler for OPENVM_TARGET {
    type Error = OpenVMError;

    type Program = OpenVMProgram;

    // Inlining `openvm_sdk::Sdk::build` in order to get raw elf bytes.
    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let pkg = openvm_build::get_package(guest_directory);
        let guest_opts = GuestOptions::default().with_profile("release".to_string());
        let target_dir = match openvm_build::build_guest_package(&pkg, &guest_opts, None, &None) {
            Ok(target_dir) => target_dir,
            Err(Some(code)) => return Err(CompileError::BuildFailed(code))?,
            Err(None) => return Err(CompileError::BuildSkipped)?,
        };

        let elf_path = openvm_build::find_unique_executable(guest_directory, target_dir, &None)
            .map_err(|e| CompileError::UniqueElfNotFound(e.into()))?;
        let elf = fs::read(&elf_path).map_err(|source| CompileError::ReadElfFailed {
            source,
            path: elf_path.to_path_buf(),
        })?;

        let app_config_path = guest_directory.join("openvm.toml");
        let app_config = if app_config_path.exists() {
            let toml = fs::read_to_string(&app_config_path).map_err(|source| {
                CompileError::ReadConfigFailed {
                    source,
                    path: app_config_path.to_path_buf(),
                }
            })?;
            toml::from_str(&toml).map_err(CompileError::DeserializeConfigFailed)?
        } else {
            // The default `AppConfig` copied from https://github.com/openvm-org/openvm/blob/ca36de3/crates/cli/src/default.rs#L31.
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

        Ok(OpenVMProgram { elf, app_config })
    }
}

pub struct EreOpenVM {
    app_config: AppConfig<SdkVmConfig>,
    app_exe: Arc<VmExe<F>>,
    agg_vk: AggVerifyingKey,
    app_commit: AppExecutionCommit,
    _resource: ProverResourceType,
}

impl EreOpenVM {
    pub fn new(program: OpenVMProgram, _resource: ProverResourceType) -> Result<Self, zkVMError> {
        let sdk = Sdk::new(program.app_config.clone()).map_err(CommonError::SdkInit)?;

        let elf = Elf::decode(&program.elf, MEM_SIZE as u32)
            .map_err(|e| CommonError::ElfDecode(e.into()))?;

        let app_exe = sdk.convert_to_exe(elf).map_err(CommonError::Transpile)?;

        let (_, agg_vk) = sdk.agg_keygen().map_err(CommonError::AggKeyGen)?;

        let app_commit = sdk
            .prover(app_exe.clone())
            .map_err(CommonError::ProverInit)?
            .app_commit();

        Ok(Self {
            app_config: program.app_config,
            app_exe,
            agg_vk,
            app_commit,
            _resource,
        })
    }

    fn sdk(&self) -> Result<Sdk, CommonError> {
        Sdk::new_without_transpiler(self.app_config.clone()).map_err(CommonError::SdkInit)
    }
}

impl zkVM for EreOpenVM {
    fn execute(
        &self,
        inputs: &Input,
    ) -> Result<(PublicValues, zkvm_interface::ProgramExecutionReport), zkVMError> {
        let sdk = self
            .sdk()
            .map_err(|e| OpenVMError::from(ExecuteError::from(e)))?;

        let mut stdin = StdIn::default();
        serialize_inputs(&mut stdin, inputs);

        let start = Instant::now();
        let _outputs = sdk
            .execute(self.app_exe.clone(), stdin)
            .map_err(|e| OpenVMError::from(ExecuteError::Execute(e)))?;

        // TODO: Public values
        let public_values = Vec::new();

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
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, zkvm_interface::ProgramProvingReport), zkVMError> {
        let sdk = self
            .sdk()
            .map_err(|e| OpenVMError::from(ProveError::from(e)))?;

        let mut stdin = StdIn::default();
        serialize_inputs(&mut stdin, inputs);

        let now = std::time::Instant::now();
        let (proof, app_commit) = sdk
            .prove(self.app_exe.clone(), stdin)
            .map_err(|e| OpenVMError::from(ProveError::Prove(e)))?;
        let elapsed = now.elapsed();

        if app_commit != self.app_commit {
            return Err(OpenVMError::from(ProveError::UnexpectedAppCommit {
                preprocessed: self.app_commit,
                proved: app_commit,
            }))?;
        }

        let proof_bytes = proof
            .encode_to_vec()
            .map_err(|e| OpenVMError::from(ProveError::SerializeProof(e)))?;

        // TODO: Public values
        let public_values = Vec::new();

        Ok((
            public_values,
            proof_bytes,
            ProgramProvingReport::new(elapsed),
        ))
    }

    fn verify(&self, mut proof: &[u8]) -> Result<PublicValues, zkVMError> {
        let proof = VmStarkProof::<SC>::decode(&mut proof)
            .map_err(|e| OpenVMError::from(VerifyError::DeserializeProof(e)))?;

        Sdk::verify_proof(&self.agg_vk, self.app_commit, &proof)
            .map_err(|e| OpenVMError::Verify(VerifyError::Verify(e)))?;

        // TODO: Public values
        let public_values = Vec::new();

        Ok(public_values)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, _reader: R) -> Result<T, zkVMError> {
        todo!()
    }
}

fn serialize_inputs(stdin: &mut StdIn, inputs: &Input) {
    for input in inputs.iter() {
        match input {
            InputItem::Object(obj) => stdin.write(obj),
            InputItem::SerializedObject(bytes) | InputItem::Bytes(bytes) => {
                stdin.write_bytes(bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;
    use test_utils::host::{
        BasicProgramIo, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };

    fn basic_program() -> OpenVMProgram {
        static PROGRAM: OnceLock<OpenVMProgram> = OnceLock::new();
        PROGRAM
            .get_or_init(|| {
                OPENVM_TARGET
                    .compile(&testing_guest_directory("openvm", "basic"))
                    .unwrap()
            })
            .clone()
    }

    fn basic_program_ere_openvm() -> &'static EreOpenVM {
        static ERE_OPENVM: OnceLock<EreOpenVM> = OnceLock::new();
        ERE_OPENVM.get_or_init(|| EreOpenVM::new(basic_program(), ProverResourceType::Cpu).unwrap())
    }

    #[test]
    fn test_compiler_impl() {
        let program = basic_program();
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let zkvm = basic_program_ere_openvm();

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        let zkvm = basic_program_ere_openvm();

        for inputs in [
            BasicProgramIo::empty(),
            BasicProgramIo::invalid_type(),
            BasicProgramIo::invalid_data(),
        ] {
            zkvm.execute(&inputs).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let zkvm = basic_program_ere_openvm();

        let io = BasicProgramIo::valid();
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let zkvm = basic_program_ere_openvm();

        for inputs in [
            BasicProgramIo::empty(),
            BasicProgramIo::invalid_type(),
            BasicProgramIo::invalid_data(),
        ] {
            zkvm.prove(&inputs).unwrap_err();
        }
    }
}
