use crate::program::Program;
use alloc::vec::Vec;
use core::panic;
use ere_io_serde::{IoSerde, bincode::Bincode};
use serde::{Deserialize, Serialize};

/// The basic program takes `BasicProgramInput` as input, and computes
/// `BasicProgramOutput` as output.
pub struct BasicProgram;

impl Program for BasicProgram {
    type Input = BasicProgramInput;
    type Output = BasicProgramOutput;

    fn io_serde() -> impl IoSerde {
        Bincode::legacy()
    }

    fn compute(input: BasicProgramInput) -> BasicProgramOutput {
        if input.should_panic {
            panic!("invalid data");
        }
        BasicProgramOutput {
            a: input.a.wrapping_add(1),
            b: input.b.wrapping_add(1),
            c: input.c.wrapping_add(1),
            d: input.d.wrapping_add(1),
            e: input.e.iter().map(|byte| byte.wrapping_add(1)).collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BasicProgramInput {
    pub should_panic: bool,
    pub a: u8,
    pub b: u16,
    pub c: u32,
    pub d: u64,
    pub e: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicProgramOutput {
    pub e: Vec<u8>,
    pub d: u64,
    pub c: u32,
    pub b: u16,
    pub a: u8,
}

#[cfg(feature = "host")]
mod host {
    use crate::{
        host::{OutputHashedTestCase, TestCase},
        program::{
            ProgramInput,
            basic::{BasicProgram, BasicProgramInput},
        },
    };
    use rand::{Rng, rng};
    use sha2::Sha256;

    impl ProgramInput for BasicProgramInput {
        type Program = BasicProgram;
    }

    impl BasicProgramInput {
        pub fn valid() -> Self {
            let mut rng = rng();
            let n = rng.random_range(16..32);
            Self {
                should_panic: false,
                a: rng.random(),
                b: rng.random(),
                c: rng.random(),
                d: rng.random(),
                e: rng.random_iter().take(n).collect(),
            }
        }

        /// Invalid input that causes panic in guest program.
        pub fn invalid() -> Self {
            Self {
                should_panic: true,
                ..Default::default()
            }
        }

        /// Wrap into [`OutputHashedTestCase`] with [`Sha256`].
        pub fn into_output_sha256(self) -> impl TestCase {
            OutputHashedTestCase::<_, Sha256>::new(self)
        }
    }
}
