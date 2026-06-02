use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use zeroize::Zeroize;

/// AES-256-GCM encryption key and nonce pair.
/// Key material is zeroized on drop.
#[derive(Clone)]
pub struct FileKey {
    key: [u8; 32],
    nonce: [u8; 12],
}

impl Drop for FileKey {
    fn drop(&mut self) {
        self.key.zeroize();
        self.nonce.zeroize();
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = FileKey::generate();
        let data = b"hello world swerve";
        let encrypted = key.encrypt(data).unwrap();
        assert_ne!(encrypted, data);
        let decrypted = key.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn encrypt_decrypt_empty() {
        let key = FileKey::generate();
        let data = b"";
        let encrypted = key.encrypt(data).unwrap();
        let decrypted = key.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key1 = FileKey::generate();
        let key2 = FileKey::generate();
        let data = b"secret data";
        let encrypted = key1.encrypt(data).unwrap();
        assert!(key2.decrypt(&encrypted).is_err());
    }

    #[test]
    fn different_keys_produce_different_ciphertext() {
        let key1 = FileKey::generate();
        let key2 = FileKey::generate();
        let data = b"same input";
        let enc1 = key1.encrypt(data).unwrap();
        let enc2 = key2.encrypt(data).unwrap();
        assert_ne!(enc1, enc2);
    }

    #[test]
    fn large_data_roundtrip() {
        let key = FileKey::generate();
        let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
        let encrypted = key.encrypt(&data).unwrap();
        let decrypted = key.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, data);
    }
}
