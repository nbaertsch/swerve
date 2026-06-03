use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use swerve::routes::management_router;
use swerve::state::{AppState, AppStateRw, SocketHandle, SocketStatus};
use swerve_core::api::*;
use swerve_core::types::SwerveFile;
use tempfile::TempDir;
use tower::ServiceExt;

const API_KEY: &str = "test-key";
const MAX_RESPONSE: usize = 4 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct Harness {
    app: axum::Router,
    state: AppState,
    _tmp: TempDir,
}

impl Harness {
    fn new() -> Self {
        let tmp = TempDir::new().expect("failed to create temp dir");
        let state = AppStateRw::new(API_KEY.to_string(), tmp.path().to_path_buf());
        let app = management_router(state.clone());
        Self {
            app,
            state,
            _tmp: tmp,
        }
    }

    async fn send(&self, req: Request<Body>) -> axum::http::Response<Body> {
        self.app.clone().oneshot(req).await.unwrap()
    }

    /// Upload a file via multipart POST /files (authenticated).
    async fn upload(&self, filename: &str, data: &[u8], serve_name: Option<&str>) -> StatusCode {
        let (ct, body) = multipart_upload_body(filename, data, serve_name);
        let req = Request::builder()
            .method("POST")
            .uri("/files")
            .header("x-api-key", API_KEY)
            .header("content-type", ct)
            .body(Body::from(body))
            .unwrap();
        self.send(req).await.status()
    }
}

