use std::{env, path::Path};

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::CargoBuildCmd;

use crate::Error;

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to https://github.com/openvm-org/openvm/blob/v1.4.3/crates/toolchain/build/src/lib.rs#L291
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

/// Compiler for Rust guest program to RV32IMA architecture.
pub struct OpenVMRustRv32ima;

impl Compiler for OpenVMRustRv32ima {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)?;
        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use ere_compiler_core::Compiler;
    use ere_prover_core::{Input, ProverResource, zkVMProver};
    use ere_prover_openvm::OpenVMProver;
    use ere_util_test::host::testing_guest_directory;

    use crate::OpenVMRustRv32ima;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("openvm", "stock_nightly_no_std");
        let elf = OpenVMRustRv32ima.compile(guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("openvm", "stock_nightly_no_std");
        let elf = OpenVMRustRv32ima.compile(guest_directory).unwrap();
        let zkvm = OpenVMProver::new(elf, ProverResource::Cpu).unwrap();
        zkvm.execute(&Input::new()).unwrap();
    }
}
