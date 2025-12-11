//! # Ere Dockerized
//!
//! A Docker-based wrapper for other zkVM crates `ere-{zkvm}`.
//!
//! This crate provides a unified interface to dockerize the `Compiler` and
//! `zkVM` implementation of other zkVM crates `ere-{zkvm}`, it requires only
//! `docker` to be installed, but no zkVM specific SDK.
//!
//! ## Docker image building
//!
//! It builds 4 Docker images in sequence if they don't exist:
//! 1. `ere-base:{version}` - Base image with common dependencies
//! 2. `ere-base-{zkvm}:{version}` - zkVM-specific base image with the zkVM SDK
//! 3. `ere-compiler-{zkvm}:{version}` - Compiler image with the `ere-compiler`
//!    binary built with the selected zkVM feature
//! 4. `ere-server-{zkvm}:{version}` - Server image with the `ere-server` binary
//!    built with the selected zkVM feature
//!
//! When [`ProverResourceType::Gpu`] is selected, the image with GPU support
//! will be built and tagged with specific suffix.
//!
//! To force rebuild all images, set the environment variable
//! `ERE_FORCE_REBUILD_DOCKER_IMAGE` to non-empty value.
//!
//! ## Example
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use ere_dockerized::{CompilerKind, DockerizedCompiler, DockerizedzkVM, zkVMKind};
//! use ere_zkvm_interface::{
//!     compiler::Compiler,
//!     zkvm::{Input, ProofKind, ProverResourceType, zkVM},
//! };
//! use std::path::Path;
//!
//! // The zkVM we plan to use
//! let zkvm_kind = zkVMKind::SP1;
//!
//! // The compiler we plan to use
//! let compiler_kind = CompilerKind::RustCustomized;
//!
//! // Compile a guest program
//! let compiler = DockerizedCompiler::new(zkvm_kind, compiler_kind, "mounting/directory")?;
//! let guest_path = Path::new("relative/path/to/guest/program");
//! let program = compiler.compile(&guest_path)?;
//!
//! // Create zkVM instance
//! let resource = ProverResourceType::Cpu;
//! let zkvm = DockerizedzkVM::new(zkvm_kind, program, resource)?;
//!
//! // Serialize input
//! let input = Input::new().with_stdin(42u32.to_le_bytes().to_vec());
//!
//! // Execute program
//! let (public_values, execution_report) = zkvm.execute(&input)?;
//! println!("Execution cycles: {}", execution_report.total_num_cycles);
//!
//! // Generate proof
//! let (public_values, proof, proving_report) = zkvm.prove(&input, ProofKind::Compressed)?;
//! println!("Proof generated in: {:?}", proving_report.proving_time);
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
pub mod zkvm;

pub use crate::{
    compiler::{DockerizedCompiler, SerializedProgram},
    zkvm::DockerizedzkVM,
};
pub use ere_common::{CRATE_VERSION, CompilerKind, zkVMKind};
