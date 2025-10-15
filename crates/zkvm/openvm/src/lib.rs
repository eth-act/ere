#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    compiler::OpenVMProgram,
    error::{CommonError, ExecuteError, OpenVMError, ProveError, VerifyError},
};
use ere_zkvm_interface::{
    Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};
use openvm_circuit::arch::instructions::exe::VmExe;
use openvm_continuations::verifier::internal::types::VmStarkProof;
use openvm_sdk::{
    CpuSdk, F, SC, StdIn,
    codec::{Decode, Encode},
    commit::AppExecutionCommit,
    config::{AppConfig, SdkVmConfig},
    fs::read_object_from_file,
    keygen::{AggProvingKey, AggVerifyingKey, AppProvingKey},
};
use openvm_stark_sdk::openvm_stark_backend::p3_field::PrimeField32;
use openvm_transpiler::{elf::Elf, openvm_platform::memory::MEM_SIZE};
use serde::de::DeserializeOwned;
use std::{env, io::Read, path::PathBuf, sync::Arc, time::Instant};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod compiler;
pub mod error;

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
    pub fn new(program: OpenVMProgram, resource: ProverResourceType) -> Result<Self, zkVMError> {
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

        let sdk = CpuSdk::new(program.app_config.clone()).map_err(CommonError::SdkInit)?;

        let elf = Elf::decode(&program.elf, MEM_SIZE as u32)
            .map_err(|e| CommonError::ElfDecode(e.into()))?;

        let app_exe = sdk.convert_to_exe(elf).map_err(CommonError::Transpile)?;

        let (app_pk, _) = sdk.app_keygen();

        let agg_pk = read_object_from_file::<AggProvingKey, _>(agg_pk_path())
            .map_err(|e| CommonError::ReadAggKeyFailed(e.into()))?;
        let agg_vk = agg_pk.get_agg_vk();

        let _ = sdk.set_agg_pk(agg_pk.clone());

        let app_commit = sdk
            .prover(app_exe.clone())
            .map_err(CommonError::ProverInit)?
            .app_commit();

        Ok(Self {
            app_config: program.app_config,
            app_exe,
            app_pk,
            agg_pk,
            agg_vk,
            app_commit,
            resource,
        })
    }

    fn cpu_sdk(&self) -> Result<CpuSdk, CommonError> {
        let sdk = CpuSdk::new_without_transpiler(self.app_config.clone())
            .map_err(CommonError::SdkInit)?;
        let _ = sdk.set_app_pk(self.app_pk.clone());
        let _ = sdk.set_agg_pk(self.agg_pk.clone());
        Ok(sdk)
    }

    #[cfg(feature = "cuda")]
    fn gpu_sdk(&self) -> Result<openvm_sdk::GpuSdk, CommonError> {
        let sdk = openvm_sdk::GpuSdk::new_without_transpiler(self.app_config.clone())
            .map_err(CommonError::SdkInit)?;
        let _ = sdk.set_app_pk(self.app_pk.clone());
        let _ = sdk.set_agg_pk(self.agg_pk.clone());
        Ok(sdk)
    }
}

impl zkVM for EreOpenVM {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let mut stdin = StdIn::default();
        serialize_inputs(&mut stdin, inputs);

        let start = Instant::now();
        let public_values = self
            .cpu_sdk()?
            .execute(self.app_exe.clone(), stdin)
            .map_err(|e| OpenVMError::from(ExecuteError::Execute(e)))?;

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
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        if proof_kind != ProofKind::Compressed {
            panic!("Only Compressed proof kind is supported.");
        }

        let mut stdin = StdIn::default();
        serialize_inputs(&mut stdin, inputs);

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
        .map_err(|e| OpenVMError::from(ProveError::Prove(e)))?;
        let elapsed = now.elapsed();

        if app_commit != self.app_commit {
            return Err(OpenVMError::from(ProveError::UnexpectedAppCommit {
                preprocessed: self.app_commit,
                proved: app_commit,
            }))?;
        }

        let public_values = extract_public_values(&proof.user_public_values)?;
        let proof_bytes = proof
            .encode_to_vec()
            .map_err(|e| OpenVMError::from(ProveError::SerializeProof(e)))?;

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(elapsed),
        ))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        let Proof::Compressed(proof) = proof else {
            return Err(zkVMError::other("Only Compressed proof kind is supported."));
        };

        let proof = VmStarkProof::<SC>::decode(&mut proof.as_slice())
            .map_err(|e| OpenVMError::from(VerifyError::DeserializeProof(e)))?;

        CpuSdk::verify_proof(&self.agg_vk, self.app_commit, &proof)
            .map_err(|e| OpenVMError::Verify(VerifyError::Verify(e)))?;

        let public_values = extract_public_values(&proof.user_public_values)?;

        Ok(public_values)
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

/// Extract public values in bytes from field elements.
///
/// The public values revealed in guest program will be flatten into `Vec<u8>`
/// then converted to field elements `Vec<F>`, so here we try to downcast it.
fn extract_public_values(user_public_values: &[F]) -> Result<Vec<u8>, CommonError> {
    user_public_values
        .iter()
        .map(|v| u8::try_from(v.as_canonical_u32()).ok())
        .collect::<Option<_>>()
        .ok_or(CommonError::InvalidPublicValue)
}

fn agg_pk_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
        .join(".openvm/agg_stark.pk")
}

#[cfg(test)]
mod tests {
    use crate::{
        EreOpenVM,
        compiler::{OpenVMProgram, RustRv32imaCustomized},
    };
    use ere_test_utils::host::{
        BasicProgramIo, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };
    use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM};
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

        let io = BasicProgramIo::valid().into_output_hashed_io();
        run_zkvm_execute(&zkvm, &io);
    }

    #[test]
    fn test_execute_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

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
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        let io = BasicProgramIo::valid().into_output_hashed_io();
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = EreOpenVM::new(program, ProverResourceType::Cpu).unwrap();

        for inputs in [
            BasicProgramIo::empty(),
            BasicProgramIo::invalid_type(),
            BasicProgramIo::invalid_data(),
        ] {
            zkvm.prove(&inputs, ProofKind::default()).unwrap_err();
        }
    }
}
