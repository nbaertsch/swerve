use crate::config::Config;
use reqwest::Client;
use swerve_core::api::*;

pub struct SwerveClient {
    client: Client,
    base_url: String,
    api_key: String,
}

impl SwerveClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            base_url: config.server_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub async fn upload_file(
        &self,
        file_path: &str,
        serve_as: Option<&str>,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let path = std::path::Path::new(file_path);
        let file_name = path
            .file_name()
            .ok_or("Invalid file path")?
            .to_string_lossy()
            .to_string();

        let file_bytes = tokio::fs::read(file_path).await?;

        let file_part = reqwest::multipart::Part::bytes(file_bytes)
            .file_name(file_name.clone())
            .mime_str("application/octet-stream")?;

        let mut form = reqwest::multipart::Form::new().part("file", file_part);

        if let Some(name) = serve_as {
            form = form.text("serve_name", name.to_string());
        }

        let resp = self
            .client
            .post(self.url("/files"))
            .header(API_KEY_HEADER, &self.api_key)
            .multipart(form)
            .send()
            .await?;

        let status = resp.json::<StatusResponse>().await?;
        Ok(status)
    }

    pub async fn list_files(
        &self,
    ) -> Result<Vec<swerve_core::types::SwerveFile>, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .get(self.url("/files"))
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        let list = resp.json::<FileListResponse>().await?;
        Ok(list.files)
    }

    pub async fn download_file(
        &self,
        real_name: &str,
        output_path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let resp = self
            .client
            .get(self.url(&format!("/files/{}", urlencoding(real_name))))
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let err = resp.json::<StatusResponse>().await?;
            return Err(err.message.into());
        }

        let bytes = resp.bytes().await?;
        tokio::fs::write(output_path, bytes).await?;
        Ok(())
    }

    pub async fn destroy_file(
        &self,
        real_name: &str,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .delete(self.url(&format!("/files/{}", urlencoding(real_name))))
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        let status = resp.json::<StatusResponse>().await?;
        Ok(status)
    }

    pub async fn set_serve_state(
        &self,
        real_name: &str,
        serving: bool,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .put(self.url(&format!(
                "/files/{}/serve-state",
                urlencoding(real_name)
            )))
            .header(API_KEY_HEADER, &self.api_key)
            .json(&SetServeStateRequest { serving })
            .send()
            .await?;

        let status = resp.json::<StatusResponse>().await?;
        Ok(status)
    }

    pub async fn set_serve_name(
        &self,
        real_name: &str,
        serve_name: &str,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .put(self.url(&format!(
                "/files/{}/serve-name",
                urlencoding(real_name)
            )))
            .header(API_KEY_HEADER, &self.api_key)
            .json(&SetServeNameRequest {
                serve_name: serve_name.to_string(),
            })
            .send()
            .await?;

        let status = resp.json::<StatusResponse>().await?;
        Ok(status)
    }

    pub async fn list_sockets(
        &self,
    ) -> Result<Vec<swerve_core::types::SwerveSocket>, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .get(self.url("/sockets"))
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        let list = resp.json::<SocketListResponse>().await?;
        Ok(list.sockets)
    }

    pub async fn bind_socket(
        &self,
        addr: &str,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .post(self.url("/sockets"))
            .header(API_KEY_HEADER, &self.api_key)
            .json(&BindSocketRequest {
                addr: addr.to_string(),
            })
            .send()
            .await?;

        let status = resp.json::<StatusResponse>().await?;
        Ok(status)
    }

    pub async fn unbind_socket(
        &self,
        addr: &str,
    ) -> Result<StatusResponse, Box<dyn std::error::Error>> {
        let resp = self
            .client
            .delete(self.url(&format!("/sockets?addr={}", urlencoding(addr))))
            .header(API_KEY_HEADER, &self.api_key)
            .send()
            .await?;

        let status = resp.json::<StatusResponse>().await?;
        Ok(status)
    }
}

/// Simple URL encoding for path segments and query values
fn urlencoding(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23")
        .replace('?', "%3F")
        .replace('&', "%26")
        .replace('+', "%2B")
}
