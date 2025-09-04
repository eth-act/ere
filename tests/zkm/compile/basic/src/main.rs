//! A simple program that takes a number `n` as input, and writes the `n-1`th and `n`th fibonacci
//! number as an output.

// These two lines are necessary for the program to properly compile.
//
// Under the hood, we wrap your main function with some extra code so that it behaves properly
// inside the zkVM.
#![no_std]
#![no_main]
zkm_zkvm::entrypoint!(main);

pub fn main() {
    // Read an input to the program.
    let n = zkm_zkvm::io::read::<u32>();
    // Write n*2 to output
    zkm_zkvm::io::commit(&(n * 2));
}
