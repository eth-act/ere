/// Aggregation verifying key for VadcopFinalMinimal proofs in zisk v0.18.0.
///
/// To reproduce:
///
/// ```bash
/// cat $HOME/.zisk/provingKey/zisk/vadcop_final_compressed/vadcop_final_compressed.verkey.json
/// ```
pub const VADCOP_FINAL_COMPRESSED_VK: [u64; 4] = [
    371850295254322978,
    2764832171281751502,
    14747498303081942412,
    8181136173693786776,
];

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
