//! Google OAuth (installed-app/loopback flow) and token storage.

mod flow;
mod keychain;
mod tokens;

pub use flow::{ensure_fresh, fetch_user_info, LoginFlow};
pub use keychain::{clear_tokens, load_tokens, store_tokens};
pub use tokens::{Tokens, UserInfo};

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("failed to (de)serialize stored tokens: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("oauth configuration error: {0}")]
    Config(String),
    #[error("oauth token exchange failed: {0}")]
    Exchange(String),
    #[error("keychain access failed: {0}")]
    Keyring(String),
    #[error("no authorization code in the redirect callback")]
    MissingCode,
    #[error("state parameter did not match; possible CSRF")]
    StateMismatch,
    #[error("timed out waiting for the browser redirect")]
    Timeout,
    #[error("access token expired and no refresh token is available; login again")]
    Expired,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
}
