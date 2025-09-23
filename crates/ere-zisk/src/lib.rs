#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    client::{ZiskOptions, ZiskSdk, ZiskServer},
    compile::compile_zisk_program,
    error::ZiskError,
};
use serde::de::DeserializeOwned;
use std::{
    io::Read,
    path::Path,
    sync::{Mutex, MutexGuard},
    time::Instant,
};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

mod client;
mod compile;
mod error;

#[allow(non_camel_case_types)]
pub struct RV64_IMA_ZISK_ZKVM_ELF;

impl Compiler for RV64_IMA_ZISK_ZKVM_ELF {
    type Error = ZiskError;

    type Program = Vec<u8>;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        compile_zisk_program(guest_directory)
    }
}

pub struct EreZisk {
    sdk: ZiskSdk,
    /// Use `Mutex` because the server can only handle signle proving task at a
    /// time.
    ///
    /// Use `Option` inside to lazily initialize only when `prove` is called.
    server: Mutex<Option<ZiskServer>>,
}

impl EreZisk {
    pub fn new(elf: Vec<u8>, resource: ProverResourceType) -> Result<Self, zkVMError> {
        if matches!(resource, ProverResourceType::Network(_)) {
            panic!("Network proving not yet implemented for ZisK. Use CPU or GPU resource type.");
        }
        let sdk = ZiskSdk::new(elf, resource, ZiskOptions::from_env())?;
        Ok(Self {
            sdk,
            server: Mutex::new(None),
        })
    }

    fn server(&'_ self) -> Result<MutexGuard<'_, Option<ZiskServer>>, ZiskError> {
        let mut server = self.server.lock().map_err(|_| ZiskError::MutexPoisoned)?;

        match &mut *server {
            // Recreate the server if it has been created but failed to get status.
            Some(s) => {
                if s.status().is_err() {
                    *server = Some(self.sdk.server()?);
                }
            }
            // Create the server if it has not been created.
            None => {
                *server = Some(self.sdk.server()?);
            }
        }

        // FIXME: Use `MutexGuard::map` to unwrap the inner `Option` when it's stabilized.
        Ok(server)
    }
}

impl zkVM for EreZisk {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let input_bytes = serialize_inputs(inputs)?;

        let start = Instant::now();
        let (public_values, total_num_cycles) = self.sdk.execute(&input_bytes)?;
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
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        let mut server = self.server()?;
        let server = server.as_mut().expect("server initialized");

        let input_bytes = serialize_inputs(inputs)?;

        let start = Instant::now();
        let (public_values, proof) = server.prove(&input_bytes)?;
        let proving_time = start.elapsed();

        Ok((
            public_values,
            proof,
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &[u8]) -> Result<PublicValues, zkVMError> {
        Ok(self.sdk.verify(proof)?)
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, _: R) -> Result<T, zkVMError> {
        unimplemented!("no native serialization in this platform")
    }
}

/// Serialize `Input` into sequence of bytes.
///
/// Because ZisK doesn't provide stdin API so we need to handle multiple inputs,
/// the current approach naively serializes each `InputItem` individually, then
/// concat them into single `Vec<u8>`.
fn serialize_inputs(inputs: &Input) -> Result<Vec<u8>, ZiskError> {
    inputs.iter().try_fold(Vec::new(), |mut acc, item| {
        match item {
            InputItem::Object(obj) => bincode::serialize_into(&mut acc, &**obj)?,
            InputItem::SerializedObject(bytes) => acc.extend(bytes),
            InputItem::Bytes(bytes) => bincode::serialize_into(&mut acc, bytes)?,
        };
        Ok(acc)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use test_utils::host::{
        BasicProgramIo, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };

    /// It fails if multiple servers created concurrently using the same port,
    /// so we have a lock to avoid that.
    static PROVE_LOCK: Mutex<()> = Mutex::new(());

    static BASIC_PROGRAM: OnceLock<Vec<u8>> = OnceLock::new();

    fn basic_program() -> Vec<u8> {
        BASIC_PROGRAM
            .get_or_init(|| {
                RV64_IMA_ZISK_ZKVM_ELF
                    .compile(&testing_guest_directory("zisk", "basic"))
                    .unwrap()
            })
            .to_vec()
    }

    #[test]
    fn test_execute() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let io = BasicProgramIo::valid().into_output_hashed_io();
        run_zkvm_execute(&zkvm, &io);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

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
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        let io = BasicProgramIo::valid().into_output_hashed_io();
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreZisk::new(program, ProverResourceType::Cpu).unwrap();

        let _guard = PROVE_LOCK.lock().unwrap();

        for inputs in [
            BasicProgramIo::empty(),
            BasicProgramIo::invalid_type(),
            BasicProgramIo::invalid_data(),
        ] {
            zkvm.prove(&inputs).unwrap_err();
        }
    }
}
