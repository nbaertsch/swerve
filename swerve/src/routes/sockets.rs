use axum::{
    Json, Router,
    extract::{Query, State},
    routing::{delete, get, post},
};

use swerve_core::api::*;

use crate::error::{AppError, AppResult};
use crate::serve;
use crate::state::{AppState, SocketHandle};

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

async fn shutdown_socket_handle(addr: &str, mut handle: SocketHandle) {
    if let Some(tx) = handle.shutdown_tx.take() {
        let _ = tx.send(());
    }

    let abort_handle = handle.handle.abort_handle();
    match tokio::time::timeout(std::time::Duration::from_secs(5), handle.handle).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            tracing::warn!("Socket {} shutdown join error: {}", addr, e);
        }
        Err(_) => {
            tracing::warn!("Socket {} shutdown timed out, aborting", addr);
            abort_handle.abort();
        }
    }
}

async fn bind_socket(
    State(state): State<AppState>,
    Json(body): Json<BindSocketRequest>,
) -> AppResult<Json<StatusResponse>> {
    let requested_addr = body.addr;

    let (handle, actual_addr) = serve::spawn_serve_listener(state.clone(), &requested_addr)
        .await
        .map_err(|e| AppError::internal(format!("Failed to bind socket: {}", e)))?;

    if let Err((handle, message)) = state.try_insert_socket(actual_addr.clone(), handle).await {
        shutdown_socket_handle(&actual_addr, handle).await;
        return Err(AppError::conflict(message));
    }

    Ok(Json(StatusResponse::success(format!(
        "Bound swerve socket on {}",
        actual_addr
    ))))
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

    match state.remove_socket(&addr).await {
        Some(handle) => {
            shutdown_socket_handle(&addr, handle).await;
            Ok(Json(StatusResponse::success(format!(
                "Unbound swerve socket on {}",
                addr
            ))))
        }
        None => Err(AppError::not_found(format!("Socket '{}' not found", addr))),
    }
}
