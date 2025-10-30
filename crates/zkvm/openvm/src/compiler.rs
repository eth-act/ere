use ere_compile_utils::CommonError;
use openvm_sdk::config::{AppConfig, DEFAULT_APP_LOG_BLOWUP, DEFAULT_LEAF_LOG_BLOWUP, SdkVmConfig};
use openvm_stark_sdk::config::FriParameters;
use std::{fs, path::Path};

mod error;
mod rust_rv32ima;
mod rust_rv32ima_customized;

pub use error::Error;
pub use rust_rv32ima::RustRv32ima;
pub use rust_rv32ima_customized::RustRv32imaCustomized;

fn read_app_config(app_config_path: impl AsRef<Path>) -> Result<AppConfig<SdkVmConfig>, Error> {
    Ok(if app_config_path.as_ref().exists() {
        let toml = fs::read_to_string(app_config_path.as_ref())
            .map_err(|err| CommonError::read_file("app_config", &app_config_path, err))?;
        toml::from_str(&toml).map_err(|err| CommonError::deserialize("app_config", "toml", err))?
    } else {
        // The default `AppConfig` copied from https://github.com/openvm-org/openvm/blob/v1.4.0/crates/cli/src/default.rs#L35.
        AppConfig {
            app_fri_params: FriParameters::standard_with_100_bits_conjectured_security(
                DEFAULT_APP_LOG_BLOWUP,
            )
            .into(),
            // By default it supports RISCV32IM with IO but no precompiles.
            app_vm_config: SdkVmConfig::builder()
                .system(Default::default())
                .rv32i(Default::default())
                .rv32m(Default::default())
                .io(Default::default())
                .build(),
            leaf_fri_params: FriParameters::standard_with_100_bits_conjectured_security(
                DEFAULT_LEAF_LOG_BLOWUP,
            )
            .into(),
            compiler_options: Default::default(),
        }
    })
}
