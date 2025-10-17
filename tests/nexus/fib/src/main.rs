#![cfg_attr(target_arch = "riscv32", no_std, no_main)]

extern crate alloc;

use alloc::vec::Vec;
use nexus_rt::{read_private_input, write_public_output};
use postcard;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct FibInput {
    n: u32,
}

#[nexus_rt::main]
fn main() {
    let input_bytes: Vec<u8> = read_private_input().expect("failed to read input");

    // Deserialize FibInput from the postcard bytes
    let fib_input: FibInput =
        postcard::from_bytes(&input_bytes).expect("failed to deserialize input");

    let n = fib_input.n;
    let result = fibonacci(n);

    // Serialize result to bytes before writing
    let output_bytes = postcard::to_allocvec(&result).expect("failed to serialize output");

    write_public_output(&output_bytes).expect("failed to write output");
}

fn fibonacci(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    if n == 1 {
        return 1;
    }

    let mut a = 0u32;
    let mut b = 1u32;

    for _ in 2..=n {
        let temp = a.wrapping_add(b);
        a = b;
        b = temp;
    }

    b
}
