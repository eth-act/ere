use crate::{
    compiler::PicoProgram,
    error::{CompileError, PicoError},
};
use ere_compile_utils::CargoBuildCmd;
use ere_zkvm_interface::Compiler;
use std::{env, path::Path};

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

/// Compiler for Rust guest program to RV32IMA architecture.
pub struct RustRv32ima;

impl Compiler for RustRv32ima {
    type Error = PicoError;

    type Program = PicoProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)
            .map_err(CompileError::CompileUtilError)?;
        Ok(elf)
    }
}

#[cfg(test)]
mod tests {
    use crate::{ErePico, compiler::RustRv32ima};
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{Compiler, Input, ProverResourceType, zkVM};

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("pico", "stock_nightly_no_std");
        let elf = RustRv32ima.compile(&guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("pico", "stock_nightly_no_std");
        let program = RustRv32ima.compile(&guest_directory).unwrap();
        let zkvm = ErePico::new(program, ProverResourceType::Cpu);

        zkvm.execute(&Input::new()).unwrap();
    }
}
