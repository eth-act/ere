/// Aggregation verifying key for VadcopFinal proofs in zisk v1.4.0-alpha, under the blake3 hash
/// family.
///
/// To reproduce:
///
/// ```bash
/// cat $HOME/.zisk/provingKey/zisk/vadcop_final/vadcop_final.verkey.json
/// ```
pub const VADCOP_FINAL_VK: [u64; 4] = [
    17146226293136250724,
    16833344589786796689,
    10321050178608571017,
    2095991909911874349,
];

/// Hash family the [`VADCOP_FINAL_VK`] was generated under. Proofs from any other family cannot
/// authenticate against it and are rejected.
pub const VADCOP_FINAL_HASH_FAMILY: &str = "blake3";

#[cfg(test)]
mod tests {
    use zisk_verifier::vadcop_vk;

    use crate::verifier::vk::{VADCOP_FINAL_HASH_FAMILY, VADCOP_FINAL_VK};

    #[test]
    fn test_vk_matches_upstream() {
        assert_eq!(
            vadcop_vk(VADCOP_FINAL_HASH_FAMILY, false),
            Some(VADCOP_FINAL_VK)
        );
    }
}
