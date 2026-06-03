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
use crate::state::{AppState, ManagedFile};

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
    let file_size = data.len() as u64;

    // Encrypt in a blocking task (CPU-intensive for large files)
    let key = FileKey::generate();
    let key_clone = key.clone();
    let encrypted = tokio::task::spawn_blocking(move || key_clone.encrypt(&data))
        .await
        .map_err(|_| AppError::internal("Encryption task failed"))?
        .map_err(|_| AppError::internal("Encryption failed"))?;

    // Atomic write: write to temp file, then rename
    let storage_dir = state.storage_dir().clone();
    let temp_name = format!(".tmp-{}", uuid::Uuid::new_v4());
    let temp_path = storage_dir.join(&temp_name);
    let final_path = storage_dir.join(&storage_name);

    tokio::fs::write(&temp_path, &encrypted).await.map_err(|e| {
        AppError::internal(format!("Failed to write file: {}", e))
    })?;

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

    if let Err(e) = state
        .upload_file_atomic(&temp_path, &final_path, storage_name, managed)
        .await
    {
        let tp = temp_path.clone();
        tokio::spawn(async move {
            let _ = tokio::fs::remove_file(tp).await;
        });
        return Err(AppError::internal(e));
    }

    Ok(Json(StatusResponse::success(format!("Uploaded '{}'", real_name))))
}

/// Sanitize a filename for use in Content-Disposition header
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

    let (info, key) = state.get_file_for_download(&storage_name).await
        .ok_or_else(|| AppError::not_found(format!("File '{}' not found", real_name)))?;

    let file_path = state.storage_dir().join(&storage_name);

    let encrypted = tokio::fs::read(&file_path).await.map_err(|e| {
        AppError::internal(format!("Failed to read file: {}", e))
    })?;

    // Decrypt in blocking task
    let decrypted = tokio::task::spawn_blocking(move || key.decrypt(&encrypted))
        .await
        .map_err(|_| AppError::internal("Decryption task failed"))?
        .map_err(|_| AppError::internal("Decryption failed"))?;

    let safe_name = sanitize_filename(&info.real_name);
    let headers = [
        (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", safe_name)),
        (axum::http::header::CONTENT_TYPE, "application/octet-stream".to_string()),
    ];

    Ok((headers, decrypted))
}

async fn destroy_file(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
) -> AppResult<Json<StatusResponse>> {
    let storage_name = storage_name_for(&real_name);

    let removed = state.remove_file(&storage_name).await;

    if removed.is_none() {
        return Err(AppError::not_found(format!("File '{}' not found", real_name)));
    }

    let file_path = state.storage_dir().join(&storage_name);
    if let Err(e) = tokio::fs::remove_file(&file_path).await {
        tracing::warn!(
            "Failed to delete file from disk at {}: {}",
            file_path.display(),
            e
        );
    }

    Ok(Json(StatusResponse::success(format!("Destroyed '{}'", real_name))))
}

async fn set_serve_state(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
    Json(body): Json<SetServeStateRequest>,
) -> AppResult<Json<StatusResponse>> {
    let storage_name = storage_name_for(&real_name);

    state.set_serve_state(&storage_name, body.serving).await.map_err(|msg| {
        if msg == "File not found" {
            AppError::not_found(format!("File '{}' not found", real_name))
        } else {
            AppError::conflict(msg)
        }
    })?;

    let state_str = if body.serving { "enabled" } else { "disabled" };
    Ok(Json(StatusResponse::success(format!("Serving {} for '{}'", state_str, real_name))))
}

async fn set_serve_name(
    State(state): State<AppState>,
    Path(real_name): Path<String>,
    Json(body): Json<SetServeNameRequest>,
) -> AppResult<Json<StatusResponse>> {
    let storage_name = storage_name_for(&real_name);

    state.set_serve_name(&storage_name, body.serve_name.clone()).await.map_err(|msg| {
        if msg == "File not found" {
            AppError::not_found(format!("File '{}' not found", real_name))
        } else {
            AppError::conflict(msg)
        }
    })?;

    Ok(Json(StatusResponse::success(format!(
        "Serve name updated to '{}' for '{}'",
        body.serve_name, real_name
    ))))
}
