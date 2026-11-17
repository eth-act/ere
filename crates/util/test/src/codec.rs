//! Type-level markers used to select a codec via `PhantomData`, and the macro that implements the
//! codec of `BincodeLegacy`.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BincodeLegacy;

/// Implements `Encode` and `Decode` of `ere_codec` for `$ty` via `bincode::serde` with
/// `bincode::config::legacy()`.
macro_rules! impl_codec_by_bincode_legacy {
    ($ty:ty) => {
        impl ere_codec::Encode for $ty {
            type Error = bincode::error::EncodeError;

            fn encode_to_vec(&self) -> Result<alloc::vec::Vec<u8>, Self::Error> {
                bincode::serde::encode_to_vec(self, bincode::config::legacy())
            }
        }

        impl ere_codec::Decode for $ty {
            type Error = bincode::error::DecodeError;

            fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
                bincode::serde::decode_from_slice(slice, bincode::config::legacy())
                    .map(|(value, _)| value)
            }
        }
    };
}

pub(crate) use impl_codec_by_bincode_legacy;
