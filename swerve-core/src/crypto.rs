use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use zeroize::Zeroize;

/// AES-256-GCM encryption key.
/// Key material is zeroized on drop.
#[derive(Clone)]
pub struct FileKey {
    key: [u8; 32],
}

impl Drop for FileKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl FileKey {
    /// Generate a new random key
    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        Self { key }
    }

    /// Encrypt plaintext data with a fresh random nonce prepended to the ciphertext
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, data)?;

        let mut output = Vec::with_capacity(12 + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);
        Ok(output)
    }

    /// Decrypt ciphertext data with the nonce stored in the first 12 bytes
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        if data.len() < 12 {
            return Err(aes_gcm::Error);
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&self.key));
        let nonce = Nonce::from_slice(nonce_bytes);
        cipher.decrypt(nonce, ciphertext)
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
