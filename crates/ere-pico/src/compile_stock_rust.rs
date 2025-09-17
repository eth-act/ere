use crate::error::CompileError;
use cargo_metadata::MetadataCommand;
use std::fs;
use std::path::Path;
use std::process::Command;

static CARGO_ENCODED_RUSTFLAGS_SEPARATOR: &str = "\x1f";

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
const CARGO_ARGS: &[&str] = &[
    "build",
    "--target",
    TARGET_TRIPLE,
    "--release",
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
    let metadata = MetadataCommand::new().current_dir(guest_directory).exec()?;
    let package = metadata
        .root_package()
        .ok_or_else(|| CompileError::MissingPackageName {
            path: guest_directory.to_path_buf(),
        })?;

    let plus_toolchain = format!("+{}", toolchain);
    let mut cargo_args = [plus_toolchain.as_str()].to_vec();
    cargo_args.append(&mut CARGO_ARGS.to_vec());

    let encoded_rust_flags = RUSTFLAGS.to_vec().join(CARGO_ENCODED_RUSTFLAGS_SEPARATOR);

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
        .env("CARGO_ENCODED_RUSTFLAGS", &encoded_rust_flags)
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
