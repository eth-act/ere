use std::path::PathBuf;

pub mod cuda;
pub mod docker;

pub fn workspace_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop();
    dir.pop();
    dir.canonicalize().unwrap()
}

pub fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").expect("env `$HOME` should be set"))
}
