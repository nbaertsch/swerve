pub mod files;
pub mod sockets;

use axum::{Json, Router, extract::DefaultBodyLimit, middleware, routing::get};
use swerve_core::api::{StatusResponse, MAX_UPLOAD_SIZE};

use crate::state::AppState;

pub fn management_router(state: AppState) -> Router {
    let health_route = Router::new()
        .route("/health", get(health));

    let authed_routes = Router::new()
        .merge(files::file_routes())
        .merge(sockets::socket_routes())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::api_key_auth,
        ))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE));

    health_route
        .merge(authed_routes)
        .with_state(state)
}

async fn health() -> Json<StatusResponse> {
    Json(StatusResponse::success("swerve is running"))
}
