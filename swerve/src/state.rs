use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use swerve_core::crypto::FileKey;
use swerve_core::types::SwerveFile;

pub struct ManagedFile {
    pub info: SwerveFile,
    pub key: FileKey,
}

pub struct SocketHandle {
    pub shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    pub handle: tokio::task::JoinHandle<()>,
    pub addr: String,
}

pub struct AppStateInner {
    pub files: RwLock<HashMap<String, ManagedFile>>,
    pub sockets: RwLock<HashMap<String, SocketHandle>>,
    pub storage_dir: PathBuf,
    pub api_key: String,
}

pub type AppState = Arc<AppStateInner>;

impl AppStateInner {
    pub fn new(api_key: String, storage_dir: PathBuf) -> AppState {
        Arc::new(Self {
            files: RwLock::new(HashMap::new()),
            sockets: RwLock::new(HashMap::new()),
            storage_dir,
            api_key,
        })
    }
}
