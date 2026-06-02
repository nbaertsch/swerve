use serde::{Deserialize, Serialize};

use crate::types::{SwerveFile, SwerveSocket};

// -- Request types --

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

/// Maximum number of swerve sockets allowed
pub const MAX_SWERVE_SOCKETS: usize = 64;

/// Default max upload size (50 MB)
pub const MAX_UPLOAD_SIZE: usize = 50 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_response_success() {
        let r = StatusResponse::success("ok");
        assert!(r.ok);
        assert_eq!(r.message, "ok");
    }

    #[test]
    fn status_response_error() {
        let r = StatusResponse::error("fail");
        assert!(!r.ok);
        assert_eq!(r.message, "fail");
    }

    #[test]
    fn status_response_serde_roundtrip() {
        let r = StatusResponse::success("test message");
        let json = serde_json::to_string(&r).unwrap();
        let parsed: StatusResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.ok, r.ok);
        assert_eq!(parsed.message, r.message);
    }

    #[test]
    fn set_serve_state_request_serde() {
        let req = SetServeStateRequest { serving: true };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("true"));
        let parsed: SetServeStateRequest = serde_json::from_str(&json).unwrap();
        assert!(parsed.serving);
    }

    #[test]
    fn bind_socket_request_serde() {
        let req = BindSocketRequest {
            addr: "0.0.0.0:8080".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: BindSocketRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.addr, "0.0.0.0:8080");
    }
}
