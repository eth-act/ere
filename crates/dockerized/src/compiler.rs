use std::{
    fs, iter,
    path::{Path, PathBuf},
};

use ere_compiler_core::{Compiler, Elf};
use ere_prover_core::CommonError;
use tempfile::TempDir;

use crate::{
    CompilerKind,
    image::compiler_zkvm_image,
    util::{
        docker::{DockerRunCmd, docker_ensure_image},
        env::{docker_memory, docker_memory_swap},
    },
    zkVMKind,
};

mod error;

pub use error::Error;

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
        docker_ensure_image(&compiler_zkvm_image(zkvm_kind))?;
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

    fn compile(
        &self,
        guest_directory: impl AsRef<Path>,
        args: &[String],
    ) -> Result<Elf, Self::Error> {
        let guest_directory = guest_directory.as_ref();
        let guest_relative_path = guest_directory
            .strip_prefix(&self.mount_directory)
            .map_err(|_| Error::GuestNotInMountingDirecty {
                mounting_directory: self.mount_directory.to_path_buf(),
                guest_directory: guest_directory.to_path_buf(),
            })?;
        let guest_path_in_docker = PathBuf::from("/guest")
            .join(guest_relative_path)
            .to_string_lossy()
            .to_string();

        let tempdir = TempDir::new().map_err(CommonError::tempdir)?;

        let mut cmd = DockerRunCmd::new(compiler_zkvm_image(self.zkvm_kind))
            .rm()
            .memory(docker_memory()?, docker_memory_swap())
            .inherit_env("RUST_LOG")
            .inherit_env("NO_COLOR")
            .inherit_env("ERE_RUST_TOOLCHAIN")
            .inherit_env("ERE_RUSTFLAGS")
            .inherit_env("ERE_PROFILE")
            .volume(&self.mount_directory, "/guest")
            .volume(tempdir.path(), "/output");

        cmd = match self.zkvm_kind {
            // OpenVM allows to select Rust toolchain for guest compilation.
            zkVMKind::OpenVM => cmd.inherit_env("OPENVM_RUST_TOOLCHAIN"),
            _ => cmd,
        };

        const ELF_NAME: &str = "guest.elf";
        let extra_args = (!args.is_empty())
            .then(|| {
                iter::empty()
                    .chain(["--"])
                    .chain(args.iter().map(|arg| arg.as_str()))
            })
            .into_iter()
            .flatten();
        cmd.exec(
            [
                "--compiler-kind",
                self.compiler_kind.as_str(),
                "--guest-dir",
                &guest_path_in_docker,
                "--output-dir",
                "/output",
                "--elf-name",
                ELF_NAME,
            ]
            .into_iter()
            .chain(extra_args),
        )?;

        let elf_path = tempdir.path().join(ELF_NAME);
        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;
        Ok(Elf(elf))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::Path;

    use ere_compiler_core::{Compiler, Elf};
    use ere_util_test::host::testing_guest_directory;
    use tracing_subscriber::EnvFilter;

    use crate::{CompilerKind, compiler::DockerizedCompiler, zkVMKind};

    pub fn compile(zkvm_kind: zkVMKind, compiler_kind: CompilerKind, program: &'static str) -> Elf {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();

        DockerizedCompiler::new(
            zkvm_kind,
            compiler_kind,
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .unwrap(),
        )
        .unwrap()
        .compile(testing_guest_directory(zkvm_kind.as_str(), program), &[])
        .unwrap()
    }

    macro_rules! test_compile {
        ($zkvm_kind:ident, $compiler_kind:ident, $program:literal) => {
            paste::paste! {
                #[test]
                fn [<test_compile_ $compiler_kind:snake>]() {
                    let zkvm_kind = crate::zkVMKind::$zkvm_kind;
                    let compiler_kind = crate::CompilerKind::$compiler_kind;
                    let elf = crate::compiler::tests::compile(zkvm_kind, compiler_kind, $program);

                    assert!(!elf.is_empty(), "ELF should not be empty");
                }
            }
        };
    }

    macro_rules! test_reproducible_elf {
        ($zkvm_kind:ident, $compiler_kind:ident, $program:literal) => {
            paste::paste! {
                #[test]
                fn [<test_reproducible_elf_ $compiler_kind:snake>]() {
                    let zkvm_kind = crate::zkVMKind::$zkvm_kind;
                    let compiler_kind = crate::CompilerKind::$compiler_kind;
                    let elf_1 = crate::compiler::tests::compile(zkvm_kind, compiler_kind, $program);
                    let elf_2 = crate::compiler::tests::compile(zkvm_kind, compiler_kind, $program);

                    assert!(elf_1 == elf_2, "ELF outputs should be equal");
                }
            }
        };
    }

    mod openvm {
        test_compile!(OpenVM, RustCustomized, "basic");
        test_compile!(OpenVM, Rust, "stock_nightly_no_std");
        test_reproducible_elf!(OpenVM, RustCustomized, "basic");
        test_reproducible_elf!(OpenVM, Rust, "stock_nightly_no_std");
    }

    mod sp1 {
        test_compile!(SP1, RustCustomized, "basic");
        test_compile!(SP1, Rust, "stock_nightly_no_std");
        test_reproducible_elf!(SP1, RustCustomized, "basic");
        test_reproducible_elf!(SP1, Rust, "stock_nightly_no_std");
    }

    mod zisk {
        test_compile!(Zisk, RustCustomized, "basic_rust");
        test_compile!(Zisk, Rust, "stock_nightly_no_std");
        test_reproducible_elf!(Zisk, RustCustomized, "basic_rust");
        test_reproducible_elf!(Zisk, Rust, "stock_nightly_no_std");
    }

    mod lambdavm {
        test_compile!(LambdaVM, RustCustomized, "basic");
        test_compile!(LambdaVM, Rust, "stock_nightly_no_std");
        test_reproducible_elf!(LambdaVM, RustCustomized, "basic");
        test_reproducible_elf!(LambdaVM, Rust, "stock_nightly_no_std");
    }

    mod pico {
        test_compile!(Pico, RustCustomized, "basic");
        test_compile!(Pico, Rust, "stock_nightly_no_std");
        test_reproducible_elf!(Pico, RustCustomized, "basic");
        test_reproducible_elf!(Pico, Rust, "stock_nightly_no_std");
    }
}
