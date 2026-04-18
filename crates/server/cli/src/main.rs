use std::{
    fs,
    io::{self, Read},
};

use anyhow::{Context, Error};
use clap::Parser;
use ere_compiler_core::Elf;
use ere_prover_core::{ProverResource, zkVMProver};
use tracing::info;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
mod otel;

// Compile-time check to ensure exactly one zkVMProver feature is enabled for `ere-server`
const _: () = {
    assert!(
        (cfg!(feature = "airbender") as u8
            + cfg!(feature = "openvm") as u8
            + cfg!(feature = "risc0") as u8
            + cfg!(feature = "sp1") as u8
            + cfg!(feature = "zisk") as u8)
            == 1,
        "Exactly one zkVMProver feature must be enabled for `ere-server`"
    );
};

#[derive(Parser)]
#[command(author, version)]
struct Args {
    /// Port number for the server to listen on.
    #[arg(long, default_value = "3000")]
    port: u16,
    /// Optional path to read the ELF from. If not specified, reads from stdin.
    #[arg(long)]
    elf_path: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    #[command(flatten)]
    Server(ProverResource),
    /// Initialize the zkVM from an ELF and write the encoded program_vk to disk.
    Keygen {
        /// Path to write the encoded program verifying key.
        #[arg(long)]
        program_vk: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    // OpenTelemetry is only wired up for the long-running server; `keygen` is a
    // one-shot that just needs stderr logs.
    let (tracer_provider, otel_layer) = match &args.command {
        Command::Server(_) => crate::otel::init(),
        Command::Keygen { .. } => (None, None),
    };

    tracing_subscriber::registry()
        .with(otel_layer)
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_filter(EnvFilter::from_default_env()),
        )
        .init();

    // Read ELF from file or stdin.
    let elf = if let Some(path) = args.elf_path {
        let bytes = fs::read(&path).with_context(|| format!("failed to read ELF from {path}"))?;
        info!("loaded ELF from {path}");
        Elf(bytes)
    } else {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read ELF from stdin")?;
        info!("read ELF from stdin");
        Elf(bytes)
    };

    match args.command {
        Command::Server(resource) => commands::server::run(args.port, elf, resource).await?,
        Command::Keygen { program_vk } => commands::keygen::run(elf, &program_vk)?,
    }

    if let Some(provider) = tracer_provider {
        provider.shutdown().ok();
    }

    Ok(())
}

pub(crate) fn construct_zkvm(elf: Elf, resource: ProverResource) -> Result<impl zkVMProver, Error> {
    #[cfg(feature = "airbender")]
    let zkvm = ere_prover_airbender::AirbenderProver::new(elf, resource);

    #[cfg(feature = "openvm")]
    let zkvm = ere_prover_openvm::OpenVMProver::new(elf, resource);

    #[cfg(feature = "risc0")]
    let zkvm = ere_prover_risc0::Risc0Prover::new(elf, resource);

    #[cfg(feature = "sp1")]
    let zkvm = ere_prover_sp1::SP1Prover::new(elf, resource);

    #[cfg(feature = "zisk")]
    let zkvm = ere_prover_zisk::ZiskProver::new(elf, resource);

    zkvm.with_context(|| "failed to instantiate zkVMProver")
}
