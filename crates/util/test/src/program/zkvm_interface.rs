//! Program that runs [zkVM accelerator C interface] test vectors and checks each result.
//!
//! Each vector is one crypto call recorded while an Ethereum block executed. The recording sits at
//! the `revm::precompile::Crypto` boundary, so the arguments are already parsed and length checked
//! the way the `zkvm_*` symbols take them.
//!
//! A guest calls the accelerator for every vector and checks the result in place. It commits
//! nothing, so a mismatch fails the execution and the run costs only the accelerator calls. A host
//! build calls nothing, because it only carries the vectors to the guest.
//!
//! [zkVM accelerator C interface]: https://github.com/eth-act/zkvm-standards/blob/main/standards/c-interface-accelerators/zkvm_accelerators.h

use alloc::vec::Vec;

use ere_codec::impl_codec_by_bincode_legacy;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString, IntoEnumIterator, IntoStaticStr};

use crate::program::Program;

// Only a guest defines the accelerator symbols. A host cannot stub them either, because `ziskos`
// already defines them in the ZisK host prover.
#[cfg(target_arch = "riscv64")]
mod guest;

#[cfg(feature = "host")]
mod host;

#[cfg(feature = "host")]
pub use crate::program::zkvm_interface::host::{
    Fixture, FixtureVector, ZkvmInterfaceTestCase, from_hex, test_cases,
};

/// Accelerator one test vector runs, named after the `revm::precompile::Crypto` method that
/// recorded it. The string form is the stem of its fixture file.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Display,
    EnumIter,
    EnumString,
    IntoStaticStr,
)]
#[strum(serialize_all = "snake_case")]
pub enum Accelerator {
    Sha256,
    Ripemd160,
    Bn254G1Add,
    Bn254G1Mul,
    Bn254PairingCheck,
    Secp256k1Ecrecover,
    Modexp,
    Blake2Compress,
    Secp256r1VerifySignature,
    VerifyKzgProof,
    #[strum(serialize = "bls12_381_g1_add")]
    Bls12381G1Add,
    #[strum(serialize = "bls12_381_g1_msm")]
    Bls12381G1Msm,
    #[strum(serialize = "bls12_381_g2_add")]
    Bls12381G2Add,
    #[strum(serialize = "bls12_381_g2_msm")]
    Bls12381G2Msm,
    #[strum(serialize = "bls12_381_pairing_check")]
    Bls12381PairingCheck,
    #[strum(serialize = "bls12_381_fp_to_g1")]
    Bls12381FpToG1,
    #[strum(serialize = "bls12_381_fp2_to_g2")]
    Bls12381Fp2ToG2,
}

impl Accelerator {
    /// Every accelerator.
    pub fn iter() -> impl Iterator<Item = Self> {
        <Self as IntoEnumIterator>::iter()
    }

    /// Byte size of one pair of the packed array in `inputs[0]`. An accelerator that takes no array
    /// returns `None`.
    pub const fn pair_size(self) -> Option<usize> {
        match self {
            Self::Bn254PairingCheck => Some(192),
            Self::Bls12381G1Msm => Some(128),
            Self::Bls12381G2Msm => Some(224),
            Self::Bls12381PairingCheck => Some(288),
            _ => None,
        }
    }
}

/// One recorded call.
///
/// Every argument sits in `inputs`, including the by-value integers, which each take their own
/// little-endian entry. An array argument is one packed blob.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Vector {
    /// Accelerator this test vector runs.
    pub accelerator: Accelerator,
    /// Unpadded arguments, in the order the `revm::precompile::Crypto` method takes them. The
    /// by-value integers come last.
    pub inputs: Vec<Vec<u8>>,
    /// Outcome the reference implementation produced from the same arguments.
    pub expected: Outcome,
}

impl Vector {
    /// Number of pairs of the packed array argument. An accelerator that takes no array returns
    /// `1`.
    pub fn num_pairs(&self) -> usize {
        match self.accelerator.pair_size() {
            Some(pair_size) => {
                assert_eq!(
                    self.inputs[0].len() % pair_size,
                    0,
                    "packed array is ragged"
                );
                self.inputs[0].len() / pair_size
            }
            None => 1,
        }
    }
}

/// Outcome of one accelerator call. A failed call writes nothing, so `output` is `None` exactly
/// when `status` is non-zero. An accelerator that returns a boolean encodes it as one byte.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Outcome {
    /// `0` on success, `-1` on failure.
    pub status: i32,
    /// Output buffer, or `None` when the call failed.
    pub output: Option<Vec<u8>>,
}

impl Outcome {
    /// A failed call writes nothing, so a non-zero `status` drops `output`.
    pub fn new(status: i32, output: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            output: (status == 0).then(|| output.into()),
        }
    }
}

/// Vectors of one accelerator, in the order the guest runs them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vectors(pub Vec<Vector>);

impl_codec_by_bincode_legacy!(Vectors);

/// Runs the test vectors and checks each result against the recorded outcome.
pub struct ZkvmInterfaceProgram;

impl Program for ZkvmInterfaceProgram {
    type Input = Vectors;
    type Output = ();

    fn compute(input: Vectors) -> Self::Output {
        check(&input.0);
    }
}

/// Runs every test vector and panics on the first one that does not match.
#[cfg(target_arch = "riscv64")]
fn check(vectors: &[Vector]) {
    for (index, vector) in vectors.iter().enumerate() {
        assert!(
            guest::run(vector) == vector.expected,
            "{} vector {index} does not match the reference implementation",
            vector.accelerator
        );
    }
}

/// A host only carries the test vectors to the guest, so it runs none of them.
#[cfg(not(target_arch = "riscv64"))]
fn check(_: &[Vector]) {}
