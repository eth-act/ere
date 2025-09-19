use crate::error::CompileError;
use compile_utils::CargoBuildCmd;
use std::path::Path;

const TARGET_TRIPLE: &str = "riscv32im-unknown-none-elf";
// According to https://github.com/a16z/jolt/blob/55b9830a3944dde55d33a55c42522b81dd49f87a/jolt-core/src/host/mod.rs#L95
const RUSTFLAGS: &[&str] = &[
    "-C",
    "passes=lower-atomic",
    "-C",
    "panic=abort",
    "-C",
    "strip=symbols",
    "-C",
    "opt-level=z",
];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    "--features",
    "guest",
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

pub fn compile_jolt_program_stock_rust(
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
        .linker_script(Some(make_linker_script()))
        .toolchain(toolchain)
        .build_options(CARGO_BUILD_OPTIONS)
        .rustflags(RUSTFLAGS)
        .exec(guest_directory, TARGET_TRIPLE)?;

    Ok(elf)
}

const DEFAULT_MEMORY_SIZE: u64 = 10 * 1024 * 1024;
const DEFAULT_STACK_SIZE: u64 = 4096;
const LINKER_SCRIPT_TEMPLATE: &str = include_str!("template.ld");

fn make_linker_script() -> String {
    LINKER_SCRIPT_TEMPLATE
        .replace("{MEMORY_SIZE}", &DEFAULT_MEMORY_SIZE.to_string())
        .replace("{STACK_SIZE}", &DEFAULT_STACK_SIZE.to_string())
}

#[cfg(test)]
mod tests {
    use crate::compile_stock_rust::compile_jolt_program_stock_rust;
    use test_utils::host::testing_guest_directory;

    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory("jolt", "stock_nightly_no_std");
        let result = compile_jolt_program_stock_rust(&guest_directory, &"nightly".to_string());
        assert!(result.is_ok(), "Jolt guest program compilation failure.");
        assert!(
            !result.unwrap().is_empty(),
            "ELF bytes should not be empty."
        );
    }
}
