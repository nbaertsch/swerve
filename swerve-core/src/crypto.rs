use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;

/// 32-byte key + 12-byte nonce
#[derive(Debug, Clone)]
pub struct FileKey {
    pub key: [u8; 32],
    pub nonce: [u8; 12],
}

impl FileKey {
    /// Generate a new random key and nonce
    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut key);
        OsRng.fill_bytes(&mut nonce);
        Self { key, nonce }
    }

    /// Encrypt plaintext data
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Nonce::from_slice(&self.nonce);
        cipher.encrypt(nonce, data)
    }

    /// Decrypt ciphertext data
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Nonce::from_slice(&self.nonce);
        cipher.decrypt(nonce, data)
    }
}
