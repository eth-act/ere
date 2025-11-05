use core::{marker::PhantomData, ops::Deref};
use digest::{
    Digest, Output, OutputSizeUser,
    generic_array::{ArrayLength, GenericArray},
};

pub use digest;

/// A hasher that given the output, returns a hash of it.
pub trait OutputHasher {
    type Hash<'a>: Deref<Target = [u8]>;

    fn output_hash(output: &[u8]) -> Self::Hash<'_>;
}

/// A hasher that given the output, returns a fixed-size hash of it.
pub trait FixedOutputHasher: OutputHasher + OutputSizeUser {}

impl<T: OutputHasher + OutputSizeUser> FixedOutputHasher for T {}

/// A marker used to mark [`IdentityOutput`] to accept unsized output.
pub struct Unsized;

/// [`OutputHasher`] implementation that returns output as is.
///
/// By setting generic `U = Unsized` it takes output with any size.
///
/// By setting generic `U = typenum::U{SIZE}` it expects the output to match
/// the `SIZE`.
pub struct IdentityOutput<U = Unsized>(PhantomData<U>);

impl OutputHasher for IdentityOutput<Unsized> {
    type Hash<'a> = &'a [u8];

    fn output_hash(output: &[u8]) -> Self::Hash<'_> {
        output
    }
}

impl<U: ArrayLength<u8> + 'static> OutputSizeUser for IdentityOutput<U> {
    type OutputSize = U;
}

impl<U: ArrayLength<u8> + 'static> OutputHasher for IdentityOutput<U> {
    type Hash<'a> = GenericArray<u8, U>;

    fn output_hash(output: &[u8]) -> Self::Hash<'_> {
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

/// [`OutputHasher`] implementation that returns output with 0s padding.
///
/// By setting generic `U = typenum::U{SIZE}` it expects the output to be less
/// than or equal to the `SIZE`.
pub struct PaddedOutput<U>(PhantomData<U>);

impl<U: ArrayLength<u8> + 'static> OutputSizeUser for PaddedOutput<U> {
    type OutputSize = U;
}

impl<U: ArrayLength<u8> + 'static> OutputHasher for PaddedOutput<U> {
    type Hash<'a> = GenericArray<u8, U>;

    fn output_hash(output: &[u8]) -> Self::Hash<'_> {
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
    type Hash<'a> = Output<D>;

    fn output_hash(output: &[u8]) -> Self::Hash<'_> {
        D::digest(output)
    }
}
