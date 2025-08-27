use crate::error::CompileError;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use tracing::info;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";

pub fn stock_rust_compile(
    guest_directory: &Path,
    output_directory: &Path,
    program_name: &String,
    toolchain: &String,
) -> Result<(ExitStatus, PathBuf), CompileError> {
    info!(
        "Running `cargo build` (toolchain `{}`) → dir: {}",
        toolchain,
        output_directory.display(),
    );

    let target_name = "riscv32ima-unknown-none-elf";
    let plus_toolchain = format!("+{}", toolchain);

    let args = [
        plus_toolchain.as_str(),
        "build",
        "--target-dir",
        output_directory.to_str().unwrap(),
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
        .map_err(|e| CompileError::CargoProveBuild {
            cwd: guest_directory.to_path_buf(),
            source: e,
        });

    match result {
        Ok(status) => Ok((
            status,
            output_directory
                .join(target_name)
                .join("release")
                .join(program_name),
        )),
        Err(err) => Err(err),
    }
}
