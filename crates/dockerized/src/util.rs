use std::path::PathBuf;

use ere_zkvm_interface::CommonError;

pub mod cuda;
pub mod docker;

pub fn workspace_dir() -> Result<PathBuf, CommonError> {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.canonicalize()
        .map_err(|err| CommonError::io("Source code of Ere not found", err))
}

pub fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
}
