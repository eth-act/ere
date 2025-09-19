use crate::error::CompileError;
use compile_utils::CargoBuildCmd;
use std::path::Path;

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
const RUSTFLAGS: &[&str] = &[
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

const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

pub fn stock_rust_compile(
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
