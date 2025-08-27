use std::fs;
use std::fs::File;
use crate::error::CompileError;
use std::io::Write;
use std::path::Path;
use std::process::{Command};
use cargo_metadata::MetadataCommand;
use tempfile::TempDir;
use tracing::info;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";

pub fn compile_jolt_program_stock_rust(
    guest_directory: &Path,
    toolchain: &String,
) -> Result<Vec<u8>, CompileError> {

    let metadata = MetadataCommand::new().current_dir(guest_directory).exec()?;
    let package = metadata
        .root_package()
        .ok_or_else(|| CompileError::MissingPackageName {
            path: guest_directory.to_path_buf(),
        })?;

    let target_name = "riscv32im-unknown-none-elf";
    let plus_toolchain = format!("+{}", toolchain);

    let args = [
        plus_toolchain.as_str(),
        "build",
        "--target",
        target_name,
        "--release",
        // For bare metal we have to build core and alloc
        "-Zbuild-std=core,alloc",
        "--features",
        "guest"
    ];

    let temp_output_dir = TempDir::new_in(guest_directory).unwrap();
    let temp_output_dir_path = temp_output_dir.path();

    let linker_path = temp_output_dir_path.join(format!("{}.ld", &package.name));
    let linker_path_str = linker_path.to_str().unwrap();

    let linker_script = LINKER_SCRIPT_TEMPLATE
        .replace("{MEMORY_SIZE}", &DEFAULT_MEMORY_SIZE.to_string())
        .replace("{STACK_SIZE}", &DEFAULT_STACK_SIZE.to_string());

    let mut file = File::create(&linker_path).expect("could not create linker file");
    file.write_all(linker_script.as_bytes())
        .expect("could not save linker");

    let rust_flags = [
        "-C",
        &format!("link-arg=-T{}", linker_path_str),
        "-C",
        "passes=lower-atomic",
        "-C",
        "panic=abort",
        "-C",
        "strip=symbols",
        "-C",
        "opt-level=z",
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
        .map_err(|source| CompileError::BuildFailure {
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

    info!("Jolt program compiled (toolchain {}) OK - {} bytes", toolchain, elf.len());

    Ok(elf)
}

pub const DEFAULT_MEMORY_SIZE: u64 = 10 * 1024 * 1024;
pub const DEFAULT_STACK_SIZE: u64 = 4096;

const LINKER_SCRIPT_TEMPLATE: &str = r#"
MEMORY {
  program (rwx) : ORIGIN = 0x80000000, LENGTH = {MEMORY_SIZE}
}

SECTIONS {
  .text.boot : {
    *(.text.boot)
  } > program

  .text : {
    *(.text)
  } > program

  .data : {
    *(.data)
  } > program

  .bss : {
    *(.bss)
  } > program

  . = ALIGN(8);
  . = . + {STACK_SIZE};
  _STACK_PTR = .;
  . = ALIGN(8);
  _HEAP_PTR = .;
}
"#;


#[cfg(test)]
mod tests {
    use test_utils::host::testing_guest_directory;
    use crate::compile_stock_rust::compile_jolt_program_stock_rust;

    #[test]
    fn test_stock_compiler_impl() {
        let guest_directory = testing_guest_directory(
            "jolt",
            "stock_nightly_no_std");
        let program = compile_jolt_program_stock_rust(&guest_directory, &"nightly".to_string());

        assert!(!program.is_err(), "jolt compilation failed");
        assert!(!program.unwrap().is_empty(), "ELF bytes should not be empty.");
    }
}
