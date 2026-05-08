use std::sync::LazyLock;

use crate::vendor::AggVerifyingKey;

pub static AGG_VK: LazyLock<AggVerifyingKey> =
    LazyLock::new(|| bitcode::deserialize(include_bytes!("./agg_stark.vk")).unwrap());

#[cfg(test)]
mod tests {
    use openvm_sdk::Sdk;

    use crate::verifier::AGG_VK;

    #[test]
    fn test_agg_vk_correctness() {
        assert_eq!(
            bitcode::serialize(&Sdk::standard().agg_keygen().unwrap().1).unwrap(),
            bitcode::serialize(&*AGG_VK).unwrap()
        );
    }
}
