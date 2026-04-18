//! Type-level markers used to select a codec via `PhantomData`.
//! Pair with `ere_codec::impl_codec_by_*!` macros to derive `Encode` and `Decode`.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BincodeLegacy;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cbor;
