use std::fs;
use crate::error::CompileError;
use std::path::{Path};
use std::process::{Command};
use cargo_metadata::MetadataCommand;
use tracing::info;
use openvm_sdk::config::{AppConfig, SdkVmConfig, DEFAULT_APP_LOG_BLOWUP, DEFAULT_LEAF_LOG_BLOWUP};
use openvm_stark_sdk::config::FriParameters;
use crate::OpenVMProgram;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";

pub fn compile_openvm_program_stock_rust(
    guest_directory: &Path,
    toolchain: &String,
) -> Result<OpenVMProgram, CompileError> {

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
        .map_err(|source| CompileError::OpenVMBuildFailure {
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

    let elf = fs::read(&elf_path).map_err(|e| CompileError::ReadElfFailed {
        path: elf_path,
        source: e,
    })?;

    let app_config_path = guest_directory.join("openvm.toml");
    let app_config = if app_config_path.exists() {
        let toml = fs::read_to_string(&app_config_path).map_err(|source| {
            CompileError::ReadConfigFailed {
                source,
                path: app_config_path.to_path_buf(),
            }
        })?;
        toml::from_str(&toml).map_err(CompileError::DeserializeConfigFailed)?
    } else {
        // The default `AppConfig` copied from https://github.com/openvm-org/openvm/blob/ca36de3/crates/cli/src/default.rs#L31.
        AppConfig {
            app_fri_params: FriParameters::standard_with_100_bits_conjectured_security(
                DEFAULT_APP_LOG_BLOWUP,
            )
                .into(),
            // By default it supports RISCV32IM with IO but no precompiles.
            app_vm_config: SdkVmConfig::builder()
                .system(Default::default())
                .rv32i(Default::default())
                .rv32m(Default::default())
                .io(Default::default())
                .build(),
            leaf_fri_params: FriParameters::standard_with_100_bits_conjectured_security(
                DEFAULT_LEAF_LOG_BLOWUP,
            )
                .into(),
            compiler_options: Default::default(),
        }
    };

    info!("Openvm program compiled (toolchain {}) OK - {} bytes", toolchain, elf.len());

    Ok(OpenVMProgram { elf, app_config })
}

#[cfg(test)]
mod tests {
    use test_utils::host::testing_guest_directory;
    use crate::compile_stock_rust::compile_openvm_program_stock_rust;

    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory(
            "openvm",
            "stock_nightly_no_std");
        let program = compile_openvm_program_stock_rust(&guest_directory, &"nightly".to_string()).unwrap();
        assert!(!program.elf.is_empty(), "ELF bytes should not be empty.");
    }
}
