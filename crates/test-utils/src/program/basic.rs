use crate::program::Program;
use alloc::vec::Vec;
use core::{marker::PhantomData, panic};
use ere_io::serde::{IoSerde, Serde};
use serde::{Deserialize, Serialize};

/// The basic program takes `BasicProgramInput` as input, and computes
/// `BasicProgramOutput` as output.
pub struct BasicProgram<S>(PhantomData<S>);

impl<S> Program for BasicProgram<S>
where
    S: Serde,
{
    type Io = IoSerde<BasicProgramInput, BasicProgramOutput, S>;

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
    #[serde(with = "serde_bytes")]
    pub e: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicProgramOutput {
    #[serde(with = "serde_bytes")]
    pub e: Vec<u8>,
    pub d: u64,
    pub c: u32,
    pub b: u16,
    pub a: u8,
}

#[cfg(feature = "host")]
mod host {
    use crate::{
        host::ProgramTestCase,
        program::basic::{BasicProgram, BasicProgramInput},
    };
    use ere_io::serde::Serde;
    use rand::{Rng, rng};

    impl<S: Serde> BasicProgram<S> {
        pub fn valid_test_case() -> ProgramTestCase<Self> {
            let mut rng = rng();
            let n = rng.random_range(16..32);
            ProgramTestCase::new(BasicProgramInput {
                should_panic: false,
                a: rng.random(),
                b: rng.random(),
                c: rng.random(),
                d: rng.random(),
                e: rng.random_iter().take(n).collect(),
            })
        }

        /// Invalid input that causes panic in guest program.
        pub fn invalid_test_case() -> ProgramTestCase<Self> {
            ProgramTestCase::new(BasicProgramInput {
                should_panic: true,
                ..Default::default()
            })
        }
    }
}
