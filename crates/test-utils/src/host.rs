use crate::guest::BasicStruct;
use std::path::PathBuf;
use zkvm_interface::{Input, zkVM};

fn workspace() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

pub fn testing_guest_directory(zkvm_name: &str, program: &str) -> PathBuf {
    workspace().join("tests").join(zkvm_name).join(program)
}

pub fn run_zkvm_execute(zkvm: &impl zkVM, inputs: &Input) {
    let _report = zkvm
        .execute(inputs)
        .expect("execute should not fail with valid input");

    // TODO: Check output are expected.
}

pub fn run_zkvm_prove(zkvm: &impl zkVM, inputs: &Input) {
    let (proof, _report) = zkvm
        .prove(inputs)
        .expect("prove should not fail with valid input");

    zkvm.verify(&proof)
        .expect("verify should not fail with valid input");

    // TODO: Check output are expected.
}

/// The basic program takes 2 inputs:
/// - `Vec<u8>` that supposed to be "Hello world"
/// - `BasicStruct`
///
/// Outputs `[u8; 32]` which computed by xoring fields of `BasicStruct`.
pub struct BasicProgramInputGen;

impl BasicProgramInputGen {
    pub fn valid() -> Input {
        let mut inputs = Input::new();
        inputs.write_bytes("Hello world".as_bytes().to_vec());
        inputs.write(BasicStruct {
            a: 0xff,
            b: 0x7777,
            c: 0xffffffff,
            d: 0x7777777777777777,
            e: (0..u8::MAX).collect(),
        });
        inputs
    }

    pub fn invalid_string() -> Input {
        let mut inputs = Input::new();
        inputs.write_bytes("Unexpected string".as_bytes().to_vec());
        inputs.write(BasicStruct::default());
        inputs
    }

    pub fn invalid_type() -> Input {
        let mut inputs = Input::new();
        inputs.write(BasicStruct::default());
        inputs.write_bytes("Hello world".as_bytes().to_vec());
        inputs
    }

    pub fn empty() -> Input {
        Input::new()
    }
}
