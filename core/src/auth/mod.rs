//! Google OAuth (installed-app/loopback flow) and token storage.
//!
//! [`LoginFlow::begin`] opens a listener on an OS-assigned loopback port and
//! returns the URL the caller (the `app` crate) must open in the user's
//! browser; [`LoginFlow::complete`] waits for the redirect, exchanges the
//! authorization code, and returns [`Tokens`]. Tokens are persisted with
//! [`store_tokens`]/[`load_tokens`] via the OS keychain (through `keyring`),
//! never to a plaintext file.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse,
    TokenUrl,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::timeout;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://www.googleapis.com/oauth2/v3/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";
const KEYRING_SERVICE: &str = "com.downloadhub.app";
const KEYRING_USERNAME: &str = "google-oauth-tokens";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300);
/// Refresh proactively this long before actual expiry.
const EXPIRY_SKEW: Duration = Duration::from_secs(60);

type ConfiguredClient = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix timestamp (seconds) the access token expires at, if known.
    pub expires_at: Option<u64>,
}

impl Tokens {
    pub fn needs_refresh(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                now + EXPIRY_SKEW.as_secs() >= expires_at
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserInfo {
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
}

fn build_client(config: &AuthConfig, redirect_uri: RedirectUrl) -> Result<ConfiguredClient, AuthError> {
    let auth_url = AuthUrl::new(AUTH_URL.to_string()).map_err(|e| AuthError::Config(e.to_string()))?;
    let token_url =
        TokenUrl::new(TOKEN_URL.to_string()).map_err(|e| AuthError::Config(e.to_string()))?;

    Ok(BasicClient::new(ClientId::new(config.client_id.clone()))
        .set_client_secret(ClientSecret::new(config.client_secret.clone()))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url)
        .set_redirect_uri(redirect_uri))
}

fn http_client() -> Result<oauth2::reqwest::Client, AuthError> {
    oauth2::reqwest::ClientBuilder::new()
        // Following redirects on the token endpoint opens the client up to SSRF.
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AuthError::Config(e.to_string()))
}

/// An in-progress login: the authorization URL to open in the browser, plus
/// the loopback listener waiting for Google's redirect.
pub struct LoginFlow {
    pub authorize_url: url::Url,
    listener: TcpListener,
    csrf_token: CsrfToken,
    pkce_verifier: PkceCodeVerifier,
    client: ConfiguredClient,
}

impl LoginFlow {
    /// Binds an OS-assigned loopback port and builds the Google
    /// authorization URL pointing back at it.
    pub async fn begin(config: &AuthConfig) -> Result<Self, AuthError> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let redirect_uri = RedirectUrl::new(format!("http://127.0.0.1:{port}"))
            .map_err(|e| AuthError::Config(e.to_string()))?;

        let client = build_client(config, redirect_uri)?;

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let (authorize_url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("openid".to_string()))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/userinfo.email".to_string(),
            ))
            .add_scope(Scope::new(
                "https://www.googleapis.com/auth/userinfo.profile".to_string(),
            ))
            .set_pkce_challenge(pkce_challenge)
            .url();

        Ok(Self {
            authorize_url,
            listener,
            csrf_token,
            pkce_verifier,
            client,
        })
    }

    /// Waits for the single redirect request from Google, verifies the
    /// CSRF state, and exchanges the authorization code for tokens.
    pub async fn complete(self) -> Result<Tokens, AuthError> {
        let (code, state) = timeout(CALLBACK_TIMEOUT, wait_for_callback(&self.listener))
            .await
            .map_err(|_| AuthError::Timeout)??;

        if state.secret() != self.csrf_token.secret() {
            return Err(AuthError::StateMismatch);
        }

        let http_client = http_client()?;
        let token_response = self
            .client
            .exchange_code(code)
            .set_pkce_verifier(self.pkce_verifier)
            .request_async(&http_client)
            .await
            .map_err(|e| AuthError::Exchange(e.to_string()))?;

        Ok(tokens_from_response(&token_response))
    }
}

