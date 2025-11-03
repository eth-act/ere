use core::marker::PhantomData;
use digest::{Digest, Output, OutputSizeUser, generic_array::ArrayLength};

pub use digest;

/// A hasher that given the output, returns a fixed size hash of it.
pub trait OutputHasher: OutputSizeUser {
    fn output_hash(output: &[u8]) -> Output<Self>;
}

/// [`OutputHasher`] implementation that expects the output size to be equal to
/// the fixed size, and returns it as is.
pub struct IdentityOutput<S>(PhantomData<S>);

impl<S: ArrayLength<u8> + 'static> OutputSizeUser for IdentityOutput<S> {
    type OutputSize = S;
}

impl<S: ArrayLength<u8> + 'static> OutputHasher for IdentityOutput<S> {
    fn output_hash(output: &[u8]) -> Output<Self> {
        assert!(
            output.len() == Self::output_size(),
            "output length should be equal to {}",
            Self::output_size()
        );
        let mut hash = Output::<Self>::default();
        hash.copy_from_slice(output);
        hash
    }
}

/// [`OutputHasher`] implementation that expects the output size to be less than
/// or equal to the fixed size, and returns it with 0s padding.
pub struct PaddedOutput<S>(PhantomData<S>);

impl<S: ArrayLength<u8> + 'static> OutputSizeUser for PaddedOutput<S> {
    type OutputSize = S;
}

impl<S: ArrayLength<u8> + 'static> OutputHasher for PaddedOutput<S> {
    fn output_hash(output: &[u8]) -> Output<Self> {
        assert!(
            output.len() <= Self::output_size(),
            "output length should be less than or equal to {}",
            Self::output_size()
        );
        let mut hash = Output::<Self>::default();
        hash[..output.len()].copy_from_slice(output);
        hash
    }
}

impl<D: Digest> OutputHasher for D {
    fn output_hash(output: &[u8]) -> Output<Self> {
        D::digest(output)
    }
}
