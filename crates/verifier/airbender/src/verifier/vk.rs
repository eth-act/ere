use std::sync::LazyLock;

use airbender_execution_utils::{setups::CompiledCircuitsSet, unrolled::UnrolledProgramSetup};
use airbender_verifier_common::SecurityModel;
use serde::{Deserialize, Serialize};

pub const SECURITY: SecurityModel = SecurityModel::Security100;

pub const UNROLLED_END_PARAMS: [u32; 8] = [
    0x6541a28e, 0x551688f4, 0xc139fc5f, 0x91bb88e1, 0xc53a4615, 0x8b049ad3, 0xc0102f4e, 0x2fda6fe0,
];

pub static UNIFIED_VK: LazyLock<UnifiedVk> = LazyLock::new(|| {
    bincode::serde::decode_from_slice(include_bytes!("./unified.vk"), bincode::config::legacy())
        .unwrap()
        .0
});

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnifiedVk {
    pub unified_setup: UnrolledProgramSetup,
    pub unified_layouts: CompiledCircuitsSet,
}

pub fn unified_end_params() -> [u32; 8] {
    UNIFIED_VK.unified_setup.end_params
}

#[cfg(test)]
mod tests {
    use airbender_execution_utils::{
        RecursionArtifact::*,
        RecursionLayer::*,
        setups::{get_unified_circuit_artifact_for_machine_type, pad_binary},
        unified_circuit::compute_unified_setup_for_machine_configuration,
        unrolled::compute_setup_for_machine_configuration,
        verifier_binaries::recursion_artifact,
    };
    use airbender_riscv_transpiler::cycle::IWithoutByteAccessIsaConfigWithDelegation;

    use crate::{
        UNROLLED_END_PARAMS,
        verifier::vk::{SECURITY, UNIFIED_VK, UnifiedVk},
    };

    #[test]
    fn test_unrolled_end_params_correctness() {
        let unrolled_end_params = {
            let (binary, _) = pad_binary(recursion_artifact(SECURITY, Unrolled, Bin).to_vec());
            let (text, _) = pad_binary(recursion_artifact(SECURITY, Unrolled, Txt).to_vec());
            compute_setup_for_machine_configuration::<IWithoutByteAccessIsaConfigWithDelegation>(
                &binary, &text,
            )
            .end_params
        };
        assert_eq!(UNROLLED_END_PARAMS, unrolled_end_params);
    }

    #[test]
    fn test_unified_vk_correctness() {
        let unified_vk = {
            let (binary, binary_u32) =
                pad_binary(recursion_artifact(SECURITY, Unified, Bin).to_vec());
            let (text, _) = pad_binary(recursion_artifact(SECURITY, Unified, Txt).to_vec());
            let unified_setup = compute_unified_setup_for_machine_configuration::<
                IWithoutByteAccessIsaConfigWithDelegation,
            >(&binary, &text);
            let unified_layouts = get_unified_circuit_artifact_for_machine_type::<
                IWithoutByteAccessIsaConfigWithDelegation,
            >(&binary_u32);
            UnifiedVk {
                unified_setup,
                unified_layouts,
            }
        };
        assert_eq!(
            bincode::serde::encode_to_vec(&*UNIFIED_VK, bincode::config::legacy()).unwrap(),
            bincode::serde::encode_to_vec(&unified_vk, bincode::config::legacy()).unwrap(),
        );
    }
}
