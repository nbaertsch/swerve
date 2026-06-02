use serde::{Deserialize, Serialize};

/// Represents a file stored in swerve
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwerveFile {
    /// Original filename as uploaded by the client
    pub real_name: String,
    /// SHA-256 hash of real_name, used as on-disk filename
    pub storage_name: String,
    /// The filename served to clients on swerve sockets (spoofed name)
    pub serve_name: String,
    /// Whether this file is actively served on swerve sockets
    pub serving: bool,
    /// Size in bytes of the original (unencrypted) file
    pub size: u64,
}

/// Represents a swerve socket binding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwerveSocket {
    /// The address this socket is bound to (e.g., "0.0.0.0:8080")
    pub addr: String,
    /// Whether the socket is currently active
    pub active: bool,
}

/// Default management port
pub const DEFAULT_MGMT_PORT: u16 = 9740;

/// Compute the storage name (SHA-256 hex) for a given real filename
pub fn storage_name_for(real_name: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(real_name.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_name_is_deterministic() {
        let name = "test.bin";
        assert_eq!(storage_name_for(name), storage_name_for(name));
    }

    #[test]
    fn storage_name_differs_for_different_inputs() {
        assert_ne!(storage_name_for("file1.bin"), storage_name_for("file2.bin"));
    }

    #[test]
    fn storage_name_is_valid_hex() {
        let result = storage_name_for("test");
        assert_eq!(result.len(), 64);
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn storage_name_matches_known_sha256() {
        // SHA-256 of "hello"
        let result = storage_name_for("hello");
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn storage_name_handles_unicode() {
        let result = storage_name_for("日本語ファイル.txt");
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn swerve_file_serde_roundtrip() {
        let file = SwerveFile {
            real_name: "test.bin".to_string(),
            storage_name: "abc123".to_string(),
            serve_name: "update.exe".to_string(),
            serving: true,
            size: 1024,
        };
        let json = serde_json::to_string(&file).unwrap();
        let deserialized: SwerveFile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.real_name, file.real_name);
        assert_eq!(deserialized.storage_name, file.storage_name);
        assert_eq!(deserialized.serve_name, file.serve_name);
        assert_eq!(deserialized.serving, file.serving);
        assert_eq!(deserialized.size, file.size);
    }
}
