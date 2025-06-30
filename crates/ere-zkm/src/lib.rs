#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::time::Instant;

use compile::compile_zkm_program;
use tracing::info;
use zkm_sdk::{
    CpuProver, Prover, ProverClient, ZKMProofWithPublicValues, ZKMProvingKey, ZKMStdin,
    ZKMVerifyingKey,
};
use zkvm_interface::{
    Compiler, Input, InputItem, ProgramExecutionReport, ProgramProvingReport, ProverResourceType,
    zkVM, zkVMError,
};

mod compile;

mod error;
use error::{ExecuteError, ProveError, VerifyError, ZKMError};

enum ProverType {
    Cpu(CpuProver),
}

impl ProverType {
    fn setup(
        &self,
        program: &<RV32_IM_ZKM_ZKVM_ELF as Compiler>::Program,
    ) -> (ZKMProvingKey, ZKMVerifyingKey) {
        match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.setup(program),
            _ => unimplemented!(),
        }
    }

    fn execute(
        &self,
        program: &<RV32_IM_ZKM_ZKVM_ELF as Compiler>::Program,
        input: &ZKMStdin,
    ) -> Result<(zkm_sdk::ZKMPublicValues, zkm_sdk::ExecutionReport), ZKMError> {
        let cpu_executor_builder = match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.execute(program, input),
        };

        cpu_executor_builder
            .run()
            .map_err(|e| ZKMError::Execute(ExecuteError::Client(e.into())))
    }
    fn prove(
        &self,
        pk: &ZKMProvingKey,
        input: &ZKMStdin,
    ) -> Result<ZKMProofWithPublicValues, ZKMError> {
        match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.prove(pk, input).core().run(),
            _ => unimplemented!(),
        }
        .map_err(|e| ZKMError::Prove(ProveError::Client(e.into())))
    }

    fn verify(
        &self,
        proof: &ZKMProofWithPublicValues,
        vk: &ZKMVerifyingKey,
    ) -> Result<(), ZKMError> {
        match self {
            ProverType::Cpu(cpu_prover) => cpu_prover.verify(proof, vk),
            _ => unimplemented!(),
        }
        .map_err(|e| ZKMError::Verify(VerifyError::Client(e.into())))
    }
}

#[allow(non_camel_case_types)]
pub struct RV32_IM_ZKM_ZKVM_ELF;
pub struct EreZKM {
    program: <RV32_IM_ZKM_ZKVM_ELF as Compiler>::Program,
    /// Proving key
    pk: ZKMProvingKey,
    /// Verification key
    vk: ZKMVerifyingKey,
    /// Proof and Verification orchestrator
    client: ProverType,
}

impl Compiler for RV32_IM_ZKM_ZKVM_ELF {
    type Error = ZKMError;

    type Program = Vec<u8>;

    fn compile(path_to_program: &std::path::Path) -> Result<Self::Program, Self::Error> {
        compile_zkm_program(path_to_program).map_err(ZKMError::from)
    }
}

impl EreZKM {
    pub fn new(
        program: <RV32_IM_ZKM_ZKVM_ELF as Compiler>::Program,
        resource: ProverResourceType,
    ) -> Self {
        let client = match resource {
            ProverResourceType::Cpu => ProverType::Cpu(ProverClient::builder().build()),
            _ => unimplemented!(),
        };
        let (pk, vk) = client.setup(&program);

        Self {
            program,
            client,
            pk,
            vk,
        }
    }
}

impl zkVM for EreZKM {
    fn execute(&self, inputs: &Input) -> Result<zkvm_interface::ProgramExecutionReport, zkVMError> {
        let mut stdin = ZKMStdin::new();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::Bytes(items) => stdin.write_slice(items),
            }
        }

        let start = Instant::now();
        let (_, exec_report) = self.client.execute(&self.program, &stdin)?;
        Ok(ProgramExecutionReport {
            total_num_cycles: exec_report.total_instruction_count(),
            region_cycles: exec_report.cycle_tracker.into_iter().collect(),
            execution_duration: start.elapsed(),
        })
    }

    fn prove(
        &self,
        inputs: &zkvm_interface::Input,
    ) -> Result<(Vec<u8>, zkvm_interface::ProgramProvingReport), zkVMError> {
        info!("Generating proof…");

        let mut stdin = ZKMStdin::new();
        for input in inputs.iter() {
            match input {
                InputItem::Object(serialize) => stdin.write(serialize),
                InputItem::Bytes(items) => stdin.write_slice(items),
            };
        }

        let start = std::time::Instant::now();
        let proof_with_inputs = self.client.prove(&self.pk, &stdin)?;
        let proving_time = start.elapsed();

        let bytes = bincode::serialize(&proof_with_inputs)
            .map_err(|err| ZKMError::Prove(ProveError::Bincode(err)))?;

        Ok((bytes, ProgramProvingReport::new(proving_time)))
    }

    fn verify(&self, proof: &[u8]) -> Result<(), zkVMError> {
        info!("Verifying proof…");

        let proof: ZKMProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| ZKMError::Verify(VerifyError::Bincode(err)))?;

        self.client
            .verify(&proof, &self.vk)
            .map_err(zkVMError::from)
    }
}

