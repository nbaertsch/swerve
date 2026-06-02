use axum::{
    Json, Router,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, post, put},
};

use swerve_core::{
    api::*,
    crypto::FileKey,
    types::*,
};

use crate::serve;
use crate::state::{AppState, ManagedFile};

pub fn management_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/files", get(list_files))
        .route("/files", post(upload_file))
        .route("/files/{real_name}", get(download_file))
        .route("/files/{real_name}", delete(destroy_file))
        .route("/files/{real_name}/serve-state", put(set_serve_state))
        .route("/files/{real_name}/serve-name", put(set_serve_name))
        .route("/sockets", get(list_sockets))
        .route("/sockets", post(bind_socket))
        .route("/sockets", delete(unbind_socket))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::api_key_auth,
        ))
        .with_state(state)
}

async fn health() -> Json<StatusResponse> {
    Json(StatusResponse::success("swerve is running"))
}

async fn list_files(State(state): State<AppState>) -> Json<FileListResponse> {
    let files = state.files.read().await;
    let file_list: Vec<SwerveFile> = files.values().map(|f| f.info.clone()).collect();
    Json(FileListResponse { files: file_list })
}

async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<StatusResponse>, (StatusCode, Json<StatusResponse>)> {
    let mut file_data: Option<(String, Vec<u8>)> = None;
    let mut serve_name: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let real_name = field.file_name().unwrap_or("unknown").to_string();
                let data = field.bytes().await.map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(StatusResponse::error("Failed to read file data")),
                    )
                })?;
                file_data = Some((real_name, data.to_vec()));
            }
            "serve_name" => {
                let text = field.text().await.map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(StatusResponse::error("Failed to read serve_name")),
                    )
                })?;
                serve_name = Some(text);
            }
            _ => {}
        }
    }

    let (real_name, data) = file_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(StatusResponse::error("No file provided")),
        )
    })?;

    let serve_name = serve_name.unwrap_or_else(|| real_name.clone());
    let storage_name = storage_name_for(&real_name);
    let file_size = data.len() as u64;

    // Check for unique serve_name among actively served files
    {
        let files = state.files.read().await;
        for (sn, mf) in files.iter() {
            if *sn != storage_name && mf.info.serving && mf.info.serve_name == serve_name {
                return Err((
                    StatusCode::CONFLICT,
                    Json(StatusResponse::error(format!(
                        "Serve name '{}' is already in use by '{}'",
                        serve_name, mf.info.real_name
                    ))),
                ));
            }
        }
    }

    // Generate encryption key and encrypt
    let key = FileKey::generate();
    let encrypted = key.encrypt(&data).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatusResponse::error("Encryption failed")),
        )
    })?;

    // Write encrypted file to storage
    let file_path = state.storage_dir.join(&storage_name);
    tokio::fs::write(&file_path, &encrypted).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatusResponse::error("Failed to write file")),
        )
    })?;

    // Update in-memory state
    let managed = ManagedFile {
        info: SwerveFile {
            real_name: real_name.clone(),
            storage_name: storage_name.clone(),
            serve_name,
            serving: false,
            size: file_size,
        },
        key,
    };

    state.files.write().await.insert(storage_name, managed);

    Ok(Json(StatusResponse::success(format!(
        "Uploaded '{}'",
        real_name
    ))))
}

async fn download_file(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<StatusResponse>)> {
    let storage_name = storage_name_for(&real_name);
    let files = state.files.read().await;

    let managed = files.get(&storage_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(StatusResponse::error(format!(
                "File '{}' not found",
                real_name
            ))),
        )
    })?;

    let file_path = state.storage_dir.join(&storage_name);
    let key = managed.key.clone();
    drop(files);

    let encrypted = tokio::fs::read(&file_path).await.map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatusResponse::error("Failed to read file")),
        )
    })?;

    let decrypted = key.decrypt(&encrypted).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(StatusResponse::error("Decryption failed")),
        )
    })?;

    let headers = [
        (
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", real_name),
        ),
        (
            axum::http::header::CONTENT_TYPE,
            "application/octet-stream".to_string(),
        ),
    ];

    Ok((headers, decrypted))
}

