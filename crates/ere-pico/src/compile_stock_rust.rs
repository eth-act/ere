use crate::error::CompileError;
use compile_utils::CargoBuildCmd;
use std::path::Path;

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// According to https://github.com/brevis-network/pico/blob/v1.1.7/sdk/cli/src/build/build.rs#L104
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
];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

pub fn compile_pico_program_stock_rust(
    guest_directory: &Path,
    toolchain: &String,
) -> Result<Vec<u8>, CompileError> {
    compile_program_stock_rust(guest_directory, toolchain)
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

#[cfg(test)]
mod tests {
    use crate::compile_stock_rust::compile_pico_program_stock_rust;
    use test_utils::host::testing_guest_directory;

    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory("pico", "stock_nightly_no_std");
        let result = compile_pico_program_stock_rust(&guest_directory, &"nightly".to_string());
        assert!(result.is_ok(), "Pico guest program compilation failure.");
        assert!(
            !result.unwrap().is_empty(),
            "ELF bytes should not be empty."
        );
    }
}
