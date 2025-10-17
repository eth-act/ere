use crate::guest::Platform;
use core::fmt::Debug;
use ere_io_serde::IoSerde;
use serde::{Serialize, de::DeserializeOwned};

pub mod basic;

/// Program that can be ran given [`Platform`] implementation.
pub trait Program {
    type Input: Serialize + DeserializeOwned;
    type Output: Debug + PartialEq + Serialize + DeserializeOwned;

    fn io_serde() -> impl IoSerde;

    fn compute(input: Self::Input) -> Self::Output;

    fn run<P: Platform>() {
        let io_serde = Self::io_serde();
        let input = io_serde.deserialize(&P::read_input()).unwrap();
        let output = io_serde.serialize(&Self::compute(input)).unwrap();
        P::write_output(&output);
    }
}

/// [`Program::Input`] that has [`TestCase`] auto-implemented.
pub trait ProgramInput: Clone + Serialize {
    type Program: Program<Input = Self>;
}
