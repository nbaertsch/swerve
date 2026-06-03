use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

use swerve::{routes, state};

#[derive(Parser, Debug)]
#[command(name = "swerve", about = "Encrypted file staging and serving server")]
struct Args {
    /// Management API bind address
    #[arg(short, long, default_value = "127.0.0.1:9740")]
    bind: String,

    /// API key for management authentication
    #[arg(short = 'k', long, env = "SWERVE_API_KEY")]
    api_key: String,

    /// Storage directory for encrypted files (default: system temp)
    #[arg(short, long)]
    storage_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let storage_dir = args.storage_dir.unwrap_or_else(|| {
        let mut dir = std::env::temp_dir();
        dir.push("swerve-storage");
        dir
    });

    // Create storage directory if it doesn't exist (never delete existing)
    tokio::fs::create_dir_all(&storage_dir).await
        .map_err(|e| format!("Failed to create storage directory: {}", e))?;

    // Clean orphaned files from previous runs (state is ephemeral, keys are lost on restart)
    let mut entries = tokio::fs::read_dir(&storage_dir).await
        .map_err(|e| format!("Failed to read storage directory: {}", e))?;
    let mut cleaned = 0u64;
    while let Some(entry) = entries.next_entry().await
        .map_err(|e| format!("Failed to read storage directory entry: {}", e))? {
        if entry.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
            let _ = tokio::fs::remove_file(entry.path()).await;
            cleaned += 1;
        }
    }
    if cleaned > 0 {
        tracing::info!("Cleaned {} orphaned file(s) from previous run", cleaned);
    }

    tracing::info!("Storage directory: {}", storage_dir.display());

    let state = state::AppStateRw::new(args.api_key, storage_dir);

    let app = routes::management_router(state.clone());

    let addr: SocketAddr = args.bind.parse()
        .map_err(|e| format!("Invalid bind address '{}': {}", args.bind, e))?;
    tracing::info!("Management API listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await
        .map_err(|e| format!("Failed to bind to {}: {}", addr, e))?;
    axum::serve(listener, app).await
        .map_err(|e| format!("Server error: {}", e))?;

    Ok(())
}
