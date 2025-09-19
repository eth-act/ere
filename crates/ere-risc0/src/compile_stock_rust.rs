use crate::compile::Risc0Program;
use crate::error::CompileError;
use compile_utils::CargoBuildCmd;
use risc0_binfmt::ProgramBinary;
use std::path::Path;
use tracing::info;

// TODO: Make this with `zkos` package building to avoid binary file storing in repo.
// File taken from https://github.com/risc0/risc0/blob/v3.0.3/risc0/zkos/v1compat/elfs/v1compat.elf
const V1COMPAT_ELF: &[u8] = include_bytes!("kernel_elf/v1compat.elf");
const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to https://github.com/risc0/risc0/blob/v3.0.3/risc0/build/src/lib.rs#L455
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
const CARGO_BUILD_OPTIONS: &[&str] = &[
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
    let elf = CargoBuildCmd::new()
        .toolchain(toolchain)
        .build_options(CARGO_BUILD_OPTIONS)
        .rustflags(RUSTFLAGS)
        .exec(guest_directory, TARGET_TRIPLE)?;

    Ok(elf)
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
