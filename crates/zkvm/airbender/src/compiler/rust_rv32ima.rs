use crate::{compiler::AirbenderProgram, error::AirbenderError};
use ere_compile_utils::CargoBuildCmd;
use ere_zkvm_interface::Compiler;
use std::{env, path::Path};

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to TODO
const RUSTFLAGS: &[&str] = &[
    // Replace atomic ops with nonatomic versions since the guest is single threaded.
    "-C",
    "passes=lower-atomic",
    "-C",
    "link-arg=--save-temps",
    "-C",
    "force-frame-pointers",
    "-C",
    "panic=abort",
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
pub struct RustRv32ima;

impl Compiler for RustRv32ima {
    type Error = AirbenderError;

    type Program = AirbenderProgram;

    fn compile(&self, guest_directory: &Path) -> Result<Self::Program, Self::Error> {
        let toolchain = env::var("ERE_RUST_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
        let elf = CargoBuildCmd::new()
            .linker_script(Some(LINKER_SCRIPT))
            .toolchain(toolchain)
            .build_options(CARGO_BUILD_OPTIONS)
            .rustflags(RUSTFLAGS)
            .exec(guest_directory, TARGET_TRIPLE)?;
        let bin = objcopy_binary(&elf);
        Ok(bin)
    }
}

fn objcopy_binary(elf: &[u8]) -> Vec<u8> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("rust-objcopy")
        .args(["-O", "binary", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn rust-objcopy");

    child
        .stdin
        .as_mut()
        .expect("Failed to open stdin")
        .write_all(elf)
        .expect("Failed to write ELF to stdin");

    let output = child
        .wait_with_output()
        .expect("Failed to wait for rust-objcopy");

    if !output.status.success() {
        panic!(
            "rust-objcopy failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    output.stdout
}

#[cfg(test)]
mod tests {
    use crate::{EreAirbender, compiler::RustRv32ima};
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::{Compiler, ProverResourceType, zkVM};

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("airbender", "stock_nightly_no_std");
        let bin = RustRv32ima.compile(&guest_directory).unwrap();
        assert!(!bin.is_empty(), "ELF bytes should not be empty.");
    }

    #[test]
    fn test_execute() {
        let guest_directory = testing_guest_directory("airbender", "stock_nightly_no_std");
        let program = RustRv32ima.compile(&guest_directory).unwrap();
        let zkvm = EreAirbender::new(program, ProverResourceType::Cpu);

        zkvm.execute(&[]).unwrap();
    }
}
