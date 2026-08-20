/// Aggregation verifying key for VadcopFinalMinimal proofs in zisk v1.1.0-alpha, under the default
/// Poseidon1 hash family.
///
/// To reproduce:
///
/// ```bash
/// cat $HOME/.zisk/provingKey/zisk/vadcop_final_compressed/vadcop_final_compressed.verkey.json
/// ```
pub const VADCOP_FINAL_COMPRESSED_VK: [u64; 4] = [
    16370293013691533579,
    6864437907249201182,
    12605075787481351281,
    11365126049963943544,
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
