use std::fs;
use crate::error::CompileError;
use crate::compile::Risc0Program;
use std::path::{Path};
use std::process::{Command};
use cargo_metadata::MetadataCommand;
use tracing::info;
use risc0_binfmt::ProgramBinary;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";
// TODO: Make this with `zkos` package building to avoid binary file storing in repo. 
const V1COMPAT_ELF: &[u8] = include_bytes!("kernel_elf/v1compat.elf"); 

pub fn compile_risc0_program_stock_rust(
    guest_directory: &Path,
    toolchain: &String,
) -> Result<Risc0Program, CompileError> {

    let metadata = MetadataCommand::new().current_dir(guest_directory).exec()?;
    let package = metadata
        .root_package()
        .ok_or_else(|| CompileError::MissingPackageName {
            path: guest_directory.to_path_buf(),
        })?;

    let target_name = "riscv32ima-unknown-none-elf";
    let plus_toolchain = format!("+{}", toolchain);

    let args = [
        plus_toolchain.as_str(),
        "build",
        "--target",
        target_name,
        "--release",
        // For bare metal we have to build core and alloc
        "-Zbuild-std=core,alloc",
    ];

    let rust_flags = [
        "-C",
        "passes=lower-atomic", // Only for rustc > 1.81
        "-C",
        // Start of the code section
        "link-arg=-Ttext=0x00201000",
        "-C",
        // The lowest memory location that will be used when your program is loaded
        "link-arg=--image-base=0x00200800",
        "-C",
        "panic=abort",
        "--cfg",
        "getrandom_backend=\"custom\"",
        "-C",
        "llvm-args=-misched-prera-direction=bottomup",
        "-C",
        "llvm-args=-misched-postra-direction=bottomup",
    ];

    let encoded_rust_flags = rust_flags
        .into_iter()
        .collect::<Vec<_>>()
        .join(CARGO_ENCODED_RUSTFLAGS_SEPARATOR);

    let result = Command::new("cargo")
        .current_dir(guest_directory)
        .env("CARGO_ENCODED_RUSTFLAGS", &encoded_rust_flags)
        .args(args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|source| CompileError::Risc0BuildFailure {
            source: source.into(),
            crate_path: guest_directory.to_path_buf()
        });

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    let elf_path =
        guest_directory
            .join("target")
            .join(target_name)
            .join("release")
            .join(&package.name);

    let elf = fs::read(&elf_path).map_err(|e| CompileError::ReadFile {
        path: elf_path,
        source: e,
    })?;

    let program = ProgramBinary::new(elf.as_slice(), V1COMPAT_ELF);
    let image_id = program.compute_image_id().unwrap();
    info!("Risc0 program compiled (toolchain {}) OK - {} bytes", toolchain, elf.len());
    info!("Image ID - {image_id}");

    Ok(Risc0Program{elf: program.encode(), image_id})
}

#[cfg(test)]
mod tests {
    use test_utils::host::testing_guest_directory;
    use crate::compile_stock_rust::compile_risc0_program_stock_rust;

    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory(
            "risc0",
            "stock_nightly_no_std");
        let program = compile_risc0_program_stock_rust(&guest_directory, &"nightly".to_string()).unwrap();
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }
}
