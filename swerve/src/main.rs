mod auth;
mod mgmt;
mod serve;
mod state;

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "swerve", about = "Encrypted file staging and serving server")]
struct Args {
    /// Management API bind address
    #[arg(short, long, default_value = "0.0.0.0:9740")]
    bind: String,

    /// API key for management authentication
    #[arg(short = 'k', long, env = "SWERVE_API_KEY")]
    api_key: String,

    /// Storage directory for encrypted files (default: system temp)
    #[arg(short, long)]
    storage_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let storage_dir = args.storage_dir.unwrap_or_else(|| {
        let mut dir = std::env::temp_dir();
        dir.push("swerve-storage");
        dir
    });

    // Clean and create storage directory
    if storage_dir.exists() {
        let _ = std::fs::remove_dir_all(&storage_dir);
    }
    std::fs::create_dir_all(&storage_dir).expect("Failed to create storage directory");

    tracing::info!("Storage directory: {}", storage_dir.display());

    let state = state::AppStateInner::new(args.api_key, storage_dir);

    let app = mgmt::management_router(state.clone());

    let addr: SocketAddr = args.bind.parse().expect("Invalid bind address");
    tracing::info!("Management API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server error");
}
