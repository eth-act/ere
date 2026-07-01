use std::sync::LazyLock;

use openvm_stark_sdk::{
    config::baby_bear_poseidon2::BabyBearPoseidon2Config as SC,
    openvm_stark_backend::keygen::types::MultiStarkVerifyingKey,
};

pub static AGG_VK: LazyLock<MultiStarkVerifyingKey<SC>> =
    LazyLock::new(|| bitcode::deserialize(include_bytes!("./agg_stark.vk")).unwrap());

#[cfg(test)]
mod tests {
    use openvm_sdk::{Sdk, config::AggregationSystemParams};
    use openvm_stark_sdk::config::{MAX_APP_LOG_STACKED_HEIGHT, app_params_with_100_bits_security};

    use crate::verifier::AGG_VK;

    #[test]
    fn test_agg_vk_correctness() {
        let app_params = app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT);
        let agg_params = AggregationSystemParams::default();
        assert_eq!(
            bitcode::serialize(&Sdk::standard(app_params, agg_params).agg_keygen().1).unwrap(),
            bitcode::serialize(&*AGG_VK).unwrap()
        );
    }
}
