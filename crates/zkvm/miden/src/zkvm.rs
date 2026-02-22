use crate::program::{MidenProgram, MidenProgramInfo, MidenSerdeWrapper};
use anyhow::bail;
use ere_zkvm_interface::zkvm::{
    CommonError, Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResource, ProverResourceKind, PublicValues, zkVM, zkVMProgramDigest,
};
use miden_core::{
    field::{PrimeField64, QuotientMap},
    program::Program,
    serde::{Deserializable, Serializable},
};
use miden_core_lib::CoreLibrary;
use miden_processor::{
    DefaultHost, ExecutionOptions, ProgramInfo, StackInputs, StackOutputs,
    execute_sync as miden_execute,
};
use miden_prover::{
    AdviceInputs, ExecutionProof, HashFunction, ProvingOptions, prove_sync as miden_prove,
};
use miden_verifier::verify_with_precompiles as miden_verify;
use std::{env, time::Instant};

mod error;

pub use error::Error;
pub use miden_core::{Felt, field};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

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
    pub fn new(program: MidenProgram, resource: ProverResource) -> Result<Self, Error> {
        if !matches!(resource, ProverResource::Cpu) {
            Err(CommonError::unsupported_prover_resource_kind(
                resource.kind(),
                [ProverResourceKind::Cpu],
            ))?;
        }
        Ok(Self { program: program.0 })
    }

    fn setup_host() -> Result<DefaultHost, Error> {
        let mut host = DefaultHost::default();

        host.load_library(&CoreLibrary::default())
            .map_err(Error::Execute)?;

        Ok(host)
    }
}

impl zkVM for EreMiden {
    fn execute(&self, input: &Input) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        if input.proofs.is_some() {
            bail!(Error::from(CommonError::unsupported_input(
                "no dedicated proofs stream"
            )))
        }

        let stack_inputs = StackInputs::default();
        let advice_inputs = AdviceInputs::default().with_stack(bytes_to_felts(input.stdin())?);
        let mut host = Self::setup_host()?;

        let start = Instant::now();
        let trace = miden_execute(
            &self.program,
            stack_inputs,
            advice_inputs,
            &mut host,
            ExecutionOptions::default(),
        )
        .map_err(Error::Execute)?;

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
        input: &Input,
        proof_kind: ProofKind,
    ) -> anyhow::Result<(PublicValues, Proof, ProgramProvingReport)> {
        if input.proofs.is_some() {
            bail!(Error::from(CommonError::unsupported_input(
                "no dedicated proofs stream"
            )))
        }
        if proof_kind != ProofKind::Compressed {
            bail!(Error::from(CommonError::unsupported_proof_kind(
                proof_kind,
                [ProofKind::Compressed]
            )))
        }

        let stack_inputs = StackInputs::default();
        let advice_inputs = AdviceInputs::default().with_stack(bytes_to_felts(input.stdin())?);
        let mut host = Self::setup_host()?;
        let proving_options = ProvingOptions::new(HashFunction::Blake3_256);

        let start = Instant::now();
        let (stack_outputs, proof) = miden_prove(
            &self.program,
            stack_inputs,
            advice_inputs,
            &mut host,
            proving_options,
        )
        .map_err(Error::Prove)?;
        let proving_time = start.elapsed();

        let public_values = felts_to_bytes(stack_outputs.as_slice());
        let proof_bytes = (stack_outputs, proof).to_bytes();

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(proving_time),
        ))
    }

    fn verify(&self, proof: &Proof) -> anyhow::Result<PublicValues> {
        let Proof::Compressed(proof) = proof else {
            bail!(Error::from(CommonError::unsupported_proof_kind(
                proof.kind(),
                [ProofKind::Compressed]
            )))
        };

        let program_info: ProgramInfo = self.program.clone().into();

        let stack_inputs = StackInputs::default();
        let (stack_outputs, proof): (StackOutputs, ExecutionProof) =
            Deserializable::read_from_bytes(proof)
                .map_err(|err| CommonError::deserialize("proof", "miden", err))?;

        // Security level is hardcoded as 96, so we skip the check.

        let registry = CoreLibrary::default().verifier_registry();
        let (_security_level, _) =
            miden_verify(program_info, stack_inputs, stack_outputs, proof, &registry)
                .map_err(Error::Verify)?;

        Ok(felts_to_bytes(stack_outputs.as_slice()))
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn sdk_version(&self) -> &'static str {
        SDK_VERSION
    }
}

