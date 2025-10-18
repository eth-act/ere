use crate::{compiler::AirbenderProgram, error::AirbenderError};
use ere_compile_utils::CargoBuildCmd;
use ere_zkvm_interface::Compiler;
use std::{
    env,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

const TARGET_TRIPLE: &str = "riscv32ima-unknown-none-elf";
// Rust flags according to https://github.com/matter-labs/zksync-airbender/blob/v0.5.0/examples/dynamic_fibonacci/.cargo/config.toml.
const RUSTFLAGS: &[&str] = &[
    // Replace atomic ops with nonatomic versions since the guest is single threaded.
    "-C",
    "passes=lower-atomic",
    "-C",
    "target-feature=-unaligned-scalar-mem,+relax",
    "-C",
    "link-arg=--save-temps",
    "-C",
    "force-frame-pointers",
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
        let bin = objcopy_binary(&elf)?;
        Ok(bin)
    }
}

fn objcopy_binary(elf: &[u8]) -> Result<Vec<u8>, AirbenderError> {
    let mut child = Command::new("rust-objcopy")
        .args(["-O", "binary", "-", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(AirbenderError::RustObjcopy)?;

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(elf)
        .map_err(AirbenderError::RustObjcopyStdin)?;

    let output = child
        .wait_with_output()
        .map_err(AirbenderError::RustObjcopy)?;

    if !output.status.success() {
        return Err(AirbenderError::RustObjcopyFailed {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use crate::compiler::RustRv32ima;
    use ere_test_utils::host::testing_guest_directory;
    use ere_zkvm_interface::Compiler;

    #[test]
    fn test_compile() {
        let guest_directory = testing_guest_directory("airbender", "basic");
        let bin = RustRv32ima.compile(&guest_directory).unwrap();
        assert!(!bin.is_empty(), "Binary should not be empty.");
    }
}
