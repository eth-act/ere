use core::ops::Deref;

use serde::{Deserialize, Serialize};

/// Public values committed/revealed by guest program.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PublicValues(pub Vec<u8>);

impl Deref for PublicValues {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.0
    }
}

impl AsRef<[u8]> for PublicValues {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl From<&[u8]> for PublicValues {
    fn from(public_values: &[u8]) -> Self {
        Self(public_values.to_vec())
    }
}

impl From<Vec<u8>> for PublicValues {
    fn from(public_values: Vec<u8>) -> Self {
        Self(public_values)
    }
}

impl<const N: usize> From<[u8; N]> for PublicValues {
    fn from(public_values: [u8; N]) -> Self {
        Self(public_values.to_vec())
    }
}

impl From<PublicValues> for Vec<u8> {
    fn from(public_values: PublicValues) -> Vec<u8> {
        public_values.0
    }
}
