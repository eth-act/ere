pub mod compile;
pub mod error;
pub mod io;

use self::error::{ExecuteError, MidenError, VerifyError};
use self::io::{generate_miden_inputs, outputs_to_public_values};
use miden_core::{
    Program,
    utils::{Deserializable, Serializable},
};
use miden_processor::{
    DefaultHost, ExecutionOptions, ProgramInfo, StackInputs, StackOutputs, execute as miden_execute,
};
use miden_prover::{ExecutionProof, ProvingOptions, prove as miden_prove};
use miden_stdlib::StdLibrary;
use miden_verifier::verify as miden_verify;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{env, io::Read, time::Instant};
use zkvm_interface::{
    Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProverResourceType, PublicValues,
    zkVM, zkVMError,
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

#[allow(non_camel_case_types)]
pub struct MIDEN_TARGET;

#[derive(Clone, Serialize, Deserialize)]
pub struct MidenProgram {
    pub program_bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct MidenProofBundle {
    stack_inputs: Vec<u8>,
    stack_outputs: Vec<u8>,
    proof: Vec<u8>,
}

pub struct EreMiden {
    program: Program,
}

impl EreMiden {
    pub fn new(program: MidenProgram, _resource: ProverResourceType) -> Result<Self, MidenError> {
        let program = Program::read_from_bytes(&program.program_bytes)
            .map_err(ExecuteError::ProgramDeserialization)
            .map_err(MidenError::Execute)?;

        Ok(Self { program })
    }

    fn setup_host() -> Result<DefaultHost, MidenError> {
        let mut host = DefaultHost::default();

        host.load_library(&StdLibrary::default())
            .map_err(ExecuteError::Execution)
            .map_err(MidenError::Execute)?;

        Ok(host)
    }
}

impl zkVM for EreMiden {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let (stack_inputs, advice_inputs) = generate_miden_inputs(inputs)?;
        let mut host = Self::setup_host()?;

        let start = Instant::now();
        let trace = miden_execute(
            &self.program,
            stack_inputs,
            advice_inputs,
            &mut host,
            ExecutionOptions::default(),
        )
        .map_err(|e| MidenError::Execute(e.into()))?;

        let public_values = outputs_to_public_values(trace.stack_outputs())
            .map_err(|e| MidenError::Execute(e.into()))?;

        let report = ProgramExecutionReport {
            total_num_cycles: trace.trace_len_summary().main_trace_len() as u64,
            execution_duration: start.elapsed(),
            ..Default::default()
        };

        Ok((public_values, report))
    }

    fn prove(
        &self,
        inputs: &Input,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        let (stack_inputs, advice_inputs) = generate_miden_inputs(inputs)?;
        let mut host = Self::setup_host()?;

        let start = Instant::now();
        let proving_options = ProvingOptions::with_96_bit_security(env::var("MIDEN_DEBUG").is_ok());

        let (stack_outputs, proof) = miden_prove(
            &self.program,
            stack_inputs.clone(),
            advice_inputs,
            &mut host,
            proving_options,
        )
        .map_err(|e| MidenError::Prove(e.into()))?;

        let public_values =
            outputs_to_public_values(&stack_outputs).map_err(|e| MidenError::Prove(e.into()))?;

        let bundle = MidenProofBundle {
            stack_inputs: stack_inputs.to_bytes(),
            stack_outputs: stack_outputs.to_bytes(),
            proof: proof.to_bytes(),
        };

        let proof_bytes = bincode::serialize(&bundle).map_err(|e| MidenError::Prove(e.into()))?;

        Ok((
            public_values,
            proof_bytes,
            ProgramProvingReport::new(start.elapsed()),
        ))
    }

    fn verify(&self, proof: &[u8]) -> Result<PublicValues, zkVMError> {
        let bundle: MidenProofBundle = bincode::deserialize(proof)
            .map_err(|e| MidenError::Verify(VerifyError::BundleDeserialization(e)))?;

        let program_info: ProgramInfo = self.program.clone().into();

        let stack_inputs = StackInputs::read_from_bytes(&bundle.stack_inputs)
            .map_err(|e| MidenError::Verify(VerifyError::MidenDeserialization(e)))?;
        let stack_outputs = StackOutputs::read_from_bytes(&bundle.stack_outputs)
            .map_err(|e| MidenError::Verify(VerifyError::MidenDeserialization(e)))?;
        let execution_proof = ExecutionProof::from_bytes(&bundle.proof)
            .map_err(|e| MidenError::Verify(VerifyError::MidenDeserialization(e)))?;

        miden_verify(
            program_info,
            stack_inputs,
            stack_outputs.clone(),
            execution_proof,
        )
        .map_err(|e| MidenError::Verify(e.into()))?;

        Ok(outputs_to_public_values(&stack_outputs)
            .map_err(|e| MidenError::Verify(VerifyError::BundleDeserialization(e)))?)
    }

    fn deserialize_from<R: Read, T: DeserializeOwned>(&self, reader: R) -> Result<T, zkVMError> {
        bincode::deserialize_from(reader).map_err(|e| MidenError::Execute(e.into()).into())
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
    use super::*;
    use test_utils::host::testing_guest_directory;
    use zkvm_interface::Compiler;

    fn load_miden_program(guest_name: &str) -> MidenProgram {
        MIDEN_TARGET
            .compile(&testing_guest_directory("miden", guest_name))
            .unwrap()
    }

    #[test]
    fn test_prove_and_verify_add() {
        let program = load_miden_program("add");
        let zkvm = EreMiden::new(program, ProverResourceType::Cpu).unwrap();

        let const_a = 2518446814u64;
        let const_b = 1949327098u64;
        let expected_sum = const_a + const_b;

        let mut inputs = Input::new();
        inputs.write(const_a);
        inputs.write(const_b);

        // Prove
        let (prover_public_values, proof, _) = zkvm.prove(&inputs).unwrap();

        // Verify
        let verifier_public_values = zkvm.verify(&proof).unwrap();
        assert_eq!(prover_public_values, verifier_public_values,);

        // Assert output
        let output: Vec<u64> = zkvm
            .deserialize_from(verifier_public_values.as_slice())
            .unwrap();
        assert_eq!(output[0], expected_sum);
    }

    #[test]
    fn test_prove_and_verify_fib() {
        let program = load_miden_program("fib");
        let zkvm = EreMiden::new(program, ProverResourceType::Cpu).unwrap();

        let n_iterations = 50u64;
        let expected_fib = 12_586_269_025u64;

        let mut inputs = Input::new();
        inputs.write(0u64);
        inputs.write(1u64);
        inputs.write(n_iterations);

        // Prove
        let (prover_public_values, proof, _) = zkvm.prove(&inputs).unwrap();

        // Verify
        let verifier_public_values = zkvm.verify(&proof).unwrap();
        assert_eq!(prover_public_values, verifier_public_values,);

        // Assert output
        let output: Vec<u64> = zkvm
            .deserialize_from(verifier_public_values.as_slice())
            .unwrap();
        assert_eq!(output[0], expected_fib);
    }

    #[test]
    fn test_invalid_inputs() {
        let program = load_miden_program("add");
        let zkvm = EreMiden::new(program, ProverResourceType::Cpu).unwrap();

        let empty_inputs = Input::new();
        assert!(zkvm.execute(&empty_inputs).is_err());

        let mut insufficient_inputs = Input::new();
        insufficient_inputs.write(5u64);
        assert!(zkvm.execute(&insufficient_inputs).is_err());
    }
}
