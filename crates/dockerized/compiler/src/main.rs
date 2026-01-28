use anyhow::{Context, Error, bail};
use clap::Parser;
use ere_common::CompilerKind;
use ere_zkvm_interface::compiler::Compiler;
use serde::Serialize;
use std::{fs::File, path::PathBuf};
use tracing_subscriber::EnvFilter;

// Compile-time check to ensure exactly one zkVM feature is enabled for `ere-compiler`
const _: () = {
    assert!(
        (cfg!(feature = "airbender") as u8
            + cfg!(feature = "jolt") as u8
            + cfg!(feature = "miden") as u8
            + cfg!(feature = "nexus") as u8
            + cfg!(feature = "openvm") as u8
            + cfg!(feature = "pico") as u8
            + cfg!(feature = "risc0") as u8
            + cfg!(feature = "sp1") as u8
            + cfg!(feature = "ziren") as u8
            + cfg!(feature = "zisk") as u8)
            == 1,
        "Exactly one zkVM feature must be enabled for `ere-compiler`"
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
    /// Directory where the compiled program/artifacts will be written
    #[arg(long)]
    output_dir: PathBuf,
    /// Name of the output ELF file (optional)
    #[arg(long)]
    elf_name: Option<String>,
    /// Name of the output digest file (optional, only for supported zkVMs)
    #[arg(long)]
    digest_name: Option<String>,
    /// Name of the output serialized program file (optional)
    #[arg(long)]
    program_name: Option<String>,
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

    let (elf, digest, program) = compile(args.guest_dir, args.compiler_kind)?;

    if let Some(elf_name) = args.elf_name {
        if let Some(elf_bytes) = elf {
            let path = args.output_dir.join(elf_name);
            std::fs::write(&path, &elf_bytes)
                .with_context(|| format!("Failed to write ELF to {path:?}"))?;
        } else {
            tracing::warn!("ELF output requested but not available/supported for this zkVM.");
        }
    }

    if let Some(digest_name) = args.digest_name {
        if let Some(digest_bytes) = digest {
            let path = args.output_dir.join(digest_name);
            std::fs::write(&path, &digest_bytes)
                .with_context(|| format!("Failed to write digest to {path:?}"))?;
        } else {
            tracing::warn!("Digest output requested but not available/supported for this zkVM.");
        }
    }

    if let Some(program_name) = args.program_name {
        let path = args.output_dir.join(program_name);
        let mut output =
            File::create(&path).with_context(|| "Failed to create program output file")?;
        bincode::serde::encode_into_std_write(&program, &mut output, bincode::config::legacy())
            .with_context(|| "Failed to serialize program")?;
    }

    Ok(())
}

/// Compiles the guest and returns (Optional ELF bytes, Optional Digest bytes, Serialized Program)
fn compile(
    guest_dir: PathBuf,
    compiler_kind: CompilerKind,
) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>, impl Serialize), Error> {
    #[cfg(feature = "airbender")]
    let result = {
        use ere_airbender::compiler::*;
        match compiler_kind {
            CompilerKind::Rust | CompilerKind::RustCustomized => {
                let program = RustRv32ima.compile(&guest_dir)?;
                let elf = program.elf().to_vec();
                (Some(elf), None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "jolt")]
    let result = {
        use ere_jolt::compiler::*;
        match compiler_kind {
            CompilerKind::Rust => {
                let program = RustRv64imac.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            CompilerKind::RustCustomized => {
                let program = RustRv64imacCustomized.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "miden")]
    let result = {
        use ere_miden::compiler::*;
        match compiler_kind {
            CompilerKind::MidenAsm => {
                let program = MidenAsm.compile(&guest_dir)?;
                (None, None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::MidenAsm]
            )),
        }
    };

    #[cfg(feature = "nexus")]
    let result = {
        use ere_nexus::compiler::*;
        match compiler_kind {
            CompilerKind::Rust | CompilerKind::RustCustomized => {
                let program = RustRv32i.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "openvm")]
    let result = {
        use ere_openvm::compiler::*;
        match compiler_kind {
            CompilerKind::Rust => {
                let program = RustRv32ima.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            CompilerKind::RustCustomized => {
                let program = RustRv32imaCustomized.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "pico")]
    let result = {
        use ere_pico::compiler::*;
        match compiler_kind {
            CompilerKind::Rust => {
                let program = RustRv32ima.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            CompilerKind::RustCustomized => {
                let program = RustRv32imaCustomized.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "risc0")]
    let result = {
        use ere_risc0::compiler::*;
        match compiler_kind {
            CompilerKind::Rust => {
                let program = RustRv32ima.compile(&guest_dir)?;
                let elf = program.elf().to_vec();
                let digest = program.image_id().as_bytes().to_vec();
                (Some(elf), Some(digest), program)
            }
            CompilerKind::RustCustomized => {
                let program = RustRv32imaCustomized.compile(&guest_dir)?;
                let elf = program.elf().to_vec();
                let digest = program.image_id().as_bytes().to_vec();
                (Some(elf), Some(digest), program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "sp1")]
    let result = {
        use ere_sp1::compiler::*;
        match compiler_kind {
            CompilerKind::Rust => {
                let program = RustRv32ima.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            CompilerKind::RustCustomized => {
                let program = RustRv32imaCustomized.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::Rust, CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "ziren")]
    let result = {
        use ere_ziren::compiler::*;
        match compiler_kind {
            CompilerKind::RustCustomized => {
                let program = RustMips32r2Customized.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::RustCustomized]
            )),
        }
    };

    #[cfg(feature = "zisk")]
    let result = {
        use ere_zisk::compiler::*;
        match compiler_kind {
            CompilerKind::RustCustomized => {
                let program = RustRv64imaCustomized.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            CompilerKind::GoCustomized => {
                let program = GoCustomized.compile(&guest_dir)?;
                (Some(program.elf().to_vec()), None, program)
            }
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::RustCustomized, CompilerKind::GoCustomized]
            )),
        }
    };

    Ok(result)
}

fn unsupported_compiler_kind_err(
    compiler_kind: CompilerKind,
    supported: impl IntoIterator<Item = CompilerKind>,
) -> anyhow::Error {
    let supported = supported.into_iter().collect::<Vec<_>>();
    anyhow::anyhow!("Unsupported compiler kind {compiler_kind:?}, expect one of {supported:?}",)
}
