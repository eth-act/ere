/// Aggregation verifying key for VadcopFinalMinimal proofs in zisk v1.2.0-alpha, under the default
/// Poseidon1 hash family.
///
/// To reproduce:
///
/// ```bash
/// cat $HOME/.zisk/provingKey/zisk/vadcop_final_compressed/vadcop_final_compressed.verkey.json
/// ```
pub const VADCOP_FINAL_COMPRESSED_VK: [u64; 4] = [
    15008563959707073304,
    10715099813120081992,
    18339358923736659668,
    13838445471377553159,
];

/// Hash family the [`VADCOP_FINAL_COMPRESSED_VK`] was generated under. Proofs from any other family
/// cannot authenticate against it and are rejected.
pub const VADCOP_FINAL_HASH_FAMILY: &str = "Poseidon1";

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf};

    use crate::verifier::vk::VADCOP_FINAL_COMPRESSED_VK;

    const VERKEY_BIN_PATH: &str =
        ".zisk/provingKey/zisk/vadcop_final_compressed/vadcop_final_compressed.verkey.bin";

    #[test]
    fn test_vk_correctness() {
        assert_eq!(
            VADCOP_FINAL_COMPRESSED_VK
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>(),
            fs::read(PathBuf::from(env::var("HOME").unwrap()).join(VERKEY_BIN_PATH)).unwrap(),
        );
    }
}
