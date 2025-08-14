#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::error::{CompileError, ExecuteError, OpenVMError, VerifyError};
use openvm_build::GuestOptions;
use openvm_circuit::{
    arch::{ContinuationVmProof, instructions::exe::VmExe},
    system::program::trace::VmCommittedExe,
};
use openvm_sdk::{
    F, SC, Sdk, StdIn,
    codec::{Decode, Encode},
    config::{AppConfig, DEFAULT_APP_LOG_BLOWUP, DEFAULT_LEAF_LOG_BLOWUP, SdkVmConfig},
    keygen::AppProvingKey,
};
use openvm_stark_sdk::config::FriParameters;
use openvm_transpiler::{elf::Elf, openvm_platform::memory::MEM_SIZE};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, sync::Arc, time::Instant};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, ProverResourceType,
    zkVM, zkVMError,
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
            Err(Some(code)) => return Err(CompileError::BuildFailed(code).into()),
            Err(None) => return Err(CompileError::BuildSkipped.into()),
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
            toml::from_str(&toml)
                .map_err(|err| CompileError::DeserializeConfigFailed(err.into()))?
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
    app_exe: VmExe<F>,
    app_committed_exe: Arc<VmCommittedExe<SC>>,
    app_pk: Arc<AppProvingKey<SdkVmConfig>>,
    _resource: ProverResourceType,
}

impl EreOpenVM {
    pub fn new(program: OpenVMProgram, _resource: ProverResourceType) -> Result<Self, zkVMError> {
        let sdk = Sdk::new();

        let elf = Elf::decode(&program.elf, MEM_SIZE as u32)
            .map_err(|e| OpenVMError::from(CompileError::DecodeFailed(e.into())))?;

        let app_exe = sdk
            .transpile(elf, program.app_config.app_vm_config.transpiler())
            .map_err(|e| OpenVMError::from(CompileError::TranspileFailed(e.into())))?;

        let app_pk = sdk
            .app_keygen(program.app_config.clone())
            .map_err(|e| zkVMError::Other(e.into()))?;

        let app_committed_exe = sdk
            .commit_app_exe(app_pk.app_fri_params(), app_exe.clone())
            .map_err(|e| zkVMError::Other(e.into()))?;

        Ok(Self {
            app_config: program.app_config,
            app_exe,
            app_committed_exe,
            app_pk: Arc::new(app_pk),
            _resource,
        })
    }
}

impl zkVM for EreOpenVM {
    fn execute(&self, inputs: &Input) -> Result<zkvm_interface::ProgramExecutionReport, zkVMError> {
        let sdk = Sdk::new();

        let mut stdin = StdIn::default();
        serialize_inputs(&mut stdin, inputs);

        let start = Instant::now();
        let _outputs = sdk
            .execute(
                self.app_exe.clone(),
                self.app_config.app_vm_config.clone(),
                stdin,
            )
            .map_err(|e| OpenVMError::from(ExecuteError::Client(e.into())))?;

        Ok(ProgramExecutionReport {
            execution_duration: start.elapsed(),
            ..Default::default()
        })
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), zkVMError> {
        let sdk = Sdk::new();

        let mut stdin = StdIn::default();
        serialize_inputs(&mut stdin, inputs);

        let now = std::time::Instant::now();
        let proof = sdk
            .generate_app_proof(self.app_pk.clone(), self.app_committed_exe.clone(), stdin)
            .unwrap();
        let elapsed = now.elapsed();

        let proof_bytes = proof.encode_to_vec().unwrap();

        Ok((proof_bytes, ProgramProvingReport::new(elapsed)))
    }

    fn verify(&self, mut proof: &[u8]) -> Result<(), zkVMError> {
        let sdk = Sdk::new();

        let proof = ContinuationVmProof::<SC>::decode(&mut proof).unwrap();

        let app_vk = self.app_pk.get_app_vk();
        sdk.verify_app_proof(&app_vk, &proof)
            .map(|_payload| ())
            .map_err(|e| OpenVMError::Verify(VerifyError::Client(e.into())))
            .map_err(zkVMError::from)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
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
    use std::{panic, sync::OnceLock};
    use test_utils::host::{
        BasicProgramInputGen, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };

    static BASIC_PRORGAM: OnceLock<OpenVMProgram> = OnceLock::new();

    fn basic_program() -> OpenVMProgram {
        BASIC_PRORGAM
            .get_or_init(|| {
                OPENVM_TARGET
                    .compile(&testing_guest_directory("openvm", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_compiler_impl() {
        let program = basic_program();
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        let inputs = BasicProgramInputGen::valid();
        run_zkvm_execute(&zkvm, &inputs);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        for inputs in [
            BasicProgramInputGen::empty(),
            BasicProgramInputGen::invalid_string(),
            BasicProgramInputGen::invalid_type(),
        ] {
            zkvm.execute(&inputs).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        let inputs = BasicProgramInputGen::valid();
        run_zkvm_prove(&zkvm, &inputs);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        for inputs_gen in [
            BasicProgramInputGen::empty,
            BasicProgramInputGen::invalid_string,
            BasicProgramInputGen::invalid_type,
        ] {
            panic::catch_unwind(|| zkvm.prove(&inputs_gen()).unwrap_err()).unwrap_err();
        }
    }
}
