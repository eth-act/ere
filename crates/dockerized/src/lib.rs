//! # Ere Dockerized
//!
//! A Docker-based wrapper for other zkVM crates `ere-compiler-{zkvm}` and `ere-prover-{zkvm}`.
//!
//! This crate provides a unified interface to dockerize the `Compiler` and `zkVMProver`
//! implementation of other zkVM crates `ere-compiler-{zkvm}` and `ere-prover-{zkvm}`, it requires
//! only `docker` to be installed, but no zkVM specific SDK.
//!
//! ## Docker images
//!
//! It uses 2 Docker images for each zkVM:
//! 1. `ere-compiler-{zkvm}:{tag}` - Compiler image with the `ere-compiler` binary built with the
//!    selected zkVM feature
//! 2. `ere-server-{zkvm}:{tag}` - Server image with the `ere-server` binary built with the selected
//!    zkVM feature
//!
//! The tag is `ERE_IMAGE_TAG` when it is set, else the short git revision of Ere.
//!
//! When [`ProverResource::Gpu`] is selected, the server image with GPU support is used, and its tag
//! has a specific suffix.
//!
//! The image names start with the registry `ERE_IMAGE_REGISTRY`, which is `ghcr.io/eth-act/ere`
//! when unset, and an empty value gives no registry. The crate uses a local image, and else pulls
//! it. A failed pull is an error. A local build is made with `.github/scripts/build-image.sh`.
//!
//! ## Example
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use ere_dockerized::{
//!     CompilerKind, DockerizedCompiler, DockerizedzkVM, DockerizedzkVMConfig, zkVMKind,
//! };
//! use ere_compiler_core::Compiler;
//! use ere_prover_core::{Input, ProverResource};
//!
//! // The zkVM we plan to use
//! let zkvm_kind = zkVMKind::SP1;
//!
//! // The compiler we plan to use
//! let compiler_kind = CompilerKind::RustCustomized;
//!
//! // Compile a guest program
//! let compiler = DockerizedCompiler::new(zkvm_kind, compiler_kind, "mounting/directory")?;
//! let guest_path = "mounting/directory/path/to/guest/program";
//! let elf = compiler.compile(guest_path, &[])?;
//!
//! // Create zkVM instance
//! let resource = ProverResource::Cpu;
//! let zkvm = DockerizedzkVM::new(
//!     zkvm_kind,
//!     elf,
//!     resource,
//!     DockerizedzkVMConfig::default(),
//! )?;
//!
//! // Serialize input
//! let input = Input::new().with_stdin(42u32.to_le_bytes().to_vec());
//!
//! // Execute program
//! let (public_values, execution_duration) = zkvm.execute(&input)?;
//! println!("Execution duration: {execution_duration:?}");
//!
//! // Generate proof
//! let (public_values, proof, proving_time) = zkvm.prove(&input)?;
//! println!("Proof generated in: {proving_time:?}");
//!
//! // Verify proof
//! let public_values = zkvm.verify(&proof)?;
//! println!("Proof verified successfully!");
//! # Ok(())
//! # }
//! ```

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod util;

pub mod compiler;
pub mod image;
pub mod prover;

pub use ere_catalog::{CompilerKind, DOCKER_IMAGE_TAG, zkVMKind};
pub use ere_compiler_core::{Compiler, Elf};
pub use ere_prover_core::*;
pub use ere_server_client::{EncodedProgramVk, EncodedProof};

pub use crate::{
    compiler::DockerizedCompiler,
    prover::{DockerizedzkVM, DockerizedzkVMConfig},
};
