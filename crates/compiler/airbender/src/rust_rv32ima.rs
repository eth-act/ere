use std::{env, path::Path};

use ere_compiler_core::{Compiler, Elf};
use ere_util_compile::CargoBuildCmd;

use crate::Error;

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to https://github.com/matter-labs/zksync-airbender/blob/v0.5.2/examples/dynamic_fibonacci/.cargo/config.toml.
const RUSTFLAGS: &[&str] = &[
    // Replace atomic ops with nonatomic versions since the guest is single threaded.
    "-C",
    "passes=lower-atomic",
    "-C",
    "target-feature=-unaligned-scalar-mem",
];
const CARGO_BUILD_OPTIONS: &[&str] = &[
    // For bare metal we have to build core and alloc
    "-Zbuild-std=core,alloc",
];

const LINKER_SCRIPT: &str = concat!(
    include_str!("rust_rv32ima/memory.x"),
    include_str!("rust_rv32ima/link.x"),
);

/// Compiler for Rust guest program to RV32IMA architecture.
pub struct AirbenderRustRv32ima;

impl Compiler for AirbenderRustRv32ima {
    type Error = Error;

    fn compile(&self, guest_directory: impl AsRef<Path>) -> Result<Elf, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .linker_script(Some(LINKER_SCRIPT))
            .toolchain(&toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)?;
        Ok(Elf(elf))
    }
}

#[cfg(test)]
mod tests {
    use ere_compiler_core::Compiler;
    use ere_util_test::host::testing_guest_directory;

    use crate::AirbenderRustRv32ima;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("airbender", "basic");
        let elf = AirbenderRustRv32ima.compile(guest_directory).unwrap();
        assert!(!elf.is_empty(), "ELF should not be empty.");
    }
}
