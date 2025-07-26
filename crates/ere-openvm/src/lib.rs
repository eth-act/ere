use std::{path::Path, time::Instant};

use openvm_build::GuestOptions;
use openvm_circuit::arch::ContinuationVmProof;
use openvm_sdk::{
    Sdk, StdIn,
    codec::{Decode, Encode},
    config::{AppConfig, SdkVmConfig},
    prover::AppProver,
};
use openvm_stark_sdk::config::{
    FriParameters, baby_bear_poseidon2::BabyBearPoseidon2Config,
    baby_bear_poseidon2::BabyBearPoseidon2Engine,
};
use openvm_transpiler::elf::Elf;
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, ProverResourceType,
    zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));
mod error;
use error::{CompileError, OpenVMError, VerifyError};

#[allow(non_camel_case_types)]
pub struct OPENVM_TARGET;

impl Compiler for OPENVM_TARGET {
    type Error = OpenVMError;

    type Program = Elf;

    fn compile(workspace_path: &Path, guest_relative: &Path) -> Result<Self::Program, Self::Error> {
        let sdk = Sdk::new();

        // Build the guest crate
        let elf: Elf = sdk
            .build(
                GuestOptions::default(),
                workspace_path.join(guest_relative),
                &Default::default(),
            )
            .map_err(|e| CompileError::Client(e.into()))?;
        // TODO: note that this does not transpile (check to see how expensive that is)

        Ok(elf)
    }
}

pub struct EreOpenVM {
    program: <OPENVM_TARGET as Compiler>::Program,
}

impl EreOpenVM {
    pub fn new(
        program: <OPENVM_TARGET as Compiler>::Program,
        _resource_type: ProverResourceType,
    ) -> Self {
        Self { program }
    }
}
impl zkVM for EreOpenVM {
    fn execute(&self, inputs: &Input) -> Result<zkvm_interface::ProgramExecutionReport, zkVMError> {
        let sdk = Sdk::new();
        let vm_cfg = SdkVmConfig::builder()
            .system(Default::default())
            .rv32i(Default::default())
            .rv32m(Default::default())
            .io(Default::default())
            .build();

        let exe = sdk
            .transpile(self.program.clone(), vm_cfg.transpiler())
            .map_err(|e| CompileError::Client(e.into()))
            .map_err(OpenVMError::from)?;

        let mut stdin = StdIn::default();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::Bytes(items) => stdin.write_bytes(items),
            }
        }

        let start = Instant::now();
        let _outputs = sdk
            .execute(exe.clone(), vm_cfg.clone(), stdin)
            .map_err(|e| CompileError::Client(e.into()))
            .map_err(OpenVMError::from)?;

        Ok(ProgramExecutionReport {
            execution_duration: start.elapsed(),
            ..Default::default()
        })
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), zkVMError> {
        // TODO: We need a stateful version in order to not spend a lot of time
        // TODO doing things like computing the pk and vk.

        let sdk = Sdk::new();
        let vm_cfg = SdkVmConfig::builder()
            .system(Default::default())
            .rv32i(Default::default())
            .rv32m(Default::default())
            .io(Default::default())
            .build();

        let app_exe = sdk
            .transpile(self.program.clone(), vm_cfg.transpiler())
            .map_err(|e| CompileError::Client(e.into()))
            .map_err(OpenVMError::from)?;

        let mut stdin = StdIn::default();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::Bytes(items) => stdin.write_bytes(items),
            }
        }

        let app_config = AppConfig::new(FriParameters::standard_fast(), vm_cfg);

        let app_pk = sdk.app_keygen(app_config).unwrap();

        let app_committed_exe = sdk
            .commit_app_exe(app_pk.app_fri_params(), app_exe)
            .unwrap();

        let prover = AppProver::<_, BabyBearPoseidon2Engine>::new(
            app_pk.app_vm_pk.clone(),
            app_committed_exe,
        );
        let now = std::time::Instant::now();
        let proof = prover.generate_app_proof(stdin);
        let elapsed = now.elapsed();

        let proof_bytes = proof.encode_to_vec().unwrap();

        Ok((proof_bytes, ProgramProvingReport::new(elapsed)))
    }

    fn verify(&self, mut proof: &[u8]) -> Result<(), zkVMError> {
        let sdk = Sdk::new();
        let vm_cfg = SdkVmConfig::builder()
            .system(Default::default())
            .rv32i(Default::default())
            .rv32m(Default::default())
            .io(Default::default())
            .build();

        let app_config = AppConfig::new(FriParameters::standard_fast(), vm_cfg);

        let app_pk = sdk.app_keygen(app_config).unwrap();

        let proof = ContinuationVmProof::<BabyBearPoseidon2Config>::decode(&mut proof).unwrap();

        let app_vk = app_pk.get_app_vk();
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
        let elf =
            OPENVM_TARGET::compile(&test_guest_path, Path::new("")).expect("compilation failed");
        assert!(
            !elf.instructions.is_empty(),
            "ELF bytes should not be empty."
        );
    }

    #[test]
    #[should_panic]
    fn test_execute_empty_input_panic() {
        // Panics because the program expects input arguments, but we supply none
        let test_guest_path = get_compile_test_guest_program_path();
        let elf =
            OPENVM_TARGET::compile(&test_guest_path, Path::new("")).expect("compilation failed");
        let empty_input = Input::new();
        let zkvm = EreOpenVM::new(elf, ProverResourceType::Cpu);

        zkvm.execute(&empty_input).unwrap();
    }

    #[test]
    fn test_execute() {
        let test_guest_path = get_compile_test_guest_program_path();
        let elf =
            OPENVM_TARGET::compile(&test_guest_path, Path::new("")).expect("compilation failed");
        let mut input = Input::new();
        input.write(10u64);

        let zkvm = EreOpenVM::new(elf, ProverResourceType::Cpu);
        zkvm.execute(&input).unwrap();
    }

    #[test]
    fn test_prove_verify() {
        let test_guest_path = get_compile_test_guest_program_path();
        let elf =
            OPENVM_TARGET::compile(&test_guest_path, Path::new("")).expect("compilation failed");
        let mut input = Input::new();
        input.write(10u64);

        let zkvm = EreOpenVM::new(elf, ProverResourceType::Cpu);
        let (proof, _) = zkvm.prove(&input).unwrap();
        zkvm.verify(&proof).expect("proof should verify");
    }
}
