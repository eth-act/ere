use anyhow::{Context, Error};
use clap::Parser;
use ere_catalog::CompilerKind;
use ere_compiler_core::{Compiler, Elf};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

// Compile-time check to ensure exactly one zkVMProver feature is enabled for `ere-compiler`
const _: () = {
    assert!(
        (cfg!(feature = "airbender") as u8
            + cfg!(feature = "openvm") as u8
            + cfg!(feature = "risc0") as u8
            + cfg!(feature = "sp1") as u8
            + cfg!(feature = "zisk") as u8)
            == 1,
        "Exactly one zkVMProver feature must be enabled for `ere-compiler`"
    );
};

#[derive(Parser)]
#[command(author, version)]
struct Args {
    /// Compiler kind to use
    #[arg(long, value_parser = <CompilerKind as std::str::FromStr>::from_str)]
    compiler_kind: CompilerKind,
    /// Directory of the guest program
    #[arg(long)]
    guest_dir: PathBuf,
    /// Directory where the compiled ELF will be written
    #[arg(long)]
    output_dir: PathBuf,
    /// Name of the output ELF file (optional)
    #[arg(long)]
    elf_name: Option<String>,
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Create output directory if it doesn't exist
    if !args.output_dir.exists() {
        std::fs::create_dir_all(&args.output_dir)
            .with_context(|| "Failed to create output directory")?;
    }

    let elf = compile(args.guest_dir, args.compiler_kind)?;

    if let Some(elf_name) = args.elf_name {
        let path = args.output_dir.join(elf_name);
        std::fs::write(&path, &elf).with_context(|| format!("Failed to write ELF to {path:?}"))?;
    }

    Ok(())
}

fn compile(guest_dir: PathBuf, compiler_kind: CompilerKind) -> Result<Elf, Error> {
    #[cfg(feature = "airbender")]
    let elf = {
        use ere_compiler_airbender::*;
        match compiler_kind {
            CompilerKind::Rust | CompilerKind::RustCustomized => {
                AirbenderRustRv32ima.compile(guest_dir)?
            }
            _ => anyhow::bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "openvm")]
    let elf = {
        use ere_compiler_openvm::*;
        match compiler_kind {
            CompilerKind::Rust => OpenVMRustRv32ima.compile(guest_dir)?,
            CompilerKind::RustCustomized => OpenVMRustRv32imaCustomized.compile(guest_dir)?,
            _ => anyhow::bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "risc0")]
    let elf = {
        use ere_compiler_risc0::*;
        match compiler_kind {
            CompilerKind::Rust => Risc0RustRv32ima.compile(guest_dir)?,
            CompilerKind::RustCustomized => Risc0RustRv32imaCustomized.compile(guest_dir)?,
            _ => anyhow::bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "sp1")]
    let elf = {
        use ere_compiler_sp1::*;
        match compiler_kind {
            CompilerKind::Rust => SP1RustRv64ima.compile(guest_dir)?,
            CompilerKind::RustCustomized => SP1RustRv64imaCustomized.compile(guest_dir)?,
            _ => anyhow::bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "zisk")]
    let elf = {
        use ere_compiler_zisk::*;
        match compiler_kind {
            CompilerKind::Rust => ZiskRustRv64ima.compile(guest_dir)?,
            CompilerKind::RustCustomized => ZiskRustRv64imaCustomized.compile(guest_dir)?,
            CompilerKind::GoCustomized => ZiskGoCustomized.compile(guest_dir)?,
        }
    };

    Ok(elf)
}

#[allow(dead_code)]
fn unsupported_compiler_kind_err(
    compiler_kind: CompilerKind,
    supported: impl IntoIterator<Item = CompilerKind>,
) -> anyhow::Error {
    let supported = supported.into_iter().collect::<Vec<_>>();
    anyhow::anyhow!("Unsupported compiler kind {compiler_kind:?}, expect one of {supported:?}",)
}
