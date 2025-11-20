use ere_compile_utils::CommonError;
use std::{fs, path::Path};

mod error;
mod rust_rv32ima;
mod rust_rv32ima_customized;

pub use error::Error;
pub use rust_rv32ima::RustRv32ima;
pub use rust_rv32ima_customized::RustRv32imaCustomized;

fn read_app_config(app_config_path: impl AsRef<Path>) -> Result<Option<String>, Error> {
    if !app_config_path.as_ref().exists() {
        return Ok(None);
    }

    let value = fs::read_to_string(app_config_path.as_ref())
        .map_err(|err| CommonError::read_file("app_config", &app_config_path, err))?;
    toml::from_str::<toml::Value>(&value)
        .map_err(|err| CommonError::deserialize("app_config", "toml", err))?;
    Ok(Some(value))
}
