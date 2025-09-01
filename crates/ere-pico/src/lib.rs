#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use pico_sdk::client::DefaultProverClient;
use pico_vm::{configs::stark_config::KoalaBearPoseidon2, emulator::stdin::EmulatorStdinBuilder};
use serde::de::DeserializeOwned;
use std::{io::Read, path::Path, process::Command, time::Instant};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));
mod error;
use error::PicoError;

#[allow(non_camel_case_types)]
pub struct PICO_TARGET;

impl Compiler for PICO_TARGET {
    type Error = PicoError;

    type Program = Vec<u8>;

    fn compile(&self, guest_path: &Path) -> Result<Self::Program, Self::Error> {
        // 1. Check guest path
        if !guest_path.exists() {
            return Err(PicoError::PathNotFound(guest_path.to_path_buf()));
        }

        // 2. Run `cargo pico build`
        let status = Command::new("cargo")
            .current_dir(guest_path)
            .env("RUST_LOG", "info")
            .args(["pico", "build"])
            .status()?; // From<io::Error> → Spawn

        if !status.success() {
            return Err(PicoError::CargoFailed { status });
        }

        // 3. Locate the ELF file
        let elf_path = guest_path.join("elf/riscv32im-pico-zkvm-elf");

        if !elf_path.exists() {
            return Err(PicoError::ElfNotFound(elf_path));
        }

        // 4. Read the ELF file
        let elf_bytes = std::fs::read(&elf_path).map_err(|e| PicoError::ReadElf {
            path: elf_path,
            source: e,
        })?;

        Ok(elf_bytes)
    }
}

pub struct ErePico {
    program: <PICO_TARGET as Compiler>::Program,
}

impl ErePico {
    pub fn new(
        program_bytes: <PICO_TARGET as Compiler>::Program,
        _resource_type: ProverResourceType,
    ) -> Self {
        ErePico {
            program: program_bytes,
        }
    }
}
impl zkVM for ErePico {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let client = DefaultProverClient::new(&self.program);

        let mut stdin = client.new_stdin_builder();
        serialize_inputs(&mut stdin, inputs);

        let start = Instant::now();
        let (total_num_cycles, public_values) = client.emulate(stdin);

        Ok((
            public_values,
            ProgramExecutionReport {
                total_num_cycles,
                execution_duration: start.elapsed(),
                ..Default::default()
            },
        ))
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, zkvm_interface::ProgramProvingReport), zkVMError> {
        let client = DefaultProverClient::new(&self.program);

        let mut stdin = client.new_stdin_builder();
        serialize_inputs(&mut stdin, inputs);

        let now = std::time::Instant::now();
        let meta_proof = client.prove(stdin).expect("Failed to generate proof");
        let elapsed = now.elapsed();

        let mut proof_serialized = Vec::new();
        for p in meta_proof.0.proofs().iter() {
            bincode::serialize_into(&mut proof_serialized, p).unwrap();
        }
        for p in meta_proof.1.proofs().iter() {
            bincode::serialize_into(&mut proof_serialized, p).unwrap();
        }

        for p in meta_proof.0.pv_stream.iter() {
            bincode::serialize_into(&mut proof_serialized, p).unwrap();
        }
        for p in meta_proof.1.pv_stream.iter() {
            bincode::serialize_into(&mut proof_serialized, p).unwrap();
        }

        // TODO: Public values
        let public_values = Vec::new();

        Ok((
            public_values,
            proof_serialized,
            ProgramProvingReport::new(elapsed),
        ))
    }

    fn verify(&self, _proof: &[u8]) -> Result<PublicValues, zkVMError> {
        let client = DefaultProverClient::new(&self.program);
        let _vk = client.riscv_vk();
        // TODO: Verification method missing from sdk
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

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, reader: R) -> Result<T, zkVMError> {
        bincode::deserialize_from(reader).map_err(zkVMError::other)
    }
}

fn serialize_inputs(stdin: &mut EmulatorStdinBuilder<Vec<u8>, KoalaBearPoseidon2>, inputs: &Input) {
    for input in inputs.iter() {
        match input {
            InputItem::Object(serialize) => stdin.write(serialize),
            InputItem::SerializedObject(items) | InputItem::Bytes(items) => {
                stdin.write_slice(items)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{panic, sync::OnceLock};
    use test_utils::host::{BasicProgramIo, run_zkvm_execute, testing_guest_directory};

    static BASIC_PRORGAM: OnceLock<Vec<u8>> = OnceLock::new();

    fn basic_program() -> Vec<u8> {
        BASIC_PRORGAM
            .get_or_init(|| {
                PICO_TARGET
                    .compile(&testing_guest_directory("pico", "basic"))
                    .unwrap()
            })
            .clone()
    }

    #[test]
    fn test_compiler_impl() {
        let elf_bytes = basic_program();
        assert!(!elf_bytes.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        run_zkvm_execute(&zkvm, &io);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        let program = basic_program();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu);

        for inputs_gen in [
            BasicProgramIo::empty,
            BasicProgramIo::invalid_type,
            BasicProgramIo::invalid_data,
        ] {
            panic::catch_unwind(|| zkvm.execute(&inputs_gen()).unwrap_err()).unwrap_err();
        }
    }
}
