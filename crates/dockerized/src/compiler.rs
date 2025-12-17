use crate::{
    CompilerKind,
    image::{base_image, base_zkvm_image, compiler_zkvm_image},
    util::{
        docker::{DockerBuildCmd, DockerRunCmd, docker_image_exists, force_rebuild},
        workspace_dir,
    },
    zkVMKind,
};
use ere_zkvm_interface::{CommonError, compiler::Compiler};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use tracing::info;

mod error;

pub use error::Error;

/// This method builds 3 Docker images in sequence:
/// 1. `ere-base:{version}` - Base image with common dependencies
/// 2. `ere-base-{zkvm}:{version}` - zkVM-specific base image with the zkVM SDK
/// 3. `ere-compiler-{zkvm}:{version}` - Compiler image with the `ere-compiler`
///    binary built with the selected zkVM feature
///
/// Images are cached and only rebuilt if they don't exist or if the
/// `ERE_FORCE_REBUILD_DOCKER_IMAGE` environment variable is set.
fn build_compiler_image(zkvm_kind: zkVMKind) -> Result<(), Error> {
    let force_rebuild = force_rebuild();
    let base_image = base_image(zkvm_kind, false);
    let base_zkvm_image = base_zkvm_image(zkvm_kind, false);
    let compiler_zkvm_image = compiler_zkvm_image(zkvm_kind);

    if !force_rebuild && docker_image_exists(&compiler_zkvm_image)? {
        info!("Image {compiler_zkvm_image} exists, skip building");
        return Ok(());
    }

    let workspace_dir = workspace_dir()?;
    let docker_dir = workspace_dir.join("docker");
    let docker_zkvm_dir = docker_dir.join(zkvm_kind.as_str());

    // Build `ere-base`
    if force_rebuild || !docker_image_exists(&base_image)? {
        info!("Building image {base_image}...");

        DockerBuildCmd::new()
            .file(docker_dir.join("Dockerfile.base"))
            .tag(&base_image)
            .exec(&workspace_dir)?;
    }

    // Build `ere-base-{zkvm_kind}`
    if force_rebuild || !docker_image_exists(&base_zkvm_image)? {
        info!("Building image {base_zkvm_image}...");

        DockerBuildCmd::new()
            .file(docker_zkvm_dir.join("Dockerfile.base"))
            .tag(&base_zkvm_image)
            .build_arg("BASE_IMAGE", &base_image)
            .build_arg_from_env("RUSTFLAGS")
            .exec(&workspace_dir)?;
    }

    // Build `ere-compiler-{zkvm_kind}`
    info!("Building image {compiler_zkvm_image}...");

    DockerBuildCmd::new()
        .file(docker_zkvm_dir.join("Dockerfile.compiler"))
        .tag(&compiler_zkvm_image)
        .build_arg("BASE_ZKVM_IMAGE", &base_zkvm_image)
        .exec(&workspace_dir)?;

    Ok(())
}

/// Wrapper for serialized program.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SerializedProgram(pub Vec<u8>);

pub struct DockerizedCompiler {
    zkvm_kind: zkVMKind,
    compiler_kind: CompilerKind,
    mount_directory: PathBuf,
}

impl DockerizedCompiler {
    pub fn new(
        zkvm_kind: zkVMKind,
        compiler_kind: CompilerKind,
        mount_directory: impl AsRef<Path>,
    ) -> Result<Self, Error> {
        build_compiler_image(zkvm_kind)?;
        Ok(Self {
            zkvm_kind,
            compiler_kind,
            mount_directory: mount_directory.as_ref().to_path_buf(),
        })
    }

    pub fn zkvm_kind(&self) -> zkVMKind {
        self.zkvm_kind
    }

    pub fn compiler_kind(&self) -> CompilerKind {
        self.compiler_kind
    }
}

