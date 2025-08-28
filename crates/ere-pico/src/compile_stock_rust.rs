use std::fs;
use crate::error::PicoError;
use std::path::Path;
use std::process::Command;
use cargo_metadata::MetadataCommand;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";

pub fn compile_pico_program_stock_rust(
    guest_directory: &Path,
    toolchain: &String,
) -> Result<Vec<u8>, PicoError> {

    let metadata = MetadataCommand::new().current_dir(guest_directory).exec()?;
    let package = metadata
        .root_package()
        .ok_or_else(|| PicoError::MissingPackageName {
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
        .map_err(|source| PicoError::BuildFailure {
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

    fs::read(&elf_path).map_err(|e| PicoError::ReadFile {
        path: elf_path,
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use test_utils::host::testing_guest_directory;
    use crate::compile_stock_rust::compile_pico_program_stock_rust;

    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory(
            "pico",
            "stock_nightly_no_std");
        let elf = compile_pico_program_stock_rust(&guest_directory, &"nightly".to_string());
        assert!(!elf.is_err(), "jolt compilation failed");
        assert!(!elf.unwrap().is_empty(), "ELF bytes should not be empty.");
    }
}