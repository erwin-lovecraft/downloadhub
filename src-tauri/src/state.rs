//! App-wide state read from the environment once at startup.

use downloadhub_core::auth::AuthConfig;

pub struct AppState {
    /// `None` means `GOOGLE_OAUTH_CLIENT_ID`/`_SECRET` weren't set; the app
    /// still runs, login just reports it isn't configured.
    pub auth_config: Option<AuthConfig>,
    /// `None` means `YOUTUBE_API_KEY` wasn't set; search reports the same.
    pub youtube_api_key: Option<String>,
}

impl AppState {
    pub fn from_env() -> Self {
        let auth_config = match (
            std::env::var("GOOGLE_OAUTH_CLIENT_ID"),
            std::env::var("GOOGLE_OAUTH_CLIENT_SECRET"),
        ) {
            (Ok(client_id), Ok(client_secret)) => Some(AuthConfig {
                client_id,
                client_secret,
            }),
            _ => None,
        };
        let youtube_api_key = std::env::var("YOUTUBE_API_KEY").ok();
        Self {
            auth_config,
            youtube_api_key,
        }
    }
}
