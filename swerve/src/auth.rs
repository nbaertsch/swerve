use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use subtle::ConstantTimeEq;

use swerve_core::api::API_KEY_HEADER;

use crate::state::AppState;

pub async fn api_key_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let provided = request
        .headers()
        .get(API_KEY_HEADER)
        .and_then(|v| v.to_str().ok());

    match provided {
        Some(key) => {
            let expected = state.api_key().as_bytes();
            let given = key.as_bytes();
            if expected.len() == given.len() && expected.ct_eq(given).into() {
                Ok(next.run(request).await)
            } else {
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}
