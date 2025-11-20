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
//!     zkvm::{ProofKind, ProverResourceType, zkVM},
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
//! let input = 42u32.to_le_bytes();
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

use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

mod util;

pub mod compiler;
pub mod zkvm;

pub use ere_compiler::CompilerKind;

include!(concat!(env!("OUT_DIR"), "/crate_version.rs"));
include!(concat!(env!("OUT_DIR"), "/zkvm_sdk_version_impl.rs"));

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum zkVMKind {
    Airbender,
    Jolt,
    Miden,
    Nexus,
    OpenVM,
    Pico,
    Risc0,
    SP1,
    Ziren,
    Zisk,
}

impl zkVMKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Airbender => "airbender",
            Self::Jolt => "jolt",
            Self::Miden => "miden",
            Self::Nexus => "nexus",
            Self::OpenVM => "openvm",
            Self::Pico => "pico",
            Self::Risc0 => "risc0",
            Self::SP1 => "sp1",
            Self::Ziren => "ziren",
            Self::Zisk => "zisk",
        }
    }
}

impl FromStr for zkVMKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "airbender" => Self::Airbender,
            "jolt" => Self::Jolt,
            "miden" => Self::Miden,
            "nexus" => Self::Nexus,
            "openvm" => Self::OpenVM,
            "pico" => Self::Pico,
            "risc0" => Self::Risc0,
            "sp1" => Self::SP1,
            "ziren" => Self::Ziren,
            "zisk" => Self::Zisk,
            _ => return Err(format!("Unsupported zkvm kind {s}")),
        })
    }
}

impl Display for zkVMKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tag of images in format of `{version}{suffix}`.
fn image_tag(zkvm_kind: zkVMKind, gpu: bool) -> String {
    let suffix = match (zkvm_kind, gpu) {
        // Only the following zkVMs requires CUDA setup in the base image
        // when GPU support is required.
        (zkVMKind::Airbender | zkVMKind::OpenVM | zkVMKind::Risc0 | zkVMKind::Zisk, true) => {
            "-cuda"
        }
        _ => "",
    };
    format!("{CRATE_VERSION}{suffix}")
}

fn base_image(zkvm_kind: zkVMKind, gpu: bool) -> String {
    let image_tag = image_tag(zkvm_kind, gpu);
    format!("ere-base:{image_tag}")
}

fn base_zkvm_image(zkvm_kind: zkVMKind, gpu: bool) -> String {
    let image_tag = image_tag(zkvm_kind, gpu);
    format!("ere-base-{zkvm_kind}:{image_tag}")
}

fn server_zkvm_image(zkvm_kind: zkVMKind, gpu: bool) -> String {
    let image_tag = image_tag(zkvm_kind, gpu);
    format!("ere-server-{zkvm_kind}:{image_tag}")
}

fn compiler_zkvm_image(zkvm_kind: zkVMKind) -> String {
    let image_tag = image_tag(zkvm_kind, false);
    format!("ere-compiler-{zkvm_kind}:{image_tag}")
}
