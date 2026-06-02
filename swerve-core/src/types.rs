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
