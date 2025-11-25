use ere_zkvm_interface::CommonError;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    CommonError(#[from] CommonError),
    #[error(
        "Guest directory must be in mounting directory, mounting_directory: {mounting_directory}, guest_directory: {guest_directory}"
    )]
    GuestNotInMountingDirecty {
        mounting_directory: PathBuf,
        guest_directory: PathBuf,
    },
}
