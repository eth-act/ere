use std::{env, fs, iter, path::Path};

use airbender_build::{
    BuildConfig, BuildError, DEFAULT_GUEST_TARGET, DEFAULT_GUEST_TOOLCHAIN, build_dist,
};
use cargo_metadata::TargetKind;
use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::{CommonError, cargo_metadata, rustup_add_rust_src};
use tempfile::tempdir;

use crate::Error;

const LINKER_SCRIPT: &str = concat!(
    include_str!("rust_rv32ima/memory.x"),
    include_str!("rust_rv32ima/link.x"),
);

const CARGO_BUILD_OPTIONS: &[&str] = &[
    "-Zbuild-std=core,alloc,panic_abort,compiler_builtins,std",
    "-Zbuild-std-features=compiler-builtins-mem",
];

const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic",
    "-C",
    "target-feature=-unaligned-scalar-mem",
];

/// Compiler for Rust guest program to RV32IMA architecture, using customized
/// target `riscv32im-risc0-zkvm-elf`.
pub struct AirbenderRustRv32imaCustomized;

impl Compiler for AirbenderRustRv32imaCustomized {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        let toolchain =
            env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| DEFAULT_GUEST_TOOLCHAIN.into());
        rustup_add_rust_src(&toolchain)?;

        let guest_directory = guest_directory.as_ref();

        let metadata = cargo_metadata(guest_directory)?;
        let package = metadata.root_package().unwrap();

        let bin = package
            .targets
            .iter()
            .find(|t| t.kind.contains(&TargetKind::Bin))
            .ok_or_else(|| {
                let msg = format!("package `{}` has no binary targets", package.name);
                BuildError::InvalidConfig(msg)
            })?;

        let tempdir = tempdir().map_err(CommonError::tempdir)?;
        let linker_script_path = tempdir.path().join("linker_script");
        fs::write(&linker_script_path, LINKER_SCRIPT)
            .map_err(|err| CommonError::write_file("linker_script", &linker_script_path, err))?;

        let mut config = BuildConfig::new(guest_directory);
        config.bin_name = Some(bin.name.clone());
        config.dist_dir = Some(tempdir.path().to_path_buf());
        config.target = Some(DEFAULT_GUEST_TARGET.into());
        config.cargo_args = cargo_args(&linker_script_path);
        build_dist(&config)?;

        let elf_path = metadata
            .target_directory
            .join(DEFAULT_GUEST_TARGET)
            .join("release")
            .join(&bin.name);
        let elf =
            fs::read(&elf_path).map_err(|err| CommonError::read_file("elf", &elf_path, err))?;
        Ok(Elf(elf))
    }
}

fn cargo_args(linker_script_path: &Path) -> Vec<String> {
    let rustflags = {
        let linker_args = format!("link-arg=-T{}", linker_script_path.display());
        iter::empty()
            .chain(RUSTFLAGS.iter().copied())
            .chain(["-C", &linker_args])
            .map(|s| format!(r#""{s}""#))
            .collect::<Vec<_>>()
    };
    iter::empty()
        .chain(CARGO_BUILD_OPTIONS.iter().map(|option| option.to_string()))
        .chain([
            "--config".to_string(),
            format!("build.rustflags=[{}]", rustflags.join(",")),
        ])
        .collect()
}

#[cfg(test)]
mod tests {
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    use crate::AirbenderRustRv32imaCustomized;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("airbender", "basic");
        let elf = AirbenderRustRv32imaCustomized
            .compile(guest_directory)
            .unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }
}
