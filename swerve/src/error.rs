use axum::{
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use swerve_core::api::StatusResponse;

pub type AppResult<T> = Result<T, AppError>;

pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::BAD_REQUEST, message: msg.into() }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, message: msg.into() }
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::CONFLICT, message: msg.into() }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: msg.into() }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(StatusResponse::error(self.message))).into_response()
    }
}
