use crate::compile::Risc0Program;
use crate::error::CompileError;
use cargo_metadata::MetadataCommand;
use risc0_binfmt::ProgramBinary;
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::info;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";
// TODO: Make this with `zkos` package building to avoid binary file storing in repo.
// File taken from https://github.com/risc0/risc0/blob/v3.0.1/risc0/zkos/v1compat/elfs/v1compat.elf
const V1COMPAT_ELF: &[u8] = include_bytes!("kernel_elf/v1compat.elf");
const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to https://github.com/risc0/risc0/blob/v3.0.1/risc0/build/src/lib.rs#L455
const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic", // Only for rustc > 1.81
    "-C",
    // Start of the code section
    "link-arg=-Ttext=0x00200800",
    "-C",
    "link-arg=--fatal-warnings",
    "-C",
    "panic=abort",
    "--cfg",
    "getrandom_backend=\"custom\"",
];
const CARGO_ARGS: &[&str] = &[
    "build",
    "--target",
    TARGET_TRIPLE,
    "--release",
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

pub fn compile_risc0_program_stock_rust(
    guest_directory: &Path,
    toolchain: &String,
) -> Result<Risc0Program, CompileError> {
    wrap_into_risc0_program(compile_program_stock_rust(guest_directory, toolchain)?)
}

fn compile_program_stock_rust(
    guest_directory: &Path,
    toolchain: &String,
) -> Result<Vec<u8>, CompileError> {
    let metadata = MetadataCommand::new().current_dir(guest_directory).exec()?;
    let package = metadata
        .root_package()
        .ok_or_else(|| CompileError::MissingPackageName {
            path: guest_directory.to_path_buf(),
        })?;

    let plus_toolchain = format!("+{}", toolchain);
    let mut cargo_args = [plus_toolchain.as_str()].to_vec();
    cargo_args.append(&mut CARGO_ARGS.to_vec());

    let encoded_rust_flags = RUSTFLAGS.to_vec().join(CARGO_ENCODED_RUSTFLAGS_SEPARATOR);

    let target_direcotry = guest_directory
        .join("target")
        .join(TARGET_TRIPLE)
        .join("release");

    // Remove target directory.
    if target_direcotry.exists() {
        fs::remove_dir_all(&target_direcotry).unwrap();
    }

    let result = Command::new("cargo")
        .current_dir(guest_directory)
        .env("CARGO_ENCODED_RUSTFLAGS", &encoded_rust_flags)
        .args(cargo_args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|source| CompileError::BuildFailure {
            source: source.into(),
            crate_path: guest_directory.to_path_buf(),
        });

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    let elf_path = target_direcotry.join(&package.name);

    fs::read(&elf_path).map_err(|e| CompileError::ReadFile {
        path: elf_path,
        source: e,
    })
}

fn wrap_into_risc0_program(elf: Vec<u8>) -> Result<Risc0Program, CompileError> {
    let program = ProgramBinary::new(elf.as_slice(), V1COMPAT_ELF);
    let image_id = program.compute_image_id()?;
    info!("Risc0 program compiled OK - {} bytes", elf.len());
    info!("Image ID - {image_id}");

    Ok(Risc0Program {
        elf: program.encode(),
        image_id,
    })
}

#[cfg(test)]
mod tests {
    use crate::compile_stock_rust::compile_risc0_program_stock_rust;
    use test_utils::host::testing_guest_directory;

    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory("risc0", "stock_nightly_no_std");
        let result = compile_risc0_program_stock_rust(&guest_directory, &"nightly".to_string());
        assert!(result.is_ok(), "Risc0 guest program compilation failure.");
        assert!(
            !result.unwrap().elf.is_empty(),
            "ELF bytes should not be empty."
        );
    }
}
