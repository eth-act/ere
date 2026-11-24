use std::sync::LazyLock;

use openvm_stark_sdk::{
    config::baby_bear_poseidon2::BabyBearPoseidon2Config as SC,
    openvm_stark_backend::keygen::types::MultiStarkVerifyingKey,
};
use openvm_verify_stark_host::vk::VerificationBaseline;

pub static AGG_VK: LazyLock<MultiStarkVerifyingKey<SC>> =
    LazyLock::new(|| bitcode::deserialize(include_bytes!("./agg_stark.vk")).unwrap());

/// Baseline of `ere-prover-openvm` with a zero `app_exe_commit`, the only field that differs
/// between programs.
pub static BASELINE: LazyLock<VerificationBaseline> =
    LazyLock::new(|| bitcode::deserialize(include_bytes!("./baseline.bin")).unwrap());

#[cfg(test)]
mod tests {
    use openvm_sdk::{
        Sdk,
        config::{AggregationSystemParams, AppConfig},
        openvm_circuit::arch::instructions::{
            LocalOpcode, SystemOpcode, exe::VmExe, instruction::Instruction, program::Program,
        },
    };
    use openvm_sdk_config::SdkVmConfig;
    use openvm_stark_sdk::{
        config::{
            MAX_APP_LOG_STACKED_HEIGHT, app_params_with_100_bits_security,
            baby_bear_poseidon2::{DIGEST_SIZE, F},
        },
        openvm_stark_backend::p3_field::PrimeCharacteristicRing,
    };
    use openvm_verify_stark_host::vk::VerificationBaseline;

    use crate::verifier::{AGG_VK, NUM_PUBLIC_VALUES_BYTES, vk::BASELINE};

    #[test]
    fn test_agg_vk_correctness() {
        let app_params = app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT);
        let agg_params = AggregationSystemParams::default();
        assert_eq!(
            bitcode::serialize(&Sdk::standard(app_params, agg_params).agg_keygen().1).unwrap(),
            bitcode::serialize(&*AGG_VK).unwrap()
        );
    }

    #[test]
    fn test_baseline_correctness() {
        let app_config = {
            let mut config = SdkVmConfig::standard();
            config.system.config = config
                .system
                .config
                .with_public_values_bytes(NUM_PUBLIC_VALUES_BYTES);
            let system_params = app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT);
            AppConfig::new(config.optimize(), system_params)
        };
        let sdk = Sdk::builder()
            .app_config(app_config)
            .agg_params(AggregationSystemParams::default())
            .build()
            .unwrap();
        let terminate = Instruction::from_usize(SystemOpcode::TERMINATE.global_opcode(), [0, 0, 0]);
        let app_exe = VmExe::new(Program::from_instructions(&[terminate]));
        let baseline = VerificationBaseline {
            app_exe_commit: [F::ZERO; DIGEST_SIZE],
            ..sdk.prover(app_exe).unwrap().generate_baseline()
        };
        assert_eq!(
            bitcode::serialize(&baseline).unwrap(),
            bitcode::serialize(&*BASELINE).unwrap()
        );
    }
}
