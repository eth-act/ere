#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::error::Error;

/// Serializes a value into the canonical byte representation for transport.
pub trait Encode {
    type Error: 'static + Send + Sync + Error;

    fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error>;
}

/// Deserializes a value from its canonical byte representation.
pub trait Decode: Sized {
    type Error: 'static + Send + Sync + Error;

    fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error>;
}

/// Implements `TryFrom<&[u8]>`, `TryFrom<&Vec<u8>>`, and `TryFrom<Vec<u8>>` for
/// `$ty` by delegating to [`Decode`].
#[macro_export]
macro_rules! impl_try_from_bytes_by_decode {
    ($ty:ty) => {
        impl TryFrom<&[u8]> for $ty {
            type Error = <$ty as $crate::Decode>::Error;

            fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
                <$ty as $crate::Decode>::decode_from_slice(slice)
            }
        }

        impl TryFrom<&Vec<u8>> for $ty {
            type Error = <$ty as $crate::Decode>::Error;

            fn try_from(vec: &Vec<u8>) -> Result<Self, Self::Error> {
                <$ty as $crate::Decode>::decode_from_slice(vec.as_slice())
            }
        }

        impl TryFrom<Vec<u8>> for $ty {
            type Error = <$ty as $crate::Decode>::Error;

            fn try_from(vec: Vec<u8>) -> Result<Self, Self::Error> {
                <$ty as $crate::Decode>::decode_from_slice(vec.as_slice())
            }
        }
    };
}

/// Implements `TryFrom<&$ty>` and `TryFrom<$ty>` for `Vec<u8>` by delegating
/// to [`Encode`].
#[macro_export]
macro_rules! impl_try_into_bytes_by_encode {
    ($ty:ty) => {
        impl TryFrom<&$ty> for Vec<u8> {
            type Error = <$ty as $crate::Encode>::Error;

            fn try_from(value: &$ty) -> Result<Self, Self::Error> {
                <$ty as $crate::Encode>::encode_to_vec(value)
            }
        }

        impl TryFrom<$ty> for Vec<u8> {
            type Error = <$ty as $crate::Encode>::Error;

            fn try_from(value: $ty) -> Result<Self, Self::Error> {
                <$ty as $crate::Encode>::encode_to_vec(&value)
            }
        }
    };
}

/// Implements `From<&$ty>` and `From<$ty>` for `Vec<u8>` by delegating to
/// [`Encode`]. Intended for types whose `Encode::Error` is [`Infallible`];
/// the generated impls `unwrap` the encode result.
///
/// [`Infallible`]: core::convert::Infallible
#[macro_export]
macro_rules! impl_into_bytes_by_encode {
    ($ty:ty) => {
        impl From<&$ty> for Vec<u8> {
            fn from(value: &$ty) -> Self {
                let result: Result<_, core::convert::Infallible> =
                    <$ty as $crate::Encode>::encode_to_vec(value);
                result.unwrap()
            }
        }

        impl From<$ty> for Vec<u8> {
            fn from(value: $ty) -> Self {
                let result: Result<_, core::convert::Infallible> =
                    <$ty as $crate::Encode>::encode_to_vec(&value);
                result.unwrap()
            }
        }
    };
}

/// Implements [`Encode`] and [`Decode`] for `$ty` via `bincode::serde` with
/// `bincode::config::legacy()`.
///
/// Requires the `alloc` and `serde` features of `bincode` to be enabled in
/// the caller's `Cargo.toml`.
#[macro_export]
macro_rules! impl_codec_by_bincode_legacy {
    ($ty:ty) => {
        impl $crate::Encode for $ty {
            type Error = bincode::error::EncodeError;

            fn encode_to_vec(&self) -> Result<Vec<u8>, Self::Error> {
                bincode::serde::encode_to_vec(self, bincode::config::legacy())
            }
        }

        impl $crate::Decode for $ty {
            type Error = bincode::error::DecodeError;

            fn decode_from_slice(slice: &[u8]) -> Result<Self, Self::Error> {
                bincode::serde::decode_from_slice(slice, bincode::config::legacy()).map(|(v, _)| v)
            }
        }
    };
}
