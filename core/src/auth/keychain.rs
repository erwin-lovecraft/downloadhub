//! Token persistence via the OS keychain (`keyring`).

use super::tokens::Tokens;
use super::AuthError;

const KEYRING_SERVICE: &str = "com.downloadhub.app";
const KEYRING_USERNAME: &str = "google-oauth-tokens";

fn keyring_entry() -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
        .map_err(|e| AuthError::Keyring(e.to_string()))
}

pub fn store_tokens(tokens: &Tokens) -> Result<(), AuthError> {
    let json = serde_json::to_string(tokens)?;
    keyring_entry()?
        .set_password(&json)
        .map_err(|e| AuthError::Keyring(e.to_string()))
}

pub fn load_tokens() -> Result<Option<Tokens>, AuthError> {
    match keyring_entry()?.get_password() {
        Ok(json) => Ok(Some(serde_json::from_str(&json)?)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AuthError::Keyring(e.to_string())),
    }
}

pub fn clear_tokens() -> Result<(), AuthError> {
    match keyring_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AuthError::Keyring(e.to_string())),
    }
}
