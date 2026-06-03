use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use swerve_core::crypto::FileKey;
use swerve_core::types::{SwerveFile, SwerveSocket};

pub struct ManagedFile {
    pub info: SwerveFile,
    pub key: FileKey,
    pub disk_name: String,
}

#[derive(Debug)]
pub enum StateError {
    NotFound,
    ServeNameConflict,
    Internal(&'static str),
    Io(String),
}

#[derive(Debug)]
pub enum SocketStatus {
    Pending,
    Running,
    Stopped,
    Error(String),
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

    pub async fn upload_file(
        &self,
        storage_name: String,
        managed: ManagedFile,
    ) -> Result<Option<ManagedFile>, StateError> {
        let mut inner = self.inner.write().await;

        if managed.info.serving
            && let Some(existing) = inner.serve_index.get(&managed.info.serve_name)
            && existing != &storage_name
        {
            return Err(StateError::ServeNameConflict);
        }

        let old = inner.files.insert(storage_name.clone(), managed);
        if let Some(ref old_file) = old && old_file.info.serving {
            inner.serve_index.remove(&old_file.info.serve_name);
        }
        let new_serve_name = inner.files.get(&storage_name).and_then(|new_file| {
            new_file
                .info
                .serving
                .then(|| new_file.info.serve_name.clone())
        });
        if let Some(new_serve_name) = new_serve_name {
            inner.serve_index.insert(new_serve_name, storage_name);
        }

        Ok(old)
    }

    pub async fn remove_file(&self, storage_name: &str) -> Option<ManagedFile> {
        let mut inner = self.inner.write().await;
        let removed = inner.files.remove(storage_name);
        if let Some(ref f) = removed && f.info.serving {
            inner.serve_index.remove(&f.info.serve_name);
        }
        removed
    }

    pub async fn remove_file_if_disk_name(
        &self,
        storage_name: &str,
        disk_name: &str,
    ) -> Option<ManagedFile> {
        let mut inner = self.inner.write().await;
        if inner.files.get(storage_name)?.disk_name != disk_name {
            return None;
        }
        let removed = inner.files.remove(storage_name);
        if let Some(ref f) = removed && f.info.serving {
            inner.serve_index.remove(&f.info.serve_name);
        }
        removed
    }

    pub async fn get_disk_name(&self, storage_name: &str) -> Option<String> {
        let inner = self.inner.read().await;
        inner.files.get(storage_name).map(|f| f.disk_name.clone())
    }

    pub async fn get_file_for_download(
        &self,
        storage_name: &str,
    ) -> Option<(SwerveFile, String, FileKey)> {
        let inner = self.inner.read().await;
        inner.files.get(storage_name).map(|f| {
            (f.info.clone(), f.disk_name.clone(), f.key.clone())
        })
    }

    pub async fn list_files(&self) -> Vec<SwerveFile> {
        let inner = self.inner.read().await;
        inner.files.values().map(|f| f.info.clone()).collect()
    }

    pub async fn set_serve_state(&self, storage_name: &str, serving: bool) -> Result<(), StateError> {
        let mut inner = self.inner.write().await;
        let file = inner.files.get(storage_name).ok_or(StateError::NotFound)?;
        let serve_name = file.info.serve_name.clone();
        let was_serving = file.info.serving;

        if serving && !was_serving {
            if let Some(existing) = inner.serve_index.get(&serve_name) && existing != storage_name {
                return Err(StateError::ServeNameConflict);
            }
            inner.serve_index.insert(serve_name, storage_name.to_string());
        } else if !serving && was_serving {
            inner.serve_index.remove(&serve_name);
        }

        let file = inner
            .files
            .get_mut(storage_name)
            .ok_or(StateError::Internal("File disappeared under write lock"))?;
        file.info.serving = serving;
        Ok(())
    }

    pub async fn set_serve_name(
        &self,
        storage_name: &str,
        new_serve_name: String,
    ) -> Result<(), StateError> {
        let mut inner = self.inner.write().await;
        let file = inner.files.get(storage_name).ok_or(StateError::NotFound)?;
        let old_serve_name = file.info.serve_name.clone();
        let is_serving = file.info.serving;

        if is_serving {
            if let Some(existing) = inner.serve_index.get(&new_serve_name) && existing != storage_name {
                return Err(StateError::ServeNameConflict);
            }
            inner.serve_index.remove(&old_serve_name);
            inner.serve_index
                .insert(new_serve_name.clone(), storage_name.to_string());
        }

        let file = inner
            .files
            .get_mut(storage_name)
            .ok_or(StateError::Internal("File disappeared under write lock"))?;
        file.info.serve_name = new_serve_name;
        Ok(())
    }

    pub async fn get_file_for_serving(&self, serve_name: &str) -> Option<(String, FileKey)> {
        let inner = self.inner.read().await;
        let storage_name = inner.serve_index.get(serve_name)?;
        let file = inner.files.get(storage_name)?;
        if file.info.serving {
            Some((file.disk_name.clone(), file.key.clone()))
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

    pub async fn reserve_socket_slot(&self, addr: &str) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        if inner.sockets.len() >= swerve_core::api::MAX_SWERVE_SOCKETS {
            return Err(format!(
                "Maximum number of swerve sockets ({}) reached",
                swerve_core::api::MAX_SWERVE_SOCKETS
            ));
        }
        if inner.sockets.contains_key(addr) {
            return Err(format!("Socket '{}' already bound", addr));
        }
        inner.sockets.insert(
            addr.to_string(),
            SocketHandle {
                shutdown_tx: None,
                handle: tokio::spawn(async {}),
                status: SocketStatus::Pending,
            },
        );
        Ok(())
    }

    pub async fn fulfill_socket_reservation(
        &self,
        addr: &str,
        handle: SocketHandle,
    ) -> Result<(), SocketHandle> {
        let mut inner = self.inner.write().await;
        match inner.sockets.get(addr) {
            Some(existing) if matches!(existing.status, SocketStatus::Pending) => {
                inner.sockets.insert(addr.to_string(), handle);
                Ok(())
            }
            _ => Err(handle),
        }
    }

    pub async fn cancel_socket_reservation(&self, addr: &str) {
        let mut inner = self.inner.write().await;
        inner.sockets.remove(addr);
    }

    pub async fn remove_socket(&self, addr: &str) -> Option<SocketHandle> {
        let mut inner = self.inner.write().await;
        inner.sockets.remove(addr)
    }

    pub async fn update_socket_status(&self, addr: &str, status: SocketStatus) {
        let mut inner = self.inner.write().await;
        if let Some(socket) = inner.sockets.get_mut(addr) {
            socket.status = status;
        }
    }

    pub async fn list_sockets(&self) -> Vec<SwerveSocket> {
        let inner = self.inner.read().await;
        inner.sockets
            .iter()
            .map(|(addr, h)| SwerveSocket {
                addr: addr.clone(),
                active: matches!(h.status, SocketStatus::Running),
            })
            .collect()
    }
}
