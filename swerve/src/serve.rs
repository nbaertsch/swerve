use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use std::net::SocketAddr;

use crate::state::{AppState, SocketHandle, SocketStatus};

/// Sanitize a filename for Content-Disposition
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '"' | '\\' | '/' | '\n' | '\r' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

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
        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
        {
            tracing::error!("Swerve socket on {} error: {}", addr_string, e);
        }
        tracing::info!("Swerve socket on {} stopped", addr_string);
    });

    Ok(SocketHandle {
        shutdown_tx: Some(shutdown_tx),
        handle,
        status: SocketStatus::Running,
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
    // O(1) lookup via serve_name index
    let (storage_name, key) = state
        .get_file_for_serving(&filename)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;

    let file_path = state.storage_dir().join(&storage_name);

    let encrypted = tokio::fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Decrypt in blocking task
    let decrypted = tokio::task::spawn_blocking(move || key.decrypt(&encrypted))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let safe_name = sanitize_filename(&filename);
    let headers = [
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", safe_name),
        ),
        (
            axum::http::header::CONTENT_TYPE,
            "application/octet-stream".to_string(),
        ),
    ];

    Ok((headers, decrypted))
}
