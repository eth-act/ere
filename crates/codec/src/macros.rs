/// Implements `TryFrom<&[u8]>`, `TryFrom<&Vec<u8>>`, and `TryFrom<Vec<u8>>` for
/// `$ty` by delegating to [`Decode`](crate::Decode).
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
/// to [`Encode`](crate::Encode).
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
/// [`Encode`](crate::Encode). Intended for types whose `Encode::Error` is
/// [`Infallible`]; the generated impls `unwrap` the encode result.
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
