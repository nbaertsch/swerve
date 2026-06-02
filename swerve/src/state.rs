use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use swerve_core::crypto::FileKey;
use swerve_core::types::{SwerveFile, SwerveSocket};

pub struct ManagedFile {
    pub info: SwerveFile,
    pub key: FileKey,
}

#[derive(Debug)]
pub enum SocketStatus {
    Running,
    _Stopped,
    _Error(String),
}

pub struct SocketHandle {
    pub shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    pub handle: tokio::task::JoinHandle<()>,
    pub status: SocketStatus,
}

struct StateInner {
    /// Files keyed by storage_name (SHA-256 of real_name)
    files: HashMap<String, ManagedFile>,
    /// Reverse index: serve_name → storage_name (only for files with serving=true)
    serve_index: HashMap<String, String>,
    /// Active swerve socket listeners
    sockets: HashMap<String, SocketHandle>,
}

pub type AppState = Arc<AppStateRw>;

pub struct AppStateRw {
    inner: RwLock<StateInner>,
    storage_dir: PathBuf,
    api_key: String,
}

impl AppStateRw {
    pub fn new(api_key: String, storage_dir: PathBuf) -> AppState {
        Arc::new(Self {
            inner: RwLock::new(StateInner {
                files: HashMap::new(),
                serve_index: HashMap::new(),
                sockets: HashMap::new(),
            }),
            storage_dir,
            api_key,
        })
    }

    pub fn storage_dir(&self) -> &PathBuf {
        &self.storage_dir
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    // -- File operations --

    /// Upload a new file: insert atomically under one write lock
    pub async fn upload_file(
        &self,
        storage_name: String,
        managed: ManagedFile,
    ) -> Result<Option<ManagedFile>, String> {
        let mut inner = self.inner.write().await;
        let old = inner.files.insert(storage_name, managed);
        if let Some(ref old_file) = old {
            if old_file.info.serving {
                inner.serve_index.remove(&old_file.info.serve_name);
            }
        }
        Ok(old)
    }

    /// Remove a file by storage_name
    pub async fn remove_file(&self, storage_name: &str) -> Option<ManagedFile> {
        let mut inner = self.inner.write().await;
        let removed = inner.files.remove(storage_name);
        if let Some(ref f) = removed {
            if f.info.serving {
                inner.serve_index.remove(&f.info.serve_name);
            }
        }
        removed
    }

    /// Get file info + clone key for download (read lock)
    pub async fn get_file_for_download(&self, storage_name: &str) -> Option<(SwerveFile, FileKey)> {
        let inner = self.inner.read().await;
        inner.files.get(storage_name).map(|f| (f.info.clone(), f.key.clone()))
    }

    /// List all files
    pub async fn list_files(&self) -> Vec<SwerveFile> {
        let inner = self.inner.read().await;
        inner.files.values().map(|f| f.info.clone()).collect()
    }

    /// Set serve state for a file (atomically checks for conflicts)
    pub async fn set_serve_state(&self, storage_name: &str, serving: bool) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        let file = inner.files.get(storage_name).ok_or("File not found")?;
        let serve_name = file.info.serve_name.clone();
        let was_serving = file.info.serving;

        if serving && !was_serving {
            // Check for conflicts
            if let Some(existing) = inner.serve_index.get(&serve_name) {
                if existing != storage_name {
                    return Err("Serve name is already in use by another file".to_string());
                }
            }
            inner.serve_index.insert(serve_name, storage_name.to_string());
        } else if !serving && was_serving {
            inner.serve_index.remove(&serve_name);
        }

        let file = inner.files.get_mut(storage_name).unwrap();
        file.info.serving = serving;
        Ok(())
    }

    /// Set serve name for a file (atomically checks for conflicts)
    pub async fn set_serve_name(&self, storage_name: &str, new_serve_name: String) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        let file = inner.files.get(storage_name).ok_or("File not found")?;
        let old_serve_name = file.info.serve_name.clone();
        let is_serving = file.info.serving;

        // Check conflicts only if actively serving
        if is_serving {
            if let Some(existing) = inner.serve_index.get(&new_serve_name) {
                if existing != storage_name {
                    return Err("Serve name is already in use by another file".to_string());
                }
            }
            inner.serve_index.remove(&old_serve_name);
            inner.serve_index.insert(new_serve_name.clone(), storage_name.to_string());
        }

        let file = inner.files.get_mut(storage_name).unwrap();
        file.info.serve_name = new_serve_name;
        Ok(())
    }

    /// Look up a file by serve_name for serving (uses O(1) index)
    pub async fn get_file_for_serving(&self, serve_name: &str) -> Option<(String, FileKey)> {
        let inner = self.inner.read().await;
        let storage_name = inner.serve_index.get(serve_name)?;
        let file = inner.files.get(storage_name)?;
        if file.info.serving {
            Some((file.info.storage_name.clone(), file.key.clone()))
        } else {
            None
        }
    }

    // -- Socket operations --

    pub async fn socket_count(&self) -> usize {
        let inner = self.inner.read().await;
        inner.sockets.len()
    }

    pub async fn has_socket(&self, addr: &str) -> bool {
        let inner = self.inner.read().await;
        inner.sockets.contains_key(addr)
    }

    pub async fn insert_socket(&self, addr: String, handle: SocketHandle) {
        let mut inner = self.inner.write().await;
        inner.sockets.insert(addr, handle);
    }

    pub async fn remove_socket(&self, addr: &str) -> Option<SocketHandle> {
        let mut inner = self.inner.write().await;
        inner.sockets.remove(addr)
    }

    pub async fn list_sockets(&self) -> Vec<SwerveSocket> {
        let inner = self.inner.read().await;
        inner.sockets.iter().map(|(addr, h)| {
            SwerveSocket {
                addr: addr.clone(),
                active: matches!(h.status, SocketStatus::Running),
            }
        }).collect()
    }
}
