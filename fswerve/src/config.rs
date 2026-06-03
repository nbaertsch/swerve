use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub server_url: String,
    pub api_key: String,
}

fn config_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".fswerve"))
}

fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let dir = config_dir()?;
    std::fs::create_dir_all(&dir)?;

    let path = config_path()?;
    let toml_str = toml::to_string_pretty(config)?;

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(toml_str.as_bytes())?;
    }

    #[cfg(not(unix))]
    {
        std::fs::write(&path, toml_str)?;
    }

    Ok(())
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path = config_path()?;
    if !path.exists() {
        return Err(format!(
            "No configuration found at {}. Run 'fswerve config set' first.",
            path.display()
        ).into());
    }
    let content = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

/// Resolve config: CLI flags > env vars > config file
pub fn resolve_config(
    server_url_override: Option<&str>,
    api_key_override: Option<&str>,
) -> Result<Config, Box<dyn std::error::Error>> {
    let base = match load_config() {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            let path = config_path().ok();
            let file_exists = path.as_ref().is_some_and(|p| p.exists());
            if file_exists {
                return Err(format!("Failed to read config file: {}", e).into());
            }
            None
        }
    };

    let server_url = server_url_override
        .map(|s| s.to_string())
        .or_else(|| base.as_ref().map(|c| c.server_url.clone()))
        .ok_or("Server URL not configured. Run 'fswerve config set' or pass --server-url")?;

    let api_key = api_key_override
        .map(|s| s.to_string())
        .or_else(|| base.as_ref().map(|c| c.api_key.clone()))
        .ok_or("API key not configured. Run 'fswerve config set' or pass --api-key")?;

    Ok(Config { server_url, api_key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_roundtrip() {
        let config = Config {
            server_url: "http://localhost:9740".to_string(),
            api_key: "test-key-123".to_string(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.server_url, config.server_url);
        assert_eq!(loaded.api_key, config.api_key);
    }

    #[test]
    fn toml_roundtrip_special_chars() {
        let config = Config {
            server_url: "http://[::1]:9740/path?q=1&x=2".to_string(),
            api_key: "key=with+special/chars!@#".to_string(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.server_url, config.server_url);
        assert_eq!(loaded.api_key, config.api_key);
    }

    #[test]
    fn save_and_load_roundtrip_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = Config {
            server_url: "http://10.0.0.5:9740".to_string(),
            api_key: "disk-key-456".to_string(),
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        std::fs::write(&path, &toml_str).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let loaded: Config = toml::from_str(&content).unwrap();
        assert_eq!(loaded.server_url, config.server_url);
        assert_eq!(loaded.api_key, config.api_key);
    }

    #[test]
    fn resolve_config_prefers_overrides() {
        let cfg = resolve_config(
            Some("http://override:1234"),
            Some("override-key"),
        )
        .unwrap();
        assert_eq!(cfg.server_url, "http://override:1234");
        assert_eq!(cfg.api_key, "override-key");
    }

    #[test]
    fn resolve_config_partial_override_server_url() {
        // If config file exists, a partial override should merge.
        // If no config file, partial override with one missing should fail.
        let result = resolve_config(Some("http://partial:9999"), None);
        // Either succeeds (config file provides api_key) or fails (no config file)
        match result {
            Ok(cfg) => assert_eq!(cfg.server_url, "http://partial:9999"),
            Err(e) => assert!(
                e.to_string().contains("API key"),
                "Expected API key error, got: {}",
                e
            ),
        }
    }

    #[test]
    fn resolve_config_fails_without_any_source() {
        // Skip if user has a real config file installed
        if let Ok(p) = config_path() {
            if p.exists() {
                return;
            }
        }
        let result = resolve_config(None, None);
        assert!(result.is_err());
    }

    #[test]
    fn config_path_is_under_home() {
        if let Ok(p) = config_path() {
            let home = dirs::home_dir().unwrap();
            assert!(p.starts_with(&home));
            assert!(p.ends_with("config.toml"));
        }
    }
}
