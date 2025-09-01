use crate::OpenVMProgram;
use crate::error::CompileError;
use cargo_metadata::MetadataCommand;
use openvm_sdk::config::{AppConfig, DEFAULT_APP_LOG_BLOWUP, DEFAULT_LEAF_LOG_BLOWUP, SdkVmConfig};
use openvm_stark_sdk::config::FriParameters;
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::info;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";
const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to https://github.com/openvm-org/openvm/blob/v1.4.0-rc.8/crates/toolchain/build/src/lib.rs#L291
const RUSTFLAGS: &[&str] = &[
    // Replace atomic ops with nonatomic versions since the guest is single threaded.
    "-C",
    "passes=lower-atomic",
    // Specify where to start loading the program in
    // memory.  The clang linker understands the same
    // command line arguments as the GNU linker does; see
    // https://ftp.gnu.org/old-gnu/Manuals/ld-2.9.1/html_mono/ld.html#SEC3
    // for details.
    "-C",
    "link-arg=-Ttext=0x00200800",
    // Apparently not having an entry point is only a linker warning(!), so
    // error out in this case.
    "-C",
    "link-arg=--fatal-warnings",
    "-C",
    "panic=abort",
    // https://docs.rs/getrandom/0.3.2/getrandom/index.html#opt-in-backends
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

pub fn compile_openvm_program_stock_rust(
    guest_directory: &Path,
    toolchain: &String,
) -> Result<OpenVMProgram, CompileError> {
    wrap_into_openvm_program(
        compile_program_stock_rust(guest_directory, toolchain)?,
        guest_directory,
    )
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

fn wrap_into_openvm_program(
    elf: Vec<u8>,
    guest_directory: &Path,
) -> Result<OpenVMProgram, CompileError> {
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

    info!("Openvm program compiled OK - {} bytes", elf.len());

    Ok(OpenVMProgram { elf, app_config })
}

#[cfg(test)]
mod tests {
    use crate::compile_stock_rust::compile_openvm_program_stock_rust;
    use test_utils::host::testing_guest_directory;

    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory("openvm", "stock_nightly_no_std");
        let result = compile_openvm_program_stock_rust(&guest_directory, &"nightly".to_string());
        assert!(result.is_ok(), "Openvm guest program compilation failure.");
        assert!(
            !result.unwrap().elf.is_empty(),
            "ELF bytes should not be empty."
        );
    }
}
