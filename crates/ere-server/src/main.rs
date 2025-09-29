use anyhow::{Context, Error};
use clap::Parser;
use ere_server::server::{router, zkVMServer};
use std::{
    io::{self, Read},
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{net::TcpListener, signal};
use tower_http::catch_panic::CatchPanicLayer;
use tracing_subscriber::EnvFilter;
use twirp::{
    Router,
    axum::{self, routing::get},
    reqwest::StatusCode,
    server::not_found_handler,
};
use zkvm_interface::{ProverResourceType, zkVM};

// Compile-time check to ensure exactly one backend feature is enabled for CLI mode
const _: () = {
    if cfg!(feature = "server") {
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
            "Exactly one zkVM backend feature must be enabled for CLI mode"
        );
    }
};

#[derive(Parser)]
#[command(author, version)]
struct Args {
    #[arg(long, default_value = "3000")]
    port: u16,
    #[command(subcommand)]
    resource: ProverResourceType,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Read serialized program from stdin.
    let mut program = Vec::new();
    io::stdin().read_to_end(&mut program)?;

    let zkvm = construct_zkvm(program, args.resource)?;
    let server = Arc::new(zkVMServer::new(zkvm));
    let app = Router::new()
        .nest("/twirp", router(server))
        .route("/health", get(health))
        .fallback(not_found_handler)
        .layer(CatchPanicLayer::new());

    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), args.port);
    let tcp_listener = TcpListener::bind(addr).await?;

    tracing::info!("Listening on {}", addr);

    axum::serve(tcp_listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Shutdown gracefully");

    Ok(())
}

async fn health() -> StatusCode {
    StatusCode::OK
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
            tracing::info!("Received Ctrl+C, shutting down gracefully");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, shutting down gracefully");
        },
    }
}

fn construct_zkvm(program: Vec<u8>, resource: ProverResourceType) -> Result<impl zkVM, Error> {
    let program =
        bincode::deserialize(&program).with_context(|| "Failed to deserialize program")?;

    #[cfg(feature = "jolt")]
    let zkvm = ere_jolt::EreJolt::new(program, resource);

    #[cfg(feature = "miden")]
    let zkvm = ere_miden::EreMiden::new(program, resource);

    #[cfg(feature = "nexus")]
    let zkvm = Ok::<_, Error>(ere_nexus::EreNexus::new(program, resource));

    #[cfg(feature = "openvm")]
    let zkvm = ere_openvm::EreOpenVM::new(program, resource);

    #[cfg(feature = "pico")]
    let zkvm = Ok::<_, Error>(ere_pico::ErePico::new(program, resource));

    #[cfg(feature = "risc0")]
    let zkvm = ere_risc0::EreRisc0::new(program, resource);

    #[cfg(feature = "sp1")]
    let zkvm = Ok::<_, Error>(ere_sp1::EreSP1::new(program, resource));

    #[cfg(feature = "ziren")]
    let zkvm = Ok::<_, Error>(ere_ziren::EreZiren::new(program, resource));

    #[cfg(feature = "zisk")]
    let zkvm = ere_zisk::EreZisk::new(program, resource);

    zkvm.with_context(|| "Failed to instantiate zkVM")
}