impl Compiler for DockerizedCompiler {
    type Error = Error;
    type Program = SerializedProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let guest_relative_path = guest_directory
            .strip_prefix(&self.mount_directory)
            .map_err(|_| Error::GuestNotInMountingDirecty {
                mounting_directory: self.mount_directory.to_path_buf(),
                guest_directory: guest_directory.to_path_buf(),
            })?;
        let guest_path_in_docker = PathBuf::from("/guest").join(guest_relative_path);

        let tempdir = TempDir::new().map_err(CommonError::tempdir)?;

        let mut cmd = DockerRunCmd::new(compiler_zkvm_image(self.zkvm_kind))
            .rm()
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .inherit_env("ERE_RUST_TOOLCHAIN")
            .volume(&self.mount_directory, "/guest")
            .volume(tempdir.path(), "/output");

        cmd = match self.zkvm_kind {
            // OpenVM allows to select Rust toolchain for guest compilation.
            zkVMKind::OpenVM => cmd.inherit_env("OPENVM_RUST_TOOLCHAIN"),
            _ => cmd,
        };

        cmd.exec([
            "--compiler-kind",
            self.compiler_kind.as_str(),
            "--guest-path",
            guest_path_in_docker.to_string_lossy().as_ref(),
            "--output-path",
            "/output/program",
        ])?;

        let program_path = tempdir.path().join("program");
        let program = fs::read(&program_path)
            .map_err(|err| CommonError::read_file("program", &program_path, err))?;
        Ok(SerializedProgram(program))
    }
}

#[cfg(test)]
pub(crate) mod test {
    use crate::{
        CompilerKind,
        compiler::{DockerizedCompiler, SerializedProgram},
        util::workspace_dir,
        zkVMKind,
    };
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::compiler::Compiler;

    pub fn compile(
        zkvm_kind: zkVMKind,
        compiler_kind: CompilerKind,
        program: &'static str,
    ) -> SerializedProgram {
        DockerizedCompiler::new(zkvm_kind, compiler_kind, workspace_dir().unwrap())
            .unwrap()
            .compile(&testing_guest_directory(zkvm_kind.as_str(), program))
            .unwrap()
    }

    macro_rules! test_compile {
        ($zkvm_kind:ident, $compiler_kind:ident, $program:literal) => {
            paste::paste! {
                #[test]
                fn [<test_compile_ $compiler_kind:snake>]() {
                    let zkvm_kind = crate::zkVMKind::$zkvm_kind;
                    let compiler_kind = crate::CompilerKind::$compiler_kind;
                    let program = crate::compiler::test::compile(zkvm_kind, compiler_kind, $program);

                    assert!(!program.0.is_empty(), "Program should not be empty");
                }
            }
        };
    }

    mod airbender {
        test_compile!(Airbender, Rust, "basic");
    }

    mod jolt {
        test_compile!(Jolt, RustCustomized, "basic");
        test_compile!(Jolt, Rust, "stock_nightly_no_std");
    }

    mod miden {
        test_compile!(Miden, MidenAsm, "fib");
    }

    mod nexus {
        test_compile!(Nexus, Rust, "basic");
    }

    mod openvm {
        test_compile!(OpenVM, RustCustomized, "basic");
        test_compile!(OpenVM, Rust, "stock_nightly_no_std");
    }

    mod pico {
        test_compile!(Pico, RustCustomized, "basic");
        test_compile!(Pico, Rust, "stock_nightly_no_std");
    }

    mod risc0 {
        test_compile!(Risc0, RustCustomized, "basic");
        test_compile!(Risc0, Rust, "stock_nightly_no_std");
    }

    mod sp1 {
        test_compile!(SP1, RustCustomized, "basic");
        test_compile!(SP1, Rust, "stock_nightly_no_std");
    }

    mod ziren {
        test_compile!(Ziren, RustCustomized, "basic");
    }

    mod zisk {
        test_compile!(Zisk, RustCustomized, "basic_rust");
        test_compile!(Zisk, GoCustomized, "basic_go");
    }
}
