use crate::config::Config;
use reqwest::Client;
use swerve_core::api::*;

pub struct SwerveClient {
    client: Client,
    base_url: String,
    api_key: String,
    verbose: bool,
}

impl SwerveClient {
    pub fn new(config: &Config, verbose: bool) -> Self {
        Self {
            client: Client::new(),
            base_url: config.server_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            verbose,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Centralized response handler for endpoints returning StatusResponse
    async fn handle_status_response(
        &self,
        resp: reqwest::Response,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let status = resp.status();
        if status.is_success() {
            Ok(resp.json::<StatusResponse>().await?)
        } else {
            let body = resp.text().await.unwrap_or_default();
            if let Ok(sr) = serde_json::from_str::<StatusResponse>(&body) {
                Err(sr.message.into())
            } else {
                Err(format!("Server returned {} — {}", status, body).into())
            }
        }
    }

    /// Centralized response handler for endpoints returning JSON data
    async fn handle_json_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let status = resp.status();
        if status.is_success() {
            Ok(resp.json::<T>().await?)
        } else {
            let body = resp.text().await.unwrap_or_default();
            if let Ok(sr) = serde_json::from_str::<StatusResponse>(&body) {
                Err(sr.message.into())
            } else {
                Err(format!("Server returned {} — {}", status, body).into())
            }
        }
    }

    pub async fn health(&self) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let url = self.url("/health");
        if self.verbose { eprintln!("GET {}", url); }
        let resp = self.client.get(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;
        self.handle_status_response(resp).await
    }

    pub async fn upload_file(
        &self,
        file_path: &str,
        serve_name: Option<&str>,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let path = std::path::Path::new(file_path);
        let file_name = path
            .file_name()
            .ok_or_else(|| format!("Invalid file path '{}': no filename component", file_path))?
            .to_string_lossy()
            .to_string();

        let file_bytes = tokio::fs::read(file_path).await
            .map_err(|e| format!("Cannot read '{}': {}", file_path, e))?;

        let file_part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name.clone())
            .mime_str("application/octet-stream")?;

        let mut form = reqwest::multipart::Form::new().part("file", file_part);

        if let Some(name) = serve_name {
            form = form.text("serve_name", name.to_string());
        }

        let url = self.url("/files");
        if self.verbose { eprintln!("POST {}", url); }
        let resp = self.client
            .post(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .multipart(form)
            .send()
            .await?;

        self.handle_status_response(resp).await
    }

    pub async fn list_files(
        &self,
    ) -> Result<Vec<swerve_core::types::SwerveFile>, Box<dyn std::error::Error>> {
        let url = self.url("/files");
        if self.verbose { eprintln!("GET {}", url); }
        let resp = self.client
            .get(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        let list: FileListResponse = self.handle_json_response(resp).await?;
        Ok(list.files)
    }

    pub async fn download_file(
        &self,
        real_name: &str,
        output_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let encoded = urlencoding::encode(real_name);
        let url = self.url(&format!("/files/{}", encoded));
        if self.verbose { eprintln!("GET {}", url); }
        let resp = self.client
            .get(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            if let Ok(sr) = serde_json::from_str::<StatusResponse>(&body) {
                return Err(sr.message.into());
            }
            return Err(format!("Server returned {}", body).into());
        }

        let bytes = resp.bytes().await?;
        tokio::fs::write(output_path, bytes).await?;
        Ok(())
    }

    pub async fn destroy_file(
        &self,
        real_name: &str,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let encoded = urlencoding::encode(real_name);
        let url = self.url(&format!("/files/{}", encoded));
        if self.verbose { eprintln!("DELETE {}", url); }
        let resp = self.client
            .delete(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        self.handle_status_response(resp).await
    }

    pub async fn set_serve_state(
        &self,
        real_name: &str,
        serving: bool,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let encoded = urlencoding::encode(real_name);
        let url = self.url(&format!("/files/{}/serve-state", encoded));
        if self.verbose { eprintln!("PUT {}", url); }
        let resp = self.client
            .put(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .json(&SetServeStateRequest { serving })
            .send()
            .await?;

        self.handle_status_response(resp).await
    }

    pub async fn set_serve_name(
        &self,
        real_name: &str,
        serve_name: &str,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let encoded = urlencoding::encode(real_name);
        let url = self.url(&format!("/files/{}/serve-name", encoded));
        if self.verbose { eprintln!("PUT {}", url); }
        let resp = self.client
            .put(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .json(&SetServeNameRequest {
                serve_name: serve_name.to_string(),
            })
            .send()
            .await?;

        self.handle_status_response(resp).await
    }

    pub async fn list_sockets(
        &self,
    ) -> Result<Vec<swerve_core::types::SwerveSocket>, Box<dyn std::error::Error>> {
        let url = self.url("/sockets");
        if self.verbose { eprintln!("GET {}", url); }
        let resp = self.client
            .get(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        let list: SocketListResponse = self.handle_json_response(resp).await?;
        Ok(list.sockets)
    }

    pub async fn bind_socket(
        &self,
        addr: &str,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let url = self.url("/sockets");
        if self.verbose { eprintln!("POST {}", url); }
        let resp = self.client
            .post(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .json(&BindSocketRequest {
                addr: addr.to_string(),
            })
            .send()
            .await?;

        self.handle_status_response(resp).await
    }

    pub async fn unbind_socket(
        &self,
        addr: &str,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let encoded_addr = urlencoding::encode(addr);
        let url = self.url(&format!("/sockets?addr={}", encoded_addr));
        if self.verbose { eprintln!("DELETE {}", url); }
        let resp = self.client
            .delete(&url)
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        self.handle_status_response(resp).await
    }
}
