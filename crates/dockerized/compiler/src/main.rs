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
    /// Path to the guest program
    #[arg(long)]
    guest_path: PathBuf,
    /// Path where the compiled program will be written
    #[arg(long)]
    output_path: PathBuf,
}

fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let program = compile(args.guest_path, args.compiler_kind)?;

    let mut output = File::create(args.output_path).with_context(|| "Failed to create output")?;
    bincode::serde::encode_into_std_write(&program, &mut output, bincode::config::legacy())
        .with_context(|| "Failed to serialize program")?;

    Ok(())
}

fn compile(guest_path: PathBuf, compiler_kind: CompilerKind) -> Result<impl Serialize, Error> {
    #[cfg(feature = "airbender")]
    let result = {
        use ere_airbender::compiler::*;
        match compiler_kind {
            CompilerKind::Rust | CompilerKind::RustCustomized => RustRv32ima.compile(&guest_path),
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
            CompilerKind::Rust => RustRv64imac.compile(&guest_path),
            CompilerKind::RustCustomized => RustRv64imacCustomized.compile(&guest_path),
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
            CompilerKind::MidenAsm => MidenAsm.compile(&guest_path),
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
            CompilerKind::Rust | CompilerKind::RustCustomized => RustRv32i.compile(&guest_path),
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
            CompilerKind::Rust => RustRv32ima.compile(&guest_path),
            CompilerKind::RustCustomized => RustRv32imaCustomized.compile(&guest_path),
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
            CompilerKind::Rust => RustRv32ima.compile(&guest_path),
            CompilerKind::RustCustomized => RustRv32imaCustomized.compile(&guest_path),
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
            CompilerKind::Rust => RustRv32ima.compile(&guest_path),
            CompilerKind::RustCustomized => RustRv32imaCustomized.compile(&guest_path),
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
            CompilerKind::Rust => RustRv32ima.compile(&guest_path),
            CompilerKind::RustCustomized => RustRv32imaCustomized.compile(&guest_path),
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
            CompilerKind::RustCustomized => RustMips32r2Customized.compile(&guest_path),
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
            CompilerKind::RustCustomized => RustRv64imaCustomized.compile(&guest_path),
            CompilerKind::GoCustomized => GoCustomized.compile(&guest_path),
            _ => bail!(unsupported_compiler_kind_err(
                compiler_kind,
                [CompilerKind::RustCustomized, CompilerKind::GoCustomized]
            )),
        }
    };

    result.with_context(|| "Failed to compile program")
}

fn unsupported_compiler_kind_err(
    compiler_kind: CompilerKind,
    supported: impl IntoIterator<Item = CompilerKind>,
) -> anyhow::Error {
    let supported = supported.into_iter().collect::<Vec<_>>();
    anyhow::anyhow!("Unsupported compiler kind {compiler_kind:?}, expect one of {supported:?}",)
}
