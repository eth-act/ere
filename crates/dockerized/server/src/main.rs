use anyhow::{Context, Error};
use clap::Parser;
use ere_server::server::{router, zkVMServer};
use ere_zkvm_interface::{
    compiler::Elf,
    zkvm::{ProverResource, zkVM},
};
use std::{
    io::{self, Read},
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{net::TcpListener, signal};
use tower_http::catch_panic::CatchPanicLayer;
use tracing::info;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};
use twirp::{
    Router,
    axum::{self, routing::get},
    reqwest::StatusCode,
    server::not_found_handler,
};

mod otel;

// Compile-time check to ensure exactly one zkVM feature is enabled for `ere-server`
const _: () = {
    if cfg!(feature = "server") {
        assert!(
            (cfg!(feature = "airbender") as u8
                + cfg!(feature = "openvm") as u8
                + cfg!(feature = "risc0") as u8
                + cfg!(feature = "sp1") as u8
                + cfg!(feature = "zisk") as u8)
                == 1,
            "Exactly one zkVM feature must be enabled for `ere-server`"
        );
    }
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
    /// Prover resource type.
    #[command(subcommand)]
    resource: ProverResource,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let (tracer_provider, otel_layer) = otel::init();

    tracing_subscriber::registry()
        .with(otel_layer)
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_filter(EnvFilter::from_default_env()),
        )
        .init();

    let args = Args::parse();

    // Read ELF from file or stdin.
    let elf = if let Some(path) = args.elf_path {
        let bytes =
            std::fs::read(&path).with_context(|| format!("failed to read ELF from {path}"))?;
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

    let resource_kind = args.resource.kind().to_string();
    let zkvm = construct_zkvm(elf, args.resource)?;
    info!("initialized zkVM with {resource_kind} prover");

    let server = Arc::new(zkVMServer::new(zkvm));
    let app = Router::new()
        .nest("/twirp", router(server))
        .fallback(not_found_handler)
        .layer(CatchPanicLayer::new());
    let app = otel::layer(app).route("/health", get(StatusCode::OK));

    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), args.port);
    let tcp_listener = TcpListener::bind(addr).await?;

    info!("listening on {}", addr);

    axum::serve(tcp_listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("shutdown gracefully");

    if let Some(provider) = tracer_provider {
        provider.shutdown().ok();
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {
            info!("received Ctrl+C, shutting down gracefully");
        },
        _ = terminate => {
            info!("received SIGTERM, shutting down gracefully");
        },
    }
}

fn construct_zkvm(elf: Elf, resource: ProverResource) -> Result<impl zkVM, Error> {
    #[cfg(feature = "airbender")]
    let zkvm = ere_airbender::zkvm::EreAirbender::new(elf, resource);

    #[cfg(feature = "openvm")]
    let zkvm = ere_openvm::zkvm::EreOpenVM::new(elf, resource);

    #[cfg(feature = "risc0")]
    let zkvm = ere_risc0::zkvm::EreRisc0::new(elf, resource);

    #[cfg(feature = "sp1")]
    let zkvm = ere_sp1::zkvm::EreSP1::new(elf, resource);

    #[cfg(feature = "zisk")]
    let zkvm = ere_zisk::zkvm::EreZisk::new(elf, resource);

    zkvm.with_context(|| "failed to instantiate zkVM")
}
