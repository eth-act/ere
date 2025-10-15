#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use crate::{
    client::{MetaProof, ProverClient},
    compiler::PicoProgram,
    error::{PicoError, ProveError, VerifyError},
};
use ere_zkvm_interface::{
    Input, InputItem, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind,
    ProverResourceType, PublicValues, zkVM, zkVMError,
};
use pico_p3_field::PrimeField32;
use pico_vm::{configs::stark_config::KoalaBearPoseidon2, emulator::stdin::EmulatorStdinBuilder};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    env,
    io::Read,
    time::{self, Instant},
};

include!(concat!(env!("OUT_DIR"), "/name_and_sdk_version.rs"));

pub mod client;
pub mod compiler;
pub mod error;

#[derive(Serialize, Deserialize)]
pub struct PicoProofWithPublicValues {
    proof: MetaProof,
    public_values: Vec<u8>,
}

pub struct ErePico {
    program: PicoProgram,
}

impl ErePico {
    pub fn new(program: PicoProgram, resource: ProverResourceType) -> Self {
        if !matches!(resource, ProverResourceType::Cpu) {
            panic!("Network or GPU proving not yet implemented for Pico. Use CPU resource type.");
        }
        ErePico { program }
    }

    pub fn client(&self) -> ProverClient {
        ProverClient::new(&self.program)
    }
}

impl zkVM for ErePico {
    fn execute(&self, inputs: &Input) -> Result<(PublicValues, ProgramExecutionReport), zkVMError> {
        let client = self.client();

        let mut stdin = client.new_stdin_builder();
        serialize_inputs(&mut stdin, inputs);

        let start = Instant::now();
        let (total_num_cycles, public_values) = client.execute(stdin);

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
        proof_kind: ProofKind,
    ) -> Result<
        (
            PublicValues,
            Proof,
            ere_zkvm_interface::ProgramProvingReport,
        ),
        zkVMError,
    > {
        if proof_kind != ProofKind::Compressed {
            panic!("Only Compressed proof kind is implemented.");
        }

        let client = self.client();

        let mut stdin = client.new_stdin_builder();
        serialize_inputs(&mut stdin, inputs);

        let now = time::Instant::now();
        let (public_values, proof) = client
            .prove(stdin)
            .map_err(|err| PicoError::Prove(ProveError::Client(err)))?;
        let elapsed = now.elapsed();

        let proof_bytes = bincode::serialize(&PicoProofWithPublicValues {
            proof,
            public_values: public_values.clone(),
        })
        .map_err(|err| PicoError::Prove(ProveError::Bincode(err)))?;

        Ok((
            public_values,
            Proof::Compressed(proof_bytes),
            ProgramProvingReport::new(elapsed),
        ))
    }

    fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMError> {
        let Proof::Compressed(proof) = proof else {
            return Err(zkVMError::other(
                "Only Compressed proof kind is implemented.",
            ));
        };

        let client = self.client();

        let proof: PicoProofWithPublicValues = bincode::deserialize(proof)
            .map_err(|err| PicoError::Verify(VerifyError::Bincode(err)))?;

        client
            .verify(&proof.proof)
            .map_err(|err| PicoError::Verify(VerifyError::Client(err)))?;

        if extract_public_values_sha256_digest(&proof.proof).map_err(PicoError::Verify)?
            != <[u8; 32]>::from(Sha256::digest(&proof.public_values))
        {
            return Err(PicoError::Verify(VerifyError::InvalidPublicValuesDigest))?;
        }

        Ok(proof.public_values)
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

/// Extract public values sha256 digest from base proof of compressed proof.
/// The sha256 digest will be placed at the first 32 field elements of the
/// public values of the only base proof.
fn extract_public_values_sha256_digest(proof: &MetaProof) -> Result<[u8; 32], VerifyError> {
    if proof.proofs().len() != 1 {
        return Err(VerifyError::InvalidBaseProofLength(proof.proofs().len()));
    }

    if proof.proofs()[0].public_values.len() < 32 {
        return Err(VerifyError::InvalidPublicValuesLength(
            proof.proofs()[0].public_values.len(),
        ));
    }

    Ok(proof.proofs()[0].public_values[..32]
        .iter()
        .map(|value| u8::try_from(value.as_canonical_u32()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| VerifyError::InvalidPublicValues)?
        .try_into()
        .unwrap())
}

#[cfg(test)]
mod tests {
    use crate::{
        ErePico,
        compiler::{PicoProgram, RustRv32imaCustomized},
    };
    use ere_test_utils::host::{
        BasicProgramIo, run_zkvm_execute, run_zkvm_prove, testing_guest_directory,
    };
    use ere_zkvm_interface::{Compiler, ProofKind, ProverResourceType, zkVM};
    use std::{panic, sync::OnceLock};

    static BASIC_PROGRAM: OnceLock<PicoProgram> = OnceLock::new();

    fn basic_program() -> PicoProgram {
        BASIC_PROGRAM
            .get_or_init(|| {
                RustRv32imaCustomized
                    .compile(&testing_guest_directory("pico", "basic"))
                    .unwrap()
            })
            .clone()
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
            panic::catch_unwind(|| zkvm.execute(&inputs_gen())).unwrap_err();
        }
    }

    #[test]
    fn test_prove() {
        let program = basic_program();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu);

        let io = BasicProgramIo::valid();
        run_zkvm_prove(&zkvm, &io);
    }

    #[test]
    fn test_prove_invalid_inputs() {
        let program = basic_program();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu);

        for inputs_gen in [
            BasicProgramIo::empty,
            BasicProgramIo::invalid_type,
            BasicProgramIo::invalid_data,
        ] {
            panic::catch_unwind(|| zkvm.prove(&inputs_gen(), ProofKind::default())).unwrap_err();
        }
    }
}
