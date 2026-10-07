//! The one token mechanism (ADR 0008): 256 random bits from the OS, stored only as a SHA-256 hash.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use secrecy::SecretString;
use sha2::{Digest, Sha256};
use tada_app::store::StoreError;

/// A new token. The secret goes to its holder once. The hash goes to the database.
#[derive(Debug)]
pub(crate) struct NewToken {
    pub secret: SecretString,
    pub hash: Vec<u8>,
}

/// Creates a token: 32 random bytes in URL-safe Base64 without padding, after `prefix`.
/// The hash covers the whole secret, prefix included, so a lookup hashes what the holder sends.
pub(crate) fn new_token(prefix: &str) -> Result<NewToken, StoreError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| {
        StoreError::Internal(Box::new(std::io::Error::other(error.to_string())))
    })?;
    let secret = format!("{prefix}{}", URL_SAFE_NO_PAD.encode(bytes));
    let hash = hash_token(&secret);
    Ok(NewToken {
        secret: SecretString::from(secret),
        hash,
    })
}

/// The SHA-256 hash of a token. Lookups compare hashes, never secrets.
pub(crate) fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use super::*;

    #[test]
    fn two_tokens_differ() {
        let (a, b) = (new_token("").unwrap(), new_token("").unwrap());
        assert_ne!(a.secret.expose_secret(), b.secret.expose_secret());
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn the_hash_is_the_sha256_of_the_secret() {
        let token = new_token("").unwrap();
        assert_eq!(token.hash.len(), 32);
        assert_eq!(token.hash, hash_token(token.secret.expose_secret()));
        assert_eq!(token.secret.expose_secret().len(), 43, "256 bits in Base64");
    }

    #[test]
    fn keeps_the_prefix() {
        let token = new_token("tada_pat_").unwrap();
        assert!(token.secret.expose_secret().starts_with("tada_pat_"));
        assert_eq!(token.secret.expose_secret().len(), 9 + 43);
    }

    #[test]
    fn debug_hides_the_secret() {
        let token = new_token("").unwrap();
        assert!(!format!("{token:?}").contains(token.secret.expose_secret()));
    }
}
