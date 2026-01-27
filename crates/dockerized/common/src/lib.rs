pub mod compiler;
pub mod zkvm;

pub use compiler::CompilerKind;
pub use zkvm::zkVMKind;

include!(concat!(env!("OUT_DIR"), "/docker_image_tag.rs"));
include!(concat!(env!("OUT_DIR"), "/zkvm_sdk_version_impl.rs"));
