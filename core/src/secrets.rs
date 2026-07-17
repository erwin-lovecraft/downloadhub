//! Credential resolution shared by both binaries.
//!
//! Each secret is read in two steps, in order:
//!
//! 1. **Runtime environment** (`std::env::var`) — set by a `.env` file loaded
//!    via `dotenvy` during local development, or by any real environment
//!    variable. This keeps the existing dev workflow unchanged.
//! 2. **Compile-time embedded value** (`option_env!`) — whatever the variable
//!    was set to *when cargo compiled this crate* is baked into the binary as
//!    a string literal. This is how a shipped release build carries
//!    credentials with no `.env` file present: CI sets these variables (from
//!    GitHub Actions secrets) at build time. See `docs/ARCHITECTURE.md`.
//!
//! Security note: embedding is not encryption. The values are recoverable
//! from the binary (e.g. `strings`). For a desktop app this is an accepted
//! tradeoff — the OAuth client id/secret for a Google "Desktop app" client
//! are not confidential by design (RFC 8252), and the YouTube API key is
//! protected by Google-side API/quota restrictions rather than by secrecy.

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
