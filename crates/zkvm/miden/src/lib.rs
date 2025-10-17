use crate::{
    compiler::MidenProgram,
    error::{ExecuteError, MidenError, ProveError, VerifyError},
};
use ere_zkvm_interface::{
    ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, ProverResourceType,
    PublicValues, zkVM, zkVMError,
};
use miden_core::{
    Program,
    utils::{Deserializable, Serializable},
};
use miden_processor::{
    DefaultHost, ExecutionOptions, ProgramInfo, StackInputs, StackOutputs, execute as miden_execute,
};
use miden_prover::{AdviceInputs, ExecutionProof, ProvingOptions, prove as miden_prove};
use miden_stdlib::StdLibrary;
use miden_verifier::verify as miden_verify;
use serde::{Deserialize, Serialize};
use std::{env, time::Instant};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod compiler;
pub mod error;

pub use miden_core::{Felt, FieldElement};

#[derive(Serialize, Deserialize)]
struct MidenProofBundle {
    stack_inputs: Vec<u8>,
    stack_outputs: Vec<u8>,
    proof: Vec<u8>,
}

/// [`zkVM`] implementation for Miden.
///
/// Miden VM takes list of field elements as input instead of bytes, so in
/// [`zkVM::execute`] and [`zkVM::prove`] we require the given `input` is built
/// from [`felts_to_bytes`].
/// Similarly, the output values of Miden is also list of field elements, to
/// be compatible with [`zkVM`], we convert it into [`PublicValues`] by
/// [`felts_to_bytes`] as well.
pub struct EreMiden {
    program: Program,
}

impl EreMiden {
    pub fn new(program: MidenProgram, _resource: ProverResourceType) -> Result<Self, MidenError> {
        Ok(Self { program: program.0 })
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
    fn execute(&self, input: &[u8]) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let stack_inputs = StackInputs::default();
        let advice_inputs = AdviceInputs::default().with_stack(
            bytes_to_felts(input)
                .map_err(|err| MidenError::Execute(ExecuteError::InvalidInput(err)))?,
        );
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

        let public_values = felts_to_bytes(trace.stack_outputs().as_slice());

        let report = ProgramExecutionReport {
            total_num_cycles: trace.trace_len_summary().main_trace_len() as u64,
            execution_duration: start.elapsed(),
            ..Default::default()
        };

        Ok((public_values, report))
    }

    fn prove(
        &self,
        input: &[u8],
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMError> {
        if proof_kind != ProofKind::Compressed {
            panic!("Only Compressed proof kind is supported.");
        }

        let stack_inputs = StackInputs::default();
        let advice_inputs = AdviceInputs::default().with_stack(
            bytes_to_felts(input)
                .map_err(|err| MidenError::Prove(ProveError::InvalidInput(err)))?,
        );
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

        let public_values = felts_to_bytes(stack_outputs.as_slice());

        let bundle = MidenProofBundle {
            stack_inputs: stack_inputs.to_bytes(),
            stack_outputs: stack_outputs.to_bytes(),
            proof: proof.to_bytes(),
        };

        let proof_bytes = bincode::serde::encode_to_vec(&bundle, bincode::config::legacy())
            .map_err(|e| MidenError::Prove(e.into()))?;

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(start.elapsed()),
        ))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        let Proof::Compressed(proof) = proof else {
            return Err(zkVMError::other("Only Compressed proof kind is supported."));
        };

        let (bundle, _): (MidenProofBundle, _) =
            bincode::serde::decode_from_slice(proof, bincode::config::legacy())
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

        Ok(felts_to_bytes(stack_outputs.as_slice()))
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

/// Convert Miden field elements into bytes
pub fn felts_to_bytes(felts: &[Felt]) -> Vec<u8> {
    felts
        .iter()
        .flat_map(|felt| felt.as_int().to_le_bytes())
        .collect()
}

/// Convert bytes into Miden field elements.
pub fn bytes_to_felts(bytes: &[u8]) -> Result<Vec<Felt>, String> {
    if bytes.len() % 8 != 0 {
        return Err(format!(
            "Invalid bytes length {}, expected multiple of 8",
            bytes.len()
        ));
    }
    bytes
        .chunks(8)
        .map(|bytes| Felt::try_from(u64::from_le_bytes(bytes.try_into().unwrap())))
        .collect::<Result<Vec<Felt>, _>>()
        .map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use crate::{
        EreMiden, Felt, FieldElement, bytes_to_felts,
        compiler::{MidenAsm, MidenProgram},
        felts_to_bytes,
    };
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM};

    fn load_miden_program(guest_name: &str) -> MidenProgram {
        MidenAsm
            .compile(&testing_guest_directory("miden", guest_name))
            .unwrap()
    }

    #[test]
    fn test_prove_and_verify_add() {
        let program = load_miden_program("add");
        let zkvm = EreMiden::new(program, ProverResourceType::Cpu).unwrap();

        let const_a = -Felt::ONE;
        let const_b = Felt::ONE / Felt::ONE.double();
        let expected_sum = const_a + const_b;

        let input = felts_to_bytes(&[const_a, const_b]);

        // Prove
        let (prover_public_values, proof, _) = zkvm.prove(&input, ProofKind::default()).unwrap();

        // Verify
        let verifier_public_values = zkvm.verify(&proof).unwrap();
        assert_eq!(prover_public_values, verifier_public_values);

        // Assert output
        let output = bytes_to_felts(&verifier_public_values).unwrap();
        assert_eq!(output[0], expected_sum);
    }

    #[test]
    fn test_prove_and_verify_fib() {
        let program = load_miden_program("fib");
        let zkvm = EreMiden::new(program, ProverResourceType::Cpu).unwrap();

        let n_iterations = 50u32;
        let expected_fib = Felt::try_from(12_586_269_025u64).unwrap();

        let input = felts_to_bytes(&[Felt::from(0u32), Felt::from(1u32), Felt::from(n_iterations)]);

        // Prove
        let (prover_public_values, proof, _) = zkvm.prove(&input, ProofKind::default()).unwrap();

        // Verify
        let verifier_public_values = zkvm.verify(&proof).unwrap();
        assert_eq!(prover_public_values, verifier_public_values);

        // Assert output
        let output = bytes_to_felts(&verifier_public_values).unwrap();
        assert_eq!(output[0], expected_fib);
    }

    #[test]
    fn test_invalid_input() {
        let program = load_miden_program("add");
        let zkvm = EreMiden::new(program, ProverResourceType::Cpu).unwrap();

        let empty_inputs = Vec::new();
        assert!(zkvm.execute(&empty_inputs).is_err());

        let insufficient_inputs = felts_to_bytes(&[Felt::from(5u32)]);
        assert!(zkvm.execute(&insufficient_inputs).is_err());
    }
}
