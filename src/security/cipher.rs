//! Pluggable symmetric [`Cipher`] trait and a non-cryptographic reference
//! implementation.
//!
//! Production deployments should swap [`XorCipher`] for an AEAD cipher such as
//! AES-GCM or ChaCha20-Poly1305 by implementing [`Cipher`]. The trait is
//! intentionally narrow so that any AEAD with byte-string input/output
//! semantics can plug in.

use crate::domain::EventError;

/// Symmetric cipher abstraction used by [`super::EncryptedEventStore`].
///
/// Implementations must be deterministic: encrypting the same plaintext with
/// the same key must always produce the same ciphertext, otherwise stored
/// events cannot be decrypted on read. Real AEADs typically use a random
/// nonce — in that case, store the nonce alongside the ciphertext (for example
/// by prepending it) so that decryption remains deterministic per payload.
pub trait Cipher: Send + Sync {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, EventError>;
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, EventError>;
}

/// Non-cryptographic XOR cipher. Intended for tests and as a reference; do
/// not use in production. The key is repeated to match the plaintext length.
#[derive(Debug, Clone)]
pub struct XorCipher {
    key: Vec<u8>,
}

impl XorCipher {
    pub fn new(key: impl Into<Vec<u8>>) -> Self {
        let key = key.into();
        assert!(!key.is_empty(), "XorCipher requires a non-empty key");
        Self { key }
    }
}

impl Cipher for XorCipher {
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, EventError> {
        Ok(plaintext
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ self.key[i % self.key.len()])
            .collect())
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, EventError> {
        // XOR is symmetric.
        self.encrypt(ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xor_cipher_roundtrips_payload() {
        let cipher = XorCipher::new(b"k".to_vec());
        let plaintext = b"hello, world".to_vec();
        let ciphertext = cipher.encrypt(&plaintext).expect("encrypt");
        let recovered = cipher.decrypt(&ciphertext).expect("decrypt");
        assert_eq!(recovered, plaintext);
        assert_ne!(ciphertext, plaintext);
    }

    #[test]
    fn xor_cipher_with_long_key_roundtrips_payload() {
        let cipher = XorCipher::new(b"a-much-longer-key!".to_vec());
        let plaintext = vec![0u8, 1, 2, 3, 4, 5];
        let ciphertext = cipher.encrypt(&plaintext).expect("encrypt");
        let recovered = cipher.decrypt(&ciphertext).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    #[test]
    #[should_panic(expected = "non-empty key")]
    fn xor_cipher_rejects_empty_key() {
        let _ = XorCipher::new(Vec::<u8>::new());
    }
}
