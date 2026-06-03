use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        Multipart, Path, State,
        multipart::MultipartRejection,
    },
    response::IntoResponse,
    routing::{delete, get, post, put},
};

use swerve_core::{
    api::*,
    crypto::FileKey,
    types::*,
};

use crate::error::{AppError, AppResult};
use crate::state::{AppState, ManagedFile, StateError};

impl From<StateError> for AppError {
    fn from(e: StateError) -> Self {
        match e {
            StateError::NotFound => AppError::not_found("File not found"),
            StateError::ServeNameConflict => {
                AppError::conflict("Serve name is already in use by another file")
            }
            StateError::Internal(msg) => AppError::internal(msg),
            StateError::Io(msg) => AppError::internal(msg),
        }
    }
}

pub fn file_routes() -> Router<AppState> {
    Router::new()
        .route("/files", get(list_files))
        .route("/files", post(upload_file))
        .route("/files/{real_name}", get(download_file))
        .route("/files/{real_name}", delete(destroy_file))
        .route("/files/{real_name}/serve-state", put(set_serve_state))
        .route("/files/{real_name}/serve-name", put(set_serve_name))
}

async fn list_files(State(state): State<AppState>) -> Json<FileListResponse> {
    let files = state.list_files().await;
    Json(FileListResponse { files })
}

async fn upload_file(
    State(state): State<AppState>,
    multipart: Result<Multipart, MultipartRejection>,
) -> AppResult<Json<StatusResponse>> {
    let mut multipart = multipart
        .map_err(|e| AppError::bad_request(format!("Invalid multipart data: {}", e)))?;
    let mut file_data: Option<(String, Bytes)> = None;
    let mut serve_name: Option<String> = None;

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                match name.as_str() {
                    "file" => {
                        let real_name = field.file_name().unwrap_or("unknown").to_string();
                        let data = field.bytes().await.map_err(|e| {
                            AppError::bad_request(format!("Failed to read file data: {}", e))
                        })?;
                        file_data = Some((real_name, data));
                    }
                    "serve_name" => {
                        let text = field.text().await.map_err(|e| {
                            AppError::bad_request(format!("Failed to read serve_name: {}", e))
                        })?;
                        serve_name = Some(text);
                    }
                    _ => {}
                }
            }
            Ok(None) => break,
            Err(e) => return Err(AppError::bad_request(format!("Invalid multipart data: {}", e))),
        }
    }

    let (real_name, data) = file_data.ok_or_else(|| AppError::bad_request("No file provided"))?;

    let serve_name = serve_name.unwrap_or_else(|| real_name.clone());
    let storage_name = storage_name_for(&real_name);
    let disk_name = format!("{}-{}", storage_name, uuid::Uuid::new_v4());
    let file_size = data.len() as u64;

    let key = FileKey::generate();
    let key_clone = key.clone();
    let encrypted = tokio::task::spawn_blocking(move || key_clone.encrypt(&data))
        .await
        .map_err(|_| AppError::internal("Encryption task failed"))?
        .map_err(|_| AppError::internal("Encryption failed"))?;

    let file_path = state.storage_dir().join(&disk_name);
    tokio::fs::write(&file_path, &encrypted)
        .await
        .map_err(|e| AppError::internal(format!("Failed to write file: {}", e)))?;

    let managed = ManagedFile {
        info: SwerveFile {
            real_name: real_name.clone(),
            storage_name: storage_name.clone(),
            serve_name,
            serving: false,
            size: file_size,
        },
        key,
        disk_name: disk_name.clone(),
    };

    let old = match state.upload_file(storage_name, managed).await {
        Ok(old) => old,
        Err(e) => {
            let _ = tokio::fs::remove_file(&file_path).await;
            return Err(e.into());
        }
    };

    if let Some(old_file) = old {
        let old_path = state.storage_dir().join(old_file.disk_name);
        tokio::spawn(async move {
            if let Err(e) = tokio::fs::remove_file(&old_path).await {
                tracing::warn!("Failed to remove old file version at {}: {}", old_path.display(), e);
            }
        });
    }

    Ok(Json(StatusResponse::success(format!("Uploaded '{}'", real_name))))
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '"' | '\\' | '/' | '\n' | '\r' | '\0' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

async fn download_file(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
) -> AppResult<impl IntoResponse> {
    let storage_name = storage_name_for(&real_name);

    let (info, disk_name, key) = state
        .get_file_for_download(&storage_name)
        .await
        .ok_or_else(|| AppError::not_found(format!("File '{}' not found", real_name)))?;

    let file_path = state.storage_dir().join(&disk_name);

    let encrypted = tokio::fs::read(&file_path)
        .await
        .map_err(|e| AppError::internal(format!("Failed to read file: {}", e)))?;

    let decrypted = tokio::task::spawn_blocking(move || key.decrypt(&encrypted))
        .await
        .map_err(|_| AppError::internal("Decryption task failed"))?
        .map_err(|_| AppError::internal("Decryption failed"))?;

    let safe_name = sanitize_filename(&info.real_name);
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

async fn destroy_file(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
) -> AppResult<Json<StatusResponse>> {
    let storage_name = storage_name_for(&real_name);

    let disk_name = state
        .get_disk_name(&storage_name)
        .await
        .ok_or_else(|| AppError::not_found(format!("File '{}' not found", real_name)))?;

    let file_path = state.storage_dir().join(&disk_name);
    tokio::fs::remove_file(&file_path)
        .await
        .map_err(|e| AppError::internal(format!("Failed to delete file from disk: {}", e)))?;

    state
        .remove_file_if_disk_name(&storage_name, &disk_name)
        .await
        .ok_or_else(|| AppError::conflict("File changed during destroy"))?;

    Ok(Json(StatusResponse::success(format!("Destroyed '{}'", real_name))))
}

async fn set_serve_state(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
    Json(body): Json<SetServeStateRequest>,
) -> AppResult<Json<StatusResponse>> {
    let storage_name = storage_name_for(&real_name);

    state.set_serve_state(&storage_name, body.serving).await?;

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
) -> AppResult<Json<StatusResponse>> {
    let storage_name = storage_name_for(&real_name);

    state
        .set_serve_name(&storage_name, body.serve_name.clone())
        .await?;

    Ok(Json(StatusResponse::success(format!(
        "Serve name updated to '{}' for '{}'",
        body.serve_name, real_name
    ))))
}
