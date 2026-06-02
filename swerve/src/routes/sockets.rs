use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{delete, get, post},
};

use swerve_core::api::*;

use crate::error::{AppError, AppResult};
use crate::serve;
use crate::state::AppState;

pub fn socket_routes() -> Router<AppState> {
    Router::new()
        .route("/sockets", get(list_sockets))
        .route("/sockets", post(bind_socket))
        .route("/sockets", delete(unbind_socket))
}

async fn list_sockets(State(state): State<AppState>) -> Json<SocketListResponse> {
    let sockets = state.list_sockets().await;
    Json(SocketListResponse { sockets })
}

async fn bind_socket(
    State(state): State<AppState>,
    Json(body): Json<BindSocketRequest>,
) -> AppResult<Json<StatusResponse>> {
    let addr = body.addr;

    // Check limits
    if state.socket_count().await >= MAX_SWERVE_SOCKETS {
        return Err(AppError::conflict(format!(
            "Maximum number of swerve sockets ({}) reached",
            MAX_SWERVE_SOCKETS
        )));
    }

    if state.has_socket(&addr).await {
        return Err(AppError::conflict(format!("Socket '{}' already bound", addr)));
    }

    let handle = serve::spawn_serve_listener(state.clone(), &addr).await.map_err(|e| {
        AppError::internal(format!("Failed to bind socket: {}", e))
    })?;

    state.insert_socket(addr.clone(), handle).await;

    Ok(Json(StatusResponse::success(format!("Bound swerve socket on {}", addr))))
}

#[derive(serde::Deserialize)]
struct UnbindQuery {
    addr: String,
}

async fn unbind_socket(
    State(state): State<AppState>,
    Query(query): Query<UnbindQuery>,
) -> AppResult<Json<StatusResponse>> {
    let addr = query.addr;

    let handle = state.remove_socket(&addr).await;

    match handle {
        Some(h) => {
            if let Some(tx) = h.shutdown_tx {
                let _ = tx.send(());
            }
            // Wait briefly for graceful shutdown, then abort if stuck
            let timeout = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                h.handle,
            ).await;
            if timeout.is_err() {
                tracing::warn!("Socket {} shutdown timed out", addr);
            }
            Ok(Json(StatusResponse::success(format!("Unbound swerve socket on {}", addr))))
        }
        None => Err(AppError::not_found(format!("Socket '{}' not found", addr))),
    }
}