async fn destroy_file(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<StatusResponse>)> {
    let storage_name = storage_name_for(&real_name);

    let removed = state.files.write().await.remove(&storage_name);

    if removed.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(StatusResponse::error(format!(
                "File '{}' not found",
                real_name
            ))),
        ));
    }

    let file_path = state.storage_dir.join(&storage_name);
    let _ = tokio::fs::remove_file(&file_path).await;

    Ok(Json(StatusResponse::success(format!(
        "Destroyed '{}'",
        real_name
    ))))
}

async fn set_serve_state(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
    Json(body): Json<SetServeStateRequest>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<StatusResponse>)> {
    let storage_name = storage_name_for(&real_name);
    let mut files = state.files.write().await;

    // Verify file exists first
    if !files.contains_key(&storage_name) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(StatusResponse::error(format!(
                "File '{}' not found",
                real_name
            ))),
        ));
    }

    // If enabling serving, check for serve_name conflicts
    if body.serving {
        let serve_name = files.get(&storage_name).unwrap().info.serve_name.clone();
        for (sn, mf) in files.iter() {
            if *sn != storage_name && mf.info.serving && mf.info.serve_name == serve_name {
                return Err((
                    StatusCode::CONFLICT,
                    Json(StatusResponse::error(format!(
                        "Serve name '{}' conflicts with '{}'",
                        serve_name, mf.info.real_name
                    ))),
                ));
            }
        }
    }

    let managed = files.get_mut(&storage_name).unwrap();
    managed.info.serving = body.serving;

    let state_str = if body.serving { "enabled" } else { "disabled" };
    Ok(Json(StatusResponse::success(format!(
        "Serving {} for '{}'",
        state_str, real_name
    ))))
}

async fn set_serve_name(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
    Json(body): Json<SetServeNameRequest>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<StatusResponse>)> {
    let storage_name = storage_name_for(&real_name);
    let mut files = state.files.write().await;

    // Check for serve_name conflict
    for (sn, mf) in files.iter() {
        if *sn != storage_name && mf.info.serving && mf.info.serve_name == body.serve_name {
            return Err((
                StatusCode::CONFLICT,
                Json(StatusResponse::error(format!(
                    "Serve name '{}' is already in use by '{}'",
                    body.serve_name, mf.info.real_name
                ))),
            ));
        }
    }

    let managed = files.get_mut(&storage_name).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(StatusResponse::error(format!(
                "File '{}' not found",
                real_name
            ))),
        )
    })?;

    managed.info.serve_name = body.serve_name.clone();

    Ok(Json(StatusResponse::success(format!(
        "Serve name updated to '{}' for '{}'",
        body.serve_name, real_name
    ))))
}

async fn list_sockets(State(state): State<AppState>) -> Json<SocketListResponse> {
    let sockets = state.sockets.read().await;
    let socket_list: Vec<SwerveSocket> = sockets
        .keys()
        .map(|addr| SwerveSocket {
            addr: addr.clone(),
            active: true,
        })
        .collect();
    Json(SocketListResponse {
        sockets: socket_list,
    })
}

async fn bind_socket(
    State(state): State<AppState>,
    Json(body): Json<BindSocketRequest>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<StatusResponse>)> {
    let addr = body.addr.clone();

    // Check if already bound
    {
        let sockets = state.sockets.read().await;
        if sockets.contains_key(&addr) {
            return Err((
                StatusCode::CONFLICT,
                Json(StatusResponse::error(format!(
                    "Socket '{}' already bound",
                    addr
                ))),
            ));
        }
    }

    // Spawn serving listener
    let handle = serve::spawn_serve_listener(state.clone(), &addr)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(StatusResponse::error(format!(
                    "Failed to bind socket: {}",
                    e
                ))),
            )
        })?;

    state.sockets.write().await.insert(addr.clone(), handle);

    Ok(Json(StatusResponse::success(format!(
        "Bound swerve socket on {}",
        addr
    ))))
}

#[derive(serde::Deserialize)]
struct UnbindQuery {
    addr: String,
}

async fn unbind_socket(
    State(state): State<AppState>,
    Query(query): Query<UnbindQuery>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<StatusResponse>)> {
    let addr = query.addr;

    let handle = state.sockets.write().await.remove(&addr);

    match handle {
        Some(h) => {
            if let Some(tx) = h.shutdown_tx {
                let _ = tx.send(());
            }
            Ok(Json(StatusResponse::success(format!(
                "Unbound swerve socket on {}",
                addr
            ))))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(StatusResponse::error(format!(
                "Socket '{}' not found",
                addr
            ))),
        )),
    }
}
