use alloc::vec::Vec;
use core::{fmt::Debug, marker::PhantomData};

use ere_codec::{Decode, Encode, impl_codec_by_bincode_legacy, impl_codec_by_ciborium};
use serde::{Deserialize, Serialize};

use crate::{
    codec::{BincodeLegacy, Cbor},
    program::Program,
};

/// The basic program takes `BasicProgramInput` as input, and computes
/// `BasicProgramOutput` as output.
pub struct BasicProgram<C>(PhantomData<C>);

impl<C> Program for BasicProgram<C>
where
    C: Clone + Debug + Send + Sync + PartialEq,
    BasicProgramInput<C>: Encode + Decode,
    BasicProgramOutput<C>: Encode + Decode,
{
    type Input = BasicProgramInput<C>;
    type Output = BasicProgramOutput<C>;

    fn compute(input: BasicProgramInput<C>) -> BasicProgramOutput<C> {
        if input.should_panic {
            panic!("invalid data");
        }
        BasicProgramOutput {
            a: input.a.wrapping_add(1),
            b: input.b.wrapping_add(1),
            c: input.c.wrapping_mul(input.a as u32).wrapping_add(1),
            d: input.d.wrapping_mul(input.b as u64).wrapping_add(1),
            e: input.e.iter().map(|byte| byte.wrapping_add(1)).collect(),
            _marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BasicProgramInput<C> {
    pub should_panic: bool,
    pub a: u8,
    pub b: u16,
    pub c: u32,
    pub d: u64,
    #[serde(with = "serde_bytes")]
    pub e: Vec<u8>,
    #[serde(skip)]
    _marker: PhantomData<C>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BasicProgramOutput<C> {
    #[serde(with = "serde_bytes")]
    pub e: Vec<u8>,
    pub d: u64,
    pub c: u32,
    pub b: u16,
    pub a: u8,
    #[serde(skip)]
    _marker: PhantomData<C>,
}

impl_codec_by_bincode_legacy!(BasicProgramInput<BincodeLegacy>);
impl_codec_by_bincode_legacy!(BasicProgramOutput<BincodeLegacy>);
impl_codec_by_ciborium!(BasicProgramInput<Cbor>);
impl_codec_by_ciborium!(BasicProgramOutput<Cbor>);

#[cfg(feature = "host")]
mod host {
    use core::marker::PhantomData;

    use rand::{Rng, rng};

    use crate::{
        host::ProgramTestCase,
        program::{
            Program,
            basic::{BasicProgram, BasicProgramInput},
        },
    };

    impl<C> BasicProgram<C>
    where
        C: Default,
        Self: Program<Input = BasicProgramInput<C>>,
    {
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
                _marker: PhantomData,
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
