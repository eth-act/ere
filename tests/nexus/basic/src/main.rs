#![cfg_attr(target_arch = "riscv32", no_std, no_main)]

extern crate alloc;

use alloc::vec::Vec;
use nexus_rt::{read_private_input, write_public_output};
use serde::{Deserialize, Serialize};

#[nexus_rt::main]
fn main() {
    let input_bytes: Vec<u8> = read_private_input().expect("failed to read input");

    // Deserialize the first input (Vec<u8>)
    let (bytes, remaining): (Vec<u8>, &[u8]) =
        postcard::take_from_bytes(&input_bytes).expect("failed to deserialize bytes");

    // Deserialize the second input (BasicStruct)
    let basic_struct: BasicStruct =
        postcard::from_bytes(remaining).expect("failed to deserialize struct");

    // Check `bytes` length is as expected.
    assert_eq!(bytes.len(), BYTES_LENGTH);

    // Do some computation on `bytes` and `basic_struct`.
    let rev_bytes: Vec<u8> = bytes.iter().rev().copied().collect();
    let basic_struct_output = basic_struct.output();

    // Write `rev_bytes` and `basic_struct_output`
    let mut output_bytes = Vec::new();
    output_bytes.extend_from_slice(&rev_bytes);
    output_bytes.extend_from_slice(&postcard::to_allocvec(&basic_struct_output).unwrap());

    write_public_output(&output_bytes).expect("failed to write output");
}

// Copied from test_utils
// test_utils is not used due to no_std conflicts with sha2 dependency.
const BYTES_LENGTH: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicStruct {
    pub a: u8,
    pub b: u16,
    pub c: u32,
    pub d: u64,
    pub e: Vec<u8>,
}

impl BasicStruct {
    /// Performs some computation (Wrapping add all fields by 1).
    pub fn output(&self) -> Self {
        Self {
            a: self.a.wrapping_add(1),
            b: self.b.wrapping_add(1),
            c: self.c.wrapping_add(1),
            d: self.d.wrapping_add(1),
            e: self.e.iter().map(|byte| byte.wrapping_add(1)).collect(),
        }
    }
}
