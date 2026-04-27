use core::ops::Deref;

/// Stdin with a LE u32 length prefix.
///
/// Dereferencing it returns slice to the actual data.
pub struct LengthPrefixedStdin<T>(T);

impl<T: Deref<Target = [u8]>> Deref for LengthPrefixedStdin<T> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0.deref()[4..]
    }
}

impl<T: Deref<Target = [u8]>> LengthPrefixedStdin<T> {
    pub fn new(stdin: T) -> Self {
        assert!(
            stdin.len() >= 4,
            "stdin must have a LE u32 length prefix; use Input::with_prefixed_length(stdin) on the host side"
        );
        let len = u32::from_le_bytes(stdin[..4].try_into().unwrap()) as usize;
        assert_eq!(
            len,
            stdin.len() - 4,
            "Length mismatch: stdin length prefix indicated {len} bytes, but got {} bytes; use Input::with_prefixed_length(stdin) on the host side",
            stdin.len() - 4
        );
        Self(stdin)
    }
}
