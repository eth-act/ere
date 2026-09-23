use core::convert::Infallible;

use ere_verifier_core::codec::{Decode, Encode};
use lambda_vm_executor::elf::Elf;

use crate::Error;

/// Size of the length prefix of the encoded ELF.
const LENGTH_PREFIX_SIZE: usize = size_of::<u64>();

/// Verifying key that identifies a specific compiled guest program.
///
/// LambdaVM has no verifying key separate from the program: the verifier binds the proof to
/// `keccak256(elf)` and recomputes the decode and page commitments from the ELF itself. So the
/// program vk is the ELF bytes.
///
/// Encodes to the ELF length as `u64` little endian, followed by the ELF bytes.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct LambdaVMProgramVk(pub Vec<u8>);

impl Encode for LambdaVMProgramVk {
    type Error = Infallible;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
        let mut bytes = Vec::with_capacity(LENGTH_PREFIX_SIZE + self.0.len());
        bytes.extend_from_slice(&(self.0.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&self.0);
        Ok(bytes)
    }
}

impl Decode for LambdaVMProgramVk {
    type Error = Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
        let Some((prefix, elf)) = slice.split_first_chunk::<LENGTH_PREFIX_SIZE>() else {
            return Err(Error::InvalidProgramVkLength {
                expected: LENGTH_PREFIX_SIZE,
                got: slice.len(),
            });
        };
        let expected = LENGTH_PREFIX_SIZE
            .saturating_add(usize::try_from(u64::from_le_bytes(*prefix)).unwrap_or(usize::MAX));
        if slice.len() != expected {
            return Err(Error::InvalidProgramVkLength {
                expected,
                got: slice.len(),
            });
        }
        Elf::load(elf)?;
        Ok(Self(elf.to_vec()))
    }
}

ere_verifier_core::codec::impl_try_from_bytes_by_decode!(LambdaVMProgramVk);
ere_verifier_core::codec::impl_into_bytes_by_encode!(LambdaVMProgramVk);
