use crate::error::CompileError;
use cargo_metadata::MetadataCommand;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";
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
const CARGO_ARGS: &[&str] = &[
    "build",
    "--release",
    "--features",
    "guest",
    "--target",
    TARGET_TRIPLE,
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
    let metadata = MetadataCommand::new().current_dir(guest_directory).exec()?;
    let package = metadata
        .root_package()
        .ok_or_else(|| CompileError::MissingPackageName {
            path: guest_directory.to_path_buf(),
        })?;

    let plus_toolchain = format!("+{}", toolchain);
    let mut cargo_args = [plus_toolchain.as_str()].to_vec();
    cargo_args.append(&mut CARGO_ARGS.to_vec());

    let mut encoded_rust_flags = RUSTFLAGS.to_vec();
    let temp_output_dir = TempDir::new_in(guest_directory).unwrap();
    let linker_script_path = make_linker_script(temp_output_dir.path(), &package.name)?;
    let linker_path = format!("link-arg=-T{}", linker_script_path.display());
    encoded_rust_flags.append(&mut ["-C", &linker_path].to_vec());

    let encoded_rust_flags_str = encoded_rust_flags.join(CARGO_ENCODED_RUSTFLAGS_SEPARATOR);

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
        .env("CARGO_ENCODED_RUSTFLAGS", &encoded_rust_flags_str)
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

const DEFAULT_MEMORY_SIZE: u64 = 10 * 1024 * 1024;
const DEFAULT_STACK_SIZE: u64 = 4096;
const LINKER_SCRIPT_TEMPLATE: &str = include_str!("template.ld");

fn make_linker_script(
    temp_output_dir_path: &Path,
    program_name: &String,
) -> Result<PathBuf, CompileError> {
    let linker_path = temp_output_dir_path.join(format!("{}.ld", program_name));

    let linker_script = LINKER_SCRIPT_TEMPLATE
        .replace("{MEMORY_SIZE}", &DEFAULT_MEMORY_SIZE.to_string())
        .replace("{STACK_SIZE}", &DEFAULT_STACK_SIZE.to_string());

    let mut file = File::create(&linker_path).expect("could not create linker file");
    file.write_all(linker_script.as_bytes())
        .expect("could not save linker");

    Ok(linker_path)
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
