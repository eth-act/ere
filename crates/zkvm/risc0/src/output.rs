use risc0_zkvm::serde::{Deserializer, Error, WordRead};
use serde::de::{DeserializeOwned, Error as _};
use std::io::Read;
use zkvm_interface::zkVMError;

pub fn deserialize_from<R: Read, T: DeserializeOwned>(reader: R) -> Result<T, zkVMError> {
    struct WordReadAdapter<R>(R);

    impl<R: Read> WordRead for WordReadAdapter<R> {
        fn read_words(&mut self, words: &mut [u32]) -> Result<(), Error> {
            let bytes = bytemuck::cast_slice_mut(words);
            self.0.read_exact(bytes).map_err(Error::custom)
        }

        fn read_padded_bytes(&mut self, bytes: &mut [u8]) -> Result<(), Error> {
            let mut padded_bytes = vec![0u8; bytes.len().next_multiple_of(4) - bytes.len()];
            self.0.read_exact(bytes).map_err(Error::custom)?;
            self.0.read_exact(&mut padded_bytes).map_err(Error::custom)
        }
    }

    T::deserialize(&mut Deserializer::new(WordReadAdapter(reader))).map_err(zkVMError::other)
}
