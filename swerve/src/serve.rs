use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use std::net::SocketAddr;

use crate::state::{AppState, SocketHandle};

/// Spawn a new HTTP listener that serves swerve files
pub async fn spawn_serve_listener(
    state: AppState,
    addr: &str,
) -> Result<SocketHandle, Box<dyn std::error::Error + Send + Sync>> {
    let addr_parsed: SocketAddr = addr.parse()?;
    let addr_string = addr.to_string();

    let listener = tokio::net::TcpListener::bind(addr_parsed).await?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let app = serve_router(state.clone());

    let handle = tokio::spawn(async move {
        tracing::info!("Swerve socket serving on {}", addr_string);
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
        tracing::info!("Swerve socket on {} stopped", addr_string);
    });

    Ok(SocketHandle {
        shutdown_tx: Some(shutdown_tx),
        handle,
        addr: addr.to_string(),
    })
}

fn serve_router(state: AppState) -> Router {
    Router::new()
        .route("/{filename}", get(serve_file))
        .with_state(state)
}

async fn serve_file(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let files = state.files.read().await;

    // Find file by serve_name that is actively being served
    let managed = files
        .values()
        .find(|f| f.info.serving && f.info.serve_name == filename)
        .ok_or(StatusCode::NOT_FOUND)?;

    let file_path = state.storage_dir.join(&managed.info.storage_name);
    let key = managed.key.clone();
    let serve_name = managed.info.serve_name.clone();
    drop(files);

    let encrypted = tokio::fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let decrypted = key
        .decrypt(&encrypted)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let headers = [
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", serve_name),
        ),
        (
            axum::http::header::CONTENT_TYPE,
            "application/octet-stream".to_string(),
        ),
    ];

    Ok((headers, decrypted))
}
