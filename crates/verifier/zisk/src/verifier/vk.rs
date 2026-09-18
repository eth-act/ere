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
    use std::{io::Read, path::Path};

    use flate2::read::GzDecoder;

    use crate::verifier::vk::VADCOP_FINAL_COMPRESSED_VK;

    /// URL of the proving key of v1.2.0-alpha.
    const PROVING_KEY_URL: &str =
        "https://storage.googleapis.com/zisk-setup/zisk-provingkey-1.2.0-alpha.tar.gz";
    const VK_PATH: &str =
        "provingKey/zisk/vadcop_final_compressed/vadcop_final_compressed.verkey.bin";

    #[test]
    fn test_vk_correctness() {
        let response = reqwest::blocking::Client::builder()
            .build()
            .unwrap()
            .get(PROVING_KEY_URL)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap();
        let mut archive = tar::Archive::new(GzDecoder::new(response));
        let mut entry = archive
            .entries()
            .unwrap()
            .map(Result::unwrap)
            .find(|entry| entry.path().unwrap() == Path::new(VK_PATH))
            .unwrap();
        let mut vk = Vec::new();
        entry.read_to_end(&mut vk).unwrap();

        assert_eq!(
            VADCOP_FINAL_COMPRESSED_VK
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>(),
            vk,
        );
    }
}