#[cfg(test)]
mod execute_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_compiled_test_ZKM_elf() -> Result<Vec<u8>, ZKMError> {
        let test_guest_path = get_execute_test_guest_program_path();
        RV32_IM_ZKM_ZKVM_ELF::compile(&test_guest_path)
    }

    fn get_execute_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("zkm")
            .join("execute")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/ZKM")
    }

    #[test]
    fn test_execute_ZKM_dummy_input() {
        let elf_bytes = get_compiled_test_ZKM_elf()
            .expect("Failed to compile test ZKM guest for execution test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreZKM::new(elf_bytes, ProverResourceType::Cpu);

        let result = zkvm.execute(&input_builder);

        if let Err(e) = &result {
            panic!("Execution error: {:?}", e);
        }
    }

    #[test]
    fn test_execute_ZKM_no_input_for_guest_expecting_input() {
        let elf_bytes = get_compiled_test_ZKM_elf()
            .expect("Failed to compile test ZKM guest for execution test");

        let empty_input = Input::new();

        let zkvm = EreZKM::new(elf_bytes, ProverResourceType::Cpu);
        let result = zkvm.execute(&empty_input);

        assert!(
            result.is_err(),
            "execute should fail if guest expects input but none is provided."
        );
    }
}

#[cfg(test)]
mod prove_tests {
    use std::path::PathBuf;

    use super::*;
    use zkvm_interface::Input;

    fn get_prove_test_guest_program_path() -> PathBuf {
        let workspace_dir = env!("CARGO_WORKSPACE_DIR");
        PathBuf::from(workspace_dir)
            .join("tests")
            .join("zkm")
            .join("prove")
            .join("basic")
            .canonicalize()
            .expect("Failed to find or canonicalize test guest program at <CARGO_WORKSPACE_DIR>/tests/execute/ZKM")
    }

    fn get_compiled_test_ZKM_elf_for_prove() -> Result<Vec<u8>, ZKMError> {
        let test_guest_path = get_prove_test_guest_program_path();
        RV32_IM_ZKM_ZKVM_ELF::compile(&test_guest_path)
    }

    #[test]
    fn test_prove_ZKM_dummy_input() {
        let elf_bytes = get_compiled_test_ZKM_elf_for_prove()
            .expect("Failed to compile test ZKM guest for proving test");

        let mut input_builder = Input::new();
        let n: u32 = 42;
        let a: u16 = 42;
        input_builder.write(n);
        input_builder.write(a);

        let zkvm = EreZKM::new(elf_bytes, ProverResourceType::Cpu);

        let proof_bytes = match zkvm.prove(&input_builder) {
            Ok((prove_result, _)) => prove_result,
            Err(err) => {
                panic!("Proving error in test: {:?}", err);
            }
        };

        assert!(!proof_bytes.is_empty(), "Proof bytes should not be empty.");

        let verify_results = zkvm.verify(&proof_bytes).is_ok();
        assert!(verify_results);

        // TODO: Check public inputs
    }

    #[test]
    #[should_panic]
    fn test_prove_ZKM_fails_on_bad_input_causing_execution_failure() {
        let elf_bytes = get_compiled_test_ZKM_elf_for_prove()
            .expect("Failed to compile test ZKM guest for proving test");

        let empty_input = Input::new();

        let zkvm = EreZKM::new(elf_bytes, ProverResourceType::Cpu);
        let prove_result = zkvm.prove(&empty_input);
        assert!(prove_result.is_err())
    }
}
