/// Aggregation verifying key for VadcopFinal proofs in zisk v1.3.1-alpha, under the blake3 hash
/// family.
///
/// To reproduce:
///
/// ```bash
/// cat $HOME/.zisk/provingKey/zisk/vadcop_final/vadcop_final.verkey.json
/// ```
pub const VADCOP_FINAL_VK: [u64; 4] = [
    17362875648210006843,
    17118080347053690355,
    16676305655426731175,
    2889446131052424392,
];

/// Hash family the [`VADCOP_FINAL_VK`] was generated under. Proofs from any other family cannot
/// authenticate against it and are rejected.
pub const VADCOP_FINAL_HASH_FAMILY: &str = "blake3";

#[cfg(test)]
mod tests {
    use std::{io::Read, path::Path};

    use flate2::read::GzDecoder;

    use crate::verifier::vk::VADCOP_FINAL_VK;

    /// URL of the blake3 verifying key of v1.3.1-alpha.
    const VERIFY_KEY_URL: &str =
        "https://storage.googleapis.com/zisk-setup/zisk-verifykey-1.3.1-alpha-blake3.tar.gz";
    const VK_PATH: &str = "provingKey/zisk/vadcop_final/vadcop_final.verkey.bin";

    #[test]
    fn test_vk_correctness() {
        let response = reqwest::blocking::Client::builder()
            .build()
            .unwrap()
            .get(VERIFY_KEY_URL)
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
            VADCOP_FINAL_VK
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect::<Vec<_>>(),
            vk,
        );
    }
}