async fn wait_for_callback(
    listener: &TcpListener,
) -> Result<(AuthorizationCode, CsrfToken), AuthError> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let mut reader = BufReader::new(&mut stream);

        let mut request_line = String::new();
        reader.read_line(&mut request_line).await?;

        let path = match request_line.split_whitespace().nth(1) {
            Some(p) => p,
            None => continue,
        };
        let url = match url::Url::parse(&format!("http://127.0.0.1{path}")) {
            Ok(u) => u,
            Err(_) => continue,
        };

        let code = url
            .query_pairs()
            .find(|(k, _)| k == "code")
            .map(|(_, v)| AuthorizationCode::new(v.into_owned()));
        let state = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| CsrfToken::new(v.into_owned()));

        let body = "You're signed in. You can close this tab and go back to downloadhub.";
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes()).await;

        let (Some(code), Some(state)) = (code, state) else {
            return Err(AuthError::MissingCode);
        };
        return Ok((code, state));
    }
}

fn tokens_from_response(
    response: &impl TokenResponse<TokenType = oauth2::basic::BasicTokenType>,
) -> Tokens {
    let expires_at = response.expires_in().map(|d| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + d.as_secs()
    });

    Tokens {
        access_token: response.access_token().secret().clone(),
        refresh_token: response.refresh_token().map(|t| t.secret().clone()),
        expires_at,
    }
}

/// Returns a fresh (non-expired) copy of `tokens`, refreshing via the
/// stored refresh token and re-persisting to the keychain if needed.
pub async fn ensure_fresh(config: &AuthConfig, tokens: Tokens) -> Result<Tokens, AuthError> {
    if !tokens.needs_refresh() {
        return Ok(tokens);
    }
    let Some(refresh_token) = tokens.refresh_token.clone() else {
        return Err(AuthError::Expired);
    };

    let redirect_uri = RedirectUrl::new("http://127.0.0.1:0".to_string())
        .map_err(|e| AuthError::Config(e.to_string()))?;
    let client = build_client(config, redirect_uri)?;
    let http_client = http_client()?;

    let token_response = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.clone()))
        .request_async(&http_client)
        .await
        .map_err(|e| AuthError::Exchange(e.to_string()))?;

    let mut refreshed = tokens_from_response(&token_response);
    // Google may omit refresh_token on refresh responses; keep the original.
    if refreshed.refresh_token.is_none() {
        refreshed.refresh_token = Some(refresh_token);
    }
    store_tokens(&refreshed)?;
    Ok(refreshed)
}

pub async fn fetch_user_info(access_token: &str) -> Result<UserInfo, AuthError> {
    let client = reqwest::Client::new();
    let info = client
        .get(USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json::<UserInfo>()
        .await?;
    Ok(info)
}

fn keyring_entry() -> Result<keyring::Entry, AuthError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME).map_err(|e| AuthError::Keyring(e.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens_expiring_in(secs: i64) -> Tokens {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        Tokens {
            access_token: "at".to_string(),
            refresh_token: None,
            expires_at: Some((now + secs).max(0) as u64),
        }
    }

    #[test]
    fn no_expiry_never_needs_refresh() {
        let tokens = Tokens {
            access_token: "at".to_string(),
            refresh_token: None,
            expires_at: None,
        };
        assert!(!tokens.needs_refresh());
    }

    #[test]
    fn far_from_expiry_does_not_need_refresh() {
        assert!(!tokens_expiring_in(3600).needs_refresh());
    }

    #[test]
    fn within_skew_of_expiry_needs_refresh() {
        assert!(tokens_expiring_in(30).needs_refresh());
    }

    #[test]
    fn already_expired_needs_refresh() {
        assert!(tokens_expiring_in(-30).needs_refresh());
    }
}
