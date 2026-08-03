//! Token and user-info models, including expiry logic.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Refresh proactively this long before actual expiry.
const EXPIRY_SKEW: Duration = Duration::from_secs(60);

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
