use anyhow::{Context, Error};
use clap::Parser;
use serde::Serialize;
use std::{env, fs::File, path::PathBuf};
use tracing_subscriber::EnvFilter;
use zkvm_interface::Compiler;

// Compile-time check to ensure exactly one zkVM feature is enabled for `ere-compiler`
const _: () = {
    assert!(
        (cfg!(feature = "jolt") as u8
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

    let program = compile(args.guest_path)?;

    let output = File::create(args.output_path).with_context(|| "Failed to create output")?;
    bincode::serialize_into(output, &program).with_context(|| "Failed to serialize program")?;

    Ok(())
}

fn compile(guest_path: PathBuf) -> Result<impl Serialize, Error> {
    #[cfg(feature = "jolt")]
    let result = if use_stock_rust() {
        ere_jolt::compiler::RustRv32ima.compile(&guest_path)
    } else {
        ere_jolt::compiler::RustRv32imaCustomized.compile(&guest_path)
    };

    #[cfg(feature = "miden")]
    let result = ere_miden::compiler::MidenAsm.compile(&guest_path);

    #[cfg(feature = "nexus")]
    let result = ere_nexus::compiler::RustRv32i.compile(&guest_path);

    #[cfg(feature = "openvm")]
    let result = if use_stock_rust() {
        ere_openvm::compiler::RustRv32ima.compile(&guest_path)
    } else {
        ere_openvm::compiler::RustRv32imaCustomized.compile(&guest_path)
    };

    #[cfg(feature = "pico")]
    let result = if use_stock_rust() {
        ere_pico::compiler::RustRv32ima.compile(&guest_path)
    } else {
        ere_pico::compiler::RustRv32imaCustomized.compile(&guest_path)
    };

    #[cfg(feature = "risc0")]
    let result = if use_stock_rust() {
        ere_risc0::compiler::RustRv32ima.compile(&guest_path)
    } else {
        ere_risc0::compiler::RustRv32imaCustomized.compile(&guest_path)
    };

    #[cfg(feature = "sp1")]
    let result = if use_stock_rust() {
        ere_sp1::compiler::RustRv32ima.compile(&guest_path)
    } else {
        ere_sp1::compiler::RustRv32imaCustomized.compile(&guest_path)
    };

    #[cfg(feature = "ziren")]
    let result = ere_ziren::compiler::RustMips32r2Customized.compile(&guest_path);

    #[cfg(feature = "zisk")]
    let result = ere_zisk::compiler::RustRv64imaCustomized.compile(&guest_path);

    result.with_context(|| "Failed to compile program")
}

#[allow(dead_code)]
/// Returns whether to use stock Rust compiler instead of customized compiler.
fn use_stock_rust() -> bool {
    env::var_os("ERE_RUST_TOOLCHAIN").is_some()
}