impl zkVMProgramDigest for EreMiden {
    type ProgramDigest = MidenProgramInfo;

    fn program_digest(&self) -> anyhow::Result<Self::ProgramDigest> {
        Ok(MidenSerdeWrapper(self.program.clone().into()))
    }
}

/// Convert Miden field elements into bytes
pub fn felts_to_bytes(felts: &[Felt]) -> Vec<u8> {
    felts
        .iter()
        .flat_map(|felt| felt.as_canonical_u64().to_le_bytes())
        .collect()
}

/// Convert bytes into Miden field elements.
pub fn bytes_to_felts(bytes: &[u8]) -> Result<Vec<Felt>, Error> {
    if !bytes.len().is_multiple_of(8) {
        let err = anyhow::anyhow!(
            "Invalid bytes length {}, expected multiple of 8",
            bytes.len()
        );
        Err(CommonError::serialize("input", "miden", err))?;
    }
    Ok(bytes
        .chunks(8)
        .map(|bytes| Felt::from_canonical_checked(u64::from_le_bytes(bytes.try_into().unwrap())))
        .collect::<Option<Vec<Felt>>>()
        .ok_or_else(|| {
            let err = anyhow::anyhow!(
                "Invalid input bytes. Use ere_miden::zkvm::felts_to_bytes \
                 to convert the field elements into bytes"
            );
            CommonError::serialize("input", "miden", err)
        })?)
}

#[cfg(test)]
mod tests {
    use crate::{
        compiler::MidenAsm,
        program::MidenProgram,
        zkvm::{EreMiden, Felt, bytes_to_felts, felts_to_bytes, field::PrimeCharacteristicRing},
    };
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{
        Input,
        compiler::Compiler,
        zkvm::{ProofKind, ProverResource, zkVM},
    };

    fn load_miden_program(guest_name: &str) -> MidenProgram {
        MidenAsm
            .compile(&testing_guest_directory("miden", guest_name))
            .unwrap()
    }

    #[test]
    fn test_prove_and_verify_add() {
        let program = load_miden_program("add");
        let zkvm = EreMiden::new(program, ProverResource::Cpu).unwrap();

        let const_a = -Felt::ONE;
        let const_b = Felt::ONE / Felt::ONE.double();
        let expected_sum = const_a + const_b;

        let stdin = felts_to_bytes(&[const_a, const_b]);

        // Prove
        let (prover_public_values, proof, _) = zkvm
            .prove(&Input::new().with_stdin(stdin), ProofKind::default())
            .unwrap();

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
        let zkvm = EreMiden::new(program, ProverResource::Cpu).unwrap();

        let n_iterations = 50u32;
        let expected_fib = Felt::new(12_586_269_025u64);

        let stdin = felts_to_bytes(&[Felt::new(0), Felt::new(1), Felt::new(n_iterations as u64)]);

        // Prove
        let (prover_public_values, proof, _) = zkvm
            .prove(&Input::new().with_stdin(stdin), ProofKind::default())
            .unwrap();

        // Verify
        let verifier_public_values = zkvm.verify(&proof).unwrap();
        assert_eq!(prover_public_values, verifier_public_values);

        // Assert output
        let output = bytes_to_felts(&verifier_public_values).unwrap();
        assert_eq!(output[0], expected_fib);
    }

    #[test]
    fn test_invalid_test_case() {
        let program = load_miden_program("add");
        let zkvm = EreMiden::new(program, ProverResource::Cpu).unwrap();

        let empty_inputs = Input::new();
        assert!(zkvm.execute(&empty_inputs).is_err());

        let insufficient_inputs = Input::new().with_stdin(felts_to_bytes(&[Felt::new(5)]));
        assert!(zkvm.execute(&insufficient_inputs).is_err());
    }
}
