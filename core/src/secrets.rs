//! Credential resolution: runtime env (`std::env::var`, incl. `.env`) first,
//! then the compile-time embedded value (`option_env!`) for release builds.
//! Embedding is not encryption — see `docs/ARCHITECTURE.md`.

/// Resolves a secret: runtime environment first, then the value embedded at
/// compile time. `None` if neither is set (the app degrades gracefully, as it
/// already does for an unconfigured key).
macro_rules! resolve_secret {
    ($name:literal) => {
        std::env::var($name)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| {
                option_env!($name)
                    .filter(|v| !v.is_empty())
                    .map(String::from)
            })
    };
}

/// Google OAuth "Desktop app" client id (`GOOGLE_OAUTH_CLIENT_ID`).
pub fn google_oauth_client_id() -> Option<String> {
    resolve_secret!("GOOGLE_OAUTH_CLIENT_ID")
}

/// Google OAuth "Desktop app" client secret (`GOOGLE_OAUTH_CLIENT_SECRET`).
pub fn google_oauth_client_secret() -> Option<String> {
    resolve_secret!("GOOGLE_OAUTH_CLIENT_SECRET")
}

/// YouTube Data API v3 key (`YOUTUBE_API_KEY`).
pub fn youtube_api_key() -> Option<String> {
    resolve_secret!("YOUTUBE_API_KEY")
}
