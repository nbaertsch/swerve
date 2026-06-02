use serde::{Deserialize, Serialize};

use crate::types::{SwerveFile, SwerveSocket};

// -- Request types --

#[derive(Debug, Serialize, Deserialize)]
pub struct UploadParams {
    pub serve_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetServeStateRequest {
    pub serving: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetServeNameRequest {
    pub serve_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BindSocketRequest {
    pub addr: String,
}

// -- Response types --

#[derive(Debug, Serialize, Deserialize)]
pub struct FileListResponse {
    pub files: Vec<SwerveFile>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SocketListResponse {
    pub sockets: Vec<SwerveSocket>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub ok: bool,
    pub message: String,
}

impl StatusResponse {
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
        }
    }
}

/// API key header name
pub const API_KEY_HEADER: &str = "x-api-key";
