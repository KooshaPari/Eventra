//! Cross-cutting security primitives.
//!
//! Provides:
//! - [`cipher::Cipher`] — pluggable symmetric encryption trait and an XOR-based
//!   reference implementation suitable for tests and non-production use.
//! - [`EncryptedEventStore`] — [`EventStore`](crate::domain::EventStore)
//!   adapter that transparently encrypts event payloads at rest.
//! - [`signer::EventSigner`] — HMAC-based event signing trait plus a default
//!   [`HmacSigner`] implementation.
//! - [`SignedEventBus`] — [`EventBus`](crate::domain::EventBus) decorator that
//!   signs outbound events and verifies signatures on delivery.

pub mod cipher;
pub mod encrypted_store;
pub mod signer;
pub mod signed_bus;

pub use cipher::{Cipher, XorCipher};
pub use encrypted_store::EncryptedEventStore;
pub use signer::{EventSigner, HmacSigner};
pub use signed_bus::SignedEventBus;
