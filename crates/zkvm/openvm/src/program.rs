use openvm_sdk::config::{AppConfig, SdkVmConfig};
use serde::{Deserialize, Serialize};

/// OpenVM program that contains ELF of compiled guest and app config.
#[derive(Clone, Serialize, Deserialize)]
pub struct OpenVMProgram {
    pub(crate) elf: Vec<u8>,
    pub(crate) app_config: AppConfig<SdkVmConfig>,
}

impl OpenVMProgram {
    pub fn elf(&self) -> &[u8] {
        &self.elf
    }

    pub fn app_config(&self) -> &AppConfig<SdkVmConfig> {
        &self.app_config
    }
}