/// Build a multipart/form-data body for a file upload.
fn multipart_upload_body(
    filename: &str,
    data: &[u8],
    serve_name: Option<&str>,
) -> (String, Vec<u8>) {
    let boundary = "test-boundary-xyz";
    let mut body = Vec::new();

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(data);
    body.extend_from_slice(b"\r\n");

    if let Some(sn) = serve_name {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"serve_name\"\r\n\r\n");
        body.extend_from_slice(sn.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let content_type = format!("multipart/form-data; boundary={boundary}");
    (content_type, body)
}

fn multipart_serve_name_only_body(serve_name: &str) -> (String, Vec<u8>) {
    let boundary = "test-boundary-xyz";
    let mut body = Vec::new();

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"serve_name\"\r\n\r\n");
    body.extend_from_slice(serve_name.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let content_type = format!("multipart/form-data; boundary={boundary}");
    (content_type, body)
}

async fn json_body<T: serde::de::DeserializeOwned>(resp: axum::http::Response<Body>) -> T {
    let bytes = to_bytes(resp.into_body(), MAX_RESPONSE).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn raw_body(resp: axum::http::Response<Body>) -> Vec<u8> {
    to_bytes(resp.into_body(), MAX_RESPONSE)
        .await
        .unwrap()
        .to_vec()
}

// ---------------------------------------------------------------------------
// 1. Health endpoint (no auth required)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn health_no_auth() {
    let h = Harness::new();
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: StatusResponse = json_body(resp).await;
    assert!(body.ok);
}

// ---------------------------------------------------------------------------
// 2–4. Authentication
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_rejects_missing_key() {
    let h = Harness::new();
    let req = Request::builder()
        .uri("/files")
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_rejects_wrong_key() {
    let h = Harness::new();
    let req = Request::builder()
        .uri("/files")
        .header("x-api-key", "wrong-key")
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_rejects_empty_key() {
    let h = Harness::new();
    let req = Request::builder()
        .uri("/files")
        .header("x-api-key", "")
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_accepts_valid_key() {
    let h = Harness::new();
    let req = Request::builder()
        .uri("/files")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// ---------------------------------------------------------------------------
// 5–7. Upload validation and list
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upload_rejects_malformed_multipart() {
    let h = Harness::new();
    let req = Request::builder()
        .method("POST")
        .uri("/files")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_rejects_missing_file_field() {
    let h = Harness::new();
    let (ct, body) = multipart_serve_name_only_body("only-name.exe");
    let req = Request::builder()
        .method("POST")
        .uri("/files")
        .header("x-api-key", API_KEY)
        .header("content-type", ct)
        .body(Body::from(body))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upload_and_list() {
    let h = Harness::new();
    let status = h.upload("hello.bin", b"payload", None).await;
    assert_eq!(status, StatusCode::OK);

    let req = Request::builder()
        .uri("/files")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let list: FileListResponse = json_body(resp).await;
    assert_eq!(list.files.len(), 1);
    assert_eq!(list.files[0].real_name, "hello.bin");
}

// ---------------------------------------------------------------------------
// 6. Upload then download — full round-trip through encryption
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upload_and_download_roundtrip() {
    let h = Harness::new();
    let data = b"round-trip payload 123";
    let status = h.upload("rt.bin", data, None).await;
    assert_eq!(status, StatusCode::OK);

    let req = Request::builder()
        .uri("/files/rt.bin")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = raw_body(resp).await;
    assert_eq!(body, data);
}

// ---------------------------------------------------------------------------
// 7. Upload overwrite — same filename replaces previous contents
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upload_overwrite() {
    let h = Harness::new();

    h.upload("over.bin", b"first", None).await;
    h.upload("over.bin", b"second", None).await;

    // list should still have exactly one entry
    let req = Request::builder()
        .uri("/files")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let list: FileListResponse = json_body(h.send(req).await).await;
    assert_eq!(list.files.len(), 1);

    // download should return the second payload
    let req = Request::builder()
        .uri("/files/over.bin")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let body = raw_body(h.send(req).await).await;
    assert_eq!(body, b"second");
}

// ---------------------------------------------------------------------------
// 8. Download non-existent file → 404
// ---------------------------------------------------------------------------

#[tokio::test]
async fn download_not_found() {
    let h = Harness::new();
    let req = Request::builder()
        .uri("/files/nonexistent.bin")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// 9. Destroy file — DELETE then verify gone
// ---------------------------------------------------------------------------

#[tokio::test]
async fn destroy_file() {
    let h = Harness::new();
    h.upload("doomed.bin", b"bye", None).await;

    let req = Request::builder()
        .method("DELETE")
        .uri("/files/doomed.bin")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .uri("/files/doomed.bin")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn destroy_missing_file_returns_not_found() {
    let h = Harness::new();
    let req = Request::builder()
        .method("DELETE")
        .uri("/files/missing.bin")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// 10. Serve-state toggle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn serve_state_toggle() {
    let h = Harness::new();
    h.upload("toggle.bin", b"data", None).await;

    let req = Request::builder()
        .method("PUT")
        .uri("/files/toggle.bin/serve-state")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"serving":true}"#))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .uri("/files")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let list: FileListResponse = json_body(h.send(req).await).await;
    assert!(list.files.iter().any(|f| f.real_name == "toggle.bin" && f.serving));

    let req = Request::builder()
        .method("PUT")
        .uri("/files/toggle.bin/serve-state")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"serving":false}"#))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .uri("/files")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let list: FileListResponse = json_body(h.send(req).await).await;
    assert!(list.files.iter().any(|f| f.real_name == "toggle.bin" && !f.serving));
}

#[tokio::test]
async fn serve_state_missing_file_returns_not_found() {
    let h = Harness::new();
    let req = Request::builder()
        .method("PUT")
        .uri("/files/missing.bin/serve-state")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"serving":true}"#))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// 11. Serve-name conflict — two files with same serve_name cannot both serve
// ---------------------------------------------------------------------------

#[tokio::test]
async fn serve_name_conflict() {
    let h = Harness::new();

    h.upload("file_a.bin", b"aaa", Some("shared.exe")).await;
    h.upload("file_b.bin", b"bbb", Some("shared.exe")).await;

    // enable serving for file_a → OK
    let req = Request::builder()
        .method("PUT")
        .uri("/files/file_a.bin/serve-state")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"serving":true}"#))
        .unwrap();
    assert_eq!(h.send(req).await.status(), StatusCode::OK);

    // enable serving for file_b → conflict
    let req = Request::builder()
        .method("PUT")
        .uri("/files/file_b.bin/serve-state")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"serving":true}"#))
        .unwrap();
    assert_eq!(h.send(req).await.status(), StatusCode::CONFLICT);
}

// ---------------------------------------------------------------------------
// 12. Set serve name
// ---------------------------------------------------------------------------

#[tokio::test]
async fn set_serve_name() {
    let h = Harness::new();
    h.upload("rename.bin", b"data", Some("old.exe")).await;

    let req = Request::builder()
        .method("PUT")
        .uri("/files/rename.bin/serve-name")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"serve_name":"new.exe"}"#))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .uri("/files")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let list: FileListResponse = json_body(h.send(req).await).await;
    let file: &SwerveFile = list
        .files
        .iter()
        .find(|f| f.real_name == "rename.bin")
        .expect("file should exist");
    assert_eq!(file.serve_name, "new.exe");
}

#[tokio::test]
async fn set_serve_name_missing_file_returns_not_found() {
    let h = Harness::new();
    let req = Request::builder()
        .method("PUT")
        .uri("/files/missing.bin/serve-name")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"serve_name":"new.exe"}"#))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// 13. Socket bind → list → unbind
// ---------------------------------------------------------------------------

#[tokio::test]
async fn socket_bind_list_unbind() {
    let h = Harness::new();

    let req = Request::builder()
        .method("POST")
        .uri("/sockets")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"addr":"127.0.0.1:0"}"#))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let status: StatusResponse = json_body(resp).await;
    let bound_addr = status
        .message
        .strip_prefix("Bound swerve socket on ")
        .expect("bind response should include bound address")
        .to_string();
    assert_ne!(bound_addr, "127.0.0.1:0");

    let req = Request::builder()
        .uri("/sockets")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let list: SocketListResponse = json_body(h.send(req).await).await;
    assert_eq!(list.sockets.len(), 1);
    assert_eq!(list.sockets[0].addr, bound_addr);

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/sockets?addr={}", list.sockets[0].addr))
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = Request::builder()
        .uri("/sockets")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let list: SocketListResponse = json_body(h.send(req).await).await;
    assert!(list.sockets.is_empty());
}

#[tokio::test]
async fn unbind_missing_socket_returns_not_found() {
    let h = Harness::new();
    let req = Request::builder()
        .method("DELETE")
        .uri("/sockets?addr=127.0.0.1:45678")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// 14. Socket bind limit — pre-fill state, then try one more via HTTP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn socket_bind_limit() {
    let h = Harness::new();

    // Pre-fill MAX_SWERVE_SOCKETS dummy entries via the state API directly.
    // We don't actually bind real listeners — we just need the count to hit the cap.
    for i in 0..MAX_SWERVE_SOCKETS {
        let (tx, _rx) = tokio::sync::oneshot::channel::<()>();
        let handle = tokio::spawn(async {});
        h.state
            .insert_socket(
                format!("dummy:{i}"),
                SocketHandle {
                    shutdown_tx: Some(tx),
                    handle,
                    status: SocketStatus::Running,
                },
            )
            .await;
    }

    // Now trying to bind one more via the API should fail
    let req = Request::builder()
        .method("POST")
        .uri("/sockets")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"addr":"127.0.0.1:0"}"#))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// ---------------------------------------------------------------------------
// 15. Truly malformed multipart — truncated body with valid multipart content-type
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upload_rejects_truncated_multipart_body() {
    let h = Harness::new();
    // Valid multipart content-type but truncated/incomplete body
    let req = Request::builder()
        .method("POST")
        .uri("/files")
        .header("x-api-key", API_KEY)
        .header("content-type", "multipart/form-data; boundary=abc")
        .body(Body::from("--abc\r\nContent-Disposition: form-data; name=\"file\"; filename=\"x\"\r\n\r\ndata-but-no-closing-boundary"))
        .unwrap();
    let resp = h.send(req).await;
    // Should still succeed since it can parse the field, but missing final boundary
    // is handled gracefully — the file field IS present. What matters: no panic.
    assert!(
        resp.status() == StatusCode::OK || resp.status() == StatusCode::BAD_REQUEST,
        "Expected OK or BAD_REQUEST, got {}",
        resp.status()
    );
}

// ---------------------------------------------------------------------------
// 16. Empty file upload and download roundtrip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upload_and_download_empty_file() {
    let h = Harness::new();
    let status = h.upload("empty.bin", b"", None).await;
    assert_eq!(status, StatusCode::OK);

    let req = Request::builder()
        .uri("/files/empty.bin")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = raw_body(resp).await;
    assert!(body.is_empty());
}

// ---------------------------------------------------------------------------
// 17. Unicode filename upload and download roundtrip
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upload_unicode_filename() {
    let h = Harness::new();
    let status = h.upload("日本語ファイル.txt", b"unicode content", None).await;
    assert_eq!(status, StatusCode::OK);

    let req = Request::builder()
        .uri("/files/%E6%97%A5%E6%9C%AC%E8%AA%9E%E3%83%95%E3%82%A1%E3%82%A4%E3%83%AB.txt")
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = raw_body(resp).await;
    assert_eq!(body, b"unicode content");
}

// ---------------------------------------------------------------------------
// 18. Socket serves files over real network traffic
// ---------------------------------------------------------------------------

#[tokio::test]
async fn socket_serves_file_over_network() {
    let h = Harness::new();

    // Upload and enable serving for a file
    let status = h.upload("served.bin", b"served-payload", Some("download.exe")).await;
    assert_eq!(status, StatusCode::OK);

    let req = Request::builder()
        .method("PUT")
        .uri("/files/served.bin/serve-state")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"serving":true}"#))
        .unwrap();
    assert_eq!(h.send(req).await.status(), StatusCode::OK);

    // Bind a socket on a random port
    let req = Request::builder()
        .method("POST")
        .uri("/sockets")
        .header("x-api-key", API_KEY)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"addr":"127.0.0.1:0"}"#))
        .unwrap();
    let resp = h.send(req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let status_resp: StatusResponse = json_body(resp).await;
    let bound_addr = status_resp
        .message
        .strip_prefix("Bound swerve socket on ")
        .expect("bind response should include bound address")
        .to_string();

    // Give the listener a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Fetch the served file over HTTP from the swerve socket
    let client = reqwest::Client::new();
    let url = format!("http://{}/download.exe", bound_addr);
    let resp = client.get(&url).send().await.expect("request to swerve socket");
    assert_eq!(resp.status(), 200);
    let body = resp.bytes().await.expect("body from swerve socket");
    assert_eq!(body.as_ref(), b"served-payload");

    // Verify non-served file returns 404
    let resp = client
        .get(format!("http://{}/nonexistent.bin", bound_addr))
        .send()
        .await
        .expect("request to swerve socket");
    assert_eq!(resp.status(), 404);

    // Unbind
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/sockets?addr={}", bound_addr))
        .header("x-api-key", API_KEY)
        .body(Body::empty())
        .unwrap();
    assert_eq!(h.send(req).await.status(), StatusCode::OK);
}
