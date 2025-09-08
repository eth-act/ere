#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::error::{CompileError, ExecuteError, ProveError, VerifyError, ZirenError};
use cargo_metadata::MetadataCommand;
use serde::de::DeserializeOwned;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};
use tracing::info;
use zkm_sdk::{
    CpuProver, Prover, ZKMProofKind, ZKMProofWithPublicValues, ZKMProvingKey, ZKMStdin,
    ZKMVerifyingKey,
};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));
mod error;

const ZKM_TOOLCHAIN: &str = "zkm";

#[allow(non_camel_case_types)]
pub struct MIPS32R2_ZKM_ZKVM_ELF;

impl Compiler for MIPS32R2_ZKM_ZKVM_ELF {
    type Error = CompileError;

    type Program = Vec<u8>;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let metadata = MetadataCommand::new().current_dir(guest_directory).exec()?;
        let package = metadata
            .root_package()
            .ok_or(CompileError::MissingRootPackage)?;

        let rustc = {
            let output = Command::new("rustc")
                .env("RUSTUP_TOOLCHAIN", ZKM_TOOLCHAIN)
                .args(["--print", "sysroot"])
                .output()
                .map_err(CompileError::RustcSysrootFailed)?;

            if !output.status.success() {
                return Err(CompileError::RustcSysrootExitNonZero {
                    status: output.status,
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }

            PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
                .join("bin")
                .join("rustc")
        };

        // Use `cargo ziren build` instead of using crate `zkm-build`, because
        // it exits if the underlying `cargo build` fails, and there is no way
        // to recover.
        let output = Command::new("cargo")
            .current_dir(guest_directory)
            .env("RUSTC", rustc)
            .env("ZIREN_ZKM_CC", "mipsel-zkm-zkvm-elf-gcc")
            .args(["ziren", "build"])
            .output()
            .map_err(CompileError::CargoZirenBuildFailed)?;

        if !output.status.success() {
            return Err(CompileError::CargoZirenBuildExitNonZero {
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let elf_path = String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| {
                let line = line.strip_prefix("cargo:rustc-env=ZKM_ELF_")?;
                let (package_name, elf_path) = line.split_once("=")?;
                (package_name == package.name).then(|| PathBuf::from(elf_path))
            })
            .ok_or_else(|| CompileError::GuestNotFound {
                name: package.name.clone(),
            })?;

        let elf = fs::read(&elf_path).map_err(|source| CompileError::ReadFile {
            path: elf_path,
            source,
        })?;

        Ok(elf)
    }
}

pub struct EreZiren {
    program: <MIPS32R2_ZKM_ZKVM_ELF as Compiler>::Program,
    pk: ZKMProvingKey,
    vk: ZKMVerifyingKey,
}

impl EreZiren {
    pub fn new(
        program: <MIPS32R2_ZKM_ZKVM_ELF as Compiler>::Program,
        resource: ProverResourceType,
    ) -> Self {
        if matches!(
            resource,
            ProverResourceType::Gpu | ProverResourceType::Network(_)
        ) {
            panic!("Network or Gpu proving not yet implemented for ZKM. Use CPU resource type.");
        }
        let (pk, vk) = CpuProver::new().setup(&program);
        Self { program, pk, vk }
    }
}

impl zkVM for EreZiren {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let mut stdin = ZKMStdin::new();
        serialize_inputs(&mut stdin, inputs);

        let start = Instant::now();
        let (public_inputs, exec_report) = CpuProver::new()
            .execute(&self.program, &stdin)
            .map_err(|err| ZirenError::Execute(ExecuteError::Client(err.into())))?;
        let execution_duration = start.elapsed();

        Ok((
            public_inputs.to_vec(),
            ProgramExecutionReport {
                total_num_cycles: exec_report.total_instruction_count(),
                region_cycles: exec_report.cycle_tracker.into_iter().collect(),
                execution_duration,
            },
        ))
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        info!("Generating proof…");

        let mut stdin = ZKMStdin::new();
        serialize_inputs(&mut stdin, inputs);

        let start = std::time::Instant::now();
        let proof = CpuProver::new()
            .prove(&self.pk, stdin, ZKMProofKind::Compressed)
            .map_err(|err| ZirenError::Prove(ProveError::Client(err.into())))?;
        let proving_time = start.elapsed();

        let bytes = bincode::serialize(&proof)
            .map_err(|err| ZirenError::Prove(ProveError::Bincode(err)))?;

        Ok((
            proof.public_values.to_vec(),
            bytes,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &[u8]) -> Result<PublicValues, zkVMError> {
        info!("Verifying proof…");

        let proof: ZKMProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| ZirenError::Verify(VerifyError::Bincode(err)))?;

        let proof_kind = ZKMProofKind::from(&proof.proof);
        if !matches!(proof_kind, ZKMProofKind::Compressed) {
            return Err(ZirenError::Verify(VerifyError::InvalidProofKind(
                proof_kind,
            )))?;
        }

        CpuProver::new()
            .verify(&proof, &self.vk)
            .map_err(|err| ZirenError::Verify(VerifyError::Client(err.into())))?;

        Ok(proof.public_values.to_vec())
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, reader: R) -> Result<T, zkVMError> {
        bincode::deserialize_from(reader).map_err(zkVMError::other)
    }
}

fn serialize_inputs(stdin: &mut ZKMStdin, inputs: &Input) {
    for input in inputs.iter() {
        match input {
            InputItem::Object(obj) => stdin.write(obj),
            InputItem::SerializedObject(bytes) | InputItem::Bytes(bytes) => {
                stdin.write_slice(bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{panic, sync::OnceLock};
    use test_utils::host::{
        BasicProgramIo, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };

    static BASIC_PROGRAM: OnceLock<Vec<u8>> = OnceLock::new();

    fn basic_program() -> Vec<u8> {
        BASIC_PROGRAM
            .get_or_init(|| {
                MIPS32R2_ZKM_ZKVM_ELF
                    .compile(&testing_guest_directory("ziren", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        type F = fn() -> zkvm_interface::Input;

        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu);

        // Note that for some invalid cases the execution panics, but some not.
        for (inputs_gen, should_panic) in [
            // For empty input (insufficient input), the syscall reading input causes host to panics.
            (BasicProgramIo::empty as F, true),
            // For invalid type/data, the guest panics but handled properly by the host.
            (BasicProgramIo::invalid_type as F, false),
            (BasicProgramIo::invalid_data as F, false),
        ] {
            if should_panic {
                panic::catch_unwind(|| zkvm.execute(&inputs_gen())).unwrap_err();
            } else {
                zkvm.execute(&inputs_gen()).unwrap_err();
            }
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreZiren::new(program, ProverResourceType::Cpu);

        for inputs_gen in [
            BasicProgramIo::empty,
            BasicProgramIo::invalid_type,
            BasicProgramIo::invalid_data,
        ] {
            panic::catch_unwind(|| zkvm.prove(&inputs_gen())).unwrap_err();
        }
    }
}
