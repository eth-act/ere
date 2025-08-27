use crate::guest::{BASIC_PROGRAM_BYTES_LENGTH, BasicStruct};
use rand::{Rng, rng};
use std::{fmt::Debug, io::Read, path::PathBuf};
use zkvm_interface::{Input, PublicValues, zkVM};

fn workspace() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

pub fn testing_guest_directory(zkvm_name: &str, program: &str) -> PathBuf {
    workspace().join("tests").join(zkvm_name).join(program)
}

pub trait Io {
    type Output: Debug + PartialEq;

    fn inputs(&self) -> Input;

    fn outputs(&self) -> Self::Output;

    fn deserialize_outputs(&self, zkvm: &impl zkVM, bytes: &[u8]) -> Self::Output;
}

pub fn run_zkvm_execute(zkvm: &impl zkVM, io: &impl Io) -> PublicValues {
    let (public_values, _report) = zkvm
        .execute(&io.inputs())
        .expect("execute should not fail with valid input");

    // TODO: Uncomment when most zkVMs implement the returning of public values:
    // assert_eq!(io.deserialize_outputs(&zkvm, &public_values), io.outputs());

    public_values
}

pub fn run_zkvm_prove(zkvm: &impl zkVM, io: &impl Io) -> PublicValues {
    let (prover_public_values, proof, _report) = zkvm
        .prove(&io.inputs())
        .expect("prove should not fail with valid input");

    let verifier_public_values = zkvm
        .verify(&proof)
        .expect("verify should not fail with valid input");

    assert_eq!(prover_public_values, verifier_public_values);

    // TODO: Uncomment when most zkVMs implement the returning of public values:
    // assert_eq!(io.deserialize_outputs(&zkvm, &verifier_public_values), io.outputs());

    verifier_public_values
}

/// The basic program takes 2 inputs:
/// - `Vec<u8>` - random bytes
/// - [`BasicStruct`] - structure filled with random values
///
/// Commit 2 outputs:
/// - `Vec<u8>` that should be reverse of the input random bytes.
/// - [`BasicStruct`] that should be computed by [`BasicStruct::output`].
#[derive(Clone)]
pub struct BasicProgramIo {
    bytes: Vec<u8>,
    basic_struct: BasicStruct,
}

impl Io for BasicProgramIo {
    type Output = (Vec<u8>, BasicStruct);

    fn inputs(&self) -> Input {
        let mut inputs = Input::new();
        inputs.write_bytes(self.bytes.clone());
        inputs.write(self.basic_struct.clone());
        inputs
    }

    fn outputs(&self) -> Self::Output {
        (
            self.bytes.iter().rev().copied().collect(),
            self.basic_struct.output(),
        )
    }

    fn deserialize_outputs(&self, zkvm: &impl zkVM, mut bytes: &[u8]) -> Self::Output {
        let mut rev_bytes = vec![0; self.bytes.len()];
        bytes.read_exact(&mut rev_bytes).unwrap();
        let basic_struct_output = zkvm.deserialize_from(bytes).unwrap();
        (rev_bytes, basic_struct_output)
    }
}

impl BasicProgramIo {
    pub fn valid() -> Self {
        let rng = &mut rng();
        Self {
            bytes: rng.random_iter().take(BASIC_PROGRAM_BYTES_LENGTH).collect(),
            basic_struct: BasicStruct::random(rng),
        }
    }

    /// Empty input that should trigger deserialization failure in guest
    /// program.
    pub fn empty() -> Input {
        Input::new()
    }

    /// Input with invalid type that should trigger deserialization
    /// failure in guest program.
    pub fn invalid_type() -> Input {
        let mut inputs = Input::new();
        inputs.write(0u64);
        inputs.write_bytes(vec![0, 1, 2, 3]);
        inputs
    }

    /// Input with invalid data that should trigger assertion failure in guest
    /// program.
    pub fn invalid_data() -> Input {
        let mut inputs = Input::new();
        inputs.write_bytes(vec![0; BASIC_PROGRAM_BYTES_LENGTH + 1]);
        inputs.write(BasicStruct::default());
        inputs
    }
}
