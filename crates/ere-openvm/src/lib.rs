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
    fn compile(workspace_path: &Path, guest_relative: &Path) -> Result<Self::Program, Self::Error> {
        let guest_directory = workspace_path.join(guest_relative);
        let pkg = openvm_build::get_package(&guest_directory);
        let guest_opts = GuestOptions::default().with_profile("release".to_string());
        let target_dir = match openvm_build::build_guest_package(&pkg, &guest_opts, None, &None) {
            Ok(target_dir) => target_dir,
            Err(Some(code)) => return Err(CompileError::BuildFailed(code).into()),
            Err(None) => return Err(CompileError::BuildSkipped.into()),
        };

        let elf_path = openvm_build::find_unique_executable(&guest_directory, target_dir, &None)
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
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::Bytes(items) => stdin.write_bytes(items),
            }
        }

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
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::Bytes(items) => stdin.write_bytes(items),
            }
        }

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

#[cfg(test)]
mod tests {
    use zkvm_interface::Compiler;

    use crate::OPENVM_TARGET;

    use super::*;
    use std::path::PathBuf;

    // TODO: for now, we just get one test file
    // TODO: but this should get the whole directory and compile each test
    fn get_compile_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("openvm")
            .join("compile")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/compile/openvm")
    }

    #[test]
    fn test_compile() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program =
            OPENVM_TARGET::compile(&test_guest_path, Path::new("")).expect("compilation failed");
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    #[should_panic]
    fn test_execute_empty_input_panic() {
        // Panics because the program expects input arguments, but we supply none
        let test_guest_path = get_compile_test_guest_program_path();
        let program =
            OPENVM_TARGET::compile(&test_guest_path, Path::new("")).expect("compilation failed");
        let empty_input = Input::new();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        zkvm.execute(&empty_input).unwrap();
    }

    #[test]
    fn test_execute() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program =
            OPENVM_TARGET::compile(&test_guest_path, Path::new("")).expect("compilation failed");
        let mut input = Input::new();
        input.write(10u64);

        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();
        zkvm.execute(&input).unwrap();
    }

    #[test]
    fn test_prove_verify() {
        let test_guest_path = get_compile_test_guest_program_path();
        let program =
            OPENVM_TARGET::compile(&test_guest_path, Path::new("")).expect("compilation failed");
        let mut input = Input::new();
        input.write(10u64);

        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();
        let (proof, _) = zkvm.prove(&input).unwrap();
        zkvm.verify(&proof).expect("proof should verify");
    }
}
