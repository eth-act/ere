use crate::OpenVMProgram;
use crate::error::CompileError;
use compile_utils::CargoBuildCmd;
use openvm_sdk::config::{AppConfig, DEFAULT_APP_LOG_BLOWUP, DEFAULT_LEAF_LOG_BLOWUP, SdkVmConfig};
use openvm_stark_sdk::config::FriParameters;
use std::fs;
use std::path::Path;
use tracing::info;

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to https://github.com/openvm-org/openvm/blob/v1.4.0/crates/toolchain/build/src/lib.rs#L291
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
const CARGO_BUILD_OPTIONS: &[&str] = &[
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
    let elf = CargoBuildCmd::new()
        .toolchain(toolchain)
        .build_options(CARGO_BUILD_OPTIONS)
        .rustflags(RUSTFLAGS)
        .exec(guest_directory, TARGET_TRIPLE)?;

    Ok(elf)
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
