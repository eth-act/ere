use openvm_sdk_config::SdkVmConfig;

use crate::NUM_PUBLIC_VALUES;

/// The VM config every ere OpenVM proof is produced and verified under.
///
/// Shared by the local prover and the cluster client so both agree on the
/// number of public values.
pub fn sdk_vm_config() -> SdkVmConfig {
    let mut config = SdkVmConfig::standard();
    config.system.config = config.system.config.with_public_values(NUM_PUBLIC_VALUES);
    config.optimize()
}
